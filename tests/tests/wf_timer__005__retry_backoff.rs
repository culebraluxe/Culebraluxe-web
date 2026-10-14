//! WF.TIMER — retry/backoff (TST-WF-TIMER-005).
//!
//! Contract: a failed attempt is retried, and the retry is **delayed by an exponentially growing backoff** rather than
//! being immediately re-offered. `WorkflowEngine::fail_job` decides between the two legal outcomes with one
//! predicate — `should_retry = !permanent && job.attempts < job.max_attempts`
//! (`middle/workflow/src/engine/fire_timer_job.rs:209`) — and when it retries it pushes the deadline out by
//! `now + JOB_BACKOFF_BASE_MS * 2^attempts` (`:220-221`, base `JOB_BACKOFF_BASE_MS` = 60s,
//! `middle/workflow/src/types.rs:3`). The attempt count comes from the claim (`memory.rs:468`), so the delay grows
//! with each round: the first failure waits two base units, the next four, then eight.
//!
//! The delay is the whole point of the contract, so it is asserted as behaviour rather than as a field. A `fail_job`
//! that merely flipped the status to `Pending` and left `due_at` alone would look correct in every state assertion
//! and would re-offer the job immediately — a hot retry loop against whatever is failing. The load-bearing assertion
//! is therefore that the retried job is **not claimable** until its backoff instant arrives, checked through the
//! production claim path (`claim_due_jobs_by_type`, which filters `due_at <= now`,
//! `middle/workflow/src/memory.rs:517-522`) and through production `list_overdue_jobs`.
//!
//! The other legal outcome is asserted too: `permanent = true` is not a retry at all and must reach `Failed`
//! immediately no matter how many attempts remain — the backoff is a delay, never an excuse to retry a job that was
//! declared permanent.
//!
//! Level: L4 Adversarial, harness `DeterministicClockHarness`. The backoff is measured by moving the frozen clock to
//! exact instants, so the doubling is a fixture rather than a wait. Deterministic and isolated: no database, no
//! network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_timer__005__retry_backoff

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    DefinitionStatus, Job, JobStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, StartProcessParams, TimerSpec, Token, TransitionDefinition,
    Value, JOB_BACKOFF_BASE_MS,
};

/// The canonical harness label for this level.
const HARNESS: &str = "DeterministicClockHarness/L4 Adversarial";

/// The definition key the engine registers.
const DEFINITION_KEY: &str = "TST-WF-TIMER-005";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// The graph: `start -> wait (timer) -> done (end)`.
const START_NODE: &str = "start";
const WAIT_NODE: &str = "wait";
const DONE_NODE: &str = "done";
const BEGIN: &str = "begin";
const RESUME: &str = "resume";

/// The frozen instant the first claim happens at.
const NOW: i64 = 1_700_000_000_000;

/// The worker that claims and fails.
const WORKER: &str = "worker-a";
/// A worker that never held the lease; used for the ownership negative.
const OTHER_WORKER: &str = "worker-b";

/// The failure the executor reports.
const FAILURE: &str = "executor unavailable";

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

/// Start the timer graph and claim its one due timer job, returning the claimed job and the instance it belongs to.
fn claimed_timer(harness: &EngineHarness) -> (Job, String) {
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
        "{HARNESS}: the due timer is claimable by the first worker"
    );
    (
        claimed.into_iter().next().expect("the claimed job"),
        instance_id,
    )
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TIMER-005); the file and the assay use it.
fn wf_timer_005__retry_backoff() {
    // ── SCENARIO 1: the first failure retries, releases the lease, and records why ──────────────────────────────
    let harness = EngineHarness::at_unix_millis(NOW);
    let (job, instance_id) = claimed_timer(&harness);
    let job_id = job.id.clone();
    assert_eq!(
        job.attempts, 1,
        "{HARNESS}: the claim counted the first attempt before anything could fail"
    );
    assert_eq!(
        job.due_at, NOW,
        "{HARNESS}: the timer was due at the claim instant, so any later deadline is the backoff"
    );

    harness
        .engine()
        .fail_job(&job_id, WORKER, FAILURE, false)
        .expect("the failure step commits");

    let retried = read_job(&harness, &job_id);
    assert_eq!(
        retried.status,
        JobStatus::Pending,
        "{HARNESS}: a retryable failure returns the job to the claimable state rather than failing it"
    );
    assert_eq!(
        retried.last_error.as_deref(),
        Some(FAILURE),
        "{HARNESS}: the failure reason is recorded, so a terminal decision can explain itself"
    );
    assert_eq!(
        retried.locked_by, None,
        "{HARNESS}: a retry releases the lease"
    );
    assert_eq!(
        retried.locked_until, None,
        "{HARNESS}: a retry releases the lease window"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.retry_scheduled").len(),
        1,
        "{HARNESS}: a retry records job.retry_scheduled exactly once"
    );
    assert!(
        events_of_type(&harness, &instance_id, "job.failed").is_empty(),
        "{HARNESS}: a retried attempt is not a terminal failure and must not record job.failed"
    );

    // ── SCENARIO 2 (negative, load-bearing): the retry is NOT offered before its backoff elapses ───────────────
    // One attempt is left of the default max_attempts=5 (`handle_join.rs:158`), so the attempt cap is not what hides
    // this job — only the deadline is. A `fail_job` that forgot to push `due_at` fails here.
    assert_eq!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER, "timer", 10)
            .expect("the immediate re-claim step commits")
            .len(),
        0,
        "{HARNESS}: a retried timer must not be re-offered at the instant it failed"
    );
    assert!(
        harness
            .engine()
            .list_overdue_jobs(10)
            .expect("the overdue listing commits")
            .is_empty(),
        "{HARNESS}: a backed-off timer is not overdue — it is waiting"
    );

    // ── SCENARIO 3 (positive): the backoff is exactly base * 2^attempts, and the deadline is reachable ───────────
    let first_delay = JOB_BACKOFF_BASE_MS * 2i64.pow(1);
    assert_eq!(
        retried.due_at,
        NOW + first_delay,
        "{HARNESS}: the first backoff is now + JOB_BACKOFF_BASE_MS * 2^1"
    );

    // One millisecond early is still too early.
    harness.clock().advance_millis(first_delay - 1);
    assert_eq!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER, "timer", 10)
            .expect("the one-millisecond-early claim commits")
            .len(),
        0,
        "{HARNESS}: the retry is not claimable one millisecond before its backoff elapses"
    );

    // At the deadline the claim path admits it (`due_at <= now`).
    harness.clock().advance_millis(1);
    let second_claim = harness
        .engine()
        .claim_jobs_by_type(WORKER, "timer", 10)
        .expect("the on-time claim step commits");
    assert_eq!(
        second_claim.len(),
        1,
        "{HARNESS}: the retry is claimable the instant its backoff elapses"
    );
    let second = second_claim.into_iter().next().expect("the re-claimed job");
    assert_eq!(
        second.attempts, 2,
        "{HARNESS}: the retry is a second attempt"
    );
    let second_started_at = harness.now_millis();

    // ── SCENARIO 4: the backoff grows — each failure waits twice as long as the last ───────────────────────────
    harness
        .engine()
        .fail_job(&job_id, WORKER, FAILURE, false)
        .expect("the second failure step commits");
    let second_delay = JOB_BACKOFF_BASE_MS * 2i64.pow(2);
    let after_second = read_job(&harness, &job_id);
    assert_eq!(
        after_second.due_at,
        second_started_at + second_delay,
        "{HARNESS}: the second backoff is now + JOB_BACKOFF_BASE_MS * 2^2, doubled from the first"
    );
    assert_eq!(
        second_delay,
        first_delay * 2,
        "{HARNESS}: the backoff itself doubled between rounds"
    );
    assert_eq!(
        after_second.status,
        JobStatus::Pending,
        "{HARNESS}: the second failure is still retryable"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.retry_scheduled").len(),
        2,
        "{HARNESS}: one retry event per retryable failure, and no more"
    );

    // Third round, to show the growth is not an artefact of exactly two rounds.
    harness.clock().advance_millis(second_delay);
    let third_claim = harness
        .engine()
        .claim_jobs_by_type(WORKER, "timer", 10)
        .expect("the third claim step commits");
    assert_eq!(
        third_claim.len(),
        1,
        "{HARNESS}: the second retry became claimable on its own deadline"
    );
    let third_started_at = harness.now_millis();
    harness
        .engine()
        .fail_job(&job_id, WORKER, FAILURE, false)
        .expect("the third failure step commits");
    let third_delay = JOB_BACKOFF_BASE_MS * 2i64.pow(3);
    assert_eq!(
        read_job(&harness, &job_id).due_at,
        third_started_at + third_delay,
        "{HARNESS}: the third backoff is JOB_BACKOFF_BASE_MS * 2^3, doubling again"
    );
    assert_eq!(
        third_delay,
        second_delay * 2,
        "{HARNESS}: the backoff doubles every round, so the delay is bounded away from zero as failures accumulate"
    );

    // ── SCENARIO 5 (negative): only the lease holder may report a failure ──────────────────────────────────────
    // Without this check, a second worker's stray failure would release a lease it never held and reset the
    // schedule of work already in flight. `fail_job` checks ownership first (`fire_timer_job.rs:202-208`).
    harness.clock().advance_millis(third_delay);
    harness
        .engine()
        .claim_jobs_by_type(WORKER, "timer", 10)
        .expect("the fourth claim step commits");
    let before_theft = read_job(&harness, &job_id);
    let theft = harness
        .engine()
        .fail_job(&job_id, OTHER_WORKER, FAILURE, false)
        .expect_err("a worker that does not hold the lease must not report its failure");
    assert_eq!(
        theft.code(),
        "ERROR",
        "{HARNESS}: reporting a failure on a lease you do not hold is refused"
    );
    let after_theft = read_job(&harness, &job_id);
    assert_eq!(
        after_theft.status, before_theft.status,
        "{HARNESS}: the refused report left the status alone"
    );
    assert_eq!(
        after_theft.locked_by.as_deref(),
        Some(WORKER),
        "{HARNESS}: the refused report left the lease with its real owner"
    );
    assert_eq!(
        after_theft.due_at, before_theft.due_at,
        "{HARNESS}: the refused report did not reschedule somebody else's work"
    );

    // ── SCENARIO 6 (negative): a permanent failure is terminal, with no backoff at all ──────────────────────────
    // A permanent failure is not "retry later": it must reach `Failed` immediately even though four of the five
    // attempts are unused. This is the assertion that keeps the backoff from becoming an excuse to retry forever.
    let harness = EngineHarness::at_unix_millis(NOW);
    let (job, instance_id) = claimed_timer(&harness);
    let job_id = job.id.clone();
    assert!(
        job.attempts < job.max_attempts,
        "{HARNESS}: attempts remain, so the terminal outcome below is the permanent flag's doing"
    );

    harness
        .engine()
        .fail_job(
            &job_id,
            WORKER,
            "executor rejected the payload permanently",
            true,
        )
        .expect("the permanent failure step commits");

    let terminal = read_job(&harness, &job_id);
    assert_eq!(
        terminal.status,
        JobStatus::Failed,
        "{HARNESS}: a permanent failure is terminal, not deferred"
    );
    assert_eq!(
        terminal.attempts, job.attempts,
        "{HARNESS}: going terminal consumed no further attempt"
    );
    assert_eq!(
        terminal.locked_by, None,
        "{HARNESS}: a terminal failure releases the lease"
    );
    assert_eq!(
        terminal.last_error.as_deref(),
        Some("executor rejected the payload permanently"),
        "{HARNESS}: the permanent reason is recorded for the terminal state"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.failed").len(),
        1,
        "{HARNESS}: a permanent failure records job.failed"
    );
    assert!(
        events_of_type(&harness, &instance_id, "job.retry_scheduled").is_empty(),
        "{HARNESS}: a permanent failure schedules no retry"
    );

    // No backoff means no later claim: the job is out of the executor's hands for good.
    assert!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER, "timer", 10)
            .expect("the post-terminal claim step commits")
            .is_empty(),
        "{HARNESS}: a terminally failed timer is never re-offered, backoff or not"
    );
    assert!(
        harness
            .engine()
            .claim_job(&job_id, WORKER)
            .expect("the post-terminal exact claim commits")
            .is_none(),
        "{HARNESS}: nor by exact claim"
    );
}
