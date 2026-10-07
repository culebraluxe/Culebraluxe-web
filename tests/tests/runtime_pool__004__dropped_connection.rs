//! RUNTIME.POOL — dropped connection (TST-RUNTIME-POOL-004).
//!
//! CONTRACT. A connection can die after checkout — the socket breaks mid-use and the driver reports an I/O error
//! with no SQLSTATE at all (`Broken pipe (os error 32)`). The production taxonomy (`DbFailure::from_sqlx`,
//! `db/src/error.rs:118-137`) reads that message as a connection-class failure: `DatabaseUnavailable` +
//! `retryable`, and it opens the pool's verification window (`note_connection_failure`), because a broken socket
//! is evidence about the pool, not only about this call. "Dropped" is therefore not "the statement failed" and it
//! is not "unknown": it is a classified, retryable connection failure that converges once a live connection arrives.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The error is shaped exactly as the pool's own regression test
//! builds it (`db/src/pool.rs`: `sqlx::Error::Io` with `BrokenPipe`, whose Display is
//! `error communicating with database: Broken pipe (os error 32)`). `DbPoolFaultHarness` performs no
//! classification of its own — it runs `DbFailure::from_sqlx` and returns the production result. The retry loop
//! is production `db::retry`.
//!
//! WHY ADVERSARIAL (L4). Drops come in bursts — a proxy restart, a NAT timeout — so one caller recovering proves
//! little. The last section runs six callers that meet at a barrier and each survives a drop, asserting every one
//! converges to exactly one success in exactly two attempts: no lost caller, no unbounded retry.
//!
//! NEGATIVE CASES. A test that only saw drops could not tell the classification from a boundary that calls every
//! driver error a connection failure. So `RowNotFound` (no connection word anywhere in it) is driven through the
//! same boundary and must be `Unknown`, not `DatabaseUnavailable`, and must not be retryable; and a constraint
//! violation must be attempted exactly once. A boundary that matched on vibes instead of the message would fail
//! here.
//!
//! NO EXTERNAL I/O. No socket is opened, no provider is called, no database is touched, and the harness cannot reach
//! the PRODUCTION database (it never connects at all). Level: L4 Adversarial, harness `DbPoolFaultHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_pool__004__dropped_connection

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-RUNTIME-POOL-004); the file and the assay use it.
async fn runtime_pool_004__dropped_connection() {
    let api = "a dropped connection must be a classified, retryable DatabaseUnavailable that converges on recovery";

    // (1) THE BASELINE. A live connection succeeds, so the failures below can only be the drops'.
    let healthy = DbPoolFaultHarness::scripted(vec![PoolFault::Ready]);
    healthy
        .acquire("db.acquire")
        .expect("a live connection is a success");
    assert_eq!(
        healthy.successes(),
        1,
        "{api}: a live connection must succeed"
    );

    // (2) THE DROP IS THE DRIVER'S OWN I/O ERROR — NO SQLSTATE, JUST THE BROKEN SOCKET. This is the exact value
    //     the pool's own regression test builds (`db/src/pool.rs`); a test must not invent a spelling of "dropped".
    let driver = PoolFault::DroppedConnection
        .driver_error()
        .expect("a dropped connection is an error");
    assert_eq!(
        driver.to_string(),
        "error communicating with database: Broken pipe (os error 32)",
        "{api}: the drop must be the driver's own I/O error",
    );

    // (3) THE PRODUCTION CLASSIFIER TURNS IT INTO A RETRYABLE CONNECTION FAILURE, AND KEEPS THE LABEL. There is
    //     no sqlstate on this path, so there must also be no code — only the message, the kind, and the flag.
    let classified = db::DbFailure::from_sqlx("db.acquire", &driver);
    assert_eq!(
        classified.kind,
        DbFailureKind::DatabaseUnavailable,
        "{api}: a broken socket is evidence about the connection, not the statement",
    );
    assert!(
        classified.retryable,
        "{api}: a dropped connection is transient and must be retryable",
    );
    assert_eq!(
        classified.operation, "db.acquire",
        "{api}: the failure must name the operation that held the dead socket",
    );
    assert_eq!(
        classified.code, None,
        "{api}: an I/O drop carries no sqlstate and must claim none",
    );

    // (4) BOUNDED CONVERGENCE. A drop, then a live connection: the production retry must converge to exactly one
    //     success in exactly two attempts, leaving no fault unspent.
    let recovering =
        DbPoolFaultHarness::scripted(vec![PoolFault::DroppedConnection, PoolFault::Ready]);
    retry(bounded_attempts(5), |_| async {
        recovering.acquire("db.acquire")
    })
    .await
    .expect("the retry must converge once a live connection arrives");
    assert_eq!(
        recovering.attempts(),
        2,
        "{api}: convergence must consume exactly the drop then succeed",
    );
    assert_eq!(
        recovering.successes(),
        1,
        "{api}: convergence must reach exactly one legal success",
    );

    // (5) BOUNDED FAILURE. Drops that never end must stop at the policy's ceiling and surface the connection
    //     failure — they must not retry without end, and they must never smuggle a success.
    let falling = DbPoolFaultHarness::always(PoolFault::DroppedConnection);
    let failure = retry(bounded_attempts(3), |_| async {
        falling.acquire("db.acquire")
    })
    .await
    .expect_err("drops that never end must surface the connection failure");
    assert_eq!(
        falling.attempts(),
        3,
        "{api}: the retry must be bounded by the policy, not by the drops",
    );
    assert_eq!(
        falling.successes(),
        0,
        "{api}: endless drops must never smuggle a success",
    );
    assert_eq!(
        failure.kind,
        DbFailureKind::DatabaseUnavailable,
        "{api}: the surfaced failure must still be the drop",
    );

    // (6) NEGATIVE / REFUSAL. A driver error with no connection in it is not a dropped connection: `RowNotFound`
    //     must be `Unknown`, not `DatabaseUnavailable`, and must not be retryable. And a constraint violation
    //     must be attempted exactly once. Without this clause a boundary that called every driver error a
    //     connection failure would still pass step (3).
    let no_rows = db::DbFailure::from_sqlx("db.acquire", &sqlx::Error::RowNotFound);
    assert_eq!(
        no_rows.kind,
        DbFailureKind::Unknown,
        "{api}: a non-connection driver error must not be promoted to a drop",
    );
    assert!(!no_rows.retryable, "{api}: it must not be retryable either");
    let work = DbPoolFaultHarness::always(PoolFault::ConstraintViolation);
    let failure = retry(bounded_attempts(5), |_| async {
        work.acquire("db.acquire")
    })
    .await
    .expect_err("a work failure must surface");
    assert_ne!(
        failure.kind,
        DbFailureKind::DatabaseUnavailable,
        "{api}: a constraint violation is work evidence, not a dead socket",
    );
    assert!(
        !failure.retryable,
        "{api}: a constraint must not be retryable"
    );
    assert_eq!(
        work.attempts(),
        1,
        "{api}: a work failure must be attempted once: redialling does not fix it",
    );

    // (7) ADVERSARIAL: SIX CONCURRENT CALLERS EACH SURVIVE A DROP. They meet at a barrier (so the drops overlap,
    //     rather than arriving one after another) and each drives its own scripted pool through the same production
    //     retry. Every caller must independently converge to exactly one success in exactly two attempts — no lost
    //     caller, no unbounded retry.
    let parties = 6usize;
    let barrier = ConcurrencyBarrier::new(parties);
    let mut handles = Vec::with_capacity(parties);
    for _ in 0..parties {
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let harness =
                DbPoolFaultHarness::scripted(vec![PoolFault::DroppedConnection, PoolFault::Ready]);
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
            "{api}: every concurrent caller must survive its drop"
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
