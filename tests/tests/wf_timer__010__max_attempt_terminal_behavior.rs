//! WF.TIMER — max-attempt terminal behavior (TST-WF-TIMER-010).
//!
//! Contract: attempts are bounded, and hitting the bound is **terminal**. `fail_job` picks between the two legal
//! outcomes with `should_retry = !permanent && job.attempts < job.max_attempts`
//! (`middle/workflow/src/engine/fire_timer_job.rs:209`): while the bound holds, a failure returns the job to
//! `Pending` with a backoff (`:211-222`), and at the bound it settles as `Failed` instead, recording `job.failed`
//! rather than `job.retry_scheduled` (`:224-242`). The attempt itself is counted by the claim
//! (`middle/workflow/src/memory.rs:468`), so "one failure per attempt" holds exactly and the last attempt is spent,
//! not discarded.
//!
//! Terminal has to mean terminal at *every* door, not just the one that produced it. That is the load-bearing part of
//! this contract, and each door is a separate way a bounded job could come back:
//!
//! - The **claim paths** refuse it. `claim_job` refuses when `attempts >= max_attempts` (`memory.rs:460`) and the batch
//!   claims refuse when `attempts < max_attempts` is false (`:486`, `:521`). Without those guards a `Failed` job would
//!   be handed out again on the next sweep and fail forever, which is how a poison job becomes an outage.
//! - The **reclaim path** also converges rather than reopens: a job whose lease expires with its attempts already at
//!   the bound is marked `Failed` with `last_error = "job lease expired after max attempts"`, not returned to
//!   `Pending` (`memory.rs:565-567`). A worker that dies mid-attempt therefore terminates its job rather than
//!   resurrecting it.
//! - Only an **explicit operator action** revives it. `requeue_job` (`fire_timer_job.rs:574-619`) is the one path that
//!   moves `Failed` back to `Pending`, and it does so deliberately: attempts reset to zero, the error is cleared, and
//!   `job.requeued` is recorded. It also refuses anything that is not `Failed` (`JOB_NOT_REQUEUEABLE`), so it cannot
//!   be used to duplicate work that is still in flight.
//!
//! Everything is driven through the production `WorkflowEngine<MemoryStore>`: jobs are created by production
//! `create_job`, claimed by production `claim_job`, failed by production `fail_job`, swept by production
//! `reclaim_stale_jobs`, and revived by production `requeue_job`. The `max_attempts` under test is the value the
//! caller passed to `create_job`, not a re-declared constant.
//!
//! Level: L4 Adversarial, harness `DeterministicClockHarness`. The bound is reached by advancing a frozen clock to the
//! exact backoff deadline each round, and the reclaim branch is reached by advancing past a real lease. Deterministic
//! and isolated: no database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_timer__010__max_attempt_terminal_behavior

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    DefinitionStatus, Job, JobStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, StartProcessParams, TimerSpec, TransitionDefinition, Value,
    JOB_LEASE_MS,
};

/// The canonical harness label for this level.
const HARNESS: &str = "DeterministicClockHarness/L4 Adversarial";

/// The definition key the engine registers.
const DEFINITION_KEY: &str = "TST-WF-TIMER-010";
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

/// The bound under test. Small on purpose: the contract is about reaching the bound exactly, and a bound of three
/// reaches it in three rounds instead of five.
const MAX_ATTEMPTS: i32 = 3;

/// The worker that claims and fails.
const WORKER: &str = "worker-a";
/// A second worker, used to prove a terminal job is invisible to every executor, not just its former owner.
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

/// Start the timer graph and return the instance and its parked token.
fn started_timer(harness: &EngineHarness) -> (String, String) {
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
    (instance_id, token_id)
}

/// Create a timer job for this instance whose attempt ceiling is `max_attempts`, due at the engine's clock.
fn bounded_job(
    harness: &EngineHarness,
    instance_id: &str,
    token_id: &str,
    max_attempts: i32,
) -> String {
    harness
        .engine()
        .create_job(
            Some(instance_id),
            Some(token_id),
            None,
            "timer",
            NOW,
            Value::object(),
            Some(max_attempts),
        )
        .expect("the bounded job is created")
}

/// Claim a job by its exact id and report the attempt that claim consumed.
fn claim(harness: &EngineHarness, job_id: &str, worker: &str) -> Job {
    harness
        .engine()
        .claim_job(job_id, worker)
        .expect("the exact claim step commits")
        .unwrap_or_else(|| panic!("{HARNESS}: {job_id} was expected to be claimable"))
}

/// Advance the clock onto a job's own deadline so its backoff has elapsed.
fn advance_to(harness: &EngineHarness, job_id: &str) {
    let due_at = read_job(harness, job_id).due_at;
    assert!(
        due_at > harness.now_millis(),
        "{HARNESS}: {job_id} must carry a future deadline to advance onto it"
    );
    harness
        .clock()
        .advance_millis(due_at - harness.now_millis());
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TIMER-010); the file and the assay use it.
fn wf_timer_010__max_attempt_terminal_behavior() {
    // ── SCENARIO 1: attempts below the bound retry, and the bound is what stops them ───────────────────────────
    // One job with `max_attempts = 3`, driven to the bound by claiming and failing it exactly three times. Every
    // claim is the exact claim, so nothing else in the engine's work queue can influence the accounting.
    let harness = EngineHarness::at_unix_millis(NOW);
    let (instance_id, token_id) = started_timer(&harness);
    let job_id = bounded_job(&harness, &instance_id, &token_id, MAX_ATTEMPTS);

    let seeded = read_job(&harness, &job_id);
    assert_eq!(
        seeded.max_attempts, MAX_ATTEMPTS,
        "{HARNESS}: the ceiling under test is the one the caller asked for"
    );
    assert_eq!(
        seeded.attempts, 0,
        "{HARNESS}: a fresh job has spent no attempts"
    );
    assert_eq!(
        seeded.status,
        JobStatus::Pending,
        "{HARNESS}: a fresh job is pending"
    );

    // Round 1 and 2: below the bound, a failure retries.
    for round in 1..MAX_ATTEMPTS {
        let attempt = claim(&harness, &job_id, WORKER);
        assert_eq!(
            attempt.attempts, round,
            "{HARNESS}: attempt {round} of the bound is spent by its claim"
        );
        harness
            .engine()
            .fail_job(&job_id, WORKER, FAILURE, false)
            .expect("the failure step commits");
        let after = read_job(&harness, &job_id);
        assert_eq!(
            after.status,
            JobStatus::Pending,
            "{HARNESS}: attempt {round} is below the bound of {MAX_ATTEMPTS}, so the job is retried"
        );
        assert_eq!(
            after.last_error.as_deref(),
            Some(FAILURE),
            "{HARNESS}: the retried round still records why it failed"
        );
        assert_eq!(
            events_of_type(&harness, &instance_id, "job.retry_scheduled").len(),
            round as usize,
            "{HARNESS}: one retry event per retried round"
        );
        advance_to(&harness, &job_id);
    }

    // ── SCENARIO 2: the round that reaches the bound is terminal, not a third retry ────────────────────────────
    // This is the exact instant the contract turns on: `attempts` is now equal to `max_attempts`, and
    // `should_retry` is `attempts < max_attempts`, which is false. One more retry here would be a poison-job loop.
    let final_attempt = claim(&harness, &job_id, WORKER);
    assert_eq!(
        final_attempt.attempts, MAX_ATTEMPTS,
        "{HARNESS}: the last attempt is spent exactly on the bound, not discarded"
    );
    harness
        .engine()
        .fail_job(&job_id, WORKER, FAILURE, false)
        .expect("the final failure step commits");

    let terminal = read_job(&harness, &job_id);
    assert_eq!(
        terminal.status,
        JobStatus::Failed,
        "{HARNESS}: a job that has spent its whole attempt budget must end Failed"
    );
    assert_eq!(
        terminal.attempts, MAX_ATTEMPTS,
        "{HARNESS}: the terminal state records the attempts actually spent — the ceiling, exactly"
    );
    assert_eq!(
        terminal.last_error.as_deref(),
        Some(FAILURE),
        "{HARNESS}: the terminal state carries the last failure, so the decision is explainable"
    );
    assert_eq!(
        terminal.locked_by, None,
        "{HARNESS}: going terminal releases the lease"
    );
    assert_eq!(
        terminal.locked_until, None,
        "{HARNESS}: going terminal clears the lease window"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.failed").len(),
        1,
        "{HARNESS}: reaching the bound records job.failed"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.retry_scheduled").len(),
        (MAX_ATTEMPTS - 1) as usize,
        "{HARNESS}: no retry was scheduled for the final round — the bound stopped it"
    );

    // ── SCENARIO 3 (negative, load-bearing): no claim path will hand out a terminal job ────────────────────────
    // This is the assertion that separates "bounded" from "attempts counted". If either claim guard were missing, the
    // job would be offered again on the very next sweep and would fail again, forever.
    assert!(
        harness
            .engine()
            .claim_job(&job_id, WORKER)
            .expect("the post-terminal exact claim commits")
            .is_none(),
        "{HARNESS}: a job at its attempt ceiling must not be claimable by exact claim"
    );
    assert!(
        harness
            .engine()
            .claim_job(&job_id, OTHER_WORKER)
            .expect("the post-terminal exact claim commits")
            .is_none(),
        "{HARNESS}: nor by a different worker — terminal is terminal, not owner-scoped"
    );
    let batch: Vec<Job> = harness
        .engine()
        .claim_jobs_by_type(OTHER_WORKER, "timer", 100)
        .expect("the post-terminal batch claim commits");
    assert!(
        batch.iter().all(|claimed| claimed.id != job_id),
        "{HARNESS}: the batch claim must skip a terminal job even though other timers are due"
    );
    assert!(
        read_job(&harness, &job_id).attempts == MAX_ATTEMPTS,
        "{HARNESS}: the refused claims cost the terminal job no further attempt"
    );

    // ── SCENARIO 4 (negative): reporting a failure for a terminal job changes nothing ─────────────────────────
    // A late worker reporting its own failure after the job already gave up must be a no-op, not a resurrection and
    // not an error: `fail_job` returns early for a settled job (`fire_timer_job.rs:203-207`).
    harness
        .engine()
        .fail_job(&job_id, WORKER, "a late failure report", false)
        .expect("a late failure report on a settled job is a no-op, not an error");
    assert_eq!(
        read_job(&harness, &job_id).status,
        JobStatus::Failed,
        "{HARNESS}: a late failure report must not revive a terminal job"
    );
    assert_eq!(
        read_job(&harness, &job_id).last_error.as_deref(),
        Some(FAILURE),
        "{HARNESS}: it must not overwrite the recorded reason either"
    );

    // ── SCENARIO 5 (negative): a reclaim sweep does not reopen a terminal job ─────────────────────────────────
    // The sweep only looks at `Locked` jobs, so a `Failed` one is invisible to it — asserting the count of zero is
    // what proves the sweep is not treating "failed" as "stale".
    assert_eq!(
        harness
            .engine()
            .reclaim_stale_jobs(100)
            .expect("the sweep after the terminal state commits"),
        0,
        "{HARNESS}: a Failed job is not stale — nothing about its lease may be swept back into the queue"
    );
    assert_eq!(
        read_job(&harness, &job_id).status,
        JobStatus::Failed,
        "{HARNESS}: the sweep left the terminal status alone"
    );

    // ── SCENARIO 6: the ceiling is enforced at CLAIM time, not only at fail time ─────────────────────────────────
    // Everything above reaches the bound by failing until it runs out. That leaves one door untested: a job that is
    // already `Pending` with its whole budget spent. Production rejects it in the claim predicate itself
    // (`memory.rs:460`, `attempts >= max_attempts`), not merely by settling it later. The state is reachable through
    // the production API — `create_job` accepts any ceiling, and `0` is the smallest honest statement of "this work
    // gets no attempts" — so this is a real job a real caller can create, not a fabricated row.
    //
    // It is the assertion that separates "bounded" from "counted": if the claim stopped consulting `max_attempts` and
    // relied only on the status, this job would be handed out on the very first sweep.
    let zero = EngineHarness::at_unix_millis(NOW);
    let (zero_instance_id, zero_token_id) = started_timer(&zero);
    let zero_id = bounded_job(&zero, &zero_instance_id, &zero_token_id, 0);
    let seeded_zero = read_job(&zero, &zero_id);
    assert_eq!(
        seeded_zero.max_attempts, 0,
        "{HARNESS}: the precondition — this job was created with no attempts allowed"
    );
    assert_eq!(
        seeded_zero.status,
        JobStatus::Pending,
        "{HARNESS}: it is pending, so only the ceiling can be holding it back"
    );
    assert!(
        zero.engine()
            .claim_job(&zero_id, WORKER)
            .expect("the zero-ceiling exact claim commits")
            .is_none(),
        "{HARNESS}: a job with no attempts left must not be claimable, however pending it is"
    );
    assert!(
        zero.engine()
            .claim_jobs_by_type(WORKER, "timer", 100)
            .expect("the zero-ceiling batch claim commits")
            .iter()
            .all(|claimed| claimed.id != zero_id),
        "{HARNESS}: nor by the batch claim, which is the path a scheduler sweep actually uses"
    );
    // The all-types batch claim is a third, separately-written predicate (`memory.rs:486`), so it is asserted
    // separately too. Three claim paths, one ceiling: a guard honoured on only two of them is still a bug waiting
    // for the caller that happens to use the third.
    assert!(
        zero.engine()
            .claim_jobs(WORKER, 100)
            .expect("the zero-ceiling all-types batch claim commits")
            .iter()
            .all(|claimed| claimed.id != zero_id),
        "{HARNESS}: nor by the all-types batch claim — the ceiling holds on every claim path"
    );
    assert_eq!(
        read_job(&zero, &zero_id).attempts,
        0,
        "{HARNESS}: the refused claims cost a zero-ceiling job nothing — it never starts"
    );

    // ── SCENARIO 7: an expired lease AT the bound terminates rather than reopens ─────────────────────────────
    // A different door to the same bound: a worker dies holding a lease that has already consumed the whole attempt
    // budget. Reclaim must settle it (`memory.rs:565-567`), because returning it to `Pending` would hand out an
    // attempt that no longer exists — and then to a job whose `attempts < max_attempts` guard would refuse it
    // anyway, leaving it wedged in `Pending` for ever.
    let dying = EngineHarness::at_unix_millis(NOW);
    let (dying_instance_id, dying_token_id) = started_timer(&dying);
    let dying_job_id = bounded_job(&dying, &dying_instance_id, &dying_token_id, 1);
    claim(&dying, &dying_job_id, WORKER);
    assert_eq!(
        read_job(&dying, &dying_job_id).attempts,
        1,
        "{HARNESS}: the dying worker spent the job's only attempt"
    );
    dying.clock().advance_millis(JOB_LEASE_MS + 1);

    assert_eq!(
        dying
            .engine()
            .reclaim_stale_jobs(100)
            .expect("the sweep over the expired lease commits"),
        1,
        "{HARNESS}: the abandoned lease is stale and must be swept"
    );
    let died = read_job(&dying, &dying_job_id);
    assert_eq!(
        died.status,
        JobStatus::Failed,
        "{HARNESS}: reclaim at the attempt ceiling terminates the job rather than requeueing it"
    );
    assert_eq!(
        died.last_error.as_deref(),
        Some("job lease expired after max attempts"),
        "{HARNESS}: the expiry names its own cause, which is how an operator tells a dead worker from a failing one"
    );
    assert!(
        dying
            .engine()
            .claim_job(&dying_job_id, WORKER)
            .expect("the post-sweep exact claim commits")
            .is_none(),
        "{HARNESS}: the terminated job is not claimable afterwards either"
    );

    // ── SCENARIO 8 (positive): only an explicit requeue revives a terminal job ───────────────────────────────
    // Terminal means the automatic paths stop, not that the job is stuck forever. `requeue_job` is the deliberate
    // operator action: it resets the attempt budget and makes the work claimable again.
    harness
        .engine()
        .requeue_job(&job_id, ACTOR)
        .expect("the operator requeues the terminal job");
    let requeued = read_job(&harness, &job_id);
    assert_eq!(
        requeued.status,
        JobStatus::Pending,
        "{HARNESS}: an explicit requeue returns the job to the claimable state"
    );
    assert_eq!(
        requeued.attempts, 0,
        "{HARNESS}: requeue resets the attempt budget — that is what makes the requeue meaningful"
    );
    assert_eq!(
        requeued.last_error, None,
        "{HARNESS}: requeue clears the failure that ended the previous budget"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.requeued").len(),
        1,
        "{HARNESS}: the revival is recorded, so the operator action is auditable"
    );
    let revived_attempt = claim(&harness, &job_id, WORKER);
    assert_eq!(
        revived_attempt.attempts, 1,
        "{HARNESS}: the revived job spends its new budget from the first attempt"
    );

    // ── SCENARIO 9 (negative): requeue refuses anything that is not terminal ─────────────────────────────────
    // Otherwise the operator action becomes a way to duplicate work that is still in flight: pull a live timer back
    // to `Pending` while a worker holds it, and two workers run it.
    let in_flight = read_job(&harness, &job_id);
    assert_eq!(
        in_flight.status,
        JobStatus::Locked,
        "{HARNESS}: the precondition — the job is now leased and in flight"
    );
    let duplicate = harness
        .engine()
        .requeue_job(&job_id, ACTOR)
        .expect_err("requeueing a job that is not Failed must be refused");
    assert_eq!(
        duplicate.code(),
        "JOB_NOT_REQUEUEABLE",
        "{HARNESS}: requeue applies only to a terminal job"
    );
    assert_eq!(
        read_job(&harness, &job_id).locked_by.as_deref(),
        Some(WORKER),
        "{HARNESS}: the refused requeue left the live worker holding the job"
    );
    assert_eq!(
        read_job(&harness, &job_id).attempts,
        1,
        "{HARNESS}: the refused requeue did not reset the attempt count of live work"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.requeued").len(),
        1,
        "{HARNESS}: the refused requeue recorded nothing"
    );
}
