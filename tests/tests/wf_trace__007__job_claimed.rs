//! WF.TRACE — job claimed (TST-WF-TRACE-007).
//!
//! Contract: when a human task is completed and the engine dispatches work as a job, the claim of that job is
//! recorded against the process instance — on the row that holds the lease and on the durable event log.
//!
//! This file drives the production boundary, not a re-declaration of it. The real `WorkflowEngine<MemoryStore>`
//! dispatches through its `timer` node (`handle_join.rs:121-180`): arriving there parks the token, inserts one
//! `timer` job — `Pending`, `attempts = 0`, due at the engine's `now` — and emits `timer.scheduled` naming that
//! job. The claim is `WorkflowEngine::claim_jobs_by_type` (`engine_options.rs:555`), the same call the production
//! executor makes before it fires anything (`fire_timer_job.rs:148`): it takes the lease on the row
//! (`status = Locked`, `locked_by`, `locked_until = now + JOB_LEASE_MS`, `attempts += 1`) and announces
//! `job.claimed` with the claimant as actor and the job id on the event. A second worker asking for the same job
//! is handed nothing and cannot displace the lease, and only the claimant may fire it
//! (`TIMER_LOCK_OWNER`, `fire_timer_job.rs:37-43`).
//!
//! Corrected against the engine: the first draft gave the dispatch node `node_type: "job"` — there is no such
//! node type, so the token ran straight through to `end` and no job ever existed — and pinned a `claimId` inside
//! the event's `data`. The event's own `job_id` column is where the engine names the job (`wf_timer__001` reads
//! it the same way), and the dispatcher that parks a job is the timer node.
//!
//! Level: L2 Composition, harness `WorkflowHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__007__job_claimed

use test_harness::EngineHarness;
use workflow::{
    CompleteTaskParams, DefinitionStatus, FireTimerParams, Job, JobStatus, NodeDefinition,
    ProcessDefinition, ProcessEvent, ProcessGraph, ProcessOutcome, ProcessStatus,
    StartProcessParams, TaskStatus, TimerSpec, TransitionDefinition, Value, JOB_LEASE_MS,
};

const HARNESS: &str = "WorkflowHarness/L2 Composition";
const DEFINITION_KEY: &str = "TST-WF-TRACE-007";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const WORKER: &str = "worker-1";
const OTHER_WORKER: &str = "worker-2";
/// The instant the engine's clock is frozen at: the dispatch parks a job due exactly here.
const START_AT: i64 = 1_600_000_000_000;

const START_NODE: &str = "start";
const TASK_NODE: &str = "human_task";
const DISPATCH_NODE: &str = "dispatch";
const END_NODE: &str = "end";
const BEGIN: &str = "begin";
const COMPLETE: &str = "complete";
const RESUME: &str = "resume";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> human_task -> dispatch (timer job) -> end`.
fn human_task_with_job_definition() -> ProcessDefinition {
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, TASK_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        TASK_NODE.to_string(),
        NodeDefinition {
            id: TASK_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Human Task".to_string()),
            transitions: Some(vec![transition(COMPLETE, DISPATCH_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        DISPATCH_NODE.to_string(),
        NodeDefinition {
            id: DISPATCH_NODE.to_string(),
            node_type: "timer".to_string(),
            name: Some("Dispatch".to_string()),
            // No absolute due instant: the dispatched job is due at the engine's `now`, so the only thing
            // between the claim and a fire is the lease the claim takes.
            timer: Some(TimerSpec {
                due_at: None,
                due_at_variable: None,
                transition: Some(RESUME.to_string()),
            }),
            transitions: Some(vec![transition(RESUME, END_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        END_NODE.to_string(),
        NodeDefinition {
            id: END_NODE.to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Completed),
            ..Default::default()
        },
    );
    ProcessDefinition {
        id: format!("{DEFINITION_KEY}-def"),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: DEFINITION_KEY.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START_NODE.to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

fn start_params() -> StartProcessParams {
    StartProcessParams {
        definition_key: DEFINITION_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

fn instance_status(harness: &EngineHarness, instance: &str) -> ProcessStatus {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .status
}

fn history(harness: &EngineHarness, instance: &str) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance, 512))
        .expect("the instance history reads")
}

fn events_of_type(harness: &EngineHarness, instance: &str, event_type: &str) -> Vec<ProcessEvent> {
    history(harness, instance)
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

/// The open (unsettled) jobs of the instance, read through the production `Store` seam.
fn open_jobs(harness: &EngineHarness, instance: &str) -> Vec<Job> {
    harness
        .store()
        .with_tx(|tx| tx.open_jobs_for_instance(instance))
        .expect("the instance jobs are readable")
}

/// The job row, re-read in a fresh transaction rather than trusted from a return value.
fn job_row(harness: &EngineHarness, job_id: &str) -> Job {
    harness
        .store()
        .with_tx(|tx| tx.get_job(job_id))
        .expect("the job row is readable")
}

/// Start a process, let it commit and park on its task, and hand back `(instance_id, task_id)`.
fn start_and_park(harness: &EngineHarness) -> (String, String) {
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on its task");
    let instance_id = started.process_instance_id;
    let task_id = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&instance_id))
        .expect("the instance's tasks are readable")
        .into_iter()
        .filter(|task| task.status == TaskStatus::Ready)
        .last()
        .expect("the process parks on a ready task before the command")
        .id;
    (instance_id, task_id)
}

/// Complete the parked task, which drives the token into the dispatching node.
fn complete_task(harness: &EngineHarness, task_id: &str) {
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_id.to_string(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(COMPLETE.to_string()),
        })
        .expect("the task completes and drives the dispatch");
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TRACE-007); the file and the assay use it.
fn wf_trace_007__job_claimed() {
    let harness = EngineHarness::at_unix_millis(START_AT);
    harness
        .engine()
        .seed_definition(human_task_with_job_definition())
        .expect("the definition registers with the engine");

    // 1. Start the process and park on the human task.
    let (instance, task) = start_and_park(&harness);
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Active,
        "{HARNESS}: the process parks on a ready task after start"
    );

    // 2. Complete the human task: the engine dispatches one job and parks on it.
    complete_task(&harness, &task);
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Active,
        "{HARNESS}: the process waits on the job it dispatched"
    );
    let parked = open_jobs(&harness, &instance);
    assert_eq!(
        parked.len(),
        1,
        "{HARNESS}: the dispatch parked exactly one job"
    );
    let job = parked[0].clone();
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(
        job.job_type, "timer",
        "{HARNESS}: the executor type is named on the row"
    );
    assert_eq!(
        job.attempts, 0,
        "{HARNESS}: a freshly dispatched job has no attempts"
    );
    assert_eq!(
        job.locked_by, None,
        "{HARNESS}: a freshly dispatched job has no lease"
    );
    assert_eq!(
        job.process_instance_id.as_deref(),
        Some(instance.as_str()),
        "{HARNESS}: the job is dispatched against this process instance"
    );
    assert!(
        job.token_id.is_some(),
        "{HARNESS}: the job names the token it will resume"
    );
    assert_eq!(
        job.due_at, START_AT,
        "{HARNESS}: the dispatch node left the due instant at the engine's now"
    );
    assert_eq!(
        job.payload.get("nodeId").and_then(|v| v.as_str()),
        Some(DISPATCH_NODE),
        "{HARNESS}: the job payload names the node it resumes"
    );
    assert_eq!(
        job.payload.get("transition").and_then(|v| v.as_str()),
        Some(RESUME),
        "{HARNESS}: the job payload names the declared resume transition"
    );

    // 3. The dispatch is on the durable log, naming the job it parked.
    let scheduled = events_of_type(&harness, &instance, "timer.scheduled");
    assert_eq!(
        scheduled.len(),
        1,
        "{HARNESS}: the dispatch is announced once on the durable log"
    );
    assert_eq!(
        scheduled[0].job_id.as_deref(),
        Some(job.id.as_str()),
        "{HARNESS}: timer.scheduled names the dispatched job"
    );
    assert_eq!(
        scheduled[0].node_id.as_deref(),
        Some(DISPATCH_NODE),
        "{HARNESS}: timer.scheduled names the dispatching node"
    );

    // 4. Claim the dispatched job through the production claim path: the lease is taken on the row.
    let claimed = harness
        .engine()
        .claim_jobs_by_type(WORKER, "timer", 8)
        .expect("the worker claims the dispatched job");
    assert_eq!(
        claimed.len(),
        1,
        "{HARNESS}: the sweep hands the due job to the one worker"
    );
    let lease = claimed[0].clone();
    assert_eq!(
        lease.id, job.id,
        "{HARNESS}: the claim takes the parked job"
    );
    assert_eq!(lease.status, JobStatus::Locked);
    assert_eq!(lease.locked_by.as_deref(), Some(WORKER));
    assert_eq!(lease.attempts, 1, "{HARNESS}: one claim is one attempt");
    assert_eq!(
        lease.locked_until,
        Some(START_AT + JOB_LEASE_MS),
        "{HARNESS}: the lease runs for JOB_LEASE_MS from the claim"
    );
    // The row, not the returned copy, is what a second worker would read.
    let row = job_row(&harness, &job.id);
    assert_eq!(
        row.status,
        JobStatus::Locked,
        "{HARNESS}: the claim is durable"
    );
    assert_eq!(row.locked_by.as_deref(), Some(WORKER));
    assert_eq!(row.attempts, 1);
    assert_eq!(
        row.updated_at, START_AT,
        "{HARNESS}: the claim is stamped with the claim instant"
    );

    // 5. The claim is on the durable trace, against the process instance.
    let claims = events_of_type(&harness, &instance, "job.claimed");
    assert_eq!(
        claims.len(),
        1,
        "{HARNESS}: the claim is announced once on the durable log"
    );
    let claim = &claims[0];
    assert_eq!(
        claim.job_id.as_deref(),
        Some(job.id.as_str()),
        "{HARNESS}: the claim names the claimed job"
    );
    assert_eq!(
        claim.actor, WORKER,
        "{HARNESS}: the claim names the worker who took it"
    );
    assert_eq!(
        claim.token_id, job.token_id,
        "{HARNESS}: the claim is tied to the token the job will resume"
    );
    assert_eq!(
        claim.node_id, None,
        "{HARNESS}: a claim is a job fact, not a node visit"
    );
    assert_eq!(
        claim.data.get("type").and_then(|v| v.as_str()),
        Some("timer")
    );
    assert_eq!(
        claim.data.get("attempts").and_then(|v| v.as_i64()),
        Some(1),
        "{HARNESS}: the claim records the attempt it is"
    );
    assert!(
        scheduled[0].id < claim.id,
        "{HARNESS}: the claim follows the dispatch it took"
    );

    // 6. NEGATIVE/FAULT — the lease is exclusive: a second worker gets nothing and displaces nothing.
    let second = harness
        .engine()
        .claim_jobs_by_type(OTHER_WORKER, "timer", 8)
        .expect("the second worker's sweep runs");
    assert!(
        second.is_empty(),
        "{HARNESS}: a locked job is not offered to a second worker"
    );
    let unchanged = job_row(&harness, &job.id);
    assert_eq!(
        unchanged.locked_by.as_deref(),
        Some(WORKER),
        "{HARNESS}: the second worker did not displace the lease"
    );
    assert_eq!(
        unchanged.attempts, 1,
        "{HARNESS}: a refused claim is not an attempt"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "job.claimed").len(),
        1,
        "{HARNESS}: a refused claim is not announced"
    );

    // Only the claimant may run the job the claim took.
    let refusal = harness
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: job.id.clone(),
            worker_id: OTHER_WORKER.to_string(),
            variables: Value::object(),
        })
        .expect_err("a job locked by one worker cannot be fired by another");
    assert_eq!(
        refusal.code(),
        "TIMER_LOCK_OWNER",
        "{HARNESS}: the refusal names the lock-owner rule"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "timer.fired").len(),
        0,
        "{HARNESS}: the refused fire left no event"
    );
}
