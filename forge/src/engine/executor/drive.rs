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
use crate::engine::executor::wave::{plan_wave, WaveLane};
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::job::{
    execute_claimed_job_unsettled, ForgeJobBridge, ForgeJobLease, JobService,
};
use crate::engine::packet::ExecutionWorkspace;
use crate::engine::runner::ForgeTurnPorts;
use crate::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use crate::engine::service_binding::is_human_gate;
use crate::engine::turn_budget;
use crate::engine::worktree::{
    discard_unstarted_lane_worktree, integrate_lane_candidate, provision_worker_workspace,
    remove_lane_worktree, resolve_repo_root, DEFAULT_WORKTREES_DIRNAME,
};
use crate::roles::registry::ForgeServiceRegistry;
use crate::roles::service::ForgeLaneServices;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use workflow::{
    JobStatus, ProcessOutcome, ProcessStatus, Result, TaskStatus, TxStore, WorkflowError,
};

pub struct ForgeRoleOutcome {
    pub transition_name: Option<String>,
    pub evidence: ForgeGateEvidence,
}

pub trait ForgeRoleRunner: Send + Sync {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome>;

    fn run_scoped(
        &self,
        _execution_id: &str,
        node_id: &str,
        task: &ActiveForgeRoleTask,
    ) -> Result<ForgeRoleOutcome> {
        self.run(node_id, task)
    }

    fn turn_ports(&self) -> Option<&dyn ForgeTurnPorts> {
        None
    }

    fn fork_for_workspace(
        &self,
        workspace: crate::engine::packet::ExecutionWorkspace,
    ) -> Result<Option<Box<dyn ForgeRoleRunner>>> {
        let Some(ports) = self.turn_ports() else {
            return Ok(None);
        };
        let Some(harness) = ports.harness().fork_for_workspace(workspace)? else {
            return Ok(None);
        };
        Ok(ports.fork_with_harness(harness))
    }
}

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

fn instance_status<S: TxStore>(rt: &ForgeRuntime<S>, instance_id: &str) -> Result<String> {
    Ok(format!(
        "{:?}",
        rt.engine().get_process_instance(instance_id)?.status
    ))
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
                .as_deref(),
        )
    }

    pub fn within_story_concurrency_from_env() -> usize {
        turn_budget::resolve_within_story_concurrency(
            std::env::var(turn_budget::WITHIN_STORY_CONCURRENCY_ENV)
                .ok()
                .as_deref(),
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
}

struct LaneExecutionResult {
    claimed: ClaimedDurableLane,
    worktree_path: PathBuf,
    parent_worktree: PathBuf,
    base_commit: String,
    outcome: Result<ForgeRoleOutcome>,
}

fn fail_claimed_batch(
    jobs: &dyn JobService,
    worker_id: &str,
    lanes: &[ClaimedDurableLane],
    reason: &str,
) {
    for lane in lanes {
        if let Err(error) = jobs.fail(&lane.lease.job_id, worker_id, reason, false) {
            eprintln!(
                "forge-wave job={} could_not_release_claim error={error}",
                lane.lease.job_id
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
            let forked_runner = match runner.fork_for_workspace(execution_workspace) {
                Ok(Some(runner)) => runner,
                Ok(None) => {
                    let _ = discard_unstarted_lane_worktree(
                        &repo_root,
                        &workspace.worktree_path,
                        &branch_name,
                        &base_commit,
                    );
                    return Err(WorkflowError::conflict(
                        "FORGE_PARALLEL_LANES_UNAVAILABLE",
                        format!(
                            "service {} harness cannot create an isolated lane execution",
                            lane.lease.service_key
                        ),
                    ));
                }
                Err(error) => {
                    let _ = discard_unstarted_lane_worktree(
                        &repo_root,
                        &workspace.worktree_path,
                        &branch_name,
                        &base_commit,
                    );
                    return Err(error);
                }
            };
            if !forked_runner
                .turn_ports()
                .map(|ports| ports.harness().supports_interrupt())
                .unwrap_or(false)
            {
                let _ = discard_unstarted_lane_worktree(
                    &repo_root,
                    &workspace.worktree_path,
                    &branch_name,
                    &base_commit,
                );
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    format!(
                        "forked runner for service {} lost execution-scoped interruption",
                        lane.lease.service_key
                    ),
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
            })
        })();
        match preparation {
            Ok(prepared_lane) => prepared.push(prepared_lane),
            Err(error) => {
                for lane in &prepared {
                    let _ = discard_unstarted_lane_worktree(
                        &lane.parent_worktree,
                        &lane.worktree_path,
                        &lane.branch_name,
                        &lane.base_commit,
                    );
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
                                worktree_path,
                                branch_name: _,
                                parent_worktree,
                                base_commit,
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
                                worktree_path,
                                parent_worktree,
                                base_commit,
                                outcome,
                            })
                        },
                    ));
                    let result = match execution {
                        Ok(Ok(result)) => result,
                        Ok(Err(error)) => {
                            let _ = jobs.fail(
                                &fallback_claimed.lease.job_id,
                                worker_id,
                                &error.to_string(),
                                true,
                            );
                            LaneExecutionResult {
                                claimed: fallback_claimed,
                                worktree_path: fallback_worktree,
                                parent_worktree: fallback_parent,
                                base_commit: fallback_base,
                                outcome: Err(error),
                            }
                        }
                        Err(_) => {
                            let reason = "lane worker panicked before it could report an outcome";
                            let _ =
                                jobs.fail(&fallback_claimed.lease.job_id, worker_id, reason, true);
                            LaneExecutionResult {
                                claimed: fallback_claimed,
                                worktree_path: fallback_worktree,
                                parent_worktree: fallback_parent,
                                base_commit: fallback_base,
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
        handles
            .into_iter()
            .map(|handle| handle.join().expect("lane worker wrapper contains panics"))
            .collect()
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
        if let Some(human) = tasks
            .iter()
            .find(|t| t.node_id.as_deref().map(is_human_gate).unwrap_or(false))
        {
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
        let cap = opts
            .within_story_concurrency
            .clamp(1, turn_budget::MAX_WITHIN_STORY_CONCURRENCY);
        let lanes: Vec<WaveLane<ActiveForgeRoleTask>> = ready
            .into_iter()
            .map(|task| {
                let lane_id = task.task_id.clone();
                WaveLane {
                    // A coordinator/fanout flag does not establish that shared
                    // effects are safe. Unknown surfaces stay exclusive.
                    fanout: false,
                    surface: task.write_surface.clone(),
                    lane: lane_id,
                    task,
                }
            })
            .collect();
        let plan = plan_wave(&lanes, cap);
        wave_index += 1;
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
                let turn_ceiling = turn_ceiling(std::env::var(TURN_CEILING_ENV).ok().as_deref());
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
                for execution in executions {
                    let outcome = match execution.outcome {
                        Ok(outcome) => outcome,
                        Err(error) => {
                            let (stop_kind, role_verdict) = lane_stop_diagnostic(&error);
                            let reason = lane_failure_reason(
                                &execution.claimed.node,
                                &execution.claimed.task.task_id,
                                &error,
                            );
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
                            if is_engine_fault_error(&error) && engine_fault.is_none() {
                                engine_fault = Some(error);
                            } else if hold_reason.is_none() {
                                hold_reason = Some(reason.clone());
                            }
                            eprintln!(
                                "forge-wave task={} execution_failed={reason}; isolated worktree retained at {}",
                                execution.claimed.task.task_id,
                                execution.worktree_path.display()
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
                            if let Err(settle_error) = durable
                                .jobs
                                .complete(&execution.claimed.lease.job_id, opts.worker_id)
                            {
                                if hold_reason.is_none() {
                                    hold_reason = Some(format!(
                                        "workflow task {} was already completed but its durable job could not be settled: {settle_error}",
                                        execution.claimed.task.task_id
                                    ));
                                }
                            }
                            let _ = remove_lane_worktree(
                                &execution.parent_worktree,
                                &execution.worktree_path,
                            );
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
                            let reason = format!(
                                "task {} completed in Workflow, but candidate {candidate} could not be integrated: {error}",
                                execution.claimed.task.task_id
                            );
                            let _ = durable.jobs.fail(
                                &execution.claimed.lease.job_id,
                                opts.worker_id,
                                &reason,
                                true,
                            );
                            if hold_reason.is_none() {
                                hold_reason = Some(reason);
                            }
                            eprintln!(
                                "forge-wave task={} integration_failed={error}; isolated worktree retained at {}",
                                execution.claimed.task.task_id,
                                execution.worktree_path.display()
                            );
                            continue;
                        }
                    }
                    if let Err(settle_error) = durable
                        .jobs
                        .complete(&execution.claimed.lease.job_id, opts.worker_id)
                    {
                        if hold_reason.is_none() {
                            hold_reason = Some(format!(
                                "workflow task {} completed but durable job settlement failed: {settle_error}",
                                execution.claimed.task.task_id
                            ));
                        }
                    }
                    if let Err(error) =
                        remove_lane_worktree(&execution.parent_worktree, &execution.worktree_path)
                    {
                        eprintln!(
                            "forge-wave task={} worktree_cleanup_failed={error}; branch remains reachable",
                            execution.claimed.task.task_id
                        );
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
                if let Some(error) = engine_fault {
                    return Err(error);
                }
                if let Some(reason) = hold_reason {
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
                    // Durable production composes execution-scoped interruption from the registered
                    // service inside the job boundary, where the exact lease identity is available.
                    let _request =
                        ForgeJobBridge::new(durable.registry).job_for_ready_task(&task)?;
                    // Use the shared Forge turn budget regardless of the selected harness.
                    let turn_ceiling =
                        turn_ceiling(std::env::var(TURN_CEILING_ENV).ok().as_deref());

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

    Ok(DriveForgeStoryResult {
        instance_id: instance_id.clone(),
        status: format!("{:?}", instance.status),
        steps,
        exhausted: !tasks.is_empty(),
        blocked_reason: None,
        needs_human: tasks
            .iter()
            .any(|t| t.node_id.as_deref().map(is_human_gate).unwrap_or(false)),
        stopped_after,
        reconciled,
    })
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

    fn prepared_lane(task_id: &str, gate: &Arc<OverlapGate>, fail: bool) -> PreparedDurableLane {
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
            runner: Box::new(BlockingRunner {
                gate: Arc::clone(gate),
                fail,
            }),
            worktree_path: PathBuf::from(format!("/tmp/{task_id}")),
            branch_name: format!("agent/story/{task_id}"),
            parent_worktree: PathBuf::from("/tmp/story"),
            base_commit: "0".repeat(40),
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
}
