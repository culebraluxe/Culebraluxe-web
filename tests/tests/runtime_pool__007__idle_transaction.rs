//! RUNTIME.POOL — idle transaction (TST-RUNTIME-POOL-007).
//!
//! CONTRACT. `idle_in_transaction_session_timeout` does not cancel a statement — it TERMINATES THE SESSION, and
//! the client is told so at whatever statement it happens to send next (SQLSTATE `25P03`). The production taxonomy
//! (`db/src/error.rs:161-171`) classifies it as `DatabaseUnavailable` + `retryable`: the work was never in
//! question, only the session, so the call must be repeated on a fresh connection. Getting this wrong is what lost
//! a real eight-minute story 2m45s in at a step boundary on 2026-09-29, when `25P03` reached `Unknown` and nothing
//! retried it. "Idle transaction" is therefore the regression with a name: a terminated idle session must be
//! retryable session evidence, never a terminal unknown and never a statement timeout.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The SQLSTATE is shaped through `sqlx::error::DatabaseError`, the
//! same trait sqlx's Postgres driver implements. `DbPoolFaultHarness` performs no classification of its own — it
//! runs `DbFailure::from_sqlx` and returns the production result. The retry loop is production `db::retry`.
//!
//! WHY ADVERSARIAL (L4). An aggressive `idle_in_transaction_session_timeout` terminates many idle sessions at once,
//! so one caller recovering proves little. The last section runs six callers that meet at a barrier and each
//! survives an idle termination, asserting every one converges to exactly one success in exactly two attempts.
//!
//! NEGATIVE CASES. A test that only saw idle terminations could not tell the classification from a boundary that
//! calls every server error a session failure. So a cancelled statement (`57014`) is driven through the same
//! boundary and must stay `Timeout` — a statement cancel is not a session death — and a constraint violation must
//! be attempted exactly once. A boundary that could not tell the two apart would fail here.
//!
//! NO EXTERNAL I/O. No socket is opened, no provider is called, no database is touched, and the harness cannot reach
//! the PRODUCTION database (it never connects at all). Level: L4 Adversarial, harness `DbPoolFaultHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_pool__007__idle_transaction

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-RUNTIME-POOL-007); the file and the assay use it.
async fn runtime_pool_007__idle_transaction() {
    let api = "an idle-terminated session must be a retryable DatabaseUnavailable that converges on a fresh session";

    // (1) THE BASELINE. A live session succeeds, so the failures below can only be the terminations'.
    let healthy = DbPoolFaultHarness::scripted(vec![PoolFault::Ready]);
    healthy
        .acquire("db.acquire")
        .expect("a live session is a success");
    assert_eq!(healthy.successes(), 1, "{api}: a live session must succeed");

    // (2) THE TERMINATION IS A SERVER SQLSTATE, AND THE PRODUCTION CLASSIFIER CALLS IT A RETRYABLE SESSION
    //     FAILURE. 25P03 must be `DatabaseUnavailable` + retryable — the regression is exactly this flag, because
    //     `Unknown` is not retried and the story was lost. It must keep the operation label and carry the driver's
    //     message, so an operator sees which session the server reaped.
    let reaped = DbPoolFaultHarness::always(PoolFault::IdleInTransactionTimeout)
        .acquire("db.acquire")
        .expect_err("a reaped session is a failure");
    assert_eq!(
        reaped.kind,
        DbFailureKind::DatabaseUnavailable,
        "{api}: 25P03 ends the session, so it is session evidence, not statement evidence",
    );
    assert!(
        reaped.retryable,
        "{api}: the work was never in question — this flag is the 2026-09-29 regression",
    );
    assert_eq!(
        reaped.operation, "db.acquire",
        "{api}: the failure must name the operation that found the dead session",
    );
    assert_eq!(
        reaped.code.as_deref(),
        Some("25P03"),
        "{api}: the sqlstate must travel with the failure",
    );
    assert!(
        reaped
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("idle-in-transaction")),
        "{api}: the detail must carry the driver's message, got {:?}",
        reaped.detail,
    );

    // (3) BOUNDED CONVERGENCE — THE REPEAT THE LOST STORY NEVER GOT. A reaped session, then a fresh one: the
    //     production retry must converge to exactly one success in exactly two attempts.
    let fresh =
        DbPoolFaultHarness::scripted(vec![PoolFault::IdleInTransactionTimeout, PoolFault::Ready]);
    retry(bounded_attempts(5), |_| async {
        fresh.acquire("db.acquire")
    })
    .await
    .expect("the retry must converge once a fresh session arrives");
    assert_eq!(
        fresh.attempts(),
        2,
        "{api}: convergence must consume exactly the termination then succeed",
    );
    assert_eq!(
        fresh.successes(),
        1,
        "{api}: convergence must reach exactly one legal success",
    );

    // (4) BOUNDED FAILURE. Reaps that never end (a timeout set far too aggressively) must stop at the policy's
    //     ceiling and surface the session failure — not retry without end, and never smuggle a success.
    let aggressive = DbPoolFaultHarness::always(PoolFault::IdleInTransactionTimeout);
    let failure = retry(bounded_attempts(3), |_| async {
        aggressive.acquire("db.acquire")
    })
    .await
    .expect_err("endless reaps must surface the session failure");
    assert_eq!(
        aggressive.attempts(),
        3,
        "{api}: the retry must be bounded by the policy, not by the reaps",
    );
    assert_eq!(
        aggressive.successes(),
        0,
        "{api}: endless reaps must never smuggle a success",
    );
    assert_eq!(
        failure.kind,
        DbFailureKind::DatabaseUnavailable,
        "{api}: the surfaced failure must still be the termination",
    );

    // (5) NEGATIVE / REFUSAL. A reaped session is not a cancelled statement: `57014` must stay `Timeout` through
    //     the same boundary, or a matcher that called every server error a session death would still pass step
    //     (2). And a constraint violation must be attempted exactly once — a fresh session does not fix the work.
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

    // (6) ADVERSARIAL: SIX CONCURRENT CALLERS EACH SURVIVE A REAP. They meet at a barrier (so the reaps overlap,
    //     the way an aggressive timeout reaps many idle sessions at once) and each drives its own scripted pool
    //     through the same production retry. Every caller must independently converge to exactly one success in
    //     exactly two attempts — no lost caller, no unbounded retry.
    let parties = 6usize;
    let barrier = ConcurrencyBarrier::new(parties);
    let mut handles = Vec::with_capacity(parties);
    for _ in 0..parties {
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let harness = DbPoolFaultHarness::scripted(vec![
                PoolFault::IdleInTransactionTimeout,
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
            "{api}: every concurrent caller must survive its reap"
        );
        assert_eq!(
            attempts, 2,
            "{api}: every concurrent caller must be bounded to the same attempts",
        );
        assert_eq!(
            successes, 1,
            "{api}: every concurrent caller must reach exactly one success",
        );
    }
}
