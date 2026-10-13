use super::*;
use crate::engine::assay::CommandResult;
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::harness::TurnTermination;
use crate::engine::runner::ForgeRoleRunner;
use crate::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
use crate::roles::architect::ArchitectService;
use crate::roles::dev_ops::DevOpsService;
use crate::roles::inspector::InspectorService;
use crate::roles::lead::LeadService;
use crate::roles::qa::AssayService;
use crate::roles::scout::ScoutService;
use crate::roles::smith::SmithService;
use std::sync::{
    atomic::{AtomicBool, AtomicI64, Ordering},
    Arc, Mutex,
};
use workflow::{EngineOptions, MemoryStore, ProcessInstance, ProcessStatus, Value, WorkflowEngine};

struct HeartbeatFaultService {
    main_thread: std::thread::ThreadId,
    panic_on_background_heartbeat: bool,
    fail_final_heartbeat: bool,
}

impl JobService for HeartbeatFaultService {
    fn enqueue(&self, _: &ForgeJobRequest) -> Result<String> {
        unreachable!()
    }
    fn claim(&self, _: &str, _: usize) -> Result<Vec<ForgeJobLease>> {
        unreachable!()
    }
    fn claim_one(&self, _: &str, _: &str) -> Result<ForgeJobLease> {
        unreachable!()
    }
    fn heartbeat(&self, _: &str, _: &str) -> Result<i64> {
        let on_main = std::thread::current().id() == self.main_thread;
        if !on_main && self.panic_on_background_heartbeat {
            panic!("injected heartbeat supervisor panic");
        }
        if on_main && self.fail_final_heartbeat {
            return Err(WorkflowError::generic("injected final renewal failure"));
        }
        Ok(1)
    }
    fn inspect(&self, _: &str) -> Result<ForgeJobState> {
        unreachable!()
    }
    fn complete(&self, _: &str, _: &str) -> Result<()> {
        unreachable!()
    }
    fn fail(&self, _: &str, _: &str, _: &str, _: bool) -> Result<()> {
        unreachable!()
    }
    fn cancel(&self, _: &str, _: &str) -> Result<()> {
        unreachable!()
    }
    fn requeue(&self, _: &str, _: &str) -> Result<()> {
        unreachable!()
    }
    fn recover_stale(&self, _: usize) -> Result<usize> {
        unreachable!()
    }
}

fn heartbeat_fault_service(
    panic_on_background_heartbeat: bool,
    fail_final_heartbeat: bool,
) -> HeartbeatFaultService {
    HeartbeatFaultService {
        main_thread: std::thread::current().id(),
        panic_on_background_heartbeat,
        fail_final_heartbeat,
    }
}

struct RecordingRunner {
    calls: Mutex<Vec<String>>,
}

struct ScopedHarness {
    executions: Mutex<Vec<String>>,
    interrupted_executions: Mutex<Vec<String>>,
    interrupted: Arc<AtomicBool>,
    block_until_interrupted: bool,
}

impl RoleHarness for ScopedHarness {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        Err(WorkflowError::generic(
            "durable dispatch bypassed scoped execution",
        ))
    }

    fn run_role_scoped(
        &self,
        execution_id: &str,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        self.executions
            .lock()
            .unwrap()
            .push(execution_id.to_string());
        if self.block_until_interrupted {
            while !self.interrupted.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
            return Err(WorkflowError::generic(format!(
                "{}: supervisor cancelled model turn",
                crate::engine::harness::TURN_INTERRUPTED_CODE
            )));
        }
        Ok(HarnessOutput {
            trajectory: None,
            raw: "scout findings".into(),
            candidate_sha: None,
            assay_commands: Vec::new(),
            acceptance_mapped: false,
            refusal: None,
            execution_base: None,
            usage: None,
        })
    }

    fn interrupt_execution(&self, _reason: &str) -> Result<Option<TurnTermination>> {
        Ok(None)
    }

    fn interrupt_execution_scoped(
        &self,
        execution_id: &str,
        _reason: &str,
    ) -> Result<Option<TurnTermination>> {
        self.interrupted_executions
            .lock()
            .unwrap()
            .push(execution_id.to_string());
        self.interrupted.store(true, Ordering::SeqCst);
        Ok(None)
    }

    fn supports_interrupt(&self) -> bool {
        true
    }

    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        false
    }

    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }

    fn run_command(&self, command: &str) -> CommandResult {
        CommandResult {
            command: command.into(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            cancelled: false,
            unmeasurable: false,
            output: String::new(),
        }
    }
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
        write_surface: None,
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
    let (mut registry, scout, architect, lead, smith, inspector, assay, devops) = registry(&runner);

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
fn durable_job_dispatch_uses_registered_runner_and_exact_lease_identity() {
    let clock = Arc::new(AtomicI64::new(1_000));
    let engine = engine(clock);
    let jobs = WorkflowJobService::new(&engine);
    let harness = Arc::new(ScopedHarness {
        executions: Mutex::new(Vec::new()),
        interrupted_executions: Mutex::new(Vec::new()),
        interrupted: Arc::new(AtomicBool::new(false)),
        block_until_interrupted: false,
    });
    let runner = ProductionRoleRunner::new(harness.clone(), ForgeGateEvidence::default());
    let scout = ScoutService::new(&runner);
    let mut registry = ForgeServiceRegistry::new();
    registry
        .register(&scout)
        .expect("register production Scout");
    let ready = task("feature_scout", TaskStatus::Ready);
    let request = ForgeJobBridge::new(&registry)
        .job_for_ready_task(&ready)
        .expect("ready durable request");
    jobs.enqueue(&request).expect("enqueue");
    let lease = jobs.claim("worker-a", 1).expect("claim").remove(0);
    let expected_execution_id = format!(
        "{}:lease:{}:worker:{}:instance:{}",
        lease.job_id, lease.attempts, "worker-a", lease.process_instance_id
    );

    execute_claimed_job_unsettled(&jobs, "worker-a", &lease, &ready, &registry, None, None)
        .expect("the production-shaped registered service completes");

    assert_eq!(
        harness.executions.lock().unwrap().as_slice(),
        &[expected_execution_id],
        "durable dispatch carries its exact lease identity through the registered runner"
    );
}

#[test]
fn durable_supervisor_deadline_releases_registered_blocking_harness() {
    let clock = Arc::new(AtomicI64::new(1_000));
    let engine = engine(clock);
    let jobs = WorkflowJobService::new(&engine);
    let harness = Arc::new(ScopedHarness {
        executions: Mutex::new(Vec::new()),
        interrupted_executions: Mutex::new(Vec::new()),
        interrupted: Arc::new(AtomicBool::new(false)),
        block_until_interrupted: true,
    });
    let runner = ProductionRoleRunner::new(harness.clone(), ForgeGateEvidence::default());
    let scout = ScoutService::new(&runner);
    let mut registry = ForgeServiceRegistry::new();
    registry
        .register(&scout)
        .expect("register production Scout");
    let ready = task("feature_scout", TaskStatus::Ready);
    let request = ForgeJobBridge::new(&registry)
        .job_for_ready_task(&ready)
        .expect("ready durable request");
    jobs.enqueue(&request).expect("enqueue");
    let lease = jobs.claim("worker-a", 1).expect("claim").remove(0);
    let expected_execution_id = format!(
        "{}:lease:{}:worker:{}:instance:{}",
        lease.job_id, lease.attempts, "worker-a", lease.process_instance_id
    );

    let result = execute_claimed_job_unsettled_with_deadline(
        &jobs,
        "worker-a",
        &lease,
        &ready,
        &registry,
        None,
        Some(Duration::from_millis(20)),
    );

    let error = match result {
        Ok(_) => panic!("a cancelled model turn cannot report a business success"),
        Err(error) => error,
    };
    assert!(error
        .to_string()
        .contains(crate::engine::harness::TURN_INTERRUPTED_CODE));
    assert!(harness.interrupted.load(Ordering::SeqCst));
    assert_eq!(
        harness.interrupted_executions.lock().unwrap().as_slice(),
        &[expected_execution_id],
        "the supervisor stops only the execution identity associated with the durable lease"
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
    let engine = Arc::new(engine(clock.clone()));
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
    let job_id = lease.job_id.clone();
    let clock = clock.clone();
    let engine_for_closure = Arc::clone(&engine);

    let during = run_with_lease_heartbeat_interval(
        &jobs,
        "worker-a",
        &job_id.clone(),
        std::time::Duration::from_millis(5),
        LeaseFenceConfig::default(),
        None,
        move || {
            clock.store(61_000, Ordering::SeqCst);

            for _ in 0..50 {
                let locked_until = engine_for_closure
                    .get_job(&job_id)
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
    .expect("long-running work keeps its lease")
    .outcome;

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
fn supervisor_deadline_interrupts_only_the_execution_handle_it_received() {
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
    jobs.enqueue(&request).expect("enqueue");
    let lease = jobs.claim("worker-a", 1).expect("claim").remove(0);
    let interrupted = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let target = Arc::clone(&interrupted);
    let handle: InterruptHandle = Arc::new(move |_| target.store(true, Ordering::SeqCst));
    let result = run_with_lease_heartbeat_interval(
        &jobs,
        "worker-a",
        &lease.job_id,
        Duration::from_millis(2),
        LeaseFenceConfig {
            max_consecutive_db_failures: 2,
            supervisor_deadline: Some(Duration::from_millis(20)),
        },
        Some(handle),
        || -> Result<()> {
            while !interrupted.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(WorkflowError::generic("harness stopped"))
        },
    );
    assert!(
        interrupted.load(Ordering::SeqCst),
        "the supervisor delivered the scoped stop"
    );
    assert!(
        result.is_err(),
        "the stopped role cannot produce a successful business outcome"
    );
}

#[test]
fn final_heartbeat_failure_blocks_successful_role_outcome() {
    let jobs = heartbeat_fault_service(false, true);
    let turn = run_with_lease_heartbeat_interval(
        &jobs,
        "worker",
        "job",
        Duration::from_millis(1),
        LeaseFenceConfig::default(),
        None,
        || {
            std::thread::sleep(Duration::from_millis(8));
            Ok("role succeeded")
        },
    )
    .expect("turn result remains available for reconciliation");
    assert_eq!(turn.outcome, "role succeeded");
    assert!(turn.lease_lost, "final authority failure must be surfaced");
    assert!(turn
        .authority_failure
        .as_deref()
        .unwrap()
        .contains("final ownership renewal failed"));
}

#[test]
fn final_heartbeat_failure_rejects_success_at_the_job_boundary() {
    let jobs = heartbeat_fault_service(false, true);
    let result = run_with_lease_heartbeat(
        &jobs,
        "worker",
        "job",
        LeaseFenceConfig::default(),
        None,
        || Ok("role succeeded"),
    );
    let error = result.expect_err("unproven ownership cannot advance the task");
    assert!(error.to_string().contains("final ownership renewal failed"));
}

#[test]
fn heartbeat_supervisor_panic_blocks_success_even_if_final_renewal_succeeds() {
    let jobs = heartbeat_fault_service(true, false);
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupt_flag = Arc::clone(&interrupted);
    let interrupt: InterruptHandle = Arc::new(move |_| {
        interrupt_flag.store(true, Ordering::SeqCst);
    });
    let turn = run_with_lease_heartbeat_interval(
        &jobs,
        "worker",
        "job",
        Duration::from_millis(1),
        LeaseFenceConfig::default(),
        Some(interrupt),
        || {
            std::thread::sleep(Duration::from_millis(8));
            Ok("role succeeded")
        },
    )
    .expect("turn result remains available for reconciliation");
    assert_eq!(turn.outcome, "role succeeded");
    assert!(turn.lease_lost, "supervisor panic must not become no error");
    assert!(turn
        .authority_failure
        .as_deref()
        .unwrap()
        .contains("heartbeat supervisor panicked"));
    assert!(interrupted.load(Ordering::SeqCst));
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
