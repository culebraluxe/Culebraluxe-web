//! Forge READY-task to durable-job bridge.
//!
//! Architecture C is intentionally boring:
//! Workflow exposes READY role work, the bridge creates a standard job request,
//! JobService persists/leases that request, and a worker resolves the concrete
//! role service. Role semantics never live in this module.

use crate::engine::engine_fault::is_engine_fault_error;
use crate::engine::executor::drive::ForgeRoleOutcome;
use crate::engine::job_payload::request_payload;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::registry::ForgeServiceRegistry;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;
use workflow::{Job, JobStatus, Result, TaskStatus, TxStore, Value, WorkflowEngine, WorkflowError};

pub const FORGE_ROLE_JOB_TYPE: &str = "forge.role";
pub const DEFAULT_FORGE_JOB_ATTEMPTS: i32 = 5;
const FORGE_JOB_HEARTBEAT_INTERVAL_SECS: u64 = 60;

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

/// JobService backed by the generic Rust Workflow `jobs` table.
///
/// The Workflow kernel already owns pending/locked/completed/failed/cancelled
/// state, SKIP LOCKED claims, leases, retry backoff, cancellation and stale-job
/// recovery. Forge adds no second queue; it uses a dedicated generic job type.
pub struct WorkflowJobService<'a, S: TxStore> {
    engine: &'a WorkflowEngine<S>,
    max_attempts: i32,
}

impl<'a, S: TxStore> WorkflowJobService<'a, S> {
    pub fn new(engine: &'a WorkflowEngine<S>) -> Self {
        Self {
            engine,
            max_attempts: DEFAULT_FORGE_JOB_ATTEMPTS,
        }
    }

    pub fn with_max_attempts(mut self, max_attempts: i32) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }
}

impl<S: TxStore> JobService for WorkflowJobService<'_, S> {
    fn enqueue(&self, request: &ForgeJobRequest) -> Result<String> {
        // One Workflow role task owns one durable Forge job. Reusing the
        // task UUID as the job UUID gives the bridge a stable identity without
        // adding a Forge-specific dedupe table or schema column.
        self.engine.create_job_with_id(
            &request.task.task_id,
            Some(&request.task.process_instance_id),
            request.task.token_id.as_deref(),
            None,
            FORGE_ROLE_JOB_TYPE,
            self.engine.current_time_ms(),
            request_payload(request),
            Some(self.max_attempts),
        )
    }

    fn claim(&self, worker_id: &str, limit: usize) -> Result<Vec<ForgeJobLease>> {
        let jobs = self
            .engine
            .claim_jobs_by_type(worker_id, FORGE_ROLE_JOB_TYPE, limit)?;
        let mut leases = Vec::with_capacity(jobs.len());

        for job in jobs {
            match lease_from_job(&job) {
                Ok(lease) => leases.push(lease),
                Err(error) => {
                    // A malformed durable envelope is not a role verdict and is
                    // not retryable by another model turn. Terminalize the job
                    // itself so recovery cannot loop forever on unreadable work.
                    self.engine
                        .fail_job(&job.id, worker_id, &error.to_string(), true)?;
                }
            }
        }

        Ok(leases)
    }

    fn claim_one(&self, job_id: &str, worker_id: &str) -> Result<ForgeJobLease> {
        let existing = self.engine.get_job(job_id)?;
        if existing.job_type != FORGE_ROLE_JOB_TYPE {
            return Err(WorkflowError::generic(format!(
                "job {job_id} has type {:?}, expected {:?}",
                existing.job_type, FORGE_ROLE_JOB_TYPE
            )));
        }

        let job = self.engine.claim_job(job_id, worker_id)?.ok_or_else(|| {
            WorkflowError::conflict(
                "FORGE_JOB_NOT_CLAIMABLE",
                format!("Forge role job {job_id} is not claimable"),
            )
        })?;

        match lease_from_job(&job) {
            Ok(lease) => Ok(lease),
            Err(error) => {
                self.engine
                    .fail_job(job_id, worker_id, &error.to_string(), true)?;
                Err(error)
            }
        }
    }

    fn heartbeat(&self, job_id: &str, worker_id: &str) -> Result<i64> {
        self.engine.heartbeat_job(job_id, worker_id)
    }

    fn inspect(&self, job_id: &str) -> Result<ForgeJobState> {
        let job = self.engine.get_job(job_id)?;
        Ok(ForgeJobState {
            status: job.status,
            attempts: job.attempts,
            max_attempts: job.max_attempts,
            locked_by: job.locked_by,
            due_at: job.due_at,
            last_error: job.last_error,
        })
    }

    fn complete(&self, job_id: &str, worker_id: &str) -> Result<()> {
        self.engine.complete_job(job_id, worker_id)
    }

    fn fail(&self, job_id: &str, worker_id: &str, error: &str, permanent: bool) -> Result<()> {
        self.engine.fail_job(job_id, worker_id, error, permanent)
    }

    fn cancel(&self, job_id: &str, actor: &str) -> Result<()> {
        self.engine.cancel_job(job_id, actor)
    }

    fn requeue(&self, job_id: &str, actor: &str) -> Result<()> {
        self.engine.requeue_job(job_id, actor)
    }

    fn recover_stale(&self, batch: usize) -> Result<usize> {
        self.engine.reclaim_stale_jobs(batch)
    }
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

    let service_result = run_with_lease_heartbeat(jobs, worker_id, &lease.job_id, || {
        service.execute(&lease.node_id, task)
    });

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
                eprintln!(
                    "forge-job: job {} could not be settled after its role failed ({settle}); stale-lease recovery will reclaim it",
                    lease.job_id
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
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    run_with_lease_heartbeat_interval(
        jobs,
        worker_id,
        job_id,
        Duration::from_secs(FORGE_JOB_HEARTBEAT_INTERVAL_SECS),
        work,
    )
}

fn run_with_lease_heartbeat_interval<T>(
    jobs: &dyn JobService,
    worker_id: &str,
    job_id: &str,
    interval: Duration,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    std::thread::scope(|scope| {
        let heartbeat = scope.spawn(move || -> Option<WorkflowError> {
            loop {
                match stop_rx.recv_timeout(interval) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => return None,
                    Err(RecvTimeoutError::Timeout) => match jobs.heartbeat(job_id, worker_id) {
                        Ok(_) => {}
                        Err(error) if error.is_connection_failure() => {
                            // The database was unavailable, not the lease. Keep
                            // trying; recovery cannot make progress while the
                            // same database is unreachable either.
                            eprintln!(
                                "forge-job-heartbeat transient failure job={job_id}: {error}"
                            );
                        }
                        Err(error) => return Some(error),
                    },
                }
            }
        });

        let result = work();
        drop(stop_tx);

        let heartbeat_error = heartbeat.join().map_err(|_| {
            WorkflowError::generic(format!(
                "Forge job heartbeat thread panicked for job {job_id}"
            ))
        })?;

        if let Some(error) = heartbeat_error {
            return Err(WorkflowError::generic(format!(
                "Forge job {job_id} lost its lease while the role was running: {error}"
            )));
        }

        // Fence Workflow completion with one final ownership renewal after the
        // role returns. If recovery somehow won the race, the caller must not
        // advance the Workflow task under a lease it no longer owns.
        jobs.heartbeat(job_id, worker_id)?;
        result
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
    let outcome = execute_claimed_job_unsettled(jobs, worker_id, lease, task, registry)?;
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
mod tests {
    use super::*;
    use crate::engine::executor::drive::ForgeRoleRunner;
    use crate::engine::facts::ForgeGateEvidence;
    use crate::roles::architect::ArchitectService;
    use crate::roles::dev_ops::DevOpsService;
    use crate::roles::inspector::InspectorService;
    use crate::roles::lead::LeadService;
    use crate::roles::qa::AssayService;
    use crate::roles::scout::ScoutService;
    use crate::roles::smith::SmithService;
    use std::sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex,
    };
    use workflow::{EngineOptions, MemoryStore, ProcessInstance, ProcessStatus};

    struct RecordingRunner {
        calls: Mutex<Vec<String>>,
    }

    impl RecordingRunner {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("calls lock").clone()
        }
    }

    impl ForgeRoleRunner for RecordingRunner {
        fn run(&self, node_id: &str, _task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(node_id.to_string());
            Ok(ForgeRoleOutcome {
                transition_name: Some("complete".into()),
                evidence: ForgeGateEvidence::default(),
            })
        }
    }

    fn task_with_id(task_id: &str, node_id: &str, status: TaskStatus) -> ActiveForgeRoleTask {
        ActiveForgeRoleTask {
            task_id: task_id.into(),
            process_instance_id: "proc-1".into(),
            story_id: "ENG-FORGE-JOB-BRIDGE-01".into(),
            token_id: None,
            node_id: Some(node_id.into()),
            status,
            assignee: None,
            candidates: vec![],
        }
    }

    fn task(node_id: &str, status: TaskStatus) -> ActiveForgeRoleTask {
        task_with_id(&format!("task-{node_id}"), node_id, status)
    }

    fn registry<'a>(
        runner: &'a RecordingRunner,
    ) -> (
        ForgeServiceRegistry<'a>,
        ScoutService<'a>,
        ArchitectService<'a>,
        LeadService<'a>,
        SmithService<'a>,
        InspectorService<'a>,
        AssayService<'a>,
        DevOpsService<'a>,
    ) {
        let scout = ScoutService::new(runner);
        let architect = ArchitectService::new(runner);
        let lead = LeadService::new(runner);
        let smith = SmithService::new(runner);
        let inspector = InspectorService::new(runner);
        let assay = AssayService::new(runner);
        let devops = DevOpsService::new(runner);

        (
            ForgeServiceRegistry::new(),
            scout,
            architect,
            lead,
            smith,
            inspector,
            assay,
            devops,
        )
    }

    fn engine(clock: Arc<AtomicI64>) -> WorkflowEngine<MemoryStore> {
        let engine = WorkflowEngine::new(
            MemoryStore::new(),
            EngineOptions {
                app: None,
                now: Box::new(move || clock.load(Ordering::SeqCst)),
            },
        );
        engine
            .store()
            .with_tx(|tx| {
                tx.insert_instance(ProcessInstance {
                    id: "proc-1".into(),
                    tenant_id: None,
                    definition_id: "definition-1".into(),
                    business_key: Some("ENG-FORGE-JOB-BRIDGE-01".into()),
                    status: ProcessStatus::Active,
                    outcome: None,
                    started_at: 1_000,
                    ended_at: None,
                    started_by: Some("test".into()),
                    parent_instance_id: None,
                    root_token_id: None,
                    subject_type: Some("story".into()),
                    subject_id: Some("ENG-FORGE-JOB-BRIDGE-01".into()),
                    variables: Value::object(),
                    version: 1,
                })?;
                Ok(())
            })
            .expect("active process");
        engine
    }

    #[test]
    fn bridge_maps_every_current_lane_to_its_stable_service_key() {
        let runner = RecordingRunner::new();
        let (mut registry, scout, architect, lead, smith, inspector, assay, devops) =
            registry(&runner);

        for service in [
            &scout as &dyn crate::roles::AbstractForgeService,
            &architect,
            &lead,
            &smith,
            &inspector,
            &assay,
            &devops,
        ] {
            registry.register(service).expect("register service");
        }

        let bridge = ForgeJobBridge::new(&registry);
        let cases = [
            ("feature_scout", "forge.scout"),
            ("architect", "forge.architect"),
            ("lead_pre", "forge.lead"),
            ("smith", "forge.smith"),
            ("qa_review", "forge.inspector"),
            ("qa_verify", "forge.assay"),
            ("deploy", "forge.devops"),
        ];

        for (node, expected_service_key) in cases {
            let job = bridge
                .job_for_ready_task(&task(node, TaskStatus::Ready))
                .expect("READY task becomes a job");
            assert_eq!(job.service_key, expected_service_key);
        }
    }

    #[test]
    fn bridge_refuses_a_human_task_without_a_service_binding() {
        let runner = RecordingRunner::new();
        let (registry, _scout, _architect, _lead, _smith, _inspector, _assay, _devops) =
            registry(&runner);

        let error = ForgeJobBridge::new(&registry)
            .job_for_ready_task(&task("hold", TaskStatus::Ready))
            .expect_err("human Workflow tasks must never become agent jobs");

        assert!(error.to_string().contains("no agent service binding"));
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn bridge_refuses_a_workflow_task_that_is_not_ready() {
        let runner = RecordingRunner::new();
        let (mut registry, _scout, _architect, _lead, smith, _inspector, _assay, _devops) =
            registry(&runner);
        registry.register(&smith).expect("register Smith");

        let error = ForgeJobBridge::new(&registry)
            .job_for_ready_task(&task("smith", TaskStatus::Reserved))
            .expect_err("only READY workflow work may become a job");

        assert!(error.to_string().contains("not Ready"));
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn durable_job_service_claims_only_forge_role_jobs() {
        let clock = Arc::new(AtomicI64::new(1_000));
        let engine = engine(clock);
        let jobs = WorkflowJobService::new(&engine);
        let runner = RecordingRunner::new();
        let (mut registry, _scout, _architect, _lead, smith, _inspector, _assay, _devops) =
            registry(&runner);
        registry.register(&smith).expect("register Smith");
        let request = ForgeJobBridge::new(&registry)
            .job_for_ready_task(&task("smith", TaskStatus::Ready))
            .expect("Smith request");

        let role_job_id = jobs.enqueue(&request).expect("enqueue role");
        let timer_job_id = engine
            .create_job(
                Some("proc-1"),
                None,
                None,
                "timer",
                1_000,
                Value::object(),
                Some(5),
            )
            .expect("enqueue timer");

        let claimed = jobs.claim("forge-worker", 10).expect("claim Forge jobs");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].job_id, role_job_id);
        assert_eq!(
            engine.get_job(&timer_job_id).expect("timer").status,
            JobStatus::Pending
        );
    }

    #[test]
    fn heartbeat_renews_only_the_owning_workers_lease() {
        let clock = Arc::new(AtomicI64::new(1_000));
        let engine = engine(clock.clone());
        let jobs = WorkflowJobService::new(&engine);
        let runner = RecordingRunner::new();
        let (mut registry, _scout, _architect, _lead, smith, _inspector, _assay, _devops) =
            registry(&runner);
        registry.register(&smith).expect("register Smith");
        let request = ForgeJobBridge::new(&registry)
            .job_for_ready_task(&task("smith", TaskStatus::Ready))
            .expect("Smith request");

        jobs.enqueue(&request).expect("enqueue");
        let lease = jobs.claim("worker-a", 1).expect("claim").remove(0);
        let before = lease.locked_until.expect("initial lease");

        clock.store(61_000, Ordering::SeqCst);
        let after = jobs
            .heartbeat(&lease.job_id, "worker-a")
            .expect("owner heartbeat");
        assert!(after > before);

        let wrong_owner = jobs
            .heartbeat(&lease.job_id, "worker-b")
            .expect_err("another worker cannot renew the lease");
        assert!(wrong_owner.to_string().contains("another worker"));
    }

    #[test]
    fn long_running_role_work_renews_the_lease_before_the_turn_returns() {
        let clock = Arc::new(AtomicI64::new(1_000));
        let engine = engine(clock.clone());
        let jobs = WorkflowJobService::new(&engine);
        let runner = RecordingRunner::new();
        let (mut registry, _scout, _architect, _lead, smith, _inspector, _assay, _devops) =
            registry(&runner);
        registry.register(&smith).expect("register Smith");
        let request = ForgeJobBridge::new(&registry)
            .job_for_ready_task(&task("smith", TaskStatus::Ready))
            .expect("Smith request");

        jobs.enqueue(&request).expect("enqueue");
        let lease = jobs.claim("worker-a", 1).expect("claim").remove(0);
        let before = lease.locked_until.expect("initial lease");

        let during = run_with_lease_heartbeat_interval(
            &jobs,
            "worker-a",
            &lease.job_id,
            std::time::Duration::from_millis(5),
            || {
                clock.store(61_000, Ordering::SeqCst);

                for _ in 0..50 {
                    let locked_until = engine
                        .get_job(&lease.job_id)
                        .expect("running job")
                        .locked_until
                        .expect("running lease");
                    if locked_until > before {
                        return Ok(locked_until);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }

                Err(WorkflowError::generic(
                    "background heartbeat did not renew the lease while role work was still running",
                ))
            },
        )
        .expect("long-running work keeps its lease");

        assert!(
            during > before,
            "the lease must renew before the role closure returns, not only at the final fence"
        );
        assert_eq!(
            engine.get_job(&lease.job_id).expect("job").status,
            JobStatus::Locked,
            "heartbeat renews ownership; it does not settle the job"
        );
    }

    #[test]
    fn completed_and_cancelled_jobs_are_not_claimed_again() {
        let clock = Arc::new(AtomicI64::new(1_000));
        let engine = engine(clock);
        let jobs = WorkflowJobService::new(&engine);
        let runner = RecordingRunner::new();
        let (mut registry, _scout, _architect, _lead, smith, _inspector, _assay, _devops) =
            registry(&runner);
        registry.register(&smith).expect("register Smith");
        let bridge = ForgeJobBridge::new(&registry);

        let complete_request = bridge
            .job_for_ready_task(&task_with_id("task-complete", "smith", TaskStatus::Ready))
            .expect("complete request");
        jobs.enqueue(&complete_request).expect("enqueue complete");
        let complete_lease = jobs.claim("worker", 1).expect("claim complete").remove(0);
        jobs.complete(&complete_lease.job_id, "worker")
            .expect("complete job");

        let cancel_request = bridge
            .job_for_ready_task(&task_with_id("task-cancel", "smith", TaskStatus::Ready))
            .expect("cancel request");
        let cancel_id = jobs.enqueue(&cancel_request).expect("enqueue cancel");
        jobs.cancel(&cancel_id, "operator").expect("cancel job");

        assert!(jobs
            .claim("worker", 10)
            .expect("claim remaining")
            .is_empty());
        assert_eq!(
            engine
                .get_job(&complete_lease.job_id)
                .expect("completed")
                .status,
            JobStatus::Completed
        );
        assert_eq!(
            engine.get_job(&cancel_id).expect("cancelled").status,
            JobStatus::Cancelled
        );
    }

    #[test]
    fn stale_forge_role_job_is_recovered_and_reclaimable() {
        let clock = Arc::new(AtomicI64::new(1_000));
        let engine = engine(clock.clone());
        let jobs = WorkflowJobService::new(&engine);
        let runner = RecordingRunner::new();
        let (mut registry, _scout, _architect, _lead, smith, _inspector, _assay, _devops) =
            registry(&runner);
        registry.register(&smith).expect("register Smith");
        let request = ForgeJobBridge::new(&registry)
            .job_for_ready_task(&task("smith", TaskStatus::Ready))
            .expect("Smith request");

        jobs.enqueue(&request).expect("enqueue");
        let first = jobs.claim("dead-worker", 1).expect("first claim").remove(0);
        clock.store(first.locked_until.expect("lease") + 1, Ordering::SeqCst);

        assert_eq!(jobs.recover_stale(10).expect("recover"), 1);
        let second = jobs.claim("new-worker", 1).expect("second claim").remove(0);
        assert_eq!(second.job_id, first.job_id);
        assert_eq!(second.attempts, first.attempts + 1);
    }

    #[test]
    fn claimed_job_worker_delegates_only_by_service_key() {
        let clock = Arc::new(AtomicI64::new(1_000));
        let engine = engine(clock);
        let jobs = WorkflowJobService::new(&engine);
        let runner = RecordingRunner::new();
        let (mut registry, _scout, _architect, _lead, smith, _inspector, _assay, _devops) =
            registry(&runner);
        registry.register(&smith).expect("register Smith");
        let task = task("smith", TaskStatus::Ready);
        let request = ForgeJobBridge::new(&registry)
            .job_for_ready_task(&task)
            .expect("Smith request");

        jobs.enqueue(&request).expect("enqueue");
        let lease = jobs.claim("forge-worker", 1).expect("claim").remove(0);
        execute_claimed_job(&jobs, "forge-worker", &lease, &task, &registry)
            .expect("execute claimed Smith job");

        assert_eq!(runner.calls(), vec!["smith".to_string()]);
        assert_eq!(
            engine.get_job(&lease.job_id).expect("job").status,
            JobStatus::Completed
        );
    }
}
