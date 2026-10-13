//! Forge READY-task to durable-job bridge.
//!
//! Architecture C is intentionally boring:
//! Workflow exposes READY role work, the bridge creates a standard job request,
//! JobService persists/leases that request, and a worker resolves the concrete
//! role service. Role semantics never live in this module.

use crate::engine::config::{turn_ceiling, TURN_CEILING_ENV};
use crate::engine::engine_fault::is_engine_fault_error;
use crate::engine::runner::ForgeRoleOutcome;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::registry::ForgeServiceRegistry;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use workflow::{Job, JobStatus, Result, TaskStatus, WorkflowError};

mod workflow_service;
pub use workflow_service::WorkflowJobService;

pub const FORGE_ROLE_JOB_TYPE: &str = "forge.role";
pub const DEFAULT_FORGE_JOB_ATTEMPTS: i32 = 5;
const FORGE_JOB_HEARTBEAT_INTERVAL_SECS: u64 = 60;

/// Environment variable for the supervisor deadline (turn ceiling + slack).
/// When set, the supervisor will interrupt a turn that exceeds this ceiling.
/// Format: same as FORGE_TURN_TIMEOUT_MINUTES (minutes, or "0"/"off"/"none" for unbounded).
pub const SUPERVISOR_DEADLINE_ENV: &str = "FORGE_SUPERVISOR_DEADLINE_MINUTES";
/// Default supervisor deadline slack in minutes beyond the turn ceiling.
/// A turn has a ceiling; the supervisor deadline is ceiling + slack to allow
/// for heartbeat overhead and cleanup.
const DEFAULT_SUPERVISOR_SLACK_MINUTES: u64 = 5;

/// Configuration for the lease fence: stops a turn when the database is
/// unreachable for too long, or when the turn exceeds its supervisor deadline.
#[derive(Debug, Clone, Copy)]
pub struct LeaseFenceConfig {
    /// Maximum consecutive heartbeat intervals with DB connection failure
    /// before the turn is interrupted. Default: 5 (5 minutes at 60s interval).
    pub max_consecutive_db_failures: u32,
    /// Supervisor deadline for the entire turn (work + heartbeat). When
    /// exceeded, the turn is interrupted and the job is failed as retryable.
    /// This should be set to the model's max_turn + slack (e.g., 5 minutes).
    pub supervisor_deadline: Option<Duration>,
}

impl Default for LeaseFenceConfig {
    fn default() -> Self {
        Self {
            max_consecutive_db_failures: 5,
            supervisor_deadline: None,
        }
    }
}

/// Parse the supervisor deadline from environment.
/// Uses FORGE_SUPERVISOR_DEADLINE_MINUTES if set, otherwise derives from
/// FORGE_TURN_TIMEOUT_MINUTES + DEFAULT_SUPERVISOR_SLACK_MINUTES.
pub fn parse_supervisor_deadline(turn_ceiling: Option<Duration>) -> Option<Duration> {
    if let Ok(raw) = std::env::var(SUPERVISOR_DEADLINE_ENV) {
        let raw = raw.trim();
        if raw.is_empty()
            || raw == "0"
            || raw.eq_ignore_ascii_case("off")
            || raw.eq_ignore_ascii_case("none")
        {
            return None;
        }
        if let Ok(minutes) = raw.parse::<u64>() {
            return Some(Duration::from_secs(minutes * 60));
        }
    }
    // Derive from turn ceiling + slack
    turn_ceiling.map(|ceiling| ceiling + Duration::from_secs(DEFAULT_SUPERVISOR_SLACK_MINUTES * 60))
}

/// A thread-safe interrupt handle that can stop a running turn.
///
/// The closure is called with a reason string when the lease fence triggers.
/// It should signal the turn's execution harness to stop (e.g., via LiveTurnSlot).
pub type InterruptHandle = Arc<dyn Fn(&str) + Send + Sync>;

/// Internal state shared between the heartbeat thread and supervisor.
struct LeaseFenceState {
    interrupt: Option<InterruptHandle>,
    consecutive_db_failures: Mutex<u32>,
    work_started: Instant,
    config: LeaseFenceConfig,
}

#[derive(Debug, Clone)]
pub struct ForgeJobRequest {
    pub service_key: String,
    pub node_id: String,
    pub task: ActiveForgeRoleTask,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeJobLease {
    pub job_id: String,
    pub service_key: String,
    pub node_id: String,
    pub task_id: String,
    pub process_instance_id: String,
    pub story_id: String,
    pub token_id: Option<String>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub locked_until: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeJobState {
    pub status: JobStatus,
    pub attempts: i32,
    pub max_attempts: i32,
    pub locked_by: Option<String>,
    pub due_at: i64,
    pub last_error: Option<String>,
}

/// The only Forge-specific translation between workflow state and job execution.
///
/// The bridge validates that the workflow task is READY, reads the canonical
/// service binding from the workflow XML through `task.service_key()`, verifies
/// that key is registered, and emits a standard job request. It does not infer
/// service ownership from the node id and does not know what any role actually does.
pub struct ForgeJobBridge<'registry, 'services> {
    registry: &'registry ForgeServiceRegistry<'services>,
}

impl<'registry, 'services> ForgeJobBridge<'registry, 'services> {
    pub fn new(registry: &'registry ForgeServiceRegistry<'services>) -> Self {
        Self { registry }
    }

    pub fn job_for_ready_task(&self, task: &ActiveForgeRoleTask) -> Result<ForgeJobRequest> {
        if !matches!(task.status, TaskStatus::Ready) {
            return Err(WorkflowError::generic(format!(
                "workflow task {} is {:?}, not Ready; refusing job creation",
                task.task_id, task.status
            )));
        }

        let node_id = task
            .node_id
            .as_deref()
            .filter(|node| !node.trim().is_empty())
            .ok_or_else(|| {
                WorkflowError::generic(format!(
                    "workflow task {} has no Forge node id",
                    task.task_id
                ))
            })?;

        let service_key = task.service_key().ok_or_else(|| {
            WorkflowError::generic(format!(
                "workflow task {} node {node_id:?} has no agent service binding; refusing job creation",
                task.task_id
            ))
        })?;

        // Fail closed before a durable job is written if the XML names a service
        // this process did not register. The registry remains a dumb key lookup.
        self.registry.resolve(service_key)?;

        Ok(ForgeJobRequest {
            service_key: service_key.to_string(),
            node_id: node_id.to_string(),
            task: task.clone(),
        })
    }
}

/// Durable execution reliability only.
///
/// This service knows queue mechanics and stable execution identity. It does not
/// know Scout/Architect/Lead/Smith/Inspector/Assay/DevOps behavior, FAST routing,
/// prompts, models, OpenCode, or workflow transition semantics.
pub trait JobService: Send + Sync {
    fn enqueue(&self, request: &ForgeJobRequest) -> Result<String>;
    fn claim(&self, worker_id: &str, limit: usize) -> Result<Vec<ForgeJobLease>>;
    fn claim_one(&self, job_id: &str, worker_id: &str) -> Result<ForgeJobLease>;
    fn heartbeat(&self, job_id: &str, worker_id: &str) -> Result<i64>;
    fn inspect(&self, job_id: &str) -> Result<ForgeJobState>;
    fn complete(&self, job_id: &str, worker_id: &str) -> Result<()>;
    fn fail(&self, job_id: &str, worker_id: &str, error: &str, permanent: bool) -> Result<()>;
    fn cancel(&self, job_id: &str, actor: &str) -> Result<()>;
    fn requeue(&self, job_id: &str, actor: &str) -> Result<()>;
    fn recover_stale(&self, batch: usize) -> Result<usize>;
}

/// Execute one claimed Forge job but leave its durable lease open on success.
///
/// The worker owns only the generic lifecycle around the call. The registry
/// resolves the service; the concrete service owns every intelligent decision.
///
/// The live Workflow driver uses this form so it can commit the Workflow task
/// first and settle the durable job second. That ordering means a crash can
/// leave an orphaned job to reconcile, but it cannot leave a READY Workflow
/// task behind a Completed job and accidentally pay for the role twice.
pub fn execute_claimed_job_unsettled(
    jobs: &dyn JobService,
    worker_id: &str,
    lease: &ForgeJobLease,
    task: &ActiveForgeRoleTask,
    registry: &ForgeServiceRegistry<'_>,
    interrupt: Option<InterruptHandle>,
    turn_ceiling: Option<Duration>,
) -> Result<ForgeRoleOutcome> {
    execute_claimed_job_unsettled_with_deadline(
        jobs,
        worker_id,
        lease,
        task,
        registry,
        interrupt,
        parse_supervisor_deadline(turn_ceiling),
    )
}

fn execute_claimed_job_unsettled_with_deadline(
    jobs: &dyn JobService,
    worker_id: &str,
    lease: &ForgeJobLease,
    task: &ActiveForgeRoleTask,
    registry: &ForgeServiceRegistry<'_>,
    interrupt: Option<InterruptHandle>,
    supervisor_deadline: Option<Duration>,
) -> Result<ForgeRoleOutcome> {
    if let Err(error) = assert_task_matches_lease(lease, task) {
        jobs.fail(&lease.job_id, worker_id, &error.to_string(), true)?;
        return Err(error);
    }

    let service = match registry.resolve(&lease.service_key) {
        Ok(service) => service,
        Err(error) => {
            jobs.fail(&lease.job_id, worker_id, &error.to_string(), true)?;
            return Err(error);
        }
    };

    // Renew immediately, then keep the lease alive for the entire role turn.
    // Model-backed Smith/Architect/etc. turns can run well beyond one generic
    // job lease; without this loop another scheduler pass may reclaim live work.
    jobs.heartbeat(&lease.job_id, worker_id)?;

    // Build the lease fence config with connection failure threshold and supervisor deadline.
    let config = LeaseFenceConfig {
        max_consecutive_db_failures: 5,
        supervisor_deadline,
    };

    let execution_id = format!(
        "{}:lease:{}:worker:{}:instance:{}",
        lease.job_id, lease.attempts, worker_id, lease.process_instance_id
    );
    let harness = service
        .runner()
        .turn_ports()
        .map(|ports| ports.harness_arc());
    let scoped_interrupt = if let Some(harness) = harness.as_ref() {
        if !harness.supports_interrupt() {
            let reason = format!(
                "INTERRUPT_UNSUPPORTED: service {} cannot stop its active execution",
                lease.service_key
            );
            jobs.fail(&lease.job_id, worker_id, &reason, true)?;
            return Err(WorkflowError::conflict("INTERRUPT_UNSUPPORTED", reason));
        }
        harness.begin_execution(&execution_id)?;
        let harness = Arc::clone(harness);
        let execution_for_log = execution_id.clone();
        let story_for_log = task.story_id.clone();
        let task_for_log = task.task_id.clone();
        let job_for_log = lease.job_id.clone();
        let instance_for_log = lease.process_instance_id.clone();
        let attempt_for_log = lease.attempts;
        Some(Arc::new(move |reason: &str| {
            tracing::info!(
                story = %story_for_log,
                instance = %instance_for_log,
                task = %task_for_log,
                job = %job_for_log,
                attempt = attempt_for_log,
                execution = %execution_for_log,
                reason = %reason,
                "forge-interrupt requested=true"
            );
            match harness.interrupt_execution_scoped(&execution_for_log, reason) {
                Ok(Some(receipt)) => tracing::info!(
                    story = %story_for_log,
                    instance = %instance_for_log,
                    task = %task_for_log,
                    job = %job_for_log,
                    attempt = attempt_for_log,
                    execution = %execution_for_log,
                    existed = receipt.existed,
                    killed = receipt.killed,
                    signalled = ?receipt.signalled,
                    "forge-interrupt delivered=true"
                ),
                Ok(None) => tracing::info!(
                    story = %story_for_log,
                    instance = %instance_for_log,
                    task = %task_for_log,
                    job = %job_for_log,
                    attempt = attempt_for_log,
                    execution = %execution_for_log,
                    "forge-interrupt delivered=no_live_process_or_latched_before_launch"
                ),
                Err(error) => tracing::warn!(
                    story = %story_for_log,
                    instance = %instance_for_log,
                    task = %task_for_log,
                    job = %job_for_log,
                    attempt = attempt_for_log,
                    execution = %execution_for_log,
                    error = %error,
                    "forge-interrupt delivery_error"
                ),
            }
        }) as InterruptHandle)
    } else {
        interrupt
    };

    let service_result = run_with_lease_heartbeat(
        jobs,
        worker_id,
        &lease.job_id,
        config,
        scoped_interrupt,
        || service.execute_scoped(&execution_id, &lease.node_id, task),
    );
    if let Some(harness) = harness.as_ref() {
        harness.finish_execution(&execution_id);
    }

    match service_result {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            // A role-service error is this attempt's verdict UNLESS the failure was the engine's own plumbing — a
            // session taken away mid-turn, a dead transport, a statement cut off. That kind of failure decided
            // nothing about the story (captain, 2026-09-29: an engine fault is not the story's verdict), so the
            // durable job is settled RETRYABLE: Pending behind the backoff, inside the same `max_attempts` budget,
            // with stale-lease recovery and operator requeue unchanged.
            //
            // Everything else stays permanent — a role's refusal, an envelope that no longer matches its task, a
            // service key nobody registered — because repeating a verdict only spends the same money again.
            //
            // The classification reads the ERROR, never the service or the node: this layer stays role-agnostic,
            // and `forge_job__014` pins that one error classifies the same whichever service produced it. If
            // ownership was lost, `fail` itself refuses the stale owner.
            let permanent = !is_engine_fault_error(&error);
            // The role's error is the answer either way. A settle that fails here (a lost lease, the database
            // gone) leaves the row Locked for stale-lease recovery, and says so instead of vanishing.
            if let Err(settle) = jobs.fail(&lease.job_id, worker_id, &error.to_string(), permanent)
            {
                tracing::warn!(
                    job = %lease.job_id,
                    settle_error = %settle,
                    "forge-job: the job could not be settled after its role failed; stale-lease recovery will reclaim it"
                );
            }
            Err(error)
        }
    }
}

fn run_with_lease_heartbeat<T>(
    jobs: &dyn JobService,
    worker_id: &str,
    job_id: &str,
    config: LeaseFenceConfig,
    interrupt: Option<InterruptHandle>,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let turn_outcome = run_with_lease_heartbeat_interval(
        jobs,
        worker_id,
        job_id,
        Duration::from_secs(FORGE_JOB_HEARTBEAT_INTERVAL_SECS),
        config,
        interrupt,
        work,
    )?;
    if let Some(authority_failure) = turn_outcome.authority_failure {
        Err(WorkflowError::generic(format!(
            "Forge job {job_id} authority is unproven after role execution; outcome preserved for reconciliation: {authority_failure}"
        )))
    } else {
        Ok(turn_outcome.outcome)
    }
}

/// Outcome of a role turn with its lease state.
#[derive(Debug)]
pub struct RoleTurnOutcome<T> {
    pub outcome: T,
    pub lease_lost: bool,
    /// Typed at the role-turn boundary as a durable explanation, not just a boolean that callers can drop.
    pub authority_failure: Option<String>,
}

fn run_with_lease_heartbeat_interval<T>(
    jobs: &dyn JobService,
    worker_id: &str,
    job_id: &str,
    interval: Duration,
    config: LeaseFenceConfig,
    interrupt: Option<InterruptHandle>,
    work: impl FnOnce() -> Result<T>,
) -> Result<RoleTurnOutcome<T>> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    let state = Arc::new(LeaseFenceState {
        interrupt,
        consecutive_db_failures: Mutex::new(0),
        work_started: Instant::now(),
        config,
    });

    // Heartbeat thread with connection failure counting and supervisor deadline.
    let heartbeat_state = Arc::clone(&state);
    let worker_id_owned = worker_id.to_string();
    let job_id_owned = job_id.to_string();
    let heartbeat = std::thread::scope(|scope| -> (Option<WorkflowError>, Result<T>) {
        let heartbeat = scope.spawn(move || -> Option<WorkflowError> {
            loop {
                // Check supervisor deadline first.
                if let Some(deadline) = heartbeat_state.config.supervisor_deadline {
                    if heartbeat_state.work_started.elapsed() >= deadline {
                        if let Some(interrupt) = &heartbeat_state.interrupt {
                            interrupt("supervisor deadline exceeded");
                        }
                        return Some(WorkflowError::generic(format!(
                            "Forge job {job_id_owned} exceeded supervisor deadline of {:?}",
                            deadline
                        )));
                    }
                }

                let wait_for = heartbeat_state
                    .config
                    .supervisor_deadline
                    .map(|deadline| {
                        deadline
                            .saturating_sub(heartbeat_state.work_started.elapsed())
                            .min(interval)
                    })
                    .unwrap_or(interval);
                match stop_rx.recv_timeout(wait_for) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => return None,
                    Err(RecvTimeoutError::Timeout) => match std::panic::catch_unwind(
                        std::panic::AssertUnwindSafe(|| {
                            jobs.heartbeat(&job_id_owned, &worker_id_owned)
                        }),
                    ) {
                        Ok(Ok(_)) => {
                            // Reset consecutive failures on success.
                            let mut failures = heartbeat_state
                                .consecutive_db_failures
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            *failures = 0;
                        }
                        Ok(Err(error)) if error.is_connection_failure() => {
                            let mut failures = heartbeat_state
                                .consecutive_db_failures
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            *failures += 1;
                            tracing::warn!(
                                job = %job_id_owned,
                                consecutive = *failures,
                                error = %error,
                                "forge-job-heartbeat transient failure"
                            );
                            if *failures >= heartbeat_state.config.max_consecutive_db_failures {
                                if let Some(interrupt) = &heartbeat_state.interrupt {
                                    interrupt(&format!(
                                        "database unavailable for {} consecutive heartbeat intervals",
                                        *failures
                                    ));
                                }
                                return Some(WorkflowError::generic(format!(
                                    "Forge job {job_id_owned} lost database connectivity for {} consecutive intervals",
                                    *failures
                                )));
                            }
                        }
                        Ok(Err(error)) => {
                            if let Some(interrupt) = &heartbeat_state.interrupt {
                                interrupt("durable job was cancelled or its lease was lost");
                            }
                            return Some(error);
                        }
                        Err(payload) => {
                            let panic = payload
                                .downcast_ref::<&str>()
                                .map(|message| (*message).to_string())
                                .or_else(|| payload.downcast_ref::<String>().cloned())
                                .unwrap_or_else(|| "non-string panic payload".to_string());
                            if let Some(interrupt) = &heartbeat_state.interrupt {
                                interrupt("heartbeat supervisor panicked; lease authority is unproven");
                            }
                            return Some(WorkflowError::generic(format!(
                                "Forge job {job_id_owned} heartbeat supervisor panicked: {panic}"
                            )));
                        }
                    },
                }
            }
        });

        // Run the work in the main thread (inside the scope).
        let work_result = work();

        // Signal heartbeat thread to stop.
        drop(stop_tx);

        // Wait for heartbeat thread to finish.
        let heartbeat_error = match heartbeat.join() {
            Ok(error) => error,
            Err(payload) => {
                let panic = payload
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "non-string panic payload".to_string());
                Some(WorkflowError::generic(format!(
                    "Forge job {job_id} heartbeat supervisor panicked: {panic}"
                )))
            }
        };

        (heartbeat_error, work_result)
    });

    let (heartbeat_error, work_result) = heartbeat;

    // If the heartbeat thread lost the lease, we still return the outcome
    // but mark lease_lost = true. The caller must reconcile and not advance
    // the Workflow task under a lease it no longer owns.
    let mut authority_failures = Vec::new();
    if let Some(error) = heartbeat_error {
        authority_failures.push(error.to_string());
        if work_result.is_ok() {
            tracing::warn!(
                job = %job_id,
                error = %error,
                "forge-job: lost its lease after the role completed; outcome preserved for reconciliation"
            );
        } else {
            tracing::warn!(
                job = %job_id,
                error = %error,
                "forge-job: lost its lease and the role returned an interruption/failure; no business outcome was accepted"
            );
        }
    }

    // Fence Workflow completion with one final ownership renewal after the
    // role returns. If recovery somehow won the race, the caller must not
    // advance the Workflow task under a lease it no longer owns.
    // The final renewal is part of the authority decision. A role result is not
    // accepted when its lease could not be proven after the role returned.
    let final_heartbeat_error = jobs.heartbeat(&job_id, &worker_id).err();
    if let Some(error) = final_heartbeat_error {
        authority_failures.push(format!("final ownership renewal failed: {error}"));
        tracing::warn!(
            job = %job_id,
            error = %error,
            "forge-job: final ownership renewal failed; outcome preserved for reconciliation"
        );
    }

    let authority_failure = (!authority_failures.is_empty()).then(|| authority_failures.join("; "));
    let outcome = match work_result {
        Ok(outcome) => outcome,
        Err(work_error) if authority_failure.is_some() => {
            return Err(WorkflowError::generic(format!(
                "{work_error}; lease authority is unproven: {}",
                authority_failure.as_deref().unwrap_or_default()
            )));
        }
        Err(work_error) => return Err(work_error),
    };
    Ok(RoleTurnOutcome {
        outcome,
        lease_lost: authority_failure.is_some(),
        authority_failure,
    })
}

/// Compatibility helper for callers that own no Workflow completion step.
///
/// It preserves the established "execute then settle job" contract. Production
/// Workflow driving uses `execute_claimed_job_unsettled` so Workflow state wins
/// before the durable execution receipt is closed.
pub fn execute_claimed_job(
    jobs: &dyn JobService,
    worker_id: &str,
    lease: &ForgeJobLease,
    task: &ActiveForgeRoleTask,
    registry: &ForgeServiceRegistry<'_>,
) -> Result<ForgeRoleOutcome> {
    // The turn budget is shared Forge configuration, independent of the selected harness.
    let turn_ceiling = turn_ceiling(std::env::var(TURN_CEILING_ENV).ok().as_deref());
    let outcome = execute_claimed_job_unsettled(
        jobs,
        worker_id,
        lease,
        task,
        registry,
        None, // No interrupt handle for compatibility callers
        turn_ceiling,
    )?;
    jobs.complete(&lease.job_id, worker_id)?;
    Ok(outcome)
}

fn assert_task_matches_lease(lease: &ForgeJobLease, task: &ActiveForgeRoleTask) -> Result<()> {
    let node_id = task.node_id.as_deref().unwrap_or_default();
    if task.task_id != lease.task_id
        || task.process_instance_id != lease.process_instance_id
        || task.story_id != lease.story_id
        || task.token_id != lease.token_id
        || node_id != lease.node_id
    {
        return Err(WorkflowError::generic(format!(
            "Forge job {} no longer matches workflow task {}",
            lease.job_id, task.task_id
        )));
    }

    if !task.status.is_actionable() {
        return Err(WorkflowError::generic(format!(
            "workflow task {} is no longer actionable ({:?})",
            task.task_id, task.status
        )));
    }

    Ok(())
}

fn lease_from_job(job: &Job) -> Result<ForgeJobLease> {
    if job.job_type != FORGE_ROLE_JOB_TYPE {
        return Err(WorkflowError::generic(format!(
            "job {} has type {:?}, expected {:?}",
            job.id, job.job_type, FORGE_ROLE_JOB_TYPE
        )));
    }
    if job.status != JobStatus::Locked {
        return Err(WorkflowError::generic(format!(
            "Forge job {} is not locked ({:?})",
            job.id, job.status
        )));
    }

    Ok(ForgeJobLease {
        job_id: job.id.clone(),
        service_key: crate::engine::job_payload::required_string(&job.payload, "serviceKey")?,
        node_id: crate::engine::job_payload::required_string(&job.payload, "nodeId")?,
        task_id: crate::engine::job_payload::required_string(&job.payload, "taskId")?,
        process_instance_id: crate::engine::job_payload::required_string(
            &job.payload,
            "processInstanceId",
        )?,
        story_id: crate::engine::job_payload::required_string(&job.payload, "storyId")?,
        token_id: crate::engine::job_payload::optional_string(&job.payload, "tokenId")?,
        attempts: job.attempts,
        max_attempts: job.max_attempts,
        locked_until: job.locked_until,
    })
}

#[cfg(test)]
mod tests;
