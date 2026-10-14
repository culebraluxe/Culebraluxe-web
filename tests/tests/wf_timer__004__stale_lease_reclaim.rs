//! WF.TIMER — stale lease reclaim (TST-WF-TIMER-004).
//!
//! Contract: a timer lease is reclaimed **only once it has actually expired**. A lease is what makes a job
//! legitimately someone else's, so reclaiming a *live* one would let two workers hold the same work — but never
//! reclaiming an *expired* one would strand it forever when its worker died. The boundary production draws is
//! `reclaim_stale_jobs`'s predicate: `status == Locked && locked_until < now`
//! (`middle/workflow/src/memory.rs:551-558`). Note the strict `<`: at the instant the lease expires the job is still
//! the holder's, and only afterwards is it stale. The engine wrapper supplies the instant from its clock
//! (`middle/workflow/src/engine/fire_timer_job.rs:564-567`), which is why a frozen `TestClock` makes this exact
//! boundary an assertion instead of a race.
//!
//! Reclaim is a *convergence* contract, not just a filter. A reclaimed job must come back in exactly one legal
//! durable state — `Pending`, no owner, no lease window, attempt count intact (`memory.rs:568-574`) — so the work is
//! recoverable and recoverable exactly once. Two further production rules make that state meaningful and are
//! asserted here rather than assumed: a reclaim must not double-count (the job is no longer `Locked` afterwards, so
//! a second sweep sees nothing), and a **live heartbeat must move the expiry**, because `heartbeat_job` is an
//! ownership check rather than an advisory timestamp — only the worker holding the lock may extend it, and nobody
//! else may (`fire_timer_job.rs:378-400`).
//!
//! Everything is driven through the production `WorkflowEngine<MemoryStore>`: jobs are created by production graph
//! execution, claimed by production `claim_jobs_by_type`, renewed by production `heartbeat_job`, and swept by
//! production `reclaim_stale_jobs`.
//!
//! Level: L4 Adversarial, harness `DeterministicClockHarness`. Time is the adversary here and it is moved explicitly,
//! one instant at a time. Deterministic and isolated: no database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_timer__004__stale_lease_reclaim

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    DefinitionStatus, Job, JobStatus, NodeDefinition, ProcessDefinition, ProcessGraph,
    ProcessOutcome, StartProcessParams, TimerSpec, TransitionDefinition, Value, JOB_LEASE_MS,
};

/// The canonical harness label for this level.
const HARNESS: &str = "DeterministicClockHarness/L4 Adversarial";

/// The definition key the engine registers.
const DEFINITION_KEY: &str = "TST-WF-TIMER-004";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// The graph: `start -> wait (timer) -> done (end)`.
const START_NODE: &str = "start";
const WAIT_NODE: &str = "wait";
const DONE_NODE: &str = "done";
const BEGIN: &str = "begin";
const RESUME: &str = "resume";

/// The frozen instant the claim happens at.
const NOW: i64 = 1_700_000_000_000;

/// The two workers: the holder and the one that would inherit the work after a reclaim.
const WORKER_A: &str = "worker-a";
const WORKER_B: &str = "worker-b";

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

/// Start the timer graph and claim its one timer job with `worker`, returning the leased job and the lease deadline.
fn leased_timer(harness: &EngineHarness, worker: &str) -> Job {
    harness
        .engine()
        .seed_definition(timer_definition())
        .expect("the timer definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on the timer node");
    let token_id = started.root_token_id.clone();
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
        .claim_jobs_by_type(worker, "timer", 10)
        .expect("the claim step commits");
    assert_eq!(
        claimed.len(),
        1,
        "{HARNESS}: the due timer is claimed by the first worker"
    );
    claimed.into_iter().next().expect("the claimed job")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TIMER-004); the file and the assay use it.
fn wf_timer_004__stale_lease_reclaim() {
    // ── SCENARIO 1 (negative, load-bearing): a LIVE lease is never reclaimed ────────────────────────────────────
    // The job is due and its worker is gone or not — the only thing protecting it is the lease window. A sweep that
    // ignored `locked_until` would hand the same work to a second worker while the first still believes it owns it.
    let harness = EngineHarness::at_unix_millis(NOW);
    let job = leased_timer(&harness, WORKER_A);
    let lease_until = NOW + JOB_LEASE_MS;
    assert_eq!(
        read_job(&harness, &job.id).locked_until,
        Some(lease_until),
        "{HARNESS}: the lease window is the fixture this scenario moves the clock around"
    );

    assert_eq!(
        harness
            .engine()
            .reclaim_stale_jobs(10)
            .expect("the sweep before expiry commits"),
        0,
        "{HARNESS}: a job claimed this instant is not stale"
    );

    // One millisecond before the deadline the lease still holds. This is the exact instant a `locked_until <= now`
    // bug would wrongly steal.
    harness.clock().advance_millis(JOB_LEASE_MS - 1);
    assert_eq!(
        harness
            .engine()
            .reclaim_stale_jobs(10)
            .expect("the sweep one millisecond early commits"),
        0,
        "{HARNESS}: one millisecond before expiry the lease is still live"
    );
    let held = read_job(&harness, &job.id);
    assert_eq!(
        held.status,
        JobStatus::Locked,
        "{HARNESS}: a live lease keeps the job locked"
    );
    assert_eq!(
        held.locked_by.as_deref(),
        Some(WORKER_A),
        "{HARNESS}: a live lease keeps its owner"
    );

    // At the deadline itself the lease is still live: production compares `locked_until < now` strictly, so the
    // boundary instant belongs to the holder.
    harness.clock().advance_millis(1);
    assert_eq!(
        harness.now_millis(),
        lease_until,
        "{HARNESS}: the clock is now exactly at the lease deadline"
    );
    assert_eq!(
        harness
            .engine()
            .reclaim_stale_jobs(10)
            .expect("the sweep at the deadline commits"),
        0,
        "{HARNESS}: at the deadline instant the lease has not expired — the comparison is strictly less-than"
    );
    assert_eq!(
        read_job(&harness, &job.id).locked_by.as_deref(),
        Some(WORKER_A),
        "{HARNESS}: the boundary instant left the owner untouched"
    );

    // ── SCENARIO 2 (positive): one millisecond later the lease IS stale, and reclaim converges ──────────────────
    harness.clock().advance_millis(1);
    assert_eq!(
        harness
            .engine()
            .reclaim_stale_jobs(10)
            .expect("the sweep after expiry commits"),
        1,
        "{HARNESS}: a lease past its deadline is stale and must be reclaimed"
    );

    // Exactly one legal durable state comes back: available again, owned by nobody, no stale window, and the attempt
    // the dead worker spent is remembered rather than erased.
    let reclaimed = read_job(&harness, &job.id);
    assert_eq!(
        reclaimed.status,
        JobStatus::Pending,
        "{HARNESS}: reclaim returns the job to the claimable state"
    );
    assert_eq!(
        reclaimed.locked_by, None,
        "{HARNESS}: reclaim clears the dead worker's ownership"
    );
    assert_eq!(
        reclaimed.locked_until, None,
        "{HARNESS}: reclaim clears the expired lease window so nothing is judged stale twice"
    );
    assert_eq!(
        reclaimed.attempts, 1,
        "{HARNESS}: reclaim preserves the attempt the dead worker spent"
    );

    // ── SCENARIO 3 (negative): reclaiming twice reclaims once ───────────────────────────────────────────────────
    // Convergence means the second sweep has nothing left to do. A double-count here would report reclaimed work
    // that was never returned, which is how a scheduler ends up believing it has jobs it does not.
    assert_eq!(
        harness
            .engine()
            .reclaim_stale_jobs(10)
            .expect("the second sweep commits"),
        0,
        "{HARNESS}: an already-reclaimed job is Pending, not stale — the sweep must not count it again"
    );

    // The reclaimed work is recoverable exactly once, by the next worker.
    let inherited = harness
        .engine()
        .claim_jobs_by_type(WORKER_B, "timer", 10)
        .expect("the inheriting claim step commits");
    assert_eq!(
        inherited.len(),
        1,
        "{HARNESS}: the reclaimed timer is claimable again — the work was recovered, not stranded"
    );
    assert_eq!(
        inherited[0].locked_by.as_deref(),
        Some(WORKER_B),
        "{HARNESS}: the recovered work has exactly one new owner"
    );
    assert_eq!(
        inherited[0].attempts, 2,
        "{HARNESS}: the recovery is a second attempt, not a reset of the first"
    );
    assert!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER_A, "timer", 10)
            .expect("the original worker's re-claim commits")
            .is_empty(),
        "{HARNESS}: the original worker cannot re-take work that was reclaimed from it"
    );

    // ── SCENARIO 4: a live heartbeat moves the expiry, so renewed work is never swept ───────────────────────────
    // A worker that is alive says so. `heartbeat_job` extends the window to `now + JOB_LEASE_MS`
    // (`fire_timer_job.rs:394`), so a sweep running at the *original* deadline finds nothing — the job is not stale,
    // it is alive. This is the adversarial pairing: a stale claim racing a live heartbeat, resolved in the
    // heartbeat's favour because the heartbeat actually committed.
    let harness = EngineHarness::at_unix_millis(NOW);
    let job = leased_timer(&harness, WORKER_A);
    let original_deadline = NOW + JOB_LEASE_MS;

    // The heartbeat lands past the original deadline: work that was held for a full lease window, still alive.
    harness.clock().advance_millis(JOB_LEASE_MS + 1);
    let renewed_until = harness
        .engine()
        .heartbeat_job(&job.id, WORKER_A)
        .expect("the owner's heartbeat commits");
    assert_eq!(
        renewed_until,
        harness.now_millis() + JOB_LEASE_MS,
        "{HARNESS}: a heartbeat extends the lease to now + JOB_LEASE_MS"
    );
    assert_eq!(
        read_job(&harness, &job.id).locked_until,
        Some(renewed_until),
        "{HARNESS}: the renewed deadline is what the row carries"
    );
    assert!(
        renewed_until > original_deadline,
        "{HARNESS}: the renewal genuinely moved the deadline past the one the job was first claimed under"
    );

    // A sweep at the original deadline sees a live lease, not stale work.
    let sweep_at_original = harness
        .engine()
        .reclaim_stale_jobs(10)
        .expect("the sweep after the heartbeat commits");
    assert_eq!(
        sweep_at_original, 0,
        "{HARNESS}: a heartbeat that committed before the sweep wins — renewed work is not reclaimed"
    );
    assert_eq!(
        read_job(&harness, &job.id).locked_by.as_deref(),
        Some(WORKER_A),
        "{HARNESS}: the live worker kept its work through the sweep"
    );

    // ── SCENARIO 5 (negative): only the holder may renew, so nobody can buy a lease they do not own ─────────────
    // If a thief could heartbeat, it could extend a lease past its own deadline and take the work legitimately.
    // Ownership is checked first (`fire_timer_job.rs:387-392`), so the attempt is refused and costs the job nothing.
    let theft = harness
        .engine()
        .heartbeat_job(&job.id, WORKER_B)
        .expect_err("a worker that does not hold the lease must not renew it");
    assert_eq!(
        theft.code(),
        "JOB_LOCK_OWNER",
        "{HARNESS}: renewing a lease you do not hold is refused as a lock-owner conflict"
    );
    assert_eq!(
        read_job(&harness, &job.id).locked_until,
        Some(renewed_until),
        "{HARNESS}: the refused heartbeat did not move the deadline"
    );

    // ── SCENARIO 6: once the renewed lease really does expire, the work is recoverable ─────────────────────────
    // The heartbeat deferred the sweep; it did not cancel it. An abandoned job is eventually reclaimed whatever
    // renewals it had, which is the whole reason reclaim exists.
    harness.clock().advance_millis(JOB_LEASE_MS + 1);
    assert_eq!(
        harness
            .engine()
            .reclaim_stale_jobs(10)
            .expect("the sweep after the renewed lease expires commits"),
        1,
        "{HARNESS}: the renewed lease expires on its own terms and the work is reclaimed"
    );
    assert_eq!(
        read_job(&harness, &job.id).status,
        JobStatus::Pending,
        "{HARNESS}: the abandoned renewed job converges to the claimable state"
    );
    assert_eq!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER_B, "timer", 10)
            .expect("the final claim step commits")
            .len(),
        1,
        "{HARNESS}: the recovered work is claimable by the next worker, and by exactly one of them"
    );
    let final_owner = harness
        .engine()
        .claim_jobs_by_type(WORKER_A, "timer", 10)
        .expect("the loser's re-claim commits");
    assert!(
        final_owner.is_empty(),
        "{HARNESS}: recovered work converges to a single owner"
    );
}
