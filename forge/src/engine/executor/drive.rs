//! Forge story drive: the main execution loop.

use crate::engine::config::{turn_ceiling, TURN_CEILING_ENV};
use crate::engine::engine_fault::is_engine_fault_error;
use crate::engine::executor::completion::{
    complete_role_task_with_transient_retry, completion_failure_reason,
};
use crate::engine::executor::dispatch::{resolve_forge_stop_target, ForgeStopTarget};
use crate::engine::executor::lane_failure::{
    is_advance_conflict, settle_forge_lane_failure, LaneFailureSettlement,
};
use crate::engine::executor::wave::{plan_wave, WaveLane, WavePlan};
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::job::{
    execute_claimed_job_unsettled, ForgeJobBridge, ForgeJobLease, JobService,
};
use crate::engine::packet::ExecutionWorkspace;
use crate::engine::runner::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use crate::engine::service_binding::is_human_gate;
use crate::engine::turn_budget;
use crate::engine::worktree::{
    discard_unstarted_lane_worktree, integrate_lane_candidate, provision_worker_workspace,
    resolve_repo_root, DEFAULT_WORKTREES_DIRNAME,
};
use crate::roles::registry::ForgeServiceRegistry;
use crate::roles::service::ForgeLaneServices;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use workflow::{
    JobStatus, ProcessOutcome, ProcessStatus, Result, TaskStatus, TxStore, WorkflowError,
};

mod batch;
mod inner;
use batch::*;
use inner::*;

fn process_completes_story(status: ProcessStatus, outcome: Option<ProcessOutcome>) -> bool {
    status == ProcessStatus::Completed && outcome == Some(ProcessOutcome::Completed)
}

fn lane_stop_diagnostic(error: &WorkflowError) -> (&'static str, bool) {
    if error.code() == turn_budget::MODEL_TURN_CAP_CODE {
        ("model_attempt_cap", false)
    } else if error.code() == crate::engine::harness::TURN_INTERRUPTED_CODE {
        ("turn_interrupted", false)
    } else {
        ("role_execution_error", true)
    }
}

fn lane_failure_reason(node: &str, task_id: &str, error: &WorkflowError) -> String {
    let (kind, role_verdict) = lane_stop_diagnostic(error);
    if role_verdict {
        format!("Forge role {node} failed for task {task_id}: {error}")
    } else {
        format!("Forge execution stopped for task {task_id} ({kind}); no role verdict was accepted: {error}")
    }
}

/// The two verdict channels one failed lane feeds (`WO-302` Part A).
///
/// A lane failure is EITHER a business verdict (anything that is not the engine's own fault — a role
/// verdict, a human gate, a refusal) OR an engine fault (plumbing: `is_engine_fault_error`). The split
/// matters because the wave must weigh them differently: see `wave_disposition`. Everything here is owned
/// so the caller can move the lane's own error object into the fault slot afterwards — error identity is
/// never traded for a message (`WO-121`), so the propagated fault stays the lane's error, not a copy.
struct LaneChannels {
    /// The lane's reason, verbatim — the Hold candidate when this lane is not an engine fault.
    reason: String,
    /// Whether this lane feeds the engine-fault channel instead of the Hold channel.
    is_engine_fault: bool,
}

/// Classify one failed lane execution into the wave's verdict channels.
///
/// Pure over the execution: the reason strings are built here so the rule is testable without running a
/// lane. The per-lane diagnostic line and the worktree disposition stay with the caller — this function
/// decides meaning, not side effects.
fn classify_lane_failure(execution: &LaneExecutionResult) -> LaneChannels {
    let error = match &execution.outcome {
        Ok(_) => {
            return LaneChannels {
                reason: String::new(),
                is_engine_fault: false,
            }
        }
        Err(error) => error,
    };
    // A settlement failure that happened while this lane's error was being handled is reported WITH it
    // (`WO-121`): one lane, two facts, both named.
    let reason = match execution.settlement_failure.as_deref() {
        Some(settlement) => with_secondary_failure(
            &lane_failure_reason(
                &execution.claimed.node,
                &execution.claimed.task.task_id,
                error,
            ),
            "durable claim settlement",
            settlement,
        ),
        None => lane_failure_reason(
            &execution.claimed.node,
            &execution.claimed.task.task_id,
            error,
        ),
    };
    LaneChannels {
        reason,
        is_engine_fault: is_engine_fault_error(error),
    }
}

/// What a wave carries home after weighing its lanes' verdict channels (`WO-302` Part A).
enum WaveDisposition {
    /// A genuine business verdict survived: the story is held with this reason.
    Hold(String),
    /// No business verdict, but the engine faulted: the original fault propagates untouched.
    EngineFault(WorkflowError),
    /// Neither: the wave is clear.
    Clear,
}

/// Settle a completed lane's durable job without ever orphaning it (`WO-302` Part B).
///
/// The shape is log-and-continue with reconciliation, not swallow-and-ignore and not propagate: a transient
/// `jobs.complete` failure after real work must neither erase the completed outcome nor strand the job where
/// no later pass recognises it. `Ok` is normal completion; `Err` from the caller means something else failed.
/// Returns the Hold reason when the settlement failed, so the story is held with the failure named and the
/// paid turn preserved for reconciliation — the job row itself is untouched, which is what lets a later pass
/// settle it instead of treating it as unknown abandoned work.
fn settle_completed_lane(
    jobs: &dyn JobService,
    worker_id: &str,
    job_id: &str,
    task_id: &str,
    already_completed: bool,
) -> Option<String> {
    let situation = if already_completed {
        "was already completed but"
    } else {
        "completed but"
    };
    match jobs.complete(job_id, worker_id) {
        Ok(()) => None,
        Err(settle_error) => {
            eprintln!(
                "forge-wave task {task_id} {situation} durable job settlement failed (preserving paid turn for reconciliation): {settle_error}"
            );
            Some(format!(
                "workflow task {task_id} {situation} its durable job could not be settled (preserving paid turn for reconciliation): {settle_error}"
            ))
        }
    }
}
///
/// The sibling failure is NOT discarded: the caller logs it, and it is attached to the Hold reason, so
/// the fault stays observable while the verdict keeps its authority. Only when no lane produced a business
/// verdict does the engine fault propagate — and a fault on its own never invents a Hold.
fn wave_disposition(
    hold_reason: Option<String>,
    engine_fault: Option<WorkflowError>,
) -> WaveDisposition {
    match (hold_reason, engine_fault) {
        (Some(hold), Some(fault)) => WaveDisposition::Hold(format!(
            "{hold} [sibling engine fault also observed and logged: {fault}]"
        )),
        (Some(hold), None) => WaveDisposition::Hold(hold),
        (None, Some(fault)) => WaveDisposition::EngineFault(fault),
        (None, None) => WaveDisposition::Clear,
    }
}

fn instance_status<S: TxStore>(rt: &ForgeRuntime<S>, instance_id: &str) -> Result<String> {
    Ok(format!(
        "{:?}",
        rt.engine().get_process_instance(instance_id)?.status
    ))
}

/// The result for a lane whose thread panicked before it reported (`WO-121` §3).
///
/// Extracted so this answer is testable without provoking a panic in a real thread: a failure that names
/// the panic, a claim settled through the shared helper, and a tree that the lane's own guard already
/// dispositioned as retained during the unwind.
fn lane_that_never_reported(
    jobs: &dyn JobService,
    worker_id: &str,
    fallback: LaneFallback,
    reason: &str,
) -> LaneExecutionResult {
    let settlement_failure = fail_claimed_lane(jobs, worker_id, &fallback.claimed, reason, true);
    let worktrees =
        LaneWorktree::retained_by_its_lane(fallback.worktree_path, fallback.branch_name);
    LaneExecutionResult {
        claimed: fallback.claimed,
        parent_worktree: fallback.parent_worktree,
        base_commit: fallback.base_commit,
        worktrees,
        settlement_failure,
        outcome: Err(WorkflowError::generic(reason)),
    }
}

fn validate_story_concurrency(cap: usize, durable: bool) -> Result<()> {
    if cap <= 1 || durable {
        return Ok(());
    }
    Err(WorkflowError::conflict(
        "FORGE_PARALLEL_LANES_UNAVAILABLE",
        format!(
            "within-story concurrency {cap} requires the durable service path so each execution can receive its own worktree, lease, and interrupt handle"
        ),
    ))
}

fn validate_parallel_execution(registry: &ForgeServiceRegistry<'_>) -> Result<()> {
    if registry.registered().next().is_none() {
        return Err(WorkflowError::conflict(
            "FORGE_PARALLEL_LANES_UNAVAILABLE",
            "no role services are registered for concurrent execution",
        ));
    }
    for service in registry.registered() {
        let service_key = service.descriptor().service_id;
        let Some(ports) = service.runner().turn_ports() else {
            return Err(WorkflowError::conflict(
                "FORGE_PARALLEL_LANES_UNAVAILABLE",
                format!("service {service_key} does not expose production turn ports"),
            ));
        };
        let harness = ports.harness();
        if !harness.supports_interrupt() {
            return Err(WorkflowError::conflict(
                "FORGE_PARALLEL_LANES_UNAVAILABLE",
                format!("service {service_key} harness cannot interrupt its execution"),
            ));
        }
        let Some(workspace) = harness.execution_workspace().cloned() else {
            return Err(WorkflowError::conflict(
                "FORGE_PARALLEL_LANES_UNAVAILABLE",
                format!("service {service_key} requires a provisioned story worktree; enable FORGE_PROVISION=1"),
            ));
        };
        if !workspace.branch_name.starts_with("agent/")
            || !std::path::Path::new(&workspace.worktree_path).is_dir()
        {
            return Err(WorkflowError::conflict(
                "FORGE_PARALLEL_LANES_UNAVAILABLE",
                format!(
                    "service {service_key} is not bound to an existing isolated agent worktree"
                ),
            ));
        }
        let Some(forked) = service.runner().fork_for_workspace(workspace)? else {
            return Err(WorkflowError::conflict(
                "FORGE_PARALLEL_LANES_UNAVAILABLE",
                format!("service {service_key} runner cannot create an isolated harness"),
            ));
        };
        if !forked
            .turn_ports()
            .map(|ports| ports.harness().supports_interrupt())
            .unwrap_or(false)
        {
            return Err(WorkflowError::conflict(
                "FORGE_PARALLEL_LANES_UNAVAILABLE",
                format!("service {service_key} fork lacks scoped interruption"),
            ));
        }
    }
    Ok(())
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
    pub reconciled: usize,
}

pub struct DriveForgeStoryOptions<'a> {
    pub work_type: &'a str,
    pub evidence: ForgeGateEvidence,
    pub runner: Option<&'a dyn ForgeRoleRunner>,
    pub max_steps: usize,
    pub worker_id: &'a str,
    pub within_story_concurrency: usize,
    pub stop_after: Option<ForgeStopTarget>,
    pub turn_cap: u32,
}

impl<'a> DriveForgeStoryOptions<'a> {
    pub fn turn_cap_from_env() -> u32 {
        turn_budget::resolve_generation_turn_cap(
            std::env::var(turn_budget::GENERATION_TURN_CAP_ENV)
                .ok()
                .as_deref()
                .filter(|s| !s.is_empty()),
        )
    }

    pub fn within_story_concurrency_from_env() -> usize {
        turn_budget::resolve_within_story_concurrency(
            std::env::var(turn_budget::WITHIN_STORY_CONCURRENCY_ENV)
                .ok()
                .as_deref()
                .filter(|s| !s.is_empty()),
        )
    }

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
            within_story_concurrency: Self::within_story_concurrency_from_env(),
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

#[derive(Clone)]
struct ClaimedDurableLane {
    task: ActiveForgeRoleTask,
    actor: String,
    node: String,
    lease: ForgeJobLease,
}

struct PreparedDurableLane {
    claimed: ClaimedDurableLane,
    runner: Box<dyn ForgeRoleRunner>,
    worktree_path: PathBuf,
    branch_name: String,
    parent_worktree: PathBuf,
    base_commit: String,
    /// The tree this lane owns, alive from provisioning until the drive loop settles it (`WO-121`).
    worktrees: LaneWorktree,
}

struct LaneExecutionResult {
    claimed: ClaimedDurableLane,
    parent_worktree: PathBuf,
    base_commit: String,
    /// The lane's tree. The path that used to sit beside these fields now lives here (`WO-121`), because
    /// a guard that owns the path is the only thing that can guarantee it is spoken for on every exit.
    worktrees: LaneWorktree,
    /// The durable job could not be settled while this lane's failure was being handled. `WO-121`: that
    /// is a second failure, not a detail — the caller has to report it beside the first one.
    settlement_failure: Option<String>,
    outcome: Result<ForgeRoleOutcome>,
}

/// What the batch needs to report a lane whose thread died before it produced a result (`WO-121` §3).
/// Cloned before the lane moves into its thread, because the point is to have something left to name
/// when the thread does not come back.
#[derive(Clone)]
struct LaneFallback {
    claimed: ClaimedDurableLane,
    worktree_path: PathBuf,
    parent_worktree: PathBuf,
    branch_name: String,
    base_commit: String,
}

impl LaneFallback {
    fn of(lane: &PreparedDurableLane) -> Self {
        Self {
            claimed: lane.claimed.clone(),
            worktree_path: lane.worktree_path.clone(),
            parent_worktree: lane.parent_worktree.clone(),
            branch_name: lane.branch_name.clone(),
            base_commit: lane.base_commit.clone(),
        }
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

/// The four facts every ending of a drive carries, with one named constructor per ending (`WO-402`).
///
/// Seven near-identical `DriveForgeStoryResult` literals used to sit beside those `return`s, and each one had
/// to be read in full to be sure it was the same shape as the last. The endings are three, not seven: a human
/// gate or a hold (`needs_human`), nothing-advanced (`exhausted`), and the run ending on its own terms. The
/// facts are borrowed at the moment the ending is returned, because the loop mutates `steps` and
/// `stopped_after` between endings.
struct StoryEnding<'a> {
    instance_id: &'a str,
    steps: &'a [String],
    stopped_after: &'a Option<String>,
    reconciled: usize,
}

/// Borrow this ending's facts, so a `return` names the ending rather than restating the struct.
fn ending<'a>(
    instance_id: &'a str,
    steps: &'a [String],
    stopped_after: &'a Option<String>,
    reconciled: usize,
) -> StoryEnding<'a> {
    StoryEnding {
        instance_id,
        steps,
        stopped_after,
        reconciled,
    }
}

impl StoryEnding<'_> {
    /// A human gate, or a hold this wave raised: the story needs a person. `blocked_reason` is the hold's
    /// reason, and `None` for a gate — there the hold itself is the record.
    fn needs_human(self, status: String, blocked_reason: Option<String>) -> DriveForgeStoryResult {
        DriveForgeStoryResult {
            instance_id: self.instance_id.to_string(),
            status,
            steps: self.steps.to_vec(),
            exhausted: false,
            blocked_reason,
            needs_human: true,
            stopped_after: self.stopped_after.clone(),
            reconciled: self.reconciled,
        }
    }

    /// Nothing is ready and nothing advanced: the story is not waiting on a person, it is out of work.
    fn exhausted(self, status: String, blocked_reason: Option<String>) -> DriveForgeStoryResult {
        DriveForgeStoryResult {
            instance_id: self.instance_id.to_string(),
            status,
            steps: self.steps.to_vec(),
            exhausted: true,
            blocked_reason,
            needs_human: false,
            stopped_after: self.stopped_after.clone(),
            reconciled: self.reconciled,
        }
    }

    /// The run ended on its own terms: the two facts about the whole run come from the caller.
    fn finished(self, status: String, exhausted: bool, needs_human: bool) -> DriveForgeStoryResult {
        DriveForgeStoryResult {
            instance_id: self.instance_id.to_string(),
            status,
            steps: self.steps.to_vec(),
            exhausted,
            blocked_reason: None,
            needs_human,
            stopped_after: self.stopped_after.clone(),
            reconciled: self.reconciled,
        }
    }
}

/// The story's human gate, when this wave's tasks hold one (`WO-402`).
///
/// The hold is written here and the caller returns the `needs_human` ending: a gate is not a failure, it is the
/// engine handing the story to a person.
fn note_human_gate<S: TxStore>(
    rt: &ForgeRuntime<S>,
    story_id: &str,
    tasks: &[ActiveForgeRoleTask],
) -> Result<bool> {
    if !tasks
        .iter()
        .any(|t| t.node_id.as_deref().map(is_human_gate).unwrap_or(false))
    {
        return Ok(false);
    }
    rt.writer()
        .mark_story_human_hold(story_id, "Forge engine entered a human decision gate.")
        .map_err(|error| {
            WorkflowError::generic(format!("mark_story_human_hold({story_id}): {error}"))
        })?;
    Ok(true)
}

/// Why the drive stopped advancing (`WO-402`): every task and the status it is in, so the hold names what the
/// story is actually waiting on rather than only that nothing was ready.
fn no_ready_task_reason(tasks: &[ActiveForgeRoleTask]) -> String {
    let blocked = tasks
        .iter()
        .map(|t| format!("{}={:?}", t.node_id.as_deref().unwrap_or("?"), t.status))
        .collect::<Vec<_>>()
        .join(", ");
    format!("no ready task; active: {blocked}")
}

/// This wave, planned from the story's ready tasks (`WO-402`), with the wave and every refusal logged.
///
/// A planned lane is only a scheduling hint — the caller re-reads each task's readiness before claiming it — so
/// this step decides batching and nothing else.
fn plan_next_wave(
    tasks: &[ActiveForgeRoleTask],
    cap: usize,
    story_id: &str,
    instance_id: &str,
    wave_index: usize,
) -> WavePlan<ActiveForgeRoleTask> {
    let lanes: Vec<WaveLane<ActiveForgeRoleTask>> = tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Ready)
        .map(|task| WaveLane {
            // A coordinator/fanout flag does not establish that shared
            // effects are safe. Unknown surfaces stay exclusive.
            fanout: false,
            surface: task.write_surface.clone(),
            lane: task.task_id.clone(),
            task: task.clone(),
        })
        .collect();
    let plan = plan_wave(&lanes, cap);
    eprintln!(
        "forge-wave story={} instance={} wave={} ready_lanes={} batches={} conflicts={} configured_cap={} active_lanes=0",
        story_id,
        instance_id,
        wave_index,
        lanes.len(),
        plan.batches.len(),
        plan.refusals.len(),
        cap
    );
    for refusal in &plan.refusals {
        eprintln!(
            "forge-wave-refusal story={} instance={} wave={} task_a={} task_b={} path={} reason=overlapping_or_unknown_write_surface",
            story_id, instance_id, wave_index, refusal.lanes.0, refusal.lanes.1, refusal.path
        );
    }
    plan
}

/// The batch's turn ceiling, as the environment declares it when the batch starts (`WO-402`).
///
/// The turn cap is resolved in one named step instead of at each of the two places a batch runs:
/// `FORGE_TURN_TIMEOUT_MINUTES` is the only input and both callers mean the same thing by it.
fn turn_ceiling_from_env() -> Option<std::time::Duration> {
    turn_ceiling(
        std::env::var(TURN_CEILING_ENV)
            .ok()
            .as_deref()
            .filter(|s| !s.is_empty()),
    )
}

#[cfg(test)]
mod concurrency_tests;
