//! WF.TIMER — due timer fires once (TST-WF-TIMER-001).
//!
//! Contract: a timer whose due time has arrived fires **exactly once**. The production executor is
//! `WorkflowEngine::run_due_jobs` (`engine/fire_timer_job.rs:141-196`): it claims the timer job, `fire_timer_job`
//! marks the job `Completed`, emits one `timer.fired` event, and moves the parked token along the timer's resume
//! transition. A job in `Completed` is settled: a second fire is refused with `TIMER_ALREADY_FIRED`
//! (`fire_timer_job.rs:23-28`), and the next executor pass claims nothing — no second `timer.fired`, no second
//! token movement, no continuation token at the destination beyond the one the first fire produced.
//!
//! ```text
//! start -> wait (timer, due at t0) -> done (end)
//! ```
//!
//! Level: L4 Adversarial, harness `DeterministicClockHarness`. The clock is frozen at the due instant so "due" is
//! decided by the test, not by the wall clock. Deterministic and isolated: in-memory store, no database, no live
//! provider — the only worker is the one the test names.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_timer__001__due_timer_fires_once

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    DefinitionStatus, FireTimerParams, Job, JobStatus, NodeDefinition, ProcessDefinition,
    ProcessEvent, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, TimerSpec,
    Token, TokenStatus, TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "DeterministicClockHarness/L4 Adversarial";
const DEFINITION_KEY: &str = "TST-WF-TIMER-001";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const WAIT_NODE: &str = "wait";
const END_NODE: &str = "done";
const WORKER: &str = "worker-1";
/// The timer's due instant: the engine's frozen clock sits exactly on it, so the job is due.
const DUE_AT_MS: i64 = 1_700_000_060_000;
const DUE_AT: &str = "1700000060000";

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
        id: "tst-wf-timer-001".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.TIMER 001".to_string(),
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

fn open_jobs(harness: &EngineHarness, instance: &str) -> Vec<Job> {
    harness
        .store()
        .with_tx(|tx| tx.open_jobs_for_instance(instance))
        .expect("the instance jobs are readable")
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

fn instance_status(harness: &EngineHarness, instance: &str) -> ProcessStatus {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .status
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TIMER-001); the file and the assay use it.
fn wf_timer_001__due_timer_fires_once() {
    let harness = EngineHarness::at_unix_millis(DUE_AT_MS);
    harness
        .engine()
        .seed_definition(definition())
        .expect("the definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on its timer");
    let instance = started.process_instance_id.clone();

    // Parked at the timer node: one pending job, one active token, the process active.
    let parked_job = {
        let jobs = open_jobs(&harness, &instance);
        assert_eq!(
            jobs.len(),
            1,
            "{HARNESS}: the timer node parked exactly one job"
        );
        assert_eq!(jobs[0].status, JobStatus::Pending);
        jobs[0].clone()
    };
    let at_wait = tokens(&harness, &instance);
    assert_eq!(
        at_wait
            .iter()
            .filter(|t| t.status == TokenStatus::Active)
            .count(),
        1,
        "{HARNESS}: exactly one token is active, at the timer node"
    );
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Active,
        "{HARNESS}: the process waits on its timer"
    );

    // The due timer fires: exactly once, announced once, job settled, token moved through.
    let report = harness
        .engine()
        .run_due_jobs(WORKER, 8)
        .expect("the due timer fires through the production executor");
    assert_eq!(
        report.claimed.len(),
        1,
        "{HARNESS}: exactly one job was due"
    );
    assert_eq!(report.fired, 1, "{HARNESS}: exactly one job fired");
    assert_eq!(
        events_of_type(&harness, &instance, "timer.fired").len(),
        1,
        "{HARNESS}: exactly one timer.fired is emitted"
    );
    let fired = events_of_type(&harness, &instance, "timer.fired");
    assert_eq!(
        fired[0].job_id.as_deref(),
        Some(parked_job.id.as_str()),
        "{HARNESS}: the fired event names the parked job"
    );
    let fired_job = harness
        .store()
        .with_tx(|tx| tx.get_job(&parked_job.id))
        .expect("the job is readable");
    assert_eq!(
        fired_job.status,
        JobStatus::Completed,
        "{HARNESS}: the fired job is settled Completed"
    );
    assert_eq!(
        fired_job.locked_by, None,
        "{HARNESS}: the fired job releases its lease"
    );
    assert_eq!(
        open_jobs(&harness, &instance).len(),
        0,
        "{HARNESS}: no job remains open after the fire"
    );
    // The token moved along the resume transition and the process converged.
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Completed,
        "{HARNESS}: the fired timer drove the process to its end"
    );
    assert!(
        tokens(&harness, &instance)
            .iter()
            .any(|t| t.node_id == END_NODE && t.status == TokenStatus::Completed),
        "{HARNESS}: a token reached and settled the end node"
    );

    // ── NEGATIVE/REFUSAL: the fired job cannot be fired again ─────────────────────────────────────────────────
    let again = harness
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: parked_job.id.clone(),
            worker_id: WORKER.to_string(),
            variables: Value::object(),
        })
        .expect_err("a completed timer job cannot fire twice");
    assert_eq!(
        again.code(),
        "TIMER_ALREADY_FIRED",
        "{HARNESS}: the refusal names the already-fired rule"
    );
    // A second executor pass over the same instant claims nothing: the executor is how a retry of the whole pass
    // would double-fire, and the settled job stops it.
    let second = harness
        .engine()
        .run_due_jobs(WORKER, 8)
        .expect("the second pass runs");
    assert_eq!(
        second.claimed.len(),
        0,
        "{HARNESS}: the settled job is not reclaimed"
    );
    assert_eq!(second.fired, 0, "{HARNESS}: the second pass fires nothing");
    assert_eq!(
        events_of_type(&harness, &instance, "timer.fired").len(),
        1,
        "{HARNESS}: the durable log still holds exactly one timer.fired"
    );
}
