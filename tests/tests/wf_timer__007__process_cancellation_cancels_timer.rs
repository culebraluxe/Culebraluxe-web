//! WF.TIMER — process cancellation cancels timer (TST-WF-TIMER-007).
//!
//! Contract: cancelling a process cancels the timers it was waiting on. A timer is not an independent job — it is the
//! only thing holding its token at a `timer` node, so a timer that outlived its process would be a live job pointed
//! at a process that has already given up. Production enforces this in `terminate_process`, the single path
//! `cancel_process` delegates to (`middle/workflow/src/engine/engine_options.rs:465-488`): it settles the instance as
//! `Aborted`/`Cancelled`, cancels every active token (`:223-238`), obsoletes the open tasks (`:240-242`), and then
//! cancels **every open job of the instance** (`:244-261`) — recording a `job.cancelled` event with the reason for
//! each.
//!
//! "Every open job" is doing real work in that sentence. `open_jobs_for_instance` returns jobs in `Pending | Locked`
//! (`middle/workflow/src/memory.rs:579-590`), so a timer that has *already been claimed* by a worker is still open and
//! is still cancelled. That is the adversarial half of this contract: the interesting case is not the idle timer
//! nobody has touched, it is the timer a worker is mid-flight on. A cancellation that only swept `Pending` would leave
//! a leased timer to fire into a dead process.
//!
//! Cancellation is also a **terminal** state, which is what makes the negative assertions meaningful rather than
//! cosmetic: a cancelled timer is `is_settled` (`middle/workflow/src/types.rs:65-67`), so it cannot be claimed again,
//! cannot be fired, and cannot be rescheduled back into life. And cancelling twice must be a no-op rather than an
//! error — an idempotent cancellation is what lets a caller retry a request that lost its response.
//!
//! Level: L4 Adversarial, harness `DeterministicClockHarness`. The contention modelled here is cancellation arriving
//! while a worker holds the timer. Deterministic and isolated: no database, no network, no filesystem write, no live
//! provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_timer__007__process_cancellation_cancels_timer

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    CancelProcessParams, DefinitionStatus, FireTimerParams, Job, JobStatus, NodeDefinition,
    ProcessDefinition, ProcessEvent, ProcessGraph, ProcessOutcome, ProcessStatus,
    StartProcessParams, TimerSpec, Token, TokenOutcome, TokenStatus, TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "DeterministicClockHarness/L4 Adversarial";

/// The definition key the engine registers.
const DEFINITION_KEY: &str = "TST-WF-TIMER-007";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const ACTOR: &str = "operator";

/// The graph: `start -> wait (timer) -> done (end)`.
const START_NODE: &str = "start";
const WAIT_NODE: &str = "wait";
const DONE_NODE: &str = "done";
const BEGIN: &str = "begin";
const RESUME: &str = "resume";

/// The frozen instant the graph runs at.
const NOW: i64 = 1_700_000_000_000;

/// The worker that claims the timer in the leased scenario.
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

/// Start the timer graph and return the scheduled job, its token and its instance.
fn scheduled_timer(harness: &EngineHarness) -> (Job, String, String) {
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
    (
        jobs.into_iter().next().expect("the scheduled job"),
        token_id,
        instance_id,
    )
}

fn cancel(harness: &EngineHarness, instance_id: &str) {
    harness
        .engine()
        .cancel_process(CancelProcessParams {
            process_instance_id: instance_id.to_string(),
            actor: ACTOR.to_string(),
            reason: Some("the Captain withdrew the request".to_string()),
        })
        .expect("the cancellation commits");
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TIMER-007); the file and the assay use it.
fn wf_timer_007__process_cancellation_cancels_timer() {
    // ── SCENARIO 1: cancelling the process cancels an idle timer ───────────────────────────────────────────────
    // The base case, and the one a sweep of `Pending` alone would satisfy. It is asserted anyway, because "every open
    // job" is a claim about the whole set and the idle member is part of it.
    let harness = EngineHarness::at_unix_millis(NOW);
    let (job, token_id, instance_id) = scheduled_timer(&harness);
    let job_id = job.id.clone();
    assert_eq!(
        read_job(&harness, &job_id).status,
        JobStatus::Pending,
        "{HARNESS}: the precondition — a timer nobody has claimed yet"
    );
    assert_eq!(
        read_token(&harness, &token_id).node_id,
        WAIT_NODE,
        "{HARNESS}: the token is parked on the timer, so the timer is the only thing holding the process"
    );

    cancel(&harness, &instance_id);

    let cancelled = read_job(&harness, &job_id);
    assert_eq!(
        cancelled.status,
        JobStatus::Cancelled,
        "{HARNESS}: cancelling the process must cancel the timer it was waiting on"
    );
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance_id))
            .expect("the instance reads")
            .status,
        ProcessStatus::Aborted,
        "{HARNESS}: the process itself is aborted, not completed"
    );
    assert_eq!(
        read_token(&harness, &token_id).status,
        TokenStatus::Completed,
        "{HARNESS}: the token is settled rather than left active on a timer node"
    );
    assert_eq!(
        read_token(&harness, &token_id).outcome,
        Some(TokenOutcome::Cancelled),
        "{HARNESS}: the token's outcome records that it was cancelled, not that it completed"
    );
    let cancellations = events_of_type(&harness, &instance_id, "job.cancelled");
    assert_eq!(
        cancellations.len(),
        1,
        "{HARNESS}: the cancellation records one job.cancelled for the one timer"
    );
    assert_eq!(
        cancellations[0].job_id.as_deref(),
        Some(job_id.as_str()),
        "{HARNESS}: the event names the job it cancelled, so the history is auditable"
    );

    // ── SCENARIO 2 (load-bearing): a LEASED timer is cancelled too ────────────────────────────────────────────
    // This is the adversarial case. A worker holds the timer — `Locked`, with an owner and a lease window — and the
    // process is cancelled underneath it. `open_jobs_for_instance` counts `Locked` as open
    // (`middle/workflow/src/memory.rs:586`), so the sweep must reach it. A cancellation that only swept `Pending`
    // would leave this timer live, and it would fire into a dead process.
    let harness = EngineHarness::at_unix_millis(NOW);
    let (job, token_id, instance_id) = scheduled_timer(&harness);
    let job_id = job.id.clone();
    let claimed = harness
        .engine()
        .claim_jobs_by_type(WORKER, "timer", 10)
        .expect("the claim step commits");
    assert_eq!(
        claimed.len(),
        1,
        "{HARNESS}: the precondition — a worker is mid-flight on this timer"
    );
    let leased = read_job(&harness, &job_id);
    assert_eq!(
        leased.status,
        JobStatus::Locked,
        "{HARNESS}: the precondition — the timer is leased, not idle"
    );
    assert_eq!(
        leased.locked_by.as_deref(),
        Some(WORKER),
        "{HARNESS}: the precondition — the lease has an owner to outlive"
    );
    assert!(
        leased.locked_until.is_some(),
        "{HARNESS}: the precondition — the lease has a live window, so this is not an expired-lease case"
    );

    cancel(&harness, &instance_id);

    let swept = read_job(&harness, &job_id);
    assert_eq!(
        swept.status,
        JobStatus::Cancelled,
        "{HARNESS}: a leased timer must be cancelled with its process — an in-flight timer is still open"
    );
    assert_eq!(
        swept.locked_by, None,
        "{HARNESS}: cancelling a leased timer releases the worker's lease, so it cannot act on it afterwards"
    );
    assert_eq!(
        swept.locked_until, None,
        "{HARNESS}: cancelling a leased timer clears its lease window"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.cancelled").len(),
        1,
        "{HARNESS}: the leased timer is recorded as cancelled exactly once"
    );

    // ── SCENARIO 3 (negative): the cancelled timer cannot be claimed, fired, or rescheduled ──────────────────────
    // Cancellation has to be terminal, not a pause. Each of these three would be a way for the timer to come back to
    // life, and each is refused.
    assert!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER, "timer", 10)
            .expect("the post-cancel claim step commits")
            .is_empty(),
        "{HARNESS}: a cancelled timer is not claimable, by its former worker or any other"
    );
    assert!(
        harness
            .engine()
            .list_overdue_jobs(10)
            .expect("the post-cancel overdue listing commits")
            .is_empty(),
        "{HARNESS}: a cancelled timer is not overdue either — it is finished"
    );

    // Firing is refused on status, before any resume is attempted. A fire that got this far would move a token on a
    // cancelled process.
    let firing = harness
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: job_id.clone(),
            worker_id: WORKER.to_string(),
            variables: Value::object(),
        })
        .expect_err("a cancelled timer must not fire");
    assert_eq!(
        firing.code(),
        "TIMER_NOT_LOCKED",
        "{HARNESS}: firing a cancelled timer is refused — it holds no lease to fire under"
    );
    assert_eq!(
        read_token(&harness, &token_id).node_id,
        WAIT_NODE,
        "{HARNESS}: the refused fire did not resume the token"
    );
    assert!(
        events_of_type(&harness, &instance_id, "timer.fired").is_empty(),
        "{HARNESS}: no timer.fired may be recorded for a cancelled timer"
    );

    let reviving = harness
        .engine()
        .reschedule_timer(&job_id, NOW + 60_000, ACTOR)
        .expect_err("a cancelled timer must not be rescheduled back into life");
    assert_eq!(
        reviving.code(),
        "JOB_SETTLED",
        "{HARNESS}: a cancelled timer is settled, so rescheduling it is refused"
    );
    assert_eq!(
        read_job(&harness, &job_id).status,
        JobStatus::Cancelled,
        "{HARNESS}: the refused reschedule left it cancelled"
    );

    // ── SCENARIO 4 (negative): cancelling again is a no-op, not an error ───────────────────────────────────────
    // Idempotence is what lets a caller retry a cancellation whose response it lost. If the second call errored, the
    // retry would surface a failure for work that is already done; if it did anything, it would rewrite history.
    let instance_before = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance reads before the repeat");
    cancel(&harness, &instance_id);
    let instance_after = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance reads after the repeat");
    assert_eq!(
        instance_after.status, instance_before.status,
        "{HARNESS}: a repeated cancellation must not change the instance's status"
    );
    assert_eq!(
        instance_after.ended_at, instance_before.ended_at,
        "{HARNESS}: a repeated cancellation must not move the instant the process ended"
    );
    assert_eq!(
        read_job(&harness, &job_id).status,
        JobStatus::Cancelled,
        "{HARNESS}: a repeated cancellation left the timer cancelled"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.cancelled").len(),
        1,
        "{HARNESS}: a repeated cancellation records no second job.cancelled"
    );
}
