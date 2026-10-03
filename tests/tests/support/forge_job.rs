//! Shared fixture for the FORGE.JOB contract suite (`forge_job__0NN__*.rs`).
//!
//! Every test drives the PRODUCTION durable JobService (`forge::engine::job::WorkflowJobService`) over the production
//! workflow engine on the in-memory store (`test_harness::EngineHarness`, same transaction contract as Neon), with a
//! fixed `TestClock`. There is no network, no model, no OpenCode and no filesystem: these are infrastructure
//! contracts, and an AI turn is never part of PASS/FAIL. The role services are the production ones
//! (`forge::roles::*`) wrapped around a recording runner, so "which service executed" is observed, not assumed.
//!
//! Included with `#[path = "support/forge_job.rs"] mod support;` — a directory under `tests/` is not a test target.
#![allow(dead_code)]

use std::sync::Mutex;

use forge::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::{ForgeJobBridge, JobService, WorkflowJobService};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::roles::architect::ArchitectService;
use forge::roles::dev_ops::DevOpsService;
use forge::roles::inspector::InspectorService;
use forge::roles::lead::LeadService;
use forge::roles::qa::AssayService;
use forge::roles::scout::ScoutService;
use forge::roles::smith::SmithService;
use forge::roles::{AbstractForgeService, ForgeServiceRegistry};
use test_harness::engine::TestEngine;
use test_harness::EngineHarness;
use workflow::{Job, JobStatus, ProcessInstance, ProcessStatus, TaskStatus, Value};

pub const START_MS: i64 = 1_000_000;
pub const INSTANCE: &str = "proc-forge-job";
pub const STORY: &str = "TST-FORGE-JOB-CONTRACT";
pub const WORKER_A: &str = "worker-a";
pub const WORKER_B: &str = "worker-b";

/// The production engine on the in-memory store, at a fixed instant, with one ACTIVE process instance for jobs to
/// belong to (`create_job` refuses a job for an inactive process).
pub fn harness() -> EngineHarness {
    let harness = EngineHarness::at_unix_millis(START_MS);
    harness
        .store()
        .with_tx(|tx| {
            tx.insert_instance(ProcessInstance {
                id: INSTANCE.into(),
                tenant_id: None,
                definition_id: "FORGE_SDLC-v6".into(),
                business_key: Some(STORY.into()),
                status: ProcessStatus::Active,
                outcome: None,
                started_at: START_MS,
                ended_at: None,
                started_by: Some("forge-job-contract".into()),
                parent_instance_id: None,
                root_token_id: None,
                subject_type: Some("story".into()),
                subject_id: Some(STORY.into()),
                variables: Value::object(),
                version: 1,
            })?;
            Ok(())
        })
        .expect("an active process instance");
    harness
}

pub fn jobs(engine: &TestEngine) -> WorkflowJobService<'_, workflow::MemoryStore> {
    WorkflowJobService::new(engine)
}

pub fn task(task_id: &str, node_id: &str, status: TaskStatus) -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: task_id.into(),
        process_instance_id: INSTANCE.into(),
        story_id: STORY.into(),
        token_id: None,
        node_id: Some(node_id.into()),
        status,
        assignee: None,
        candidates: vec![],
    }
}

pub fn ready(task_id: &str, node_id: &str) -> ActiveForgeRoleTask {
    task(task_id, node_id, TaskStatus::Ready)
}

/// The runner every production service wraps here: it records which node it was asked to run, and nothing else.
#[derive(Default)]
pub struct Recording {
    calls: Mutex<Vec<String>>,
}

impl Recording {
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls lock").clone()
    }
}

impl ForgeRoleRunner for Recording {
    fn run(
        &self,
        node_id: &str,
        _task: &ActiveForgeRoleTask,
    ) -> workflow::Result<ForgeRoleOutcome> {
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

/// All seven production role services, one recording runner each, so a test can see WHICH service executed.
pub struct Services {
    pub scout_runner: Recording,
    pub architect_runner: Recording,
    pub lead_runner: Recording,
    pub smith_runner: Recording,
    pub inspector_runner: Recording,
    pub assay_runner: Recording,
    pub devops_runner: Recording,
}

impl Services {
    pub fn new() -> Self {
        Self {
            scout_runner: Recording::default(),
            architect_runner: Recording::default(),
            lead_runner: Recording::default(),
            smith_runner: Recording::default(),
            inspector_runner: Recording::default(),
            assay_runner: Recording::default(),
            devops_runner: Recording::default(),
        }
    }

    /// Every recorded call across all services, as `service:node`.
    pub fn all_calls(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (name, runner) in [
            ("scout", &self.scout_runner),
            ("architect", &self.architect_runner),
            ("lead", &self.lead_runner),
            ("smith", &self.smith_runner),
            ("inspector", &self.inspector_runner),
            ("assay", &self.assay_runner),
            ("devops", &self.devops_runner),
        ] {
            out.extend(
                runner
                    .calls()
                    .into_iter()
                    .map(|node| format!("{name}:{node}")),
            );
        }
        out
    }
}

/// The production services, built over a `Services` set of runners. Kept apart from `Services` so the registry can
/// borrow both.
pub struct Owned<'a> {
    pub scout: ScoutService<'a>,
    pub architect: ArchitectService<'a>,
    pub lead: LeadService<'a>,
    pub smith: SmithService<'a>,
    pub inspector: InspectorService<'a>,
    pub assay: AssayService<'a>,
    pub devops: DevOpsService<'a>,
}

impl<'a> Owned<'a> {
    pub fn new(runners: &'a Services) -> Self {
        Self {
            scout: ScoutService::new(&runners.scout_runner),
            architect: ArchitectService::new(&runners.architect_runner),
            lead: LeadService::new(&runners.lead_runner),
            smith: SmithService::new(&runners.smith_runner),
            inspector: InspectorService::new(&runners.inspector_runner),
            assay: AssayService::new(&runners.assay_runner),
            devops: DevOpsService::new(&runners.devops_runner),
        }
    }

    pub fn registry(&self) -> ForgeServiceRegistry<'_> {
        let mut registry = ForgeServiceRegistry::new();
        for service in [
            &self.scout as &dyn AbstractForgeService,
            &self.architect,
            &self.lead,
            &self.smith,
            &self.inspector,
            &self.assay,
            &self.devops,
        ] {
            registry
                .register(service)
                .expect("register production service");
        }
        registry
    }
}

/// READY task → bridge → durable job, exactly as production enqueues it.
pub fn enqueue_ready(
    engine: &TestEngine,
    registry: &ForgeServiceRegistry<'_>,
    task: &ActiveForgeRoleTask,
) -> String {
    let request = ForgeJobBridge::new(registry)
        .job_for_ready_task(task)
        .expect("a READY bound task becomes a job request");
    jobs(engine).enqueue(&request).expect("enqueue")
}

/// A durable `forge.role` envelope written directly, for the malformed/tampered-payload contracts.
pub fn raw_job(engine: &TestEngine, job_type: &str, payload: Value) -> String {
    engine
        .create_job(
            Some(INSTANCE),
            None,
            None,
            job_type,
            engine.current_time_ms(),
            payload,
            Some(5),
        )
        .expect("raw job")
}

/// A well-formed `forge.role` payload, as the bridge writes it.
pub fn payload(service_key: &str, node_id: &str, task_id: &str) -> Value {
    let mut payload = Value::object();
    payload.insert("serviceKey", Value::from(service_key));
    payload.insert("nodeId", Value::from(node_id));
    payload.insert("taskId", Value::from(task_id));
    payload.insert("processInstanceId", Value::from(INSTANCE));
    payload.insert("storyId", Value::from(STORY));
    payload.insert("tokenId", Value::Null);
    payload
}

pub fn job(engine: &TestEngine, id: &str) -> Job {
    engine.get_job(id).expect("job exists")
}

/// Open (pending or locked) jobs whose payload names `task_id`.
pub fn open_jobs_for_task(engine: &TestEngine, task_id: &str) -> Vec<Job> {
    engine
        .jobs_for_instance(INSTANCE)
        .expect("open jobs")
        .into_iter()
        .filter(|job| job.payload.get("taskId").and_then(Value::as_str) == Some(task_id))
        .collect()
}

pub fn assert_status(engine: &TestEngine, id: &str, status: JobStatus) {
    assert_eq!(job(engine, id).status, status, "job {id}");
}
