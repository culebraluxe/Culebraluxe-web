//! WF.TIMER — not-due timer does nothing (TST-WF-TIMER-002).
//!
//! Contract: a timer whose due time has not arrived must produce **no effect at all**. The production executor is
//! `WorkflowEngine::run_due_jobs` (`engine/fire_timer_job.rs:141-196`), whose claim gate is
//! `due_at <= now` — `claim_due_jobs_by_type` in the store (`memory.rs:505-527`). A not-due job is never claimed,
//! never fires, and leaves every durable record exactly where the timer parked it: the job stays `Pending` and
//! unlocked, the token stays `Active` at the timer node, the process stays `Active`, and the event log gains no
//! `timer.fired`, no `token.moved`, and no stray `job.*` transition. Firing the unclaimed job directly is likewise
//! refused (`TIMER_NOT_LOCKED`, `fire_timer_job.rs:30-36`), so neither the executor nor a direct call can smuggle
//! a fire past the due-time gate. Once the clock does pass the due time, the same gate admits it — proving the
//! earlier silence was the due-time check working, not a runner that was already broken.
//!
//! ```text
//! start -> wait (timer, due at t0+60s) -> done (end)
//! ```
//!
//! Level: L4 Adversarial, harness `DeterministicClockHarness`. Frozen clock keeps "not yet due" exact; the due
//! check is decided by the test. Deterministic and isolated: in-memory store, no database, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_timer__002__not_due_timer_does_nothing

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    DefinitionStatus, FireTimerParams, Job, JobStatus, NodeDefinition, ProcessDefinition,
    ProcessEvent, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, TimerSpec,
    Token, TokenStatus, TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "DeterministicClockHarness/L4 Adversarial";
const DEFINITION_KEY: &str = "TST-WF-TIMER-002";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const WAIT_NODE: &str = "wait";
const END_NODE: &str = "done";
const WORKER: &str = "worker-1";
/// The engine starts one minute before the timer is due.
const START_AT: i64 = 1_700_000_000_000;
const DUE_AT: &str = "1700000060000"; // = START_AT + 60_000

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> wait (timer) -> done (end)`.
fn definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", WAIT_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        WAIT_NODE.to_string(),
        NodeDefinition {
            id: WAIT_NODE.to_string(),
            node_type: "timer".to_string(),
            name: Some("Wait".to_string()),
            timer: Some(TimerSpec {
                due_at: Some(DUE_AT.to_string()),
                due_at_variable: None,
                transition: Some("resume".to_string()),
            }),
            transitions: Some(vec![transition("resume", END_NODE)]),
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
        id: "tst-wf-timer-002".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.TIMER 002".to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
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

fn tokens(harness: &EngineHarness, instance: &str) -> Vec<Token> {
    harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(instance))
        .expect("the instance tokens are readable")
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

fn open_jobs(harness: &EngineHarness, instance: &str) -> Vec<Job> {
    harness
        .store()
        .with_tx(|tx| tx.open_jobs_for_instance(instance))
        .expect("the instance jobs are readable")
}

fn instance_status(harness: &EngineHarness, instance: &str) -> ProcessStatus {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .status
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TIMER-002); the file and the assay use it.
fn wf_timer_002__not_due_timer_does_nothing() {
    let harness = EngineHarness::at_unix_millis(START_AT);
    harness
        .engine()
        .seed_definition(definition())
        .expect("the definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on its timer");
    let instance = started.process_instance_id.clone();

    // Parked: one pending job and one active token at the timer node, before the due time.
    let parked_jobs = open_jobs(&harness, &instance);
    assert_eq!(
        parked_jobs.len(),
        1,
        "{HARNESS}: the timer node parked one job"
    );
    assert_eq!(parked_jobs[0].status, JobStatus::Pending);
    assert_eq!(parked_jobs[0].locked_by, None);
    assert_eq!(
        tokens(&harness, &instance)
            .iter()
            .filter(|t| t.status == TokenStatus::Active)
            .count(),
        1,
        "{HARNESS}: one token is active at the timer node"
    );

    // Not due: the executor admits nothing, fires nothing, and the log and rows do not move.
    let before_history: Vec<i64> = history(&harness, &instance)
        .iter()
        .map(|event| event.id)
        .collect();
    let report = harness
        .engine()
        .run_due_jobs(WORKER, 8)
        .expect("the executor runs against a not-due timer");
    assert_eq!(
        report.claimed.len(),
        0,
        "{HARNESS}: nothing is claimed while not due"
    );
    assert_eq!(report.fired, 0, "{HARNESS}: nothing fires while not due");
    assert_eq!(report.failed, 0, "{HARNESS}: nothing fails while not due");
    let after_jobs = open_jobs(&harness, &instance);
    assert_eq!(after_jobs.len(), 1, "{HARNESS}: the job is still parked");
    assert_eq!(
        after_jobs[0].status,
        JobStatus::Pending,
        "{HARNESS}: the job stays Pending"
    );
    assert_eq!(
        after_jobs[0].locked_by, None,
        "{HARNESS}: the job was never locked"
    );
    assert_eq!(
        after_jobs[0].attempts, 0,
        "{HARNESS}: the job was never attempted"
    );
    assert_eq!(
        tokens(&harness, &instance)
            .iter()
            .filter(|t| t.status == TokenStatus::Active && t.node_id == WAIT_NODE)
            .count(),
        1,
        "{HARNESS}: the token is still Active at the timer node"
    );
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Active,
        "{HARNESS}: the process is still waiting"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "timer.fired").len(),
        0,
        "{HARNESS}: no timer.fired is emitted while not due"
    );
    assert_eq!(
        history(&harness, &instance)
            .iter()
            .map(|event| event.id)
            .collect::<Vec<_>>(),
        before_history,
        "{HARNESS}: the executor appended no event to the log while the timer was not due"
    );

    // A direct fire is refused too: the due-time gate is not bypassable by bypassing the executor's claim.
    let sneaky = harness
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: parked_jobs[0].id.clone(),
            worker_id: WORKER.to_string(),
            variables: Value::object(),
        })
        .expect_err("a not-due, never-claimed job cannot be fired directly");
    assert_eq!(
        sneaky.code(),
        "TIMER_NOT_LOCKED",
        "{HARNESS}: the direct fire is refused at the not-locked gate"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "timer.fired").len(),
        0,
        "{HARNESS}: the refused fire left no event"
    );

    // Positive control: the clock crosses the due time and the same executor fires the job exactly once.
    harness.clock().advance_millis(60_000);
    let due_report = harness
        .engine()
        .run_due_jobs(WORKER, 8)
        .expect("the executor runs once the timer is due");
    assert_eq!(
        due_report.fired, 1,
        "{HARNESS}: the same executor fires the due job now"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "timer.fired").len(),
        1,
        "{HARNESS}: exactly one timer.fired, only after the due time"
    );
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Completed,
        "{HARNESS}: the fired timer drove the process to completion"
    );
}
