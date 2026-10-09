//! Forge story drive: the main execution loop.

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
    execute_claimed_job_unsettled, ForgeJobBridge, ForgeJobLease, JobService, InterruptHandle,
};
use crate::engine::role_slice::forge_lane_surface;
use crate::engine::runner::ForgeTurnPorts;
use crate::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use crate::engine::service_binding::is_human_gate;
use crate::engine::turn_budget;
use crate::roles::registry::ForgeServiceRegistry;
use std::sync::{Arc, Weak};
use workflow::{
    JobStatus, ProcessOutcome, ProcessStatus, Result, TaskStatus, TxStore, WorkflowError,
};

pub struct ForgeRoleOutcome {
    pub transition_name: Option<String>,
    pub evidence: ForgeGateEvidence,
}

pub trait ForgeRoleRunner: Send + Sync {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome>;

    fn turn_ports(&self) -> Option<&dyn ForgeTurnPorts> {
        None
    }
}

fn has_surface<T>(lane: &WaveLane<T>) -> bool {
    lane.surface
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

fn process_completes_story(status: ProcessStatus, outcome: Option<ProcessOutcome>) -> bool {
    status == ProcessStatus::Completed && outcome == Some(ProcessOutcome::Completed)
}

fn instance_status<S: TxStore>(rt: &ForgeRuntime<S>, instance_id: &str) -> Result<String> {
    Ok(format!(
        "{:?}",
        rt.engine().get_process_instance(instance_id)?.status
    ))
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
    pub split_concurrency: usize,
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
    if durable.is_none() && opts.runner.is_none() {
        return Err(WorkflowError::generic(
            "Direct Forge execution requires an explicit real role runner; there is no synthetic runner. Durable production execution resolves concrete services through JobService + ForgeServiceRegistry.",
        ));
    }
    let stop_target = resolve_forge_stop_target(opts.stop_after.as_ref());
    let mut stopped_after = None;
    let mut steps = Vec::new();
    let mut turns_dispatched: u32 = 0;
    let mut turn_cap_stop: Option<String> = None;

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
                    // Get interrupt handle from runner if available.
                    let interrupt_handle = opts.runner.as_ref().and_then(|r| r.turn_ports()).map(|ports| {
                        let harness: Weak<dyn crate::engine::runner::RoleHarness> = Arc::downgrade(&ports.harness_arc());
                        Arc::new(move |reason: &str| {
                            if let Some(h) = harness.upgrade() {
                                let _ = h.interrupt_execution(reason);
                            }
                        }) as InterruptHandle
                    });
                    // Get turn ceiling from environment (same logic as opencode harness).
                    let turn_ceiling = crate::engine::opencode::turn_ceiling(
                        std::env::var(crate::engine::opencode::TURN_CEILING_ENV)
                            .ok()
                            .as_deref(),
                    );

                    let outcome = match execute_claimed_job_unsettled(
                        durable.jobs,
                        opts.worker_id,
                        &lease,
                        &task,
                        durable.registry,
                        interrupt_handle,
                        turn_ceiling,
                    ) {
                        Ok(outcome) => outcome,
                        Err(err) if is_engine_fault_error(&err) => return Err(err),
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
        blocked_reason: turn_cap_stop,
        needs_human: tasks
            .iter()
            .any(|t| t.node_id.as_deref().map(is_human_gate).unwrap_or(false)),
        stopped_after,
        reconciled,
    })
}
