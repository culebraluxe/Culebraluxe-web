//! WF.TRACE — timer fired (TST-WF-TRACE-008).
//!
//! Contract: when a timer's due instant arrives, the timer fires once, the token that was parked on it resumes
//! along the declared resume transition, and the process moves on — with the fire announced on the durable log
//! against the job it consumed and the worker that ran it.
//!
//! The clock is not itself an executor: advancing a `TestClock` changes what `now` answers and nothing else, so a
//! timer that has fallen due but is never swept simply stays parked. The production executor is
//! `WorkflowEngine::run_due_jobs` (`fire_timer_job.rs:141-196`), which `wf_timer__001`/`002` also drive: it
//! reclaims leases that expired, claims the due jobs of its own type (`due_at <= now`), and then `fire_timer_job`
//! marks the job `Completed` and emits `timer.fired` naming that job, releasing the lease, before moving the
//! token along the node's resume transition (`timer.transition`, falling back to the node's first transition).
//! This file drives that path: the graph declares a resume transition from the timer node to `end`, the sweep
//! refuses to fire before the due instant, and advancing the clock onto the due instant is what makes the very
//! same sweep fire it exactly once.
//!
//! Corrected against the engine: the first draft left the timer node with no outgoing transitions and no due
//! instant and then asserted that advancing the clock alone completed the process. A transition-less timer node
//! cannot resume (`fire_timer_job.rs:88-96`), and only an executor fires — so nothing completed.
//!
//! Level: L2 Composition, harness `WorkflowHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__008__timer_fired

use test_harness::EngineHarness;
use workflow::{
    DefinitionStatus, Job, JobStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, TimerSpec,
    TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L2 Composition";
const DEFINITION_KEY: &str = "TST-WF-TRACE-008";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const WORKER: &str = "worker-1";

const START_AT: i64 = 1_600_000_000_001;
const DAY_MS: i64 = 86_400_000;
/// The one-day-later instant the timer node declares as an absolute instant.
const DUE_AT: i64 = START_AT + DAY_MS;

const START_NODE: &str = "start";
const TIMER_NODE: &str = "wait_one_day";
const END_NODE: &str = "end";
const BEGIN: &str = "begin";
const RESUME: &str = "resume";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> wait_one_day (timer) -> end`.
fn timer_definition() -> ProcessDefinition {
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, TIMER_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        TIMER_NODE.to_string(),
        NodeDefinition {
            id: TIMER_NODE.to_string(),
            node_type: "timer".to_string(),
            name: Some("Wait one day".to_string()),
            timer: Some(TimerSpec {
                due_at: Some(DUE_AT.to_string()),
                due_at_variable: None,
                transition: Some(RESUME.to_string()),
            }),
            // A timer node must declare where the token goes when it fires.
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

fn instance(harness: &EngineHarness, instance_id: &str) -> workflow::ProcessInstance {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance_id))
        .expect("the instance is readable")
}

fn history(harness: &EngineHarness, instance_id: &str) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance_id, 512))
        .expect("the instance history reads")
}

fn events_of_type(
    harness: &EngineHarness,
    instance_id: &str,
    event_type: &str,
) -> Vec<ProcessEvent> {
    history(harness, instance_id)
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

fn job_row(harness: &EngineHarness, job_id: &str) -> Job {
    harness
        .store()
        .with_tx(|tx| tx.get_job(job_id))
        .expect("the job row is readable")
}

/// Start the process: it must park on the timer, leaving one Pending job due a day later.
fn start_and_park(harness: &EngineHarness) -> (String, Job) {
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on its timer");
    let instance_id = started.process_instance_id;
    let parked = harness
        .store()
        .with_tx(|tx| tx.open_jobs_for_instance(&instance_id))
        .expect("the instance jobs are readable");
    assert_eq!(
        parked.len(),
        1,
        "{HARNESS}: the timer node parks exactly one job"
    );
    let job = parked[0].clone();
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.job_type, "timer");
    assert_eq!(job.attempts, 0);
    assert_eq!(
        job.locked_by, None,
        "{HARNESS}: the parked timer holds no lease"
    );
    assert_eq!(
        job.due_at, DUE_AT,
        "{HARNESS}: the parked job is due at the instant the node declared"
    );
    assert_eq!(
        instance(harness, &instance_id).status,
        ProcessStatus::Active,
        "{HARNESS}: the process waits on the timer"
    );
    (instance_id, job)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TRACE-008); the file and the assay use it.
fn wf_trace_008__timer_fired() {
    let harness = EngineHarness::at_unix_millis(START_AT);
    harness
        .engine()
        .seed_definition(timer_definition())
        .expect("the definition registers with the engine");

    // 1. Start: the process parks on the timer and the dispatch is announced, naming the job.
    let (instance_id, job) = start_and_park(&harness);
    let scheduled = events_of_type(&harness, &instance_id, "timer.scheduled");
    assert_eq!(
        scheduled.len(),
        1,
        "{HARNESS}: parking on the timer is announced once"
    );
    assert_eq!(
        scheduled[0].job_id.as_deref(),
        Some(job.id.as_str()),
        "{HARNESS}: timer.scheduled names the parked job"
    );
    assert_eq!(
        scheduled[0].data.get("dueAt").and_then(|v| v.as_str()),
        Some(DUE_AT.to_string().as_str()),
        "{HARNESS}: timer.scheduled carries the declared due instant"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "timer.fired").len(),
        0,
        "{HARNESS}: a parked timer has not fired"
    );

    // 2. Before the due instant the sweep must not fire: `now` is still short of `due_at`.
    let before = harness
        .engine()
        .run_due_jobs(WORKER, 8)
        .expect("the executor sweeps the timer jobs");
    assert_eq!(
        before.claimed.len(),
        0,
        "{HARNESS}: a timer that is not due is never even claimed"
    );
    assert_eq!(
        before.fired, 0,
        "{HARNESS}: a timer that is not due does not fire"
    );
    assert_eq!(before.failed, 0, "{HARNESS}: nothing failed");
    assert_eq!(
        instance(&harness, &instance_id).status,
        ProcessStatus::Active,
        "{HARNESS}: the process is still parked"
    );
    let untouched = job_row(&harness, &job.id);
    assert_eq!(untouched.status, JobStatus::Pending);
    assert_eq!(
        untouched.attempts, 0,
        "{HARNESS}: a refused sweep is not an attempt"
    );
    assert_eq!(untouched.locked_by, None);
    assert_eq!(
        events_of_type(&harness, &instance_id, "timer.fired").len(),
        0,
        "{HARNESS}: no fire was recorded"
    );

    // 3. Advance onto the due instant and sweep again: now it fires, exactly once.
    harness.clock().advance_millis(DAY_MS);
    assert_eq!(
        harness.now_millis(),
        DUE_AT,
        "{HARNESS}: the clock sits on the due instant"
    );
    let sweep = harness
        .engine()
        .run_due_jobs(WORKER, 8)
        .expect("the executor sweeps the timer jobs again");
    assert_eq!(
        sweep.claimed.len(),
        1,
        "{HARNESS}: the due timer is claimed once"
    );
    assert_eq!(sweep.fired, 1, "{HARNESS}: the due timer fires");
    assert_eq!(sweep.failed, 0, "{HARNESS}: the fire succeeded");

    // 4. The fire moved the process: the token resumed along `resume` and reached the end node.
    let settled = instance(&harness, &instance_id);
    assert_eq!(
        settled.status,
        ProcessStatus::Completed,
        "{HARNESS}: firing the last timer completes the process"
    );
    assert_eq!(
        settled.outcome,
        Some(ProcessOutcome::Completed),
        "{HARNESS}: the end node's declared outcome is recorded"
    );
    assert_eq!(
        settled.ended_at,
        Some(DUE_AT),
        "{HARNESS}: the process ends at the instant it fired"
    );

    // 5. The fire is recorded on the durable trace, naming the job and the worker that ran it.
    let fired = events_of_type(&harness, &instance_id, "timer.fired");
    assert_eq!(fired.len(), 1, "{HARNESS}: the timer fired exactly once");
    assert_eq!(
        fired[0].job_id.as_deref(),
        Some(job.id.as_str()),
        "{HARNESS}: the fire names the job it consumed"
    );
    assert_eq!(
        fired[0].actor, WORKER,
        "{HARNESS}: the fire names the worker that ran it"
    );
    assert_eq!(
        fired[0].data.get("type").and_then(|v| v.as_str()),
        Some("timer"),
        "{HARNESS}: the fire records the executor type"
    );

    // The consumed job is settled and its lease released — nothing is left to fire again.
    let consumed = job_row(&harness, &job.id);
    assert_eq!(consumed.status, JobStatus::Completed);
    assert_eq!(
        consumed.locked_by, None,
        "{HARNESS}: the fire releases the lease"
    );
    assert_eq!(
        consumed.attempts, 1,
        "{HARNESS}: the claim it fired under was its one attempt"
    );
    assert!(
        consumed.completed_at.is_some(),
        "{HARNESS}: the row records when it completed"
    );

    // And no second fire is possible: the job is no longer open.
    let after = harness
        .engine()
        .run_due_jobs(WORKER, 8)
        .expect("a further sweep runs clean");
    assert_eq!(
        after.claimed.len(),
        0,
        "{HARNESS}: a completed timer is not claimable"
    );
    assert_eq!(after.fired, 0, "{HARNESS}: a timer fires once");
    assert_eq!(
        events_of_type(&harness, &instance_id, "timer.fired").len(),
        1,
        "{HARNESS}: the log still holds exactly one fire"
    );
}
