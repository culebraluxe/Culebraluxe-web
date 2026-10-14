//! WF.TIMER — timer against completed token no-ops (TST-WF-TIMER-008).
//!
//! Contract: when a timer fires but the token it was waiting on is **no longer active**, the timer is a no-op on the
//! process. `fire_timer_job` settles the job first and unconditionally — status `Completed`, lease cleared,
//! `completed_at` stamped (`middle/workflow/src/engine/fire_timer_job.rs:46-51`) — and only then considers the token,
//! and the resume is gated on `token.status == TokenStatus::Active` (`:53-55`). A settled token is therefore left
//! exactly as it is: no move, no version consumed, no reactivation, and no second run of the graph. The job still
//! settles and still records `timer.fired` (`:122-136`), because the timer *did* come due and *was* consumed — it
//! simply has nothing left to resume.
//!
//! That guard is not defensive padding. A timer and its token can diverge in the ordinary course of business: a
//! worker takes the lease (`:21-35`), and inside that lease window the token can be settled by some other path. When
//! the worker finally gets round to firing, the token it is resuming no longer exists as far as the graph is
//! concerned. Without the `Active` check the fire would `move_token` a completed token and re-enter the graph from a
//! node the process has already left.
//!
//! This test therefore runs the **same fire twice** — once against an active token and once against a settled one —
//! and contrasts them. That pairing is the load-bearing part: an assertion that the settled case "did not move" would
//! also be satisfied by a fire that never worked, so the control proves the resume path is live and that the
//! difference is the token's state and nothing else.
//!
//! Settling the token under a held lease is done through the production `Store::complete_token`
//! (`middle/workflow/src/memory.rs:251-262`) — the same call `terminate_process` and the end node use to settle a
//! token (`middle/workflow/src/engine/execute_node_leave.rs:224`). It is a production interface method, not a
//! re-declaration; the scenario it constructs is precisely the one the guard exists for.
//!
//! Level: L4 Adversarial, harness `DeterministicClockHarness`. The adversarial moment is the lease: the job is claimed
//! first, and the token settles while the claim is live. Deterministic and isolated: no database, no network, no
//! filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_timer__008__timer_against_completed_token_no_ops

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    DefinitionStatus, FireTimerParams, Job, JobStatus, NodeDefinition, ProcessDefinition,
    ProcessEvent, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, TimerSpec,
    Token, TokenOutcome, TokenStatus, TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "DeterministicClockHarness/L4 Adversarial";

/// The definition key the engine registers.
const DEFINITION_KEY: &str = "TST-WF-TIMER-008";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// The graph: `start -> wait (timer) -> done (end)`.
const START_NODE: &str = "start";
const WAIT_NODE: &str = "wait";
const DONE_NODE: &str = "done";
const BEGIN: &str = "begin";
const RESUME: &str = "resume";

/// The frozen instant the graph runs at.
const NOW: i64 = 1_700_000_000_000;

/// The worker that claims and fires.
const WORKER: &str = "worker-a";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> wait (timer) -> done (end)`.
fn timer_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, WAIT_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        WAIT_NODE.to_string(),
        NodeDefinition {
            id: WAIT_NODE.to_string(),
            node_type: "timer".to_string(),
            name: Some("Wait".to_string()),
            transitions: Some(vec![transition(RESUME, DONE_NODE)]),
            timer: Some(TimerSpec {
                due_at: None,
                due_at_variable: None,
                transition: Some(RESUME.to_string()),
            }),
            ..Default::default()
        },
    );
    nodes.insert(
        DONE_NODE.to_string(),
        NodeDefinition {
            id: DONE_NODE.to_string(),
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

fn read_job(harness: &EngineHarness, id: &str) -> Job {
    harness
        .store()
        .with_tx(|tx| tx.get_job(id))
        .expect("the job is readable")
}

fn read_token(harness: &EngineHarness, id: &str) -> Token {
    harness
        .store()
        .with_tx(|tx| tx.get_token(id))
        .expect("the token is readable")
}

fn events_of_type(
    harness: &EngineHarness,
    instance_id: &str,
    event_type: &str,
) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

/// Start the timer graph, claim its one timer job, and return the leased job, its token and its instance.
fn leased_timer(harness: &EngineHarness) -> (Job, String, String) {
    harness
        .engine()
        .seed_definition(timer_definition())
        .expect("the timer definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on the timer node");
    let token_id = started.root_token_id.clone();
    let instance_id = harness
        .store()
        .with_tx(|tx| tx.get_token(&token_id))
        .expect("the token reads")
        .process_instance_id;
    let jobs: Vec<Job> = harness
        .store()
        .with_tx(|tx| tx.open_jobs_for_token(&token_id))
        .expect("the token's jobs read");
    assert_eq!(
        jobs.len(),
        1,
        "{HARNESS}: the timer node schedules exactly one job"
    );
    let claimed = harness
        .engine()
        .claim_jobs_by_type(WORKER, "timer", 10)
        .expect("the claim step commits");
    assert_eq!(
        claimed.len(),
        1,
        "{HARNESS}: the worker holds the lease before anything settles the token"
    );
    (
        claimed.into_iter().next().expect("the claimed job"),
        token_id,
        instance_id,
    )
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TIMER-008); the file and the assay use it.
fn wf_timer_008__timer_against_completed_token_no_ops() {
    // ── SCENARIO 1: the control — an ACTIVE token resumes, so the resume path is proven live ────────────────────
    // Everything in scenario 2 is a "did not happen" assertion, and a "did not happen" assertion is worthless unless
    // the thing that should have happened demonstrably does. This is that demonstration.
    let control = EngineHarness::at_unix_millis(NOW);
    let (job, token_id, instance_id) = leased_timer(&control);
    let job_id = job.id.clone();
    let active = read_token(&control, &token_id);
    assert_eq!(
        active.status,
        TokenStatus::Active,
        "{HARNESS}: the control token is active when the timer fires"
    );
    assert_eq!(
        active.node_id, WAIT_NODE,
        "{HARNESS}: the control token is parked on the timer node"
    );
    let version_before_fire = active.version;

    control
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: job_id.clone(),
            worker_id: WORKER.to_string(),
            variables: Value::object(),
        })
        .expect("the control timer fires and resumes its token");

    let resumed = read_token(&control, &token_id);
    assert_eq!(
        resumed.node_id, DONE_NODE,
        "{HARNESS}: the control fire DID resume the token — the resume path this contract guards is real"
    );
    assert_eq!(
        resumed.status,
        TokenStatus::Completed,
        "{HARNESS}: the resumed token completed at the end node"
    );
    assert!(
        resumed.version > version_before_fire,
        "{HARNESS}: the control fire consumed at least one version"
    );
    assert_eq!(
        read_job(&control, &job_id).status,
        JobStatus::Completed,
        "{HARNESS}: the control timer settled its job"
    );
    assert_eq!(
        events_of_type(&control, &instance_id, "token.moved").len(),
        2,
        "{HARNESS}: the control run moved the token twice — onto the timer, then the resume"
    );
    assert_eq!(
        control
            .store()
            .with_tx(|tx| tx.get_instance(&instance_id))
            .expect("the control instance reads")
            .status,
        ProcessStatus::Completed,
        "{HARNESS}: the control process ran to completion"
    );

    // ── SCENARIO 2 (load-bearing): a COMPLETED token makes the same fire a no-op on the process ───────────────
    // The lease is already held (scenario 1's precondition), and now the token settles inside that window. This is
    // the divergence the `token.status == Active` guard (`fire_timer_job.rs:55`) exists to handle.
    let harness = EngineHarness::at_unix_millis(NOW);
    let (job, token_id, instance_id) = leased_timer(&harness);
    let job_id = job.id.clone();

    // Settle the token through the production store call, while the job's lease is live and due.
    harness
        .store()
        .with_tx(|tx| tx.complete_token(&token_id, TokenOutcome::Completed, NOW))
        .expect("the token settles through the production store method");
    let settled = read_token(&harness, &token_id);
    assert_eq!(
        settled.status,
        TokenStatus::Completed,
        "{HARNESS}: the precondition — the timer is now pointed at a settled token"
    );
    assert_eq!(
        settled.node_id, WAIT_NODE,
        "{HARNESS}: settling a token does not move it, so the timer still points at the timer node"
    );
    let version_before_fire = settled.version;
    let moves_before_fire = events_of_type(&harness, &instance_id, "token.moved").len();

    // The fire itself succeeds — the timer came due and was consumed.
    harness
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: job_id.clone(),
            worker_id: WORKER.to_string(),
            variables: Value::object(),
        })
        .expect("firing a timer whose token already settled is not an error — the timer is simply consumed");

    // …and it did nothing to the process.
    let after = read_token(&harness, &token_id);
    assert_eq!(
        after.node_id, WAIT_NODE,
        "{HARNESS}: a fire against a completed token must not move the token"
    );
    assert_eq!(
        after.version, version_before_fire,
        "{HARNESS}: a no-op fire consumes no version — the token's CAS was never spent"
    );
    assert_eq!(
        after.status,
        TokenStatus::Completed,
        "{HARNESS}: a fire against a completed token must not reactivate it"
    );
    assert_eq!(
        after.outcome,
        Some(TokenOutcome::Completed),
        "{HARNESS}: the token's recorded outcome is untouched — the fire did not rewrite its history"
    );
    assert_eq!(
        after.ended_at, settled.ended_at,
        "{HARNESS}: the token's end instant is untouched"
    );

    // The no-op is visible in the durable history, not merely in the current state: no resume was recorded.
    assert_eq!(
        events_of_type(&harness, &instance_id, "token.moved").len(),
        moves_before_fire,
        "{HARNESS}: a fire against a completed token records no token.moved at all"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "token.moved")
            .iter()
            .filter(|event| event.data.get("transition").and_then(Value::as_str) == Some(RESUME))
            .count(),
        0,
        "{HARNESS}: the resume transition was never taken — the graph did not run again"
    );
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance_id))
            .expect("the instance reads")
            .status,
        ProcessStatus::Active,
        "{HARNESS}: the process did not advance to completed on a no-op fire"
    );

    // The timer is still consumed: the job settled and released its lease. "No-op" is about the *process*, not a
    // promise that the job goes back on the queue to be fired again.
    let settled_job = read_job(&harness, &job_id);
    assert_eq!(
        settled_job.status,
        JobStatus::Completed,
        "{HARNESS}: the timer settles its own job even when the resume is a no-op"
    );
    assert_eq!(
        settled_job.locked_by, None,
        "{HARNESS}: the consumed timer releases the lease"
    );
    assert_eq!(
        settled_job.completed_at,
        Some(NOW),
        "{HARNESS}: the consumed timer is stamped with the engine's clock"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "timer.fired").len(),
        1,
        "{HARNESS}: the consumed timer records its one firing"
    );

    // ── SCENARIO 3 (negative): the no-op is final — the consumed timer cannot be fired again ────────────────────
    // A retry loop that treated the no-op as "not done" would re-fire until something moved. The completed status
    // refuses it, so the no-op cannot become a repeated attempt.
    let replay = harness
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: job_id.clone(),
            worker_id: WORKER.to_string(),
            variables: Value::object(),
        })
        .expect_err("a consumed timer must not be fired again, no-op or not");
    assert_eq!(
        replay.code(),
        "TIMER_ALREADY_FIRED",
        "{HARNESS}: the replay is refused by the completed status"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "token.moved").len(),
        moves_before_fire,
        "{HARNESS}: the refused replay moved nothing either"
    );
    assert!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER, "timer", 10)
            .expect("the post-no-op claim step commits")
            .is_empty(),
        "{HARNESS}: the consumed timer is not re-offered to the executor"
    );
    assert_eq!(
        read_token(&harness, &token_id).version,
        version_before_fire,
        "{HARNESS}: across the no-op and the refused replay the token's version never moved"
    );
}
