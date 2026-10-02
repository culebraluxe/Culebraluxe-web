//! Port of `workflow_app/forge/forge-executor.ts`.
//! Production drive refuses a run with no role runner; there is no synthetic fallback to fall into.

use std::collections::BTreeSet;

use crate::engine::engine_fault::is_engine_fault_error;
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::job::{
    execute_claimed_job_unsettled, ForgeJobBridge, ForgeJobLease, JobService,
};
use crate::engine::path::shared_path;
use crate::engine::role_slice::forge_lane_surface;
use crate::engine::runner::ForgeTurnPorts;
use crate::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use crate::engine::service_binding::{is_human_gate, nodes_for_service};
use crate::engine::turn_budget;
use crate::roles::registry::ForgeServiceRegistry;
use workflow::{
    JobStatus, ProcessOutcome, ProcessStatus, Result, TaskStatus, TxStore, WorkflowError,
};

pub struct ForgeRoleOutcome {
    pub transition_name: Option<String>,
    pub evidence: ForgeGateEvidence,
}

pub trait ForgeRoleRunner: Send + Sync {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome>;

    /// The envelope this runner runs a turn under, when it has one.
    ///
    /// The production runner answers `Some(self)`, which is what lets a lane service run the shared
    /// lifecycle itself: it reads the harness, the evidence it started from, the writer its records go
    /// through, and the dispatch knobs (test mode, bench intent, contract assay commands) from the very
    /// ports the runner holds. A runner that answers `None` is a double with no envelope at all — a test
    /// scrubber or the synthetic default — and for those the service boundary keeps delegating the whole
    /// turn to the runner, which is the established behavior and not role policy.
    ///
    /// The default is `None` deliberately: a runner has to *say* it has ports rather than be assumed to.
    fn turn_ports(&self) -> Option<&dyn ForgeTurnPorts> {
        None
    }
}

#[derive(Debug, Clone)]
pub enum ForgeStopTarget {
    Role(&'static str),
    Node(String),
}

/// The dispatch cap the worker carries off the claimed row, as a stop target (migration 167: `scout` | `architect` |
/// `lead`, NULL = the full chain).
///
/// The column existed and no Rust reader honoured it: the worker parsed the same three words out of argv and the
/// engine binary handed `stop_after: None` to every run, so a dispatch that asked to stop at the architect ran the
/// whole chain. `None` here is returned only for a word this does not recognise — the caller refuses it instead of
/// quietly widening the run to the full chain.
pub fn parse_forge_stop_after(raw: &str) -> Option<ForgeStopTarget> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "scout" => Some(ForgeStopTarget::Role("scout")),
        "architect" => Some(ForgeStopTarget::Role("architect")),
        "lead" => Some(ForgeStopTarget::Role("lead")),
        _ => None,
    }
}

/// The node a `stop_after: lead` cap stops at.
///
/// The only node name the drive still holds, and it is one because the cap is not a lane's group: `forge.lead`
/// binds four nodes (the pre and post decisions, the SOLO implement, and the failure classifier), while "stop after
/// the lead" means after the PRE decision — the node the next phase is chosen at. The scout and architect caps are
/// the definition's own groups (see [`resolve_forge_stop_target`]), so this is the asymmetry left over, not a table.
const LEAD_PRE_NODE: &str = "lead_pre";

pub fn resolve_forge_stop_target(stop: Option<&ForgeStopTarget>) -> Option<BTreeSet<String>> {
    /// One service's nodes, as the set of nodes whose task ends the run.
    fn cap(service_key: &str) -> BTreeSet<String> {
        nodes_for_service(service_key)
            .into_iter()
            .map(String::from)
            .collect()
    }
    match stop {
        None => None,
        Some(ForgeStopTarget::Node(n)) => Some(BTreeSet::from([n.clone()])),
        Some(ForgeStopTarget::Role("scout")) => Some(cap("forge.scout")),
        Some(ForgeStopTarget::Role("architect")) => Some(cap("forge.architect")),
        Some(ForgeStopTarget::Role("lead")) => Some(BTreeSet::from([LEAD_PRE_NODE.to_string()])),
        Some(ForgeStopTarget::Role(_)) => None,
    }
}

pub fn is_advance_conflict(err: &WorkflowError) -> bool {
    let m = err.to_string();
    let c = err.code();
    matches!(
        c,
        "STALE_TASK" | "TASK_ALREADY_COMPLETED" | "PROCESS_NOT_ACTIVE" | "TASK_NOT_CLAIMABLE"
    ) || m.to_ascii_lowercase().contains("already completed")
        || m.to_ascii_lowercase().contains("not active")
        || m.to_ascii_lowercase().contains("state changed")
}

pub fn is_completed_release_conflict(err: &WorkflowError) -> bool {
    err.code() == "TASK_ALREADY_COMPLETED"
        || err
            .to_string()
            .to_ascii_lowercase()
            .contains("cannot be released in status: completed")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneFailureSettlement {
    Released,
    AlreadyCompleted,
}

pub fn settle_forge_lane_failure<S: TxStore>(
    rt: &ForgeRuntime<S>,
    task_id: &str,
    actor: &str,
    lane_err: &WorkflowError,
) -> Result<LaneFailureSettlement> {
    match rt.release_role_task(task_id, actor) {
        Ok(()) => Ok(LaneFailureSettlement::Released),
        Err(e) if is_completed_release_conflict(&e) => {
            Ok(LaneFailureSettlement::AlreadyCompleted)
        }
        Err(e) if is_advance_conflict(&e) => Ok(LaneFailureSettlement::Released),
        Err(release_err) => Err(WorkflowError::generic(format!(
            "Forge role failed and its task could not be released: lane={lane_err}; release={release_err}"
        ))),
    }
}

#[derive(Debug, Clone)]
pub struct WaveLane<T> {
    pub lane: String,
    pub surface: Option<Vec<String>>,
    pub fanout: bool,
    pub task: T,
}

#[derive(Debug, Clone)]
pub struct WaveRefusal {
    pub lanes: (String, String),
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct WavePlan<T> {
    pub batches: Vec<Vec<WaveLane<T>>>,
    pub refusals: Vec<WaveRefusal>,
}

fn has_surface<T>(lane: &WaveLane<T>) -> bool {
    lane.surface
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

pub fn plan_wave<T: Clone>(lanes: &[WaveLane<T>], cap: usize) -> WavePlan<T> {
    let limit = cap.max(1);
    let mut refusals = Vec::new();
    if limit > 1 {
        let known: Vec<&WaveLane<T>> = lanes
            .iter()
            .filter(|l| !l.fanout && has_surface(l))
            .collect();
        for i in 0..known.len() {
            for j in i + 1..known.len() {
                if let (Some(a), Some(b)) = (&known[i].surface, &known[j].surface) {
                    if let Some(path) = shared_path(a, b) {
                        refusals.push(WaveRefusal {
                            lanes: (known[i].lane.clone(), known[j].lane.clone()),
                            path,
                        });
                    }
                }
            }
        }
    }
    let mut batches: Vec<Vec<WaveLane<T>>> = Vec::new();
    for lane in lanes {
        if !lane.fanout && !has_surface(lane) {
            batches.push(vec![lane.clone()]);
            continue;
        }
        let mut placed = false;
        for batch in batches.iter_mut() {
            if batch.len() >= limit {
                continue;
            }
            let compatible = batch.iter().all(|member| {
                if lane.fanout {
                    return member.fanout;
                }
                if member.fanout || !has_surface(member) {
                    return false;
                }
                shared_path(
                    lane.surface.as_deref().unwrap_or(&[]),
                    member.surface.as_deref().unwrap_or(&[]),
                )
                .is_none()
            });
            if !compatible {
                continue;
            }
            batch.push(lane.clone());
            placed = true;
            break;
        }
        if !placed {
            batches.push(vec![lane.clone()]);
        }
    }
    WavePlan { batches, refusals }
}

#[derive(Debug, Clone)]
pub struct DriveForgeStoryResult {
    pub instance_id: String,
    pub status: String,
    pub steps: Vec<String>,
    pub exhausted: bool,
    pub blocked_reason: Option<String>,
    pub needs_human: bool,
    pub stopped_after: Option<String>,
    /// Completion units the resume door applied before the first role turn (the crash window healed on
    /// the way in). Reported so "0 forever" and "the whole history again" are distinguishable.
    pub reconciled: usize,
}

pub struct DriveForgeStoryOptions<'a> {
    pub work_type: &'a str,
    pub evidence: ForgeGateEvidence,
    /// The role runner this drive dispatches its turns through.
    ///
    /// There is no synthetic fallback (`2026-10-02`, the seam closure). The crate's test-only
    /// `DefaultForgeRoleRunner` was unreachable — every direct drive named a runner, and the durable path ignores
    /// this field entirely — so it and its per-node seed table are gone rather than kept as a second dispatch path.
    /// `None` is refused below instead of simulated.
    pub runner: Option<&'a dyn ForgeRoleRunner>,
    pub max_steps: usize,
    pub worker_id: &'a str,
    pub split_concurrency: usize,
    pub stop_after: Option<ForgeStopTarget>,
    /// How many MODEL TURNS this generation may dispatch before it stops (§10).
    ///
    /// A "turn" here is a dispatched role — the unit V1 measured: "a healthy FEATURE generation costs five turns:
    /// architect, lead_pre, smith, post, qa" (`legacy/workflow_app/tests/forge-model-turn-budget.test.ts`). It is NOT
    /// a vendor step inside one of them: the vendor's own per-agent step ceiling is a different number in a different
    /// layer, and counting steps here would stop healthy generations while letting a generation of many short turns
    /// run forever.
    ///
    /// Checked BEFORE a turn is dispatched and failing closed, because the failure it prevents is not slowness: it is
    /// a generation that keeps looking productive one turn at a time and is read as "still working" instead of
    /// "looping". A field rather than an environment read at the point of use, so a test can cap a generation without
    /// setting a process-global variable that parallel tests share.
    pub turn_cap: u32,
}

impl<'a> DriveForgeStoryOptions<'a> {
    /// The cap the environment asks for (`FORGE_MAX_MODEL_TURNS_PER_GENERATION`).
    ///
    /// The only place the variable is read, so the cap an operator sets and the cap a test sets cannot be different
    /// code paths. A broken value falls back to the default and an absurd one is clamped — never widened, never
    /// unlimited.
    pub fn turn_cap_from_env() -> u32 {
        turn_budget::resolve_generation_turn_cap(
            std::env::var(turn_budget::GENERATION_TURN_CAP_ENV)
                .ok()
                .as_deref(),
        )
    }
}

impl<'a> DriveForgeStoryOptions<'a> {
    pub fn production(work_type: &'a str, runner: &'a dyn ForgeRoleRunner) -> Self {
        Self {
            work_type,
            evidence: ForgeGateEvidence {
                work_type: Some(work_type.into()),
                ..Default::default()
            },
            runner: Some(runner),
            max_steps: 40,
            worker_id: "forge",
            split_concurrency: 1,
            stop_after: None,
            turn_cap: Self::turn_cap_from_env(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct DurableForgeExecution<'a, 'services> {
    pub jobs: &'a dyn JobService,
    pub registry: &'a ForgeServiceRegistry<'services>,
}

/// How many times a Workflow completion write is repeated while the failure is the engine's own plumbing.
///
/// THE ROLE TURN HAS ALREADY BEEN PAID FOR when this runs, so the only thing worth repeating is the WRITE
/// (`2026-10-02`). A completion that died mid-transaction is also safe to repeat: the engine answers
/// `TASK_ALREADY_COMPLETED` when the earlier attempt actually committed, and the caller already reads that as "the
/// Workflow settled — close the job". Repeating the TURN instead is precisely what the completion-before-completion
/// ordering exists to prevent, so it is not on the table.
const COMPLETION_WRITE_ATTEMPTS: u32 = 3;

/// Settle the Workflow task the role just finished, repeating only the write.
///
/// The ordering invariant is untouched: Workflow state still wins before the durable execution receipt closes. What
/// this adds is what happens when the WRITE itself fails — a database that went away mid-commit is the engine's
/// failure and says nothing about the story, so it is retried while the lease is still held rather than being
/// recorded on the first attempt. Anything that is not plumbing goes back at once, unwrapped and unattempted.
fn complete_role_task_with_transient_retry<S: TxStore>(
    rt: &ForgeRuntime<S>,
    task_id: &str,
    actor: &str,
    outcome: &ForgeRoleOutcome,
) -> Result<()> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match rt.complete_role_task(
            task_id,
            actor,
            outcome.transition_name.as_deref(),
            outcome.evidence.clone(),
        ) {
            Ok(_) => return Ok(()),
            Err(error) if should_repeat_completion_write(attempt, &error) => {
                eprintln!(
                    "forge: Workflow completion for task {task_id} hit an engine fault (attempt \
                     {attempt}/{COMPLETION_WRITE_ATTEMPTS}); repeating the write, not the turn: {error}"
                );
            }
            Err(error) => return Err(error),
        }
    }
}

/// Whether a completion failure is repeated — the whole rule, as a function of the attempt and the error.
///
/// Stated separately so it can be railed as a table rather than inferred from a run: the repeat is bounded by the
/// budget, and only the engine's own plumbing is repeated at all.
fn should_repeat_completion_write(attempt: u32, error: &WorkflowError) -> bool {
    attempt < COMPLETION_WRITE_ATTEMPTS && is_engine_fault_error(error)
}

/// The durable reason a completion write that never landed leaves behind.
///
/// It names the one thing an operator must not do — pay for the turn again — because this row is the only evidence
/// that the model work was already spent: the Workflow task is still open, so a later run of the same story would
/// otherwise find READY work for a turn that has already been executed once.
fn completion_failure_reason(error: &WorkflowError) -> String {
    if is_engine_fault_error(error) {
        format!(
            "PAID_TURN_NOT_REDISPATCHED: the role turn for this task ran, but the Workflow completion write failed on \
             every attempt ({error}). Reconcile the Workflow task, then requeue this job; do not run the turn again."
        )
    } else {
        format!("Workflow task completion failed after role execution: {error}")
    }
}

pub fn drive_forge_story<S: TxStore>(
    rt: &ForgeRuntime<S>,
    story_id: &str,
    opts: DriveForgeStoryOptions<'_>,
) -> Result<DriveForgeStoryResult> {
    drive_forge_story_inner(rt, story_id, opts, None)
}

pub fn drive_forge_story_with_jobs<S: TxStore>(
    rt: &ForgeRuntime<S>,
    story_id: &str,
    opts: DriveForgeStoryOptions<'_>,
    durable: DurableForgeExecution<'_, '_>,
) -> Result<DriveForgeStoryResult> {
    drive_forge_story_inner(rt, story_id, opts, Some(durable))
}

fn drive_forge_story_inner<S: TxStore>(
    rt: &ForgeRuntime<S>,
    story_id: &str,
    opts: DriveForgeStoryOptions<'_>,
    durable: Option<DurableForgeExecution<'_, '_>>,
) -> Result<DriveForgeStoryResult> {
    // A direct drive must name its runner: there is no synthetic fallback to fall into, so the refusal is the only
    // answer a caller with no runner gets (`2026-10-02`, the seam closure). The durable branch resolves concrete
    // services through JobService + ForgeServiceRegistry and does not use this field.
    if durable.is_none() && opts.runner.is_none() {
        return Err(WorkflowError::generic(
            "Direct Forge execution requires an explicit real role runner; there is no synthetic runner. Durable production execution resolves concrete services through JobService + ForgeServiceRegistry.",
        ));
    }
    let stop_target = resolve_forge_stop_target(opts.stop_after.as_ref());
    let mut stopped_after = None;
    let mut steps = Vec::new();
    // §10: ONE integer per generation, and the unit is a dispatched ROLE TURN. `turns_dispatched` counts turns that
    // were actually claimed and started, so a claim conflict — a turn that nothing ran — costs nothing.
    let mut turns_dispatched: u32 = 0;
    let mut turn_cap_stop: Option<String> = None;

    let wake = rt.wake_story(story_id, opts.work_type, opts.evidence.clone())?;
    let instance_id = wake.instance_id;
    let reconciled = wake.reconciled;

    // A durable role worker owns recovery of expired role-job leases before it
    // scans READY Workflow work. Recovery is generic; typed claim below ensures
    // this driver can execute only `forge.role` rows.
    if let Some(durable) = durable {
        durable.jobs.recover_stale(256)?;
    }

    for _ in 0..opts.max_steps {
        let tasks = rt.list_role_tasks(story_id)?;
        if tasks.is_empty() {
            break;
        }
        if let Some(human) = tasks
            .iter()
            .find(|t| t.node_id.as_deref().map(is_human_gate).unwrap_or(false))
        {
            // A canonical Story Board write is not optional: if the hold cannot be recorded, the drive fails
            // visibly rather than reporting a human gate that no row describes (2026-09-29).
            rt.writer()
                .mark_story_human_hold(story_id, "Forge engine entered a human decision gate.")
                .map_err(|error| {
                    workflow::WorkflowError::generic(format!(
                        "mark_story_human_hold({story_id}): {error}"
                    ))
                })?;
            let _ = human;
            return Ok(DriveForgeStoryResult {
                instance_id: instance_id.clone(),
                status: instance_status(rt, &instance_id)?,
                steps,
                exhausted: false,
                blocked_reason: None,
                needs_human: true,
                stopped_after,
                reconciled,
            });
        }
        if !tasks.iter().any(|t| t.status == TaskStatus::Ready) {
            let blocked = tasks
                .iter()
                .map(|t| format!("{}={:?}", t.node_id.as_deref().unwrap_or("?"), t.status))
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(DriveForgeStoryResult {
                instance_id: instance_id.clone(),
                status: instance_status(rt, &instance_id)?,
                steps,
                exhausted: true,
                blocked_reason: Some(format!("no ready task; active: {blocked}")),
                needs_human: false,
                stopped_after,
                reconciled,
            });
        }

        let ready: Vec<_> = tasks
            .into_iter()
            .filter(|t| t.status == TaskStatus::Ready)
            .collect();
        let cap = opts.split_concurrency.max(1);
        let lanes: Vec<WaveLane<ActiveForgeRoleTask>> = ready
            .into_iter()
            .map(|task| {
                let node = task.node_id.clone().unwrap_or_default();
                WaveLane {
                    fanout: node == "smith_split_work",
                    surface: forge_lane_surface(None),
                    lane: node,
                    task,
                }
            })
            .collect();
        let plan = plan_wave(&lanes, cap);
        for batch in plan.batches {
            for lane in batch {
                let task = lane.task;
                let node = task.node_id.clone().unwrap_or_default();
                let actor = task
                    .candidates
                    .first()
                    .cloned()
                    .unwrap_or_else(|| opts.worker_id.to_string());
                // THE HARD STOP, and it is checked BEFORE the claim so a refused turn cannot leave a claim behind for
                // the stale sweeper to find. At the cap the generation ENDS with the code in its record rather than
                // dispatching a turn the cap already refused and paying for it.
                if let turn_budget::TurnBudgetVerdict::Refused { reason, .. } =
                    turn_budget::assess_generation_turn_budget(
                        turns_dispatched,
                        Some(opts.turn_cap),
                    )
                {
                    turn_cap_stop = Some(reason);
                    break;
                }
                let mut durable_lease: Option<ForgeJobLease> = None;
                let outcome = if let Some(durable) = durable {
                    let request =
                        ForgeJobBridge::new(durable.registry).job_for_ready_task(&task)?;
                    let job_id = durable.jobs.enqueue(&request)?;
                    let lease = match durable.jobs.claim_one(&job_id, opts.worker_id) {
                        Ok(lease) => lease,
                        Err(err) if err.code() == "FORGE_JOB_NOT_CLAIMABLE" => {
                            let state = durable.jobs.inspect(&job_id)?;
                            let detail = state
                                .last_error
                                .as_deref()
                                .unwrap_or("no durable job error recorded");
                            match state.status {
                                JobStatus::Failed | JobStatus::Cancelled | JobStatus::Completed => {
                                    let reason = format!(
                                        "Forge durable job {job_id} for task {} is {:?}: {detail}",
                                        task.task_id, state.status
                                    );
                                    rt.writer()
                                        .mark_story_human_hold(story_id, &reason)
                                        .map_err(|error| {
                                            WorkflowError::generic(format!(
                                                "mark_story_human_hold({story_id}): {error}"
                                            ))
                                        })?;
                                    return Ok(DriveForgeStoryResult {
                                        instance_id: instance_id.clone(),
                                        status: instance_status(rt, &instance_id)?,
                                        steps,
                                        exhausted: false,
                                        blocked_reason: Some(reason),
                                        needs_human: true,
                                        stopped_after,
                                        reconciled,
                                    });
                                }
                                JobStatus::Locked | JobStatus::Pending => {
                                    return Ok(DriveForgeStoryResult {
                                        instance_id: instance_id.clone(),
                                        status: instance_status(rt, &instance_id)?,
                                        steps,
                                        exhausted: true,
                                        blocked_reason: Some(format!(
                                            "Forge durable job {job_id} for task {} is {:?}; attempts={}/{} locked_by={:?}",
                                            task.task_id,
                                            state.status,
                                            state.attempts,
                                            state.max_attempts,
                                            state.locked_by
                                        )),
                                        needs_human: false,
                                        stopped_after,
                                        reconciled,
                                    });
                                }
                            }
                        }
                        Err(err) => return Err(err),
                    };
                    turns_dispatched += 1;
                    let outcome = match execute_claimed_job_unsettled(
                        durable.jobs,
                        opts.worker_id,
                        &lease,
                        &task,
                        durable.registry,
                    ) {
                        Ok(outcome) => outcome,
                        Err(err) => {
                            let reason = format!(
                                "Forge role {node} failed for task {}: {err}",
                                task.task_id
                            );
                            rt.writer()
                                .mark_story_human_hold(story_id, &reason)
                                .map_err(|error| {
                                    WorkflowError::generic(format!(
                                        "mark_story_human_hold({story_id}): {error}"
                                    ))
                                })?;
                            return Ok(DriveForgeStoryResult {
                                instance_id: instance_id.clone(),
                                status: instance_status(rt, &instance_id)?,
                                steps,
                                exhausted: false,
                                blocked_reason: Some(reason),
                                needs_human: true,
                                stopped_after,
                                reconciled,
                            });
                        }
                    };
                    durable_lease = Some(lease);
                    outcome
                } else {
                    // Compatibility/test path. Production uses the durable branch
                    // above; the established direct path remains for fixtures.
                    //
                    // The runner is resolved here rather than before the loop: the guard above proves one exists for
                    // every direct drive, and the durable branch never needs one.
                    let Some(runner) = opts.runner else {
                        return Err(WorkflowError::generic(
                            "Direct Forge execution reached a role turn with no explicit real role runner",
                        ));
                    };
                    if let Err(err) = rt.claim_role_task(&task.task_id, &actor) {
                        if is_advance_conflict(&err) {
                            continue;
                        }
                        return Err(err);
                    }
                    turns_dispatched += 1;
                    match runner.run(&node, &task) {
                        Ok(o) => o,
                        Err(err) => {
                            let settle =
                                settle_forge_lane_failure(rt, &task.task_id, &actor, &err)?;
                            if settle == LaneFailureSettlement::AlreadyCompleted {
                                continue;
                            }
                            return Err(err);
                        }
                    }
                };

                if let Err(err) =
                    complete_role_task_with_transient_retry(rt, &task.task_id, &actor, &outcome)
                {
                    if let (Some(durable), Some(lease)) = (durable, durable_lease.as_ref()) {
                        if err.code() == "TASK_ALREADY_COMPLETED" {
                            durable.jobs.complete(&lease.job_id, opts.worker_id)?;
                            continue;
                        }
                        durable.jobs.fail(
                            &lease.job_id,
                            opts.worker_id,
                            &completion_failure_reason(&err),
                            true,
                        )?;
                        return Err(err);
                    }

                    if is_advance_conflict(&err) {
                        continue;
                    }
                    let settle = settle_forge_lane_failure(rt, &task.task_id, &actor, &err)?;
                    if settle == LaneFailureSettlement::AlreadyCompleted {
                        continue;
                    }
                    return Err(err);
                }

                // Workflow state wins before the execution receipt closes. A
                // crash here can leave a stale job to recover, but the task is
                // no longer READY, so the paid role turn cannot be dispatched
                // again by the production driver.
                if let (Some(durable), Some(lease)) = (durable, durable_lease.as_ref()) {
                    durable.jobs.complete(&lease.job_id, opts.worker_id)?;
                }
                steps.push(node.clone());
                if stop_target
                    .as_ref()
                    .map(|s| s.contains(&node))
                    .unwrap_or(false)
                {
                    stopped_after = Some(node);
                }
            }
            if stopped_after.is_some() || turn_cap_stop.is_some() {
                break;
            }
        }
        if stopped_after.is_some() || turn_cap_stop.is_some() {
            break;
        }
    }

    let tasks = rt.list_role_tasks(story_id)?;
    let instance = rt.engine().get_process_instance(&instance_id)?;

    // Restore the terminal projection the legacy engine owned: workflow completion is what makes the Storyboard
    // complete. Queue settlement deliberately refuses Done while the board still says In Progress, so omitting this
    // projection creates a circular dependency (the board waits for settlement while settlement waits for the board).
    // Only the engine's real Completed/completed pair earns 100%; holds, cancellation and failures remain untouched.
    if process_completes_story(instance.status, instance.outcome) {
        rt.writer().mark_story_complete(story_id).map_err(|error| {
            WorkflowError::generic(format!("mark_story_complete({story_id}): {error}"))
        })?;
    }

    Ok(DriveForgeStoryResult {
        instance_id: instance_id.clone(),
        status: format!("{:?}", instance.status),
        steps,
        exhausted: !tasks.is_empty(),
        // The cap's reason when the generation ran into it, otherwise nothing. A stop that did not name itself here
        // would be indistinguishable from a generation that simply had no more work.
        blocked_reason: turn_cap_stop,
        needs_human: tasks
            .iter()
            .any(|t| t.node_id.as_deref().map(is_human_gate).unwrap_or(false)),
        stopped_after,
        reconciled,
    })
}

fn instance_status<S: TxStore>(rt: &ForgeRuntime<S>, instance_id: &str) -> Result<String> {
    Ok(format!(
        "{:?}",
        rt.engine().get_process_instance(instance_id)?.status
    ))
}

fn process_completes_story(status: ProcessStatus, outcome: Option<ProcessOutcome>) -> bool {
    status == ProcessStatus::Completed && outcome == Some(ProcessOutcome::Completed)
}

#[cfg(test)]
mod completion_fault_tests {
    use super::*;

    /// The rule, as a table: only the engine's plumbing is repeated, and only while the write budget lasts.
    ///
    /// The turn has already been paid for when this runs, so the difference between "repeat" and "hand back" is the
    /// difference between one lost write and a second model turn — which is why it is pinned rather than read.
    #[test]
    fn only_a_plumbing_failure_is_repeated_and_only_while_the_budget_lasts() {
        let plumbing = WorkflowError::unavailable("db: sqlstate 25P03 idle-in-transaction");
        let sqlstate_only = WorkflowError::generic("db: DatabaseUnavailable during workflow.step");
        let verdict = WorkflowError::conflict("PROCESS_NOT_ACTIVE", "the instance is not active");

        assert!(should_repeat_completion_write(1, &plumbing));
        assert!(should_repeat_completion_write(1, &sqlstate_only));
        assert!(should_repeat_completion_write(
            COMPLETION_WRITE_ATTEMPTS - 1,
            &plumbing
        ));
        assert!(
            !should_repeat_completion_write(COMPLETION_WRITE_ATTEMPTS, &plumbing),
            "the write is repeated, not repeated forever"
        );
        for attempt in 1..=COMPLETION_WRITE_ATTEMPTS {
            assert!(
                !should_repeat_completion_write(attempt, &verdict),
                "attempt {attempt} repeated a decision the engine already made"
            );
        }
    }

    /// The durable row an operator reads has to say the one thing that must not happen next.
    ///
    /// The Workflow task is still open when this reason is written, so the row is the only evidence that the model
    /// work was already spent: a reason that does not say so invites a second paid turn.
    #[test]
    fn a_completion_that_never_landed_names_the_paid_turn() {
        let plumbing = WorkflowError::unavailable("db: DatabaseUnavailable (sqlstate 25P03)");
        let reason = completion_failure_reason(&plumbing);
        assert!(reason.contains("PAID_TURN_NOT_REDISPATCHED"), "{reason}");
        assert!(reason.contains("do not run the turn again"), "{reason}");
        assert!(reason.contains("sqlstate 25P03"), "{reason}");

        let verdict = WorkflowError::conflict("PROCESS_NOT_ACTIVE", "the instance is not active");
        let reason = completion_failure_reason(&verdict);
        assert!(
            !reason.contains("PAID_TURN_NOT_REDISPATCHED"),
            "an engine decision is not a plumbing failure: {reason}"
        );
        assert!(
            reason.contains("Workflow task completion failed after role execution"),
            "{reason}"
        );
    }
}

#[cfg(test)]
mod dispatch_cap_tests {
    use super::*;

    #[test]
    fn only_completed_completed_projects_story_complete() {
        assert!(process_completes_story(
            ProcessStatus::Completed,
            Some(ProcessOutcome::Completed)
        ));
        for (status, outcome) in [
            (ProcessStatus::Active, None),
            (ProcessStatus::Completed, Some(ProcessOutcome::Cancelled)),
            (ProcessStatus::Completed, Some(ProcessOutcome::Failed)),
            (ProcessStatus::Error, Some(ProcessOutcome::Failed)),
        ] {
            assert!(!process_completes_story(status, outcome));
        }
    }

    /// The three words the column allows (migration 167) map to the three stop targets the driver understands.
    #[test]
    fn the_three_allowed_caps_parse_and_nothing_else_does() {
        for (raw, expected) in [
            ("scout", "scout"),
            ("architect", "architect"),
            ("lead", "lead"),
            (" ARCHITECT ", "architect"),
        ] {
            match parse_forge_stop_after(raw) {
                Some(ForgeStopTarget::Role(role)) => assert_eq!(role, expected, "{raw}"),
                other => panic!("{raw} parsed as {other:?}"),
            }
        }
        // A cap that cannot be read must never widen to the full chain: the caller refuses these.
        for raw in ["", "smith", "FEATURE", "all", "0"] {
            assert!(
                parse_forge_stop_after(raw).is_none(),
                "{raw} must not parse into a cap"
            );
        }
    }

    /// Each cap resolves to the nodes the driver stops after, and a cap nobody defined stops nothing.
    #[test]
    fn a_cap_resolves_to_its_own_nodes() {
        let scout = resolve_forge_stop_target(Some(&ForgeStopTarget::Role("scout"))).unwrap();
        assert!(scout.contains("feature_scout"));
        let architect =
            resolve_forge_stop_target(Some(&ForgeStopTarget::Role("architect"))).unwrap();
        assert!(architect.contains("architect"));
        let lead = resolve_forge_stop_target(Some(&ForgeStopTarget::Role("lead"))).unwrap();
        assert!(lead.contains("lead_pre"));
        assert!(
            resolve_forge_stop_target(None).is_none(),
            "NULL is the full chain"
        );
        assert!(resolve_forge_stop_target(Some(&ForgeStopTarget::Role("unknown"))).is_none());
    }
}
