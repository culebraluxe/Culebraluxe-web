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

/// Fail ONE lane's durable claim, and return a settlement failure that happens on the way (`WO-121`).
///
/// The failure is RETURNED, and the caller folds it into the reason it reports for the lane: a claim
/// that could not be released is a second fact about the lane, not a footnote to it. The line names
/// the job so the stale-claim sweep can find it.
fn fail_claimed_lane(
    jobs: &dyn JobService,
    worker_id: &str,
    lane: &ClaimedDurableLane,
    reason: &str,
    permanent: bool,
) -> Option<String> {
    match jobs.fail(&lane.lease.job_id, worker_id, reason, permanent) {
        Ok(()) => None,
        Err(error) => {
            eprintln!(
                "forge-wave job={} could_not_release_claim error={error}",
                lane.lease.job_id
            );
            Some(error.to_string())
        }
    }
}

fn fail_claimed_batch(
    jobs: &dyn JobService,
    worker_id: &str,
    lanes: &[ClaimedDurableLane],
    reason: &str,
) {
    for lane in lanes {
        fail_claimed_lane(jobs, worker_id, lane, reason, false);
    }
}

/// The primary failure decides. A failure that happens *while* it is being handled is reported beside
/// it, never instead of it (`WO-121`): the primary text stays first, so a reader — and the doctor that
/// scans these lines — still recognises the fault that stopped the lane.
fn with_secondary_failure(primary: &str, what: &str, failure: &str) -> String {
    format!("{primary}; additionally {what} failed: {failure}")
}

/// What the guard calls to remove a lane worktree: `discard_unstarted_lane_worktree`'s own signature.
/// The error is a `String` because that is the worktree module's error type — the guard reports it
/// beside whatever failure it is already handling rather than converting it (`WO-121`).
type LaneWorktreeRemover =
    Box<dyn Fn(&Path, &Path, &str, &str) -> std::result::Result<(), String> + Send>;

/// Where one lane's worktree is, and what Forge decided to do with it.
///
/// `WO-121`. From `provision_worker_workspace` until the drive loop has settled the lane, the worktree
/// is a resource this module owns. Ownership has ONE rule: the disposition is decided and named on
/// every exit, including an unwind. Four `let _ = discard_unstarted_lane_worktree(..)` calls and a
/// `join().expect(..)` were each able to end a lane with a tree nothing could explain.
///
/// Two dispositions, and the difference matters:
///   * `release` — remove the tree, and the branch with it only while the branch still sits at its
///     base. A branch that moved holds a candidate, so it stays, and the log names it.
///   * `retain` — keep the tree, and say where it is and which branch it holds, because a lane that
///     failed may hold uncommitted work and paid code outranks a tidy directory (`MEMORY.md`,
///     "PAID CODE > GIT SHA"). A panic decides this the safe way: it records the retention instead of
///     removing a tree nobody read.
///
/// `Drop` is the backstop. A lane that never reported still gets a disposition and a log line, so no
/// exit — success, error, or panic — can leave a worktree orphaned and unexplained.
struct LaneWorktree {
    repo_root: PathBuf,
    path: PathBuf,
    branch_name: String,
    base_commit: String,
    disposition: LaneWorktreeDisposition,
    remove: LaneWorktreeRemover,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LaneWorktreeDisposition {
    Undecided,
    Released,
    Retained,
}

impl LaneWorktree {
    fn new(repo_root: PathBuf, path: PathBuf, branch_name: String, base_commit: String) -> Self {
        Self::with_remover(
            repo_root,
            path,
            branch_name,
            base_commit,
            Box::new(discard_unstarted_lane_worktree),
        )
    }

    fn with_remover(
        repo_root: PathBuf,
        path: PathBuf,
        branch_name: String,
        base_commit: String,
        remove: LaneWorktreeRemover,
    ) -> Self {
        Self {
            repo_root,
            path,
            branch_name,
            base_commit,
            disposition: LaneWorktreeDisposition::Undecided,
            remove,
        }
    }

    /// A guard for a lane whose owner exited before it could hand the tree over — an early return or a
    /// panic. That exit left the tree in place and its own guard logged where, so this disposition is
    /// already settled and a second removal here would misreport what happened.
    fn retained_by_its_lane(path: PathBuf, branch_name: String) -> Self {
        Self {
            repo_root: PathBuf::new(),
            path,
            branch_name,
            base_commit: String::new(),
            disposition: LaneWorktreeDisposition::Retained,
            remove: Box::new(|_, _, _, _| Ok(())),
        }
    }

    /// What the guard decided. Read by the tests that assert a lane's exit disposition (`WO-121`).
    #[cfg(test)]
    fn disposition(&self) -> LaneWorktreeDisposition {
        self.disposition
    }

    /// Release the tree. A cleanup failure is RETURNED, never swallowed: the caller is already handling
    /// a primary failure and has to report both.
    fn release(&mut self, task_id: &str) -> Option<String> {
        if self.disposition != LaneWorktreeDisposition::Undecided {
            return None;
        }
        self.disposition = LaneWorktreeDisposition::Released;
        match (self.remove)(
            &self.repo_root,
            &self.path,
            &self.branch_name,
            &self.base_commit,
        ) {
            Ok(()) => None,
            Err(error) => {
                eprintln!(
                    "forge-wave task={task_id} worktree_cleanup_failed={error}; branch={} remains reachable",
                    self.branch_name
                );
                Some(error)
            }
        }
    }

    /// Keep the tree and name it: the path is what an operator inspects, and the branch is what
    /// survives the tree.
    fn retain(&mut self, task_id: &str, why: &str) {
        if self.disposition != LaneWorktreeDisposition::Undecided {
            return;
        }
        self.disposition = LaneWorktreeDisposition::Retained;
        eprintln!(
            "forge-wave task={task_id} {why}; isolated worktree retained at {} branch={}",
            self.path.display(),
            self.branch_name
        );
    }
}

impl Drop for LaneWorktree {
    fn drop(&mut self) {
        if self.disposition == LaneWorktreeDisposition::Undecided {
            // Nobody released or retained this tree: its lane unwound before it reported. Retain it —
            // a panic is not evidence that the tree is empty — but never silently.
            self.disposition = LaneWorktreeDisposition::Retained;
            eprintln!(
                "forge-wave worktree_disposition=retained_on_exit worktree={} branch={} reason=the lane exited before it handed the tree over",
                self.path.display(),
                self.branch_name
            );
        }
    }
}

fn current_head(repo: &std::path::Path) -> Result<String> {
    let output = Command::new(crate::engine::worktree::git_binary())
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(repo)
        .output()
        .map_err(|error| WorkflowError::generic(format!("git rev-parse HEAD: {error}")))?;
    if !output.status.success() {
        return Err(WorkflowError::generic(format!(
            "could not resolve the story worktree HEAD: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn prepare_durable_batch<S: TxStore>(
    rt: &ForgeRuntime<S>,
    story_id: &str,
    planned: &[WaveLane<ActiveForgeRoleTask>],
    durable: DurableForgeExecution<'_, '_>,
    worker_id: &str,
) -> Result<Vec<PreparedDurableLane>> {
    let mut claimed = Vec::new();
    for lane in planned {
        let task_id = lane.task.task_id.clone();
        let current_tasks = match rt.list_role_tasks(story_id) {
            Ok(tasks) => tasks,
            Err(error) => {
                fail_claimed_batch(durable.jobs, worker_id, &claimed, &error.to_string());
                return Err(error);
            }
        };
        let Some(task) = current_tasks
            .into_iter()
            .find(|task| task.task_id == task_id && task.status == TaskStatus::Ready)
        else {
            eprintln!("forge-wave task={task_id} skipped=stale-readiness");
            continue;
        };
        let node = task.node_id.clone().unwrap_or_default();
        let actor = task
            .candidates
            .first()
            .cloned()
            .unwrap_or_else(|| worker_id.to_string());
        let request = match ForgeJobBridge::new(durable.registry).job_for_ready_task(&task) {
            Ok(request) => request,
            Err(error) => {
                fail_claimed_batch(durable.jobs, worker_id, &claimed, &error.to_string());
                return Err(error);
            }
        };
        let job_id = match durable.jobs.enqueue(&request) {
            Ok(job_id) => job_id,
            Err(error) => {
                fail_claimed_batch(durable.jobs, worker_id, &claimed, &error.to_string());
                return Err(error);
            }
        };
        let lease = match durable.jobs.claim_one(&job_id, worker_id) {
            Ok(lease) => lease,
            Err(error) => {
                fail_claimed_batch(durable.jobs, worker_id, &claimed, &error.to_string());
                return Err(error);
            }
        };
        claimed.push(ClaimedDurableLane {
            task,
            actor,
            node,
            lease,
        });
    }

    let mut prepared = Vec::new();
    for lane in claimed.iter() {
        let preparation = (|| -> Result<PreparedDurableLane> {
            let service = durable.registry.resolve(&lane.lease.service_key)?;
            let runner = service.runner();
            let Some(ports) = runner.turn_ports() else {
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    format!(
                        "service {} does not expose production turn ports for isolated execution",
                        lane.lease.service_key
                    ),
                ));
            };
            let harness = ports.harness();
            if !harness.supports_interrupt() {
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    format!(
                        "harness for service {} cannot isolate and interrupt a concurrent execution",
                        lane.lease.service_key
                    ),
                ));
            }
            let Some(parent_execution) = harness.execution_workspace().cloned() else {
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    "concurrent durable lanes require a provisioned story worktree; enable FORGE_PROVISION=1",
                ));
            };
            if !parent_execution.branch_name.starts_with("agent/") {
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    format!(
                        "story worktree branch {:?} is not an isolated agent branch",
                        parent_execution.branch_name
                    ),
                ));
            }
            let parent_worktree = PathBuf::from(&parent_execution.worktree_path);
            if !parent_worktree.is_dir() {
                return Err(WorkflowError::generic(format!(
                    "story worktree is missing: {}",
                    parent_worktree.display()
                )));
            }
            let repo_root =
                resolve_repo_root(Some(&parent_worktree)).map_err(WorkflowError::generic)?;
            let worktrees_root = repo_root
                .parent()
                .unwrap_or(&repo_root)
                .join(DEFAULT_WORKTREES_DIRNAME);
            let base_commit = current_head(&parent_worktree)?;
            let run_id = format!(
                "lane-{}-{}-{}-{}",
                lane.lease.job_id,
                lane.lease.attempts,
                lane.lease.process_instance_id,
                std::process::id()
            );
            let workspace = provision_worker_workspace(
                Some(&parent_worktree),
                story_id,
                Some(&run_id),
                Some(&base_commit),
                Some(&worktrees_root),
            )
            .map_err(WorkflowError::generic)?;
            let branch_name = workspace.branch_name.clone();
            let execution_workspace = ExecutionWorkspace {
                worktree_path: workspace.worktree_path.display().to_string(),
                branch_name: branch_name.clone(),
                base_ref: workspace.base_ref,
                base_commit: workspace.base_commit,
            };
            // The lane's tree is owned from here on (`WO-121`). Every exit below either releases it or
            // retains it with a name; the guard's `Drop` covers only the exit nobody wrote.
            let mut worktrees = LaneWorktree::new(
                repo_root.clone(),
                workspace.worktree_path.clone(),
                branch_name.clone(),
                base_commit.clone(),
            );
            let forked_runner = match runner.fork_for_workspace(execution_workspace) {
                Ok(Some(runner)) => runner,
                Ok(None) => {
                    let primary = format!(
                        "service {} harness cannot create an isolated lane execution",
                        lane.lease.service_key
                    );
                    return Err(WorkflowError::conflict(
                        "FORGE_PARALLEL_LANES_UNAVAILABLE",
                        match worktrees.release(&lane.task.task_id) {
                            Some(cleanup) => {
                                with_secondary_failure(&primary, "worktree discard", &cleanup)
                            }
                            None => primary,
                        },
                    ));
                }
                Err(error) => {
                    // The fork error is returned as it came: its code is how the caller classifies it,
                    // and a message is not worth losing a classification for. The cleanup failure is
                    // reported beside it instead (`WO-121`).
                    if let Some(cleanup) = worktrees.release(&lane.task.task_id) {
                        eprintln!(
                            "forge-wave task={} lane_fork_failed={error} worktree_discard_failed={cleanup}",
                            lane.task.task_id
                        );
                    }
                    return Err(error);
                }
            };
            if !forked_runner
                .turn_ports()
                .map(|ports| ports.harness().supports_interrupt())
                .unwrap_or(false)
            {
                let primary = format!(
                    "forked runner for service {} lost execution-scoped interruption",
                    lane.lease.service_key
                );
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    match worktrees.release(&lane.task.task_id) {
                        Some(cleanup) => {
                            with_secondary_failure(&primary, "worktree discard", &cleanup)
                        }
                        None => primary,
                    },
                ));
            }
            Ok(PreparedDurableLane {
                claimed: ClaimedDurableLane {
                    task: lane.task.clone(),
                    actor: lane.actor.clone(),
                    node: lane.node.clone(),
                    lease: lane.lease.clone(),
                },
                runner: forked_runner,
                worktree_path: workspace.worktree_path,
                branch_name,
                parent_worktree,
                base_commit,
                worktrees,
            })
        })();
        match preparation {
            Ok(prepared_lane) => prepared.push(prepared_lane),
            Err(error) => {
                // The batch is abandoned, so every lane that was already provisioned is released — and
                // a release that fails is reported beside the error that abandoned the batch, never
                // instead of it (`WO-121`).
                for lane in &mut prepared {
                    if let Some(cleanup) = lane.worktrees.release(&lane.claimed.task.task_id) {
                        eprintln!(
                            "forge-wave task={} batch_abandoned={error} worktree_discard_failed={cleanup}",
                            lane.claimed.task.task_id
                        );
                    }
                }
                fail_claimed_batch(durable.jobs, worker_id, &claimed, &error.to_string());
                return Err(error);
            }
        }
    }
    Ok(prepared)
}

fn execute_durable_batch(
    prepared: Vec<PreparedDurableLane>,
    jobs: &dyn JobService,
    worker_id: &str,
    turn_ceiling: Option<std::time::Duration>,
    story_id: &str,
    wave_index: usize,
) -> Vec<LaneExecutionResult> {
    let active_lanes = Arc::new(AtomicUsize::new(0));
    let lane_fallbacks: Vec<LaneFallback> = prepared.iter().map(LaneFallback::of).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = prepared
            .into_iter()
            .map(|prepared_lane| {
                let active_lanes = Arc::clone(&active_lanes);
                let story_id = story_id.to_string();
                let fallback_claimed = prepared_lane.claimed.clone();
                let fallback_worktree = prepared_lane.worktree_path.clone();
                let fallback_parent = prepared_lane.parent_worktree.clone();
                let fallback_base = prepared_lane.base_commit.clone();
                let fallback_branch = prepared_lane.branch_name.clone();
                scope.spawn(move || {
                    let active = active_lanes.fetch_add(1, Ordering::SeqCst) + 1;
                    eprintln!(
                        "forge-wave-lane story={} instance={} wave={} task={} job={} attempt={} active_lanes={} event=start",
                        story_id,
                        fallback_claimed.task.process_instance_id,
                        wave_index,
                        fallback_claimed.task.task_id,
                        fallback_claimed.lease.job_id,
                        fallback_claimed.lease.attempts,
                        active
                    );
                    let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || -> Result<LaneExecutionResult> {
                            let PreparedDurableLane {
                                claimed,
                                runner,
                                worktree_path: _,
                                branch_name: _,
                                parent_worktree,
                                base_commit,
                                worktrees,
                            } = prepared_lane;
                            let services = ForgeLaneServices::new(runner.as_ref());
                            let registry = services.registry()?;
                            let outcome = execute_claimed_job_unsettled(
                                jobs,
                                worker_id,
                                &claimed.lease,
                                &claimed.task,
                                &registry,
                                None,
                                turn_ceiling,
                            );
                            Ok(LaneExecutionResult {
                                claimed,
                                parent_worktree,
                                base_commit,
                                worktrees,
                                settlement_failure: None,
                                outcome,
                            })
                        },
                    ));
                    let result = match execution {
                        Ok(Ok(result)) => result,
                        Ok(Err(error)) => {
                            let settlement_failure = fail_claimed_lane(
                                jobs,
                                worker_id,
                                &fallback_claimed,
                                &error.to_string(),
                                true,
                            );
                            let worktrees = LaneWorktree::retained_by_its_lane(
                                fallback_worktree.clone(),
                                fallback_branch.clone(),
                            );
                            LaneExecutionResult {
                                claimed: fallback_claimed,
                                parent_worktree: fallback_parent,
                                base_commit: fallback_base,
                                worktrees,
                                settlement_failure,
                                outcome: Err(error),
                            }
                        }
                        Err(_) => {
                            let reason = "lane worker panicked before it could report an outcome";
                            let settlement_failure =
                                fail_claimed_lane(jobs, worker_id, &fallback_claimed, reason, true);
                            let worktrees = LaneWorktree::retained_by_its_lane(
                                fallback_worktree.clone(),
                                fallback_branch.clone(),
                            );
                            LaneExecutionResult {
                                claimed: fallback_claimed,
                                parent_worktree: fallback_parent,
                                base_commit: fallback_base,
                                worktrees,
                                settlement_failure,
                                outcome: Err(WorkflowError::generic(reason)),
                            }
                        }
                    };
                    let active = active_lanes.fetch_sub(1, Ordering::SeqCst) - 1;
                    eprintln!(
                        "forge-wave-lane story={} instance={} wave={} task={} job={} attempt={} active_lanes={} event=joined",
                        story_id,
                        result.claimed.task.process_instance_id,
                        wave_index,
                        result.claimed.task.task_id,
                        result.claimed.lease.job_id,
                        result.claimed.lease.attempts,
                        active
                    );
                    result
                })
            })
            .collect();
        // A thread that panicked OUTSIDE the lane's inner `catch_unwind` used to escape through this
        // `join().expect(..)` as a panic that named no lane and left every worktree in the batch behind
        // it (`WO-121` §3). It is a recorded per-lane failure now: the tree was dispositioned by the
        // lane's own guard during the unwind, and the claim is settled here.
        let mut results = Vec::with_capacity(handles.len());
        for (handle, fallback) in handles.into_iter().zip(lane_fallbacks) {
            match handle.join() {
                Ok(result) => results.push(result),
                Err(_) => {
                    let reason = "lane worker thread panicked before it could report an outcome";
                    results.push(lane_that_never_reported(jobs, worker_id, fallback, reason));
                }
            }
        }
        results
    })
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

fn drive_forge_story_inner<S: TxStore>(
    rt: &ForgeRuntime<S>,
    story_id: &str,
    opts: DriveForgeStoryOptions<'_>,
    durable: Option<DurableForgeExecution<'_, '_>>,
) -> Result<DriveForgeStoryResult> {
    validate_story_concurrency(opts.within_story_concurrency, durable.is_some())?;
    if durable.is_none() && opts.runner.is_none() {
        return Err(WorkflowError::generic(
            "Direct Forge execution requires an explicit real role runner; there is no synthetic runner. Durable production execution resolves concrete services through JobService + ForgeServiceRegistry.",
        ));
    }
    let stop_target = resolve_forge_stop_target(opts.stop_after.as_ref());
    let mut stopped_after = None;
    let mut steps = Vec::new();
    let mut wave_index = 0usize;

    let wake = rt.wake_story(story_id, opts.work_type, opts.evidence.clone())?;
    let instance_id = wake.instance_id;
    let reconciled = wake.reconciled;

    if let Some(durable) = durable {
        durable.jobs.recover_stale(256)?;
    }

    for _ in 0..opts.max_steps {
        let tasks = rt.list_role_tasks(story_id)?;
        if tasks.is_empty() {
            break;
        }
        if note_human_gate(rt, story_id, &tasks)? {
            return Ok(ending(&instance_id, &steps, &stopped_after, reconciled)
                .needs_human(instance_status(rt, &instance_id)?, None));
        }
        if !tasks.iter().any(|t| t.status == TaskStatus::Ready) {
            return Ok(
                ending(&instance_id, &steps, &stopped_after, reconciled).exhausted(
                    instance_status(rt, &instance_id)?,
                    Some(no_ready_task_reason(&tasks)),
                ),
            );
        }

        let cap = opts
            .within_story_concurrency
            .clamp(1, turn_budget::MAX_WITHIN_STORY_CONCURRENCY);
        wave_index += 1;
        let plan = plan_next_wave(&tasks, cap, story_id, &instance_id, wave_index);
        for batch in plan.batches {
            if cap > 1 && batch.len() > 1 {
                let Some(durable) = durable else {
                    return Err(WorkflowError::conflict(
                        "FORGE_PARALLEL_LANES_UNAVAILABLE",
                        "parallel batches require durable job ownership",
                    ));
                };
                validate_parallel_execution(durable.registry)?;
                let prepared =
                    prepare_durable_batch(rt, story_id, &batch, durable, opts.worker_id)?;
                if prepared.is_empty() {
                    continue;
                }
                eprintln!(
                    "forge-wave story={} instance={} wave={} prepared_lanes={} configured_cap={}",
                    story_id,
                    instance_id,
                    wave_index,
                    prepared.len(),
                    cap
                );
                let turn_ceiling = turn_ceiling_from_env();
                let mut executions = execute_durable_batch(
                    prepared,
                    durable.jobs,
                    opts.worker_id,
                    turn_ceiling,
                    story_id,
                    wave_index,
                );
                executions.sort_by(|a, b| a.claimed.task.task_id.cmp(&b.claimed.task.task_id));
                let mut hold_reason: Option<String> = None;
                let mut engine_fault: Option<WorkflowError> = None;
                for mut execution in executions {
                    // The verdict channels are decided by one function (`WO-302` Part A) so a business
                    // Hold and a sibling engine fault cannot be weighed two ways. Classified before the
                    // outcome moves: the channels are owned, the error object stays the lane's.
                    let channels = classify_lane_failure(&execution);
                    let outcome = match execution.outcome {
                        Ok(outcome) => outcome,
                        Err(error) => {
                            let (stop_kind, role_verdict) = lane_stop_diagnostic(&error);
                            eprintln!(
                                "forge-execution-stop story={} instance={} wave={} task={} job={} attempt={} stop={} error_code={} role_verdict={} detail={}",
                                story_id,
                                instance_id,
                                wave_index,
                                execution.claimed.task.task_id,
                                execution.claimed.lease.job_id,
                                execution.claimed.lease.attempts,
                                stop_kind,
                                error.code(),
                                role_verdict,
                                error
                            );
                            if channels.is_engine_fault {
                                if engine_fault.is_none() {
                                    engine_fault = Some(error);
                                }
                            } else if hold_reason.is_none() {
                                hold_reason = Some(channels.reason.clone());
                            }
                            // The tree is RETAINED, and the guard is what says so: a lane that failed may
                            // hold work no branch has yet (`WO-121`), so the disposition is decided and
                            // named — path and branch — rather than left to a `let _ =`.
                            execution.worktrees.retain(
                                &execution.claimed.task.task_id,
                                &format!("execution_failed={}", channels.reason),
                            );
                            continue;
                        }
                    };

                    let candidate = outcome.evidence.candidate_sha.clone();
                    if let Err(error) = complete_role_task_with_transient_retry(
                        rt,
                        &execution.claimed.task.task_id,
                        &execution.claimed.actor,
                        &outcome,
                    ) {
                        if error.code() == "TASK_ALREADY_COMPLETED" {
                            // Settle through the shared helper (`WO-302` Part B): a transient complete
                            // failure here must hold with the failure named, never orphan the job.
                            if let Some(reason) = settle_completed_lane(
                                durable.jobs,
                                opts.worker_id,
                                &execution.claimed.lease.job_id,
                                &execution.claimed.task.task_id,
                                true,
                            ) {
                                if hold_reason.is_none() {
                                    hold_reason = Some(reason);
                                }
                            }
                            // This release used to be `let _ = remove_lane_worktree(..)`: a tree Forge could
                            // not remove was not worth a line. It is reported now (`WO-121`). The task is
                            // still complete, so the loop continues — but the story is held when nothing
                            // else held it, because a tree left behind is Forge's to answer for.
                            if let Some(cleanup) =
                                execution.worktrees.release(&execution.claimed.task.task_id)
                            {
                                if hold_reason.is_none() {
                                    hold_reason = Some(format!(
                                        "task {} was already completed, but its lane worktree could not be released: {cleanup}",
                                        execution.claimed.task.task_id
                                    ));
                                }
                            }
                            continue;
                        }
                        let reason = completion_failure_reason(&error);
                        if let Err(settle_error) = durable.jobs.fail(
                            &execution.claimed.lease.job_id,
                            opts.worker_id,
                            &reason,
                            true,
                        ) {
                            eprintln!(
                                "forge-wave task={} durable_settlement_failed={settle_error}",
                                execution.claimed.task.task_id
                            );
                        }
                        if hold_reason.is_none() {
                            hold_reason = Some(format!(
                                "completion failed for task {}: {error}",
                                execution.claimed.task.task_id
                            ));
                        }
                        continue;
                    }
                    if let Some(candidate) = candidate.as_deref() {
                        if let Err(error) = integrate_lane_candidate(
                            &execution.parent_worktree,
                            &execution.base_commit,
                            candidate,
                        ) {
                            let primary = format!(
                                "task {} completed in Workflow, but candidate {candidate} could not be integrated: {error}",
                                execution.claimed.task.task_id
                            );
                            // The claim is settled through the shared helper, so a settlement failure that
                            // happens HERE is reported beside the integration failure (`WO-121`) instead of
                            // being dropped by a `let _ =`.
                            let reason = match fail_claimed_lane(
                                durable.jobs,
                                opts.worker_id,
                                &execution.claimed,
                                &primary,
                                true,
                            ) {
                                Some(settlement) => with_secondary_failure(
                                    &primary,
                                    "durable claim settlement",
                                    &settlement,
                                ),
                                None => primary,
                            };
                            if hold_reason.is_none() {
                                hold_reason = Some(reason);
                            }
                            execution.worktrees.retain(
                                &execution.claimed.task.task_id,
                                &format!("integration_failed={error}"),
                            );
                            continue;
                        }
                    }
                    // Settle through the shared helper (`WO-302` Part B): the completed outcome is
                    // already recorded above, so a transient complete failure holds with the failure
                    // named instead of orphaning the job.
                    if let Some(reason) = settle_completed_lane(
                        durable.jobs,
                        opts.worker_id,
                        &execution.claimed.lease.job_id,
                        &execution.claimed.task.task_id,
                        false,
                    ) {
                        if hold_reason.is_none() {
                            hold_reason = Some(reason);
                        }
                    }
                    // The lane is done, so its tree is released through the guard (`WO-121`). A release that
                    // fails is logged by the guard AND becomes the story's reason to be held when nothing
                    // else held it: a tree Forge could not remove is a fact an operator has to see, not a
                    // line in a log nobody reads.
                    if let Some(cleanup) =
                        execution.worktrees.release(&execution.claimed.task.task_id)
                    {
                        if hold_reason.is_none() {
                            hold_reason = Some(format!(
                                "task {} completed, but its lane worktree could not be released: {cleanup}",
                                execution.claimed.task.task_id
                            ));
                        }
                    }
                    steps.push(execution.claimed.node.clone());
                    if stop_target
                        .as_ref()
                        .map(|stop| stop.contains(&execution.claimed.node))
                        .unwrap_or(false)
                    {
                        stopped_after = Some(execution.claimed.node.clone());
                    }
                }
                // A genuine business Hold beats a sibling engine fault (`WO-302` Part A): the fault
                // stays observable (its own forge-execution-stop line above, plus attached to the Hold
                // reason), but it must not erase the verdict. Only with no Hold does the fault propagate.
                match wave_disposition(hold_reason, engine_fault) {
                    WaveDisposition::Hold(reason) => {
                        eprintln!(
                            "forge-story-stop story={} instance={} wave={} reason={}",
                            story_id, instance_id, wave_index, reason
                        );
                        rt.writer()
                            .mark_story_human_hold(story_id, &reason)
                            .map_err(|error| {
                                WorkflowError::generic(format!(
                                    "mark_story_human_hold({story_id}): {error}"
                                ))
                            })?;
                        return Ok(ending(&instance_id, &steps, &stopped_after, reconciled)
                            .needs_human(instance_status(rt, &instance_id)?, Some(reason)));
                    }
                    WaveDisposition::EngineFault(error) => {
                        return Err(error);
                    }
                    WaveDisposition::Clear => {}
                }
                if stopped_after.is_some() {
                    break;
                }
                continue;
            }
            for lane in batch {
                // A planned lane is only a scheduling hint. Another lane or
                // external owner may have advanced/cancelled it since the
                // snapshot was built; re-read authoritative workflow state
                // immediately before enqueue/claim.
                let planned_task_id = lane.task.task_id.clone();
                let Some(task) = rt.list_role_tasks(story_id)?.into_iter().find(|task| {
                    task.task_id == planned_task_id && task.status == TaskStatus::Ready
                }) else {
                    eprintln!(
                        "forge-wave task={} skipped=stale-readiness",
                        planned_task_id
                    );
                    continue;
                };
                let node = task.node_id.clone().unwrap_or_default();
                let actor = task
                    .candidates
                    .first()
                    .cloned()
                    .unwrap_or_else(|| opts.worker_id.to_string());
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
                                        "Forge durable job {job_id} for task {} ({node}) was already {:?} by an \
                                         earlier run, so this run executed nothing. That run's error: {detail}. \
                                         It is waiting for that turn's completion to be reconciled.",
                                        task.task_id, state.status
                                    );
                                    rt.writer()
                                        .mark_story_human_hold(story_id, &reason)
                                        .map_err(|error| {
                                            WorkflowError::generic(format!(
                                                "mark_story_human_hold({story_id}): {error}"
                                            ))
                                        })?;
                                    return Ok(ending(
                                        &instance_id,
                                        &steps,
                                        &stopped_after,
                                        reconciled,
                                    )
                                    .needs_human(
                                        instance_status(rt, &instance_id)?,
                                        Some(reason),
                                    ));
                                }
                                JobStatus::Locked | JobStatus::Pending => {
                                    return Ok(ending(
                                        &instance_id,
                                        &steps,
                                        &stopped_after,
                                        reconciled,
                                    )
                                    .exhausted(
                                        instance_status(rt, &instance_id)?,
                                        Some(format!(
                                            "Forge durable job {job_id} for task {} is {:?}; attempts={}/{} locked_by={:?}",
                                            task.task_id,
                                            state.status,
                                            state.attempts,
                                            state.max_attempts,
                                            state.locked_by
                                        )),
                                    ));
                                }
                            }
                        }
                        Err(err) => return Err(err),
                    };
                    // Durable production composes execution-scoped interruption from the registered
                    // service inside the job boundary, where the exact lease identity is available.
                    let _request =
                        ForgeJobBridge::new(durable.registry).job_for_ready_task(&task)?;
                    // Use the shared Forge turn budget regardless of the selected harness.
                    let turn_ceiling = turn_ceiling_from_env();

                    let outcome = match execute_claimed_job_unsettled(
                        durable.jobs,
                        opts.worker_id,
                        &lease,
                        &task,
                        durable.registry,
                        None,
                        turn_ceiling,
                    ) {
                        Ok(outcome) => outcome,
                        Err(err) if is_engine_fault_error(&err) => return Err(err),
                        Err(err) => {
                            let (stop_kind, role_verdict) = lane_stop_diagnostic(&err);
                            let reason = lane_failure_reason(&node, &task.task_id, &err);
                            eprintln!(
                                "forge-execution-stop story={} instance={} wave={} task={} job={} attempt={} stop={} error_code={} role_verdict={} detail={}",
                                story_id,
                                instance_id,
                                wave_index,
                                task.task_id,
                                lease.job_id,
                                lease.attempts,
                                stop_kind,
                                err.code(),
                                role_verdict,
                                err
                            );
                            rt.writer()
                                .mark_story_human_hold(story_id, &reason)
                                .map_err(|error| {
                                    WorkflowError::generic(format!(
                                        "mark_story_human_hold({story_id}): {error}"
                                    ))
                                })?;
                            return Ok(ending(&instance_id, &steps, &stopped_after, reconciled)
                                .needs_human(instance_status(rt, &instance_id)?, Some(reason)));
                        }
                    };
                    durable_lease = Some(lease);
                    outcome
                } else {
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
                            if let Err(complete_error) =
                                durable.jobs.complete(&lease.job_id, opts.worker_id)
                            {
                                eprintln!(
                                    "forge-wave task {} already completed but durable job settlement failed (preserving paid turn for reconciliation): {complete_error}",
                                    task.task_id
                                );
                            }
                            continue;
                        }
                        if let Err(fail_error) = durable.jobs.fail(
                            &lease.job_id,
                            opts.worker_id,
                            &completion_failure_reason(&err),
                            true,
                        ) {
                            eprintln!(
                                "forge-wave task={} durable_settlement_failed={fail_error}",
                                task.task_id
                            );
                        }
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

                if let (Some(durable), Some(lease)) = (durable, durable_lease.as_ref()) {
                    if let Err(complete_error) =
                        durable.jobs.complete(&lease.job_id, opts.worker_id)
                    {
                        let reason = completion_failure_reason(&WorkflowError::generic(
                            complete_error.to_string(),
                        ));
                        eprintln!(
                            "forge-wave task {} completed but durable job settlement failed (preserving paid turn for reconciliation): {reason}",
                            task.task_id
                        );
                    }
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
            if stopped_after.is_some() {
                break;
            }
        }
        if stopped_after.is_some() {
            break;
        }
    }

    let tasks = rt.list_role_tasks(story_id)?;
    let instance = rt.engine().get_process_instance(&instance_id)?;

    if process_completes_story(instance.status, instance.outcome) {
        rt.writer().mark_story_complete(story_id).map_err(|error| {
            WorkflowError::generic(format!("mark_story_complete({story_id}): {error}"))
        })?;
    }

    Ok(
        ending(&instance_id, &steps, &stopped_after, reconciled).finished(
            format!("{:?}", instance.status),
            !tasks.is_empty(),
            tasks
                .iter()
                .any(|t| t.node_id.as_deref().map(is_human_gate).unwrap_or(false)),
        ),
    )
}

#[cfg(test)]
mod concurrency_tests {
    use super::*;
    use crate::engine::facts::ForgeGateEvidence;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};
    use std::time::Duration;
    use workflow::WorkflowError;

    #[derive(Default)]
    struct OverlapGate {
        started: Mutex<usize>,
        changed: Condvar,
        active: AtomicUsize,
        maximum_active: AtomicUsize,
        both_entered: AtomicUsize,
    }

    struct BlockingRunner {
        gate: Arc<OverlapGate>,
        fail: bool,
    }

    impl ForgeRoleRunner for BlockingRunner {
        fn run(&self, _node_id: &str, _task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
            let active = self.gate.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.gate.maximum_active.fetch_max(active, Ordering::SeqCst);
            let mut started = self.gate.started.lock().unwrap();
            *started += 1;
            self.gate.changed.notify_all();
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while *started < 2 {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                let (next, timeout) = self.gate.changed.wait_timeout(started, remaining).unwrap();
                started = next;
                if timeout.timed_out() {
                    break;
                }
            }
            if *started >= 2 {
                self.gate.both_entered.fetch_add(1, Ordering::SeqCst);
            }
            drop(started);
            self.gate.active.fetch_sub(1, Ordering::SeqCst);
            if self.fail {
                return Err(WorkflowError::generic("planned lane failure"));
            }
            Ok(ForgeRoleOutcome {
                transition_name: Some("complete".into()),
                evidence: ForgeGateEvidence::default(),
            })
        }

        fn run_scoped(
            &self,
            _execution_id: &str,
            node_id: &str,
            task: &ActiveForgeRoleTask,
        ) -> Result<ForgeRoleOutcome> {
            self.run(node_id, task)
        }
    }

    /// A lane that panics instead of reporting (`WO-121` §3): the drive wrapper has to turn that into a
    /// recorded per-lane failure instead of letting the thread take the batch with it.
    struct PanicRunner;

    impl ForgeRoleRunner for PanicRunner {
        fn run(&self, _node_id: &str, _task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
            panic!("lane runner panicked");
        }
    }

    struct NoopJobs;

    impl JobService for NoopJobs {
        fn enqueue(&self, _request: &crate::engine::job::ForgeJobRequest) -> Result<String> {
            unreachable!()
        }
        fn claim(&self, _worker_id: &str, _limit: usize) -> Result<Vec<ForgeJobLease>> {
            unreachable!()
        }
        fn claim_one(&self, _job_id: &str, _worker_id: &str) -> Result<ForgeJobLease> {
            unreachable!()
        }
        fn heartbeat(&self, _job_id: &str, _worker_id: &str) -> Result<i64> {
            Ok(1)
        }
        fn inspect(&self, _job_id: &str) -> Result<crate::engine::job::ForgeJobState> {
            Err(WorkflowError::generic("unused"))
        }
        fn complete(&self, _job_id: &str, _worker_id: &str) -> Result<()> {
            Ok(())
        }
        fn fail(
            &self,
            _job_id: &str,
            _worker_id: &str,
            _error: &str,
            _permanent: bool,
        ) -> Result<()> {
            Ok(())
        }
        fn cancel(&self, _job_id: &str, _actor: &str) -> Result<()> {
            Ok(())
        }
        fn requeue(&self, _job_id: &str, _actor: &str) -> Result<()> {
            Ok(())
        }
        fn recover_stale(&self, _batch: usize) -> Result<usize> {
            Ok(0)
        }
    }

    /// Jobs that refuse to settle a claim. `WO-121` is about exactly this: the failure that happens while
    /// another failure is being handled, which used to be dropped by a `let _ =`.
    struct RefusingFailJobs;

    impl JobService for RefusingFailJobs {
        fn enqueue(&self, _request: &crate::engine::job::ForgeJobRequest) -> Result<String> {
            unreachable!()
        }
        fn claim(&self, _worker_id: &str, _limit: usize) -> Result<Vec<ForgeJobLease>> {
            unreachable!()
        }
        fn claim_one(&self, _job_id: &str, _worker_id: &str) -> Result<ForgeJobLease> {
            unreachable!()
        }
        fn heartbeat(&self, _job_id: &str, _worker_id: &str) -> Result<i64> {
            Ok(1)
        }
        fn inspect(&self, _job_id: &str) -> Result<crate::engine::job::ForgeJobState> {
            Err(WorkflowError::generic("unused"))
        }
        fn complete(&self, _job_id: &str, _worker_id: &str) -> Result<()> {
            Ok(())
        }
        fn fail(
            &self,
            _job_id: &str,
            _worker_id: &str,
            _error: &str,
            _permanent: bool,
        ) -> Result<()> {
            Err(WorkflowError::generic("claim already lost"))
        }
        fn cancel(&self, _job_id: &str, _actor: &str) -> Result<()> {
            Ok(())
        }
        fn requeue(&self, _job_id: &str, _actor: &str) -> Result<()> {
            Ok(())
        }
        fn recover_stale(&self, _batch: usize) -> Result<usize> {
            Ok(0)
        }
    }

    fn prepared_lane(task_id: &str, gate: &Arc<OverlapGate>, fail: bool) -> PreparedDurableLane {
        prepared_lane_with_runner(
            task_id,
            Box::new(BlockingRunner {
                gate: Arc::clone(gate),
                fail,
            }),
        )
    }

    fn prepared_lane_with_runner(
        task_id: &str,
        runner: Box<dyn ForgeRoleRunner>,
    ) -> PreparedDurableLane {
        let task = ActiveForgeRoleTask {
            task_id: task_id.into(),
            process_instance_id: "instance-1".into(),
            story_id: "story-1".into(),
            token_id: Some(format!("token-{task_id}")),
            node_id: Some("smith".into()),
            status: TaskStatus::Ready,
            assignee: None,
            candidates: vec![],
            write_surface: Some(vec![format!("src/{task_id}.rs")]),
        };
        let lease = ForgeJobLease {
            job_id: format!("job-{task_id}"),
            service_key: "forge.smith".into(),
            node_id: "smith".into(),
            task_id: task_id.into(),
            process_instance_id: "instance-1".into(),
            story_id: "story-1".into(),
            token_id: task.token_id.clone(),
            attempts: 1,
            max_attempts: 5,
            locked_until: None,
        };
        PreparedDurableLane {
            claimed: ClaimedDurableLane {
                task,
                actor: "worker-1".into(),
                node: "smith".into(),
                lease,
            },
            runner,
            worktree_path: PathBuf::from(format!("/tmp/{task_id}")),
            branch_name: format!("agent/story/{task_id}"),
            parent_worktree: PathBuf::from("/tmp/story"),
            base_commit: "0".repeat(40),
            // A remover that removes nothing: a unit test must not shell out to git, and the guard's own
            // behaviour is what these tests are about (`WO-121`).
            worktrees: LaneWorktree::with_remover(
                PathBuf::from("/tmp"),
                PathBuf::from(format!("/tmp/{task_id}")),
                format!("agent/story/{task_id}"),
                "0".repeat(40),
                Box::new(|_, _, _, _| Ok(())),
            ),
        }
    }

    #[test]
    fn unsupported_overlap_is_rejected_before_claiming_work() {
        assert!(validate_story_concurrency(1, false).is_ok());
        assert!(validate_story_concurrency(2, true).is_ok());
        let error = validate_story_concurrency(2, false).unwrap_err();
        assert_eq!(error.code(), "FORGE_PARALLEL_LANES_UNAVAILABLE");
    }

    #[test]
    fn cap_refusal_and_interruption_are_stops_without_a_role_verdict() {
        let capped = WorkflowError::conflict(
            turn_budget::MODEL_TURN_CAP_CODE,
            "generation exhausted its model-attempt allowance",
        );
        assert_eq!(lane_stop_diagnostic(&capped), ("model_attempt_cap", false));
        assert!(lane_failure_reason("smith", "task-1", &capped)
            .contains("no role verdict was accepted"));
        assert!(!lane_failure_reason("smith", "task-1", &capped).contains("role smith failed"));

        let interrupted = WorkflowError::conflict(
            crate::engine::harness::TURN_INTERRUPTED_CODE,
            "the supervised turn was interrupted",
        );
        assert_eq!(
            lane_stop_diagnostic(&interrupted),
            ("turn_interrupted", false)
        );
        assert!(lane_failure_reason("smith", "task-2", &interrupted)
            .contains("no role verdict was accepted"));
    }

    #[test]
    fn durable_compatible_lanes_overlap_and_both_report_outcomes() {
        let gate = Arc::new(OverlapGate::default());
        let results = execute_durable_batch(
            vec![
                prepared_lane("task-a", &gate, false),
                prepared_lane("task-b", &gate, false),
            ],
            &NoopJobs,
            "worker-1",
            None,
            "story-1",
            1,
        );
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|result| result.outcome.is_ok()));
        assert_eq!(gate.maximum_active.load(Ordering::SeqCst), 2);
        assert_eq!(gate.both_entered.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn one_lane_failure_does_not_drop_its_completed_sibling_result() {
        let gate = Arc::new(OverlapGate::default());
        let results = execute_durable_batch(
            vec![
                prepared_lane("task-a", &gate, true),
                prepared_lane("task-b", &gate, false),
            ],
            &NoopJobs,
            "worker-1",
            None,
            "story-1",
            2,
        );
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|result| result.outcome.is_err()));
        assert!(results.iter().any(|result| result.outcome.is_ok()));
        assert_eq!(gate.maximum_active.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_secondary_failure_is_reported_beside_the_primary_one() {
        let note = with_secondary_failure("the lane failed", "worktree discard", "git said no");
        assert!(note.starts_with("the lane failed"), "{note}");
        assert!(note.contains("worktree discard"), "{note}");
        assert!(note.contains("git said no"), "{note}");
    }

    #[test]
    fn a_panicking_lane_is_a_recorded_failure_and_keeps_its_sibling_result() {
        let gate = Arc::new(OverlapGate::default());
        let results = execute_durable_batch(
            vec![
                prepared_lane("task-a", &gate, false),
                prepared_lane_with_runner("task-b", Box::new(PanicRunner)),
            ],
            &NoopJobs,
            "worker-1",
            None,
            "story-1",
            2,
        );
        assert_eq!(results.len(), 2);
        let panicked = results
            .iter()
            .find(|result| result.claimed.task.task_id == "task-b")
            .expect("the panicking lane has a result");
        assert!(panicked.outcome.is_err());
        assert!(panicked
            .outcome
            .as_ref()
            .err()
            .expect("the panic is this lane's failure")
            .to_string()
            .contains("panicked"));
        // The unwind dispositioned this lane's tree through its own guard, and the guard said so.
        assert_eq!(
            panicked.worktrees.disposition(),
            LaneWorktreeDisposition::Retained
        );
        assert!(results
            .iter()
            .any(|result| result.claimed.task.task_id == "task-a" && result.outcome.is_ok()));
    }

    #[test]
    fn a_panicking_lane_reports_the_claim_it_could_not_settle() {
        // `WO-121`: a panic bypasses the settle inside `execute_claimed_job_unsettled`, so the drive's own
        // `jobs.fail` is the only settlement this lane gets. It used to be a `let _ =`.
        let results = execute_durable_batch(
            vec![prepared_lane_with_runner("task-b", Box::new(PanicRunner))],
            &RefusingFailJobs,
            "worker-1",
            None,
            "story-1",
            1,
        );
        assert_eq!(results.len(), 1);
        assert!(results[0].outcome.is_err());
        assert_eq!(
            results[0].settlement_failure.as_deref(),
            Some("claim already lost")
        );
    }

    #[test]
    fn a_lane_that_never_reports_names_the_panic_and_settles_its_claim() {
        let gate = Arc::new(OverlapGate::default());
        let lane = prepared_lane("task-a", &gate, false);
        let fallback = LaneFallback::of(&lane);
        let reason = "lane worker thread panicked before it could report an outcome";
        let result = lane_that_never_reported(&RefusingFailJobs, "worker-1", fallback, reason);
        assert!(result.outcome.is_err());
        assert!(result
            .outcome
            .as_ref()
            .err()
            .unwrap()
            .to_string()
            .contains("panicked"));
        // Both facts survive: the panic, and the claim that could not be settled.
        assert_eq!(
            result.settlement_failure.as_deref(),
            Some("claim already lost")
        );
        assert_eq!(
            result.worktrees.disposition(),
            LaneWorktreeDisposition::Retained
        );
    }

    #[test]
    fn a_release_that_fails_is_returned_and_attempted_only_once() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&attempts);
        let mut tree = LaneWorktree::with_remover(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp/task-a"),
            "agent/story/task-a".into(),
            "0".repeat(40),
            Box::new(move |_, _, _, _| {
                seen.fetch_add(1, Ordering::SeqCst);
                Err("git worktree remove failed".to_string())
            }),
        );
        assert_eq!(
            tree.release("task-a").as_deref(),
            Some("git worktree remove failed")
        );
        // One lane, one disposition: a second release is not a second cleanup.
        assert!(tree.release("task-a").is_none());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(tree.disposition(), LaneWorktreeDisposition::Released);
    }

    #[test]
    fn an_undecided_lane_worktree_is_retained_by_its_drop_not_deleted() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&attempts);
        {
            let _tree = LaneWorktree::with_remover(
                PathBuf::from("/tmp"),
                PathBuf::from("/tmp/task-a"),
                "agent/story/task-a".into(),
                "0".repeat(40),
                Box::new(move |_, _, _, _| {
                    seen.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }),
            );
        }
        // `WO-121` §3: the backstop disposition is retention, because a lane that exited without deciding
        // is not evidence that its tree is empty. What it must never be is silent.
        assert_eq!(attempts.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_released_lane_worktree_is_removed_once_and_a_retained_one_is_not_removed() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&attempts);
        {
            let mut released = LaneWorktree::with_remover(
                PathBuf::from("/tmp"),
                PathBuf::from("/tmp/task-a"),
                "agent/story/task-a".into(),
                "0".repeat(40),
                Box::new(move |_, _, _, _| {
                    seen.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }),
            );
            assert!(released.release("task-a").is_none());
            assert_eq!(released.disposition(), LaneWorktreeDisposition::Released);
        }
        // The drop after the release must not remove the same tree a second time.
        assert_eq!(attempts.load(Ordering::SeqCst), 1);

        let retained_attempts = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&retained_attempts);
        {
            let mut retained = LaneWorktree::with_remover(
                PathBuf::from("/tmp"),
                PathBuf::from("/tmp/task-b"),
                "agent/story/task-b".into(),
                "0".repeat(40),
                Box::new(move |_, _, _, _| {
                    seen.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }),
            );
            retained.retain("task-b", "execution_failed=planned lane failure");
            assert_eq!(retained.disposition(), LaneWorktreeDisposition::Retained);
        }
        assert_eq!(retained_attempts.load(Ordering::SeqCst), 0);
    }
    /// What a scripted lane fails with (`WO-302` Part A chaos).
    enum ScriptedFailure {
        /// A genuine business verdict: says nothing about the engine.
        Hold,
        /// An internal engine fault: says nothing about the story.
        EngineFault,
    }

    /// A lane runner scripted to fail one fixed way.
    struct FailingRunner {
        failure: ScriptedFailure,
    }

    impl ForgeRoleRunner for FailingRunner {
        fn run(&self, _node_id: &str, _task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
            match self.failure {
                // A genuine business Hold verdict: a role refusal that says nothing about the
                // engine, so `is_engine_fault_error` must not claim it.
                ScriptedFailure::Hold => Err(WorkflowError::generic(
                    "human gate requires operator decision before this lane may proceed",
                )),
                // A sibling engine fault: the database went away mid-turn. Says nothing about
                // the story.
                ScriptedFailure::EngineFault => Err(WorkflowError::unavailable(
                    "db: DatabaseUnavailable during workflow.step",
                )),
            }
        }
    }

    /// Fold one wave's lane executions exactly the way the drive loop does: first Hold wins, first
    /// engine fault wins, then `wave_disposition` weighs them. Mirrors the loop so the chaos test
    /// exercises the real precedence rather than restating it.
    fn fold_wave(executions: Vec<LaneExecutionResult>) -> WaveDisposition {
        let mut hold_reason: Option<String> = None;
        let mut engine_fault: Option<WorkflowError> = None;
        for execution in executions {
            let channels = classify_lane_failure(&execution);
            if let Err(error) = execution.outcome {
                if channels.is_engine_fault {
                    if engine_fault.is_none() {
                        engine_fault = Some(error);
                    }
                } else if hold_reason.is_none() {
                    hold_reason = Some(channels.reason);
                }
            }
        }
        wave_disposition(hold_reason, engine_fault)
    }

    fn hold_lane(task_id: &str) -> PreparedDurableLane {
        prepared_lane_with_runner(
            task_id,
            Box::new(FailingRunner {
                failure: ScriptedFailure::Hold,
            }),
        )
    }

    fn fault_lane(task_id: &str) -> PreparedDurableLane {
        prepared_lane_with_runner(
            task_id,
            Box::new(FailingRunner {
                failure: ScriptedFailure::EngineFault,
            }),
        )
    }

    #[test]
    fn a_real_hold_survives_a_sibling_engine_fault() {
        // `WO-302` Part A chaos: sibling 1 returns a genuine Hold verdict while sibling 2 hits an
        // internal engine fault on the same wave.
        let results = execute_durable_batch(
            vec![hold_lane("task-hold"), fault_lane("task-fault")],
            &NoopJobs,
            "worker-1",
            None,
            "story-1",
            2,
        );
        assert_eq!(results.len(), 2);
        assert!(
            results.iter().all(|result| result.outcome.is_err()),
            "both lanes failed, one with a verdict and one with a fault"
        );
        match fold_wave(results) {
            WaveDisposition::Hold(reason) => {
                assert!(
                    reason.contains("operator decision"),
                    "the Hold verdict is the disposition: {reason}"
                );
                assert!(
                    reason.contains("sibling engine fault"),
                    "the masked fault stays observable on the Hold: {reason}"
                );
                assert!(
                    reason.contains("DatabaseUnavailable"),
                    "the fault text rides along, not just its existence: {reason}"
                );
            }
            WaveDisposition::EngineFault(error) => {
                panic!("the engine fault masked the genuine Hold: {error}")
            }
            WaveDisposition::Clear => panic!("a Hold and a fault cannot clear a wave"),
        }
    }

    #[test]
    fn an_engine_fault_alone_propagates_without_inventing_a_hold() {
        // The other direction: with no business verdict anywhere, the fault must still propagate —
        // Hold priority must not become Hold invention.
        let results = execute_durable_batch(
            vec![fault_lane("task-fault")],
            &NoopJobs,
            "worker-1",
            None,
            "story-1",
            1,
        );
        match fold_wave(results) {
            WaveDisposition::EngineFault(error) => {
                assert!(is_engine_fault_error(&error), "{error}");
            }
            WaveDisposition::Hold(reason) => {
                panic!("a fault alone must never invent a Hold: {reason}")
            }
            WaveDisposition::Clear => panic!("a faulted wave is not clear"),
        }
    }

    #[test]
    fn a_hold_alone_carries_no_fault_attachment() {
        let results = execute_durable_batch(
            vec![hold_lane("task-hold")],
            &NoopJobs,
            "worker-1",
            None,
            "story-1",
            1,
        );
        match fold_wave(results) {
            WaveDisposition::Hold(reason) => {
                assert!(reason.contains("operator decision"), "{reason}");
                assert!(
                    !reason.contains("sibling engine fault"),
                    "no fault happened, so none may be attached: {reason}"
                );
            }
            other => panic!(
                "a lone Hold must dispose as Hold, got {}",
                match other {
                    WaveDisposition::EngineFault(error) => format!("EngineFault({error})"),
                    WaveDisposition::Clear => "Clear".to_string(),
                    WaveDisposition::Hold(_) => unreachable!(),
                }
            ),
        }
    }

    /// A `JobService` whose `complete` fails transiently while recording every call (`WO-302` Part B
    /// chaos). `fail` panics when called: the settle path under test must never fail a job it just
    /// completed, so a call there is the defect, not a fixture gap.
    struct FlakyCompleteJobs {
        complete_calls: Mutex<usize>,
        fail_calls: Mutex<usize>,
        fail_complete: bool,
    }

    impl JobService for FlakyCompleteJobs {
        fn enqueue(&self, _request: &crate::engine::job::ForgeJobRequest) -> Result<String> {
            unreachable!()
        }
        fn claim(&self, _worker_id: &str, _limit: usize) -> Result<Vec<ForgeJobLease>> {
            unreachable!()
        }
        fn claim_one(&self, _job_id: &str, _worker_id: &str) -> Result<ForgeJobLease> {
            unreachable!()
        }
        fn heartbeat(&self, _job_id: &str, _worker_id: &str) -> Result<i64> {
            Ok(1)
        }
        fn inspect(&self, _job_id: &str) -> Result<crate::engine::job::ForgeJobState> {
            Err(WorkflowError::generic("unused"))
        }
        fn complete(&self, _job_id: &str, _worker_id: &str) -> Result<()> {
            *self.complete_calls.lock().unwrap() += 1;
            if self.fail_complete {
                return Err(WorkflowError::unavailable(
                    "db: DatabaseUnavailable during jobs.complete",
                ));
            }
            Ok(())
        }
        fn fail(
            &self,
            _job_id: &str,
            _worker_id: &str,
            _error: &str,
            _permanent: bool,
        ) -> Result<()> {
            *self.fail_calls.lock().unwrap() += 1;
            panic!("settle_completed_lane must never fail a job it just completed");
        }
        fn cancel(&self, _job_id: &str, _actor: &str) -> Result<()> {
            Ok(())
        }
        fn requeue(&self, _job_id: &str, _actor: &str) -> Result<()> {
            Ok(())
        }
        fn recover_stale(&self, _batch: usize) -> Result<usize> {
            Ok(0)
        }
    }

    #[test]
    fn a_settle_blip_holds_with_reconciliation_instead_of_orphaning_the_job() {
        // `WO-302` Part B chaos: the lane completed, then `jobs.complete` died transiently. The
        // settle must hold with the failure named (not propagate, not swallow), must never fail the
        // just-completed job, and must leave the row for reconciliation instead of orphaning it.
        let jobs = FlakyCompleteJobs {
            complete_calls: Mutex::new(0),
            fail_calls: Mutex::new(0),
            fail_complete: true,
        };
        let reason = settle_completed_lane(&jobs, "worker-1", "job-x", "task-x", false)
            .expect("a failed settle must hold, not vanish");
        assert!(reason.contains("task-x"), "{reason}");
        assert!(
            reason.contains("reconciliation"),
            "the Hold must name the reconciliation path, not just the failure: {reason}"
        );
        assert_eq!(
            *jobs.complete_calls.lock().unwrap(),
            1,
            "one settle attempt"
        );
        assert_eq!(
            *jobs.fail_calls.lock().unwrap(),
            0,
            "the completed job is never failed: failing it would orphan paid work"
        );
    }

    #[test]
    fn a_clean_settle_holds_nothing() {
        let jobs = FlakyCompleteJobs {
            complete_calls: Mutex::new(0),
            fail_calls: Mutex::new(0),
            fail_complete: false,
        };
        assert!(
            settle_completed_lane(&jobs, "worker-1", "job-x", "task-x", false).is_none(),
            "a settled job holds nothing"
        );
        assert!(
            settle_completed_lane(&jobs, "worker-1", "job-x", "task-x", true).is_none(),
            "an already-completed settle holds nothing either"
        );
        assert_eq!(*jobs.complete_calls.lock().unwrap(), 2);
    }
}
