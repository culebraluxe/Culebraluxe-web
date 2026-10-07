//! RUNTIME.POOL — server termination (TST-RUNTIME-POOL-005).
//!
//! CONTRACT. The server can terminate a session out from under a caller — administrator shutdown (`57P01`),
//! crash shutdown (`57P02`) — and the client is told so at whatever statement it happens to send next. The
//! production taxonomy (`db/src/error.rs:161-171`) classifies these as `DatabaseUnavailable` + `retryable`: the
//! WORK was never in question, only the session, so the call must be repeated on a fresh connection, not reported
//! as a statement failure and not dropped as unknown. This is the classification that saved an eight-minute story
//! at a step boundary on 2026-09-29, when `25P03` reaching `Unknown` made a terminated session terminal.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The SQLSTATE is shaped through `sqlx::error::DatabaseError`, the
//! same trait sqlx's Postgres driver implements. `DbPoolFaultHarness` performs no classification of its own — it
//! runs `DbFailure::from_sqlx` and returns the production result. The retry loop is production `db::retry`.
//!
//! WHY ADVERSARIAL (L4). A restart terminates every session at once, so one caller recovering proves little. The
//! last section runs six callers that meet at a barrier and each survives two terminations, asserting every one
//! converges to exactly one success in exactly three attempts.
//!
//! NEGATIVE CASES. A test that only saw terminations could not tell the classification from a boundary that calls
//! every server error a terminated session. So a cancelled statement (`57014`) is driven through the same boundary
//! and must stay `Timeout` — a session death and a statement cancel are different evidence — and a constraint
//! violation must be attempted exactly once. A boundary that matched on "the server said something" would fail here.
//!
//! NO EXTERNAL I/O. No socket is opened, no provider is called, no database is touched, and the harness cannot reach
//! the PRODUCTION database (it never connects at all). Level: L4 Adversarial, harness `DbPoolFaultHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_pool__005__server_termination

use std::time::Duration;

use db::{retry, DbFailureKind, RetryPolicy};
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::pool::{DbPoolFaultHarness, PoolFault};

/// A retry policy that does not make the test wait: the delay is not what is under test, the bounded attempt count is.
fn bounded_attempts(attempts: u32) -> RetryPolicy {
    RetryPolicy {
        attempts,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(5),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-RUNTIME-POOL-005); the file and the assay use it.
async fn runtime_pool_005__server_termination() {
    let api = "a terminated session must be a retryable DatabaseUnavailable that converges on a fresh connection";

    // (1) THE BASELINE. A live session succeeds, so the failures below can only be the terminations'.
    let healthy = DbPoolFaultHarness::scripted(vec![PoolFault::Ready]);
    healthy
        .acquire("db.acquire")
        .expect("a live session is a success");
    assert_eq!(healthy.successes(), 1, "{api}: a live session must succeed");

    // (2) THE TERMINATION IS A SERVER SQLSTATE, AND THE PRODUCTION CLASSIFIER CALLS IT A SESSION FAILURE. 57P01
    //     (administrator shutdown) must be `DatabaseUnavailable` + retryable, and it must keep the operation label
    //     and the sqlstate — an operator reading the failure must see which session died where.
    let terminated = DbPoolFaultHarness::always(PoolFault::ServerShutdown)
        .acquire("db.acquire")
        .expect_err("a terminated session is a failure");
    assert_eq!(
        terminated.kind,
        DbFailureKind::DatabaseUnavailable,
        "{api}: an administrator shutdown ends the session, not the statement",
    );
    assert!(
        terminated.retryable,
        "{api}: the work was never in question, so the call must be repeatable",
    );
    assert_eq!(
        terminated.operation, "db.acquire",
        "{api}: the failure must name the operation that found the dead session",
    );
    assert_eq!(
        terminated.code.as_deref(),
        Some("57P01"),
        "{api}: the sqlstate must travel with the failure",
    );

    // (3) BOUNDED CONVERGENCE. A termination, then a fresh connection: the production retry must converge to
    //     exactly one success in exactly two attempts — the repeat the lost 2026-09-29 story never got.
    let restarted = DbPoolFaultHarness::scripted(vec![PoolFault::ServerShutdown, PoolFault::Ready]);
    retry(bounded_attempts(5), |_| async {
        restarted.acquire("db.acquire")
    })
    .await
    .expect("the retry must converge once a fresh session arrives");
    assert_eq!(
        restarted.attempts(),
        2,
        "{api}: convergence must consume exactly the termination then succeed",
    );
    assert_eq!(
        restarted.successes(),
        1,
        "{api}: convergence must reach exactly one legal success",
    );

    // (4) BOUNDED FAILURE. Terminations that never end (a server that never comes back) must stop at the policy's
    //     ceiling and surface the session failure — not retry without end, and never smuggle a success.
    let down = DbPoolFaultHarness::always(PoolFault::ServerShutdown);
    let failure = retry(bounded_attempts(3), |_| async {
        down.acquire("db.acquire")
    })
    .await
    .expect_err("endless terminations must surface the session failure");
    assert_eq!(
        down.attempts(),
        3,
        "{api}: the retry must be bounded by the policy, not by the server",
    );
    assert_eq!(
        down.successes(),
        0,
        "{api}: endless terminations must never smuggle a success",
    );
    assert_eq!(
        failure.kind,
        DbFailureKind::DatabaseUnavailable,
        "{api}: the surfaced failure must still be the termination",
    );

    // (5) NEGATIVE / REFUSAL. A terminated session is not a cancelled statement: `57014` must stay `Timeout`
    //     through the same boundary, or a matcher that called every server error a session death would still pass
    //     step (2). And a constraint violation must be attempted exactly once — a fresh session does not fix the
    //     work.
    let cancelled = DbPoolFaultHarness::always(PoolFault::StatementTimeout)
        .acquire("db.acquire")
        .expect_err("a cancelled statement is a failure");
    assert_eq!(
        cancelled.kind,
        DbFailureKind::Timeout,
        "{api}: a statement cancel is statement evidence, not session evidence",
    );
    let work = DbPoolFaultHarness::always(PoolFault::ConstraintViolation);
    let failure = retry(bounded_attempts(5), |_| async {
        work.acquire("db.acquire")
    })
    .await
    .expect_err("a work failure must surface");
    assert_ne!(
        failure.kind,
        DbFailureKind::DatabaseUnavailable,
        "{api}: a constraint violation is work evidence, not a dead session",
    );
    assert!(
        !failure.retryable,
        "{api}: a constraint must not be retryable"
    );
    assert_eq!(
        work.attempts(),
        1,
        "{api}: a work failure must be attempted once: a fresh session does not fix it",
    );

    // (6) ADVERSARIAL: SIX CONCURRENT CALLERS EACH SURVIVE A RESTART. They meet at a barrier (so the terminations
    //     overlap, the way a real restart lands on every session at once) and each drives its own scripted pool
    //     through the same production retry. Every caller must independently converge to exactly one success in
    //     exactly three attempts — no lost caller, no unbounded retry.
    let parties = 6usize;
    let barrier = ConcurrencyBarrier::new(parties);
    let mut handles = Vec::with_capacity(parties);
    for _ in 0..parties {
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let harness = DbPoolFaultHarness::scripted(vec![
                PoolFault::ServerShutdown,
                PoolFault::ServerShutdown,
                PoolFault::Ready,
            ]);
            barrier.arrive_and_wait().await;
            let outcome = retry(bounded_attempts(5), |_| async {
                harness.acquire("db.acquire")
            })
            .await;
            (outcome.is_ok(), harness.attempts(), harness.successes())
        }));
    }
    for handle in handles {
        let (converged, attempts, successes) = handle.await.expect("a caller must not panic");
        assert!(
            converged,
            "{api}: every concurrent caller must survive the restart"
        );
        assert_eq!(
            attempts, 3,
            "{api}: every concurrent caller must be bounded to the same attempts",
        );
        assert_eq!(
            successes, 1,
            "{api}: every concurrent caller must reach exactly one success",
        );
    }
}
