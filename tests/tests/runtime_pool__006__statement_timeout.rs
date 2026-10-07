//! RUNTIME.POOL — statement timeout (TST-RUNTIME-POOL-006).
//!
//! CONTRACT. A statement the server cancels at `statement_timeout` arrives as SQLSTATE `57014` (`query_canceled`);
//! a lock wait cancelled the same way arrives as `55P03`. The production taxonomy (`DbFailure::from_sqlx`,
//! `db/src/error.rs:158-160`) classifies both as `Timeout` + `retryable`: a cancelled statement says nothing about
//! the session, so the call may be repeated, and it says nothing about the work except that it did not finish, so
//! the retry must stay bounded. "Statement timeout" is therefore not "the call hangs" and it is not "the session
//! died": it is a classified, retryable statement failure that converges or stops at the policy's attempt ceiling.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The SQLSTATEs are shaped through `sqlx::error::DatabaseError`, the
//! same trait sqlx's Postgres driver implements. `DbPoolFaultHarness` performs no classification of its own — it
//! runs `DbFailure::from_sqlx` and returns the production result. The retry loop is production `db::retry`.
//!
//! WHY ADVERSARIAL (L4). A slow dependency cancels many statements at once, so one caller recovering proves little.
//! The last section runs six callers that meet at a barrier and each survives a cancelled statement, asserting
//! every one converges to exactly one success in exactly two attempts.
//!
//! NEGATIVE CASES. A test that only saw cancelled statements could not tell the classification from a boundary that
//! calls every server error a statement timeout. So the idle-in-transaction timeout (`25P03`) — whose NAME says
//! timeout but which TERMINATES THE SESSION — is driven through the same boundary and must be
//! `DatabaseUnavailable`, not `Timeout`; and a constraint violation must be attempted exactly once. A boundary that
//! matched on the word "timeout" would fail here.
//!
//! NO EXTERNAL I/O. No socket is opened, no provider is called, no database is touched, and the harness cannot reach
//! the PRODUCTION database (it never connects at all). Level: L4 Adversarial, harness `DbPoolFaultHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_pool__006__statement_timeout

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-RUNTIME-POOL-006); the file and the assay use it.
async fn runtime_pool_006__statement_timeout() {
    let api = "a cancelled statement must be a classified, retryable Timeout that converges or stops at the ceiling";

    // (1) THE BASELINE. A statement that finishes succeeds, so the failures below can only be the cancels'.
    let healthy = DbPoolFaultHarness::scripted(vec![PoolFault::Ready]);
    healthy
        .acquire("db.acquire")
        .expect("a finished statement is a success");
    assert_eq!(
        healthy.successes(),
        1,
        "{api}: a finished statement must succeed"
    );

    // (2) THE CANCEL IS A SERVER SQLSTATE, AND THE PRODUCTION CLASSIFIER CALLS IT A TIMEOUT. Both members of the
    //     statement-cancel family — `57014` (statement timeout) and `55P03` (lock timeout) — must be `Timeout` +
    //     retryable, must keep the operation label, and must carry the driver's message in the detail, so an
    //     operator sees WHAT was cancelled.
    for fault in [PoolFault::StatementTimeout, PoolFault::LockTimeout] {
        let cancelled = DbPoolFaultHarness::always(fault)
            .acquire("db.acquire")
            .expect_err("a cancelled statement is a failure");
        assert_eq!(
            cancelled.kind,
            DbFailureKind::Timeout,
            "{fault:?} must classify as Timeout",
        );
        assert!(
            cancelled.retryable,
            "{fault:?} is repeatable and must be retryable"
        );
        assert_eq!(
            cancelled.operation, "db.acquire",
            "{fault:?} must name the operation that was cancelled",
        );
        assert!(
            cancelled
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("canceling statement")),
            "{fault:?} must carry the driver's message, got {:?}",
            cancelled.detail,
        );
    }

    // (3) BOUNDED CONVERGENCE. A cancel, then a finish: the production retry must converge to exactly one success
    //     in exactly two attempts, leaving no fault unspent.
    let recovering =
        DbPoolFaultHarness::scripted(vec![PoolFault::StatementTimeout, PoolFault::Ready]);
    retry(bounded_attempts(5), |_| async {
        recovering.acquire("db.acquire")
    })
    .await
    .expect("the retry must converge once a statement finishes");
    assert_eq!(
        recovering.attempts(),
        2,
        "{api}: convergence must consume exactly the cancel then succeed",
    );
    assert_eq!(
        recovering.successes(),
        1,
        "{api}: convergence must reach exactly one legal success",
    );

    // (4) BOUNDED FAILURE. Cancels that never end (a statement that can never finish in time) must stop at the
    //     policy's ceiling and surface the Timeout — not retry without end, and never smuggle a success.
    let hopeless = DbPoolFaultHarness::always(PoolFault::StatementTimeout);
    let failure = retry(bounded_attempts(3), |_| async {
        hopeless.acquire("db.acquire")
    })
    .await
    .expect_err("endless cancels must surface the timeout");
    assert_eq!(
        hopeless.attempts(),
        3,
        "{api}: the retry must be bounded by the policy, not by the cancels",
    );
    assert_eq!(
        hopeless.successes(),
        0,
        "{api}: endless cancels must never smuggle a success",
    );
    assert_eq!(
        failure.kind,
        DbFailureKind::Timeout,
        "{api}: the surfaced failure must still be a Timeout",
    );

    // (4b) BOUNDED BY THE POLICY, NOT BY THE SCRIPT RUNNING DRY. A script longer than the ceiling must leave the
    //      excess faults unspent.
    let overlong = DbPoolFaultHarness::scripted(vec![PoolFault::StatementTimeout; 10]);
    retry(bounded_attempts(3), |_| async {
        overlong.acquire("db.acquire")
    })
    .await
    .expect_err("endless cancels must surface the timeout");
    assert_eq!(
        overlong.attempts(),
        3,
        "{api}: the policy's ceiling, not the script's length, must bound the retry",
    );
    assert_eq!(
        overlong.remaining(),
        7,
        "{api}: the retry must stop at the ceiling and leave the rest unspent",
    );

    // (5) NEGATIVE / REFUSAL. A SQLSTATE whose NAME contains "timeout" is not automatically a `Timeout`: the
    //     idle-in-transaction timeout (`25P03`) TERMINATES THE SESSION and must be `DatabaseUnavailable` through
    //     the same boundary. And a constraint violation must be attempted exactly once. Without this clause a
    //     boundary that matched on the word "timeout" — or that called everything a timeout — would still pass
    //     step (2).
    let idle = DbPoolFaultHarness::always(PoolFault::IdleInTransactionTimeout)
        .acquire("db.acquire")
        .expect_err("an idle-in-transaction timeout is a failure");
    assert_ne!(
        idle.kind,
        DbFailureKind::Timeout,
        "{api}: 25P03 names a timeout but ends the session, so it must not classify as one",
    );
    assert_eq!(
        idle.kind,
        DbFailureKind::DatabaseUnavailable,
        "{api}: 25P03 is a session failure",
    );
    let work = DbPoolFaultHarness::always(PoolFault::ConstraintViolation);
    let failure = retry(bounded_attempts(5), |_| async {
        work.acquire("db.acquire")
    })
    .await
    .expect_err("a work failure must surface");
    assert_ne!(
        failure.kind,
        DbFailureKind::Timeout,
        "{api}: a constraint violation must not be misclassified as a timeout",
    );
    assert!(
        !failure.retryable,
        "{api}: a constraint must not be retryable"
    );
    assert_eq!(
        work.attempts(),
        1,
        "{api}: a work failure must be attempted once: waiting does not fix it",
    );

    // (6) ADVERSARIAL: SIX CONCURRENT CALLERS EACH SURVIVE A CANCEL. They meet at a barrier (so the cancels
    //     overlap, the way a slow dependency cancels many statements at once) and each drives its own scripted
    //     pool through the same production retry. Every caller must independently converge to exactly one success
    //     in exactly two attempts — no lost caller, no unbounded retry.
    let parties = 6usize;
    let barrier = ConcurrencyBarrier::new(parties);
    let mut handles = Vec::with_capacity(parties);
    for _ in 0..parties {
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let harness =
                DbPoolFaultHarness::scripted(vec![PoolFault::StatementTimeout, PoolFault::Ready]);
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
            "{api}: every concurrent caller must survive its cancel"
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
