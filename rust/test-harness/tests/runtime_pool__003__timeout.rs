//! RUNTIME.POOL — timeout (TST-RUNTIME-POOL-003).
//!
//! CONTRACT. The pool boundary has two ways to time out, and both must end the same way: a caller that cannot get a
//! connection before `acquire_timeout` expires (`sqlx::Error::PoolTimedOut`, from `PgPool::acquire`), and a statement
//! the server cancels at `statement_timeout` (SQLSTATE `57014`, `query_canceled`). Each must be classified by the
//! production taxonomy (`DbFailure::from_sqlx`, `rust/core/db/src/error.rs:104`) as `DbFailureKind::Timeout`, marked
//! `retryable`, and handled by the production retry path (`db::retry`, `rust/core/db/src/retry.rs:57`) in a BOUNDED
//! way. "Timeout" is therefore not "the call hangs" and it is not "the call fails forever": it is a classified,
//! retryable failure that converges to one success or stops at the policy's attempt ceiling.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The driver errors are exact: `sqlx::Error::PoolTimedOut` is the value
//! sqlx builds, and SQLSTATE errors are shaped through `sqlx::error::DatabaseError`, which is the same trait sqlx's
//! Postgres driver implements. `DbPoolFaultHarness` performs no classification of its own — it runs
//! `DbFailure::from_sqlx` and returns the production result. The retry loop is production `db::retry`.
//!
//! WHY ADVERSARIAL (L4). The failure of a timeout policy is not that one call times out; it is that a burst of
//! callers either stampedes (unbounded retries) or gives up inconsistently. One section runs six callers that meet at
//! a barrier and drive their own scripted pools at once, and asserts every one converges to exactly one success in
//! exactly three bounded attempts. The final section gives all six ONE shared pool holding a single connection: it
//! asserts convergence to exactly one legal success (no double handout, no lost caller) while every losing caller's
//! timeout is still retried to the policy ceiling — a `Timeout` production stopped treating as retryable would stop at
//! one attempt and fail there.
//!
//! NEGATIVE CASES. A test that only saw timeouts could not tell the classification from a boundary that calls every
//! driver error a timeout and retries everything. So a constraint violation (`23505`) and a missing relation
//! (`42P01`) are driven through the same boundary and must be neither `Timeout` nor retryable, and must be attempted
//! exactly once — waiting does not fix either. A driver error with no timeout anywhere in it (`RowNotFound`) must not
//! be promoted to `Timeout` either. And a SQLSTATE whose NAME contains "timeout" is not enough: the
//! idle-in-transaction timeout (`25P03`, which terminates the session) and "too many clients" (`53300`) are
//! connection failures, not `Timeout`.
//!
//! NO EXTERNAL I/O. No socket is opened, no provider is called, no database is touched, and the harness cannot reach
//! the PRODUCTION database (it never connects at all). Level: L4 Adversarial, harness `DbPoolFaultHarness`.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test runtime_pool__003__timeout

use std::sync::Arc;
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-RUNTIME-POOL-003); the file and the assay use it.
async fn runtime_pool_003__timeout() {
    let api = "a pool timeout must be a classified, retryable Timeout that converges or stops at the ceiling";

    // (1) THE ACQUIRE TIMEOUT IS THE DRIVER'S OWN ERROR. `PgPool::acquire` returns this exact value when
    //     `acquire_timeout` expires; a test must not invent a different spelling of "timed out".
    let driver = PoolFault::PoolTimedOut
        .driver_error()
        .expect("a pool-acquire timeout is an error");
    assert_eq!(
        driver.to_string(),
        "pool timed out while waiting for an open connection",
        "{api}: the acquire timeout must be sqlx's own `PoolTimedOut`",
    );

    // (2) THE PRODUCTION CLASSIFIER TURNS IT INTO A RETRYABLE TIMEOUT, AND KEEPS THE OPERATION LABEL.
    let classified = db::DbFailure::from_sqlx("db.acquire", &driver);
    assert_eq!(
        classified.kind,
        DbFailureKind::Timeout,
        "{api}: a pool-acquire timeout must classify as Timeout",
    );
    assert!(
        classified.retryable,
        "{api}: a Timeout is transient and must be retryable",
    );
    assert_eq!(
        classified.operation, "db.acquire",
        "{api}: the failure must name the operation that timed out",
    );

    // (3) A STATEMENT TIMEOUT (57014) AND A LOCK TIMEOUT (55P03) CLASSIFY THE SAME WAY. These arrive on the socket as
    //     a server error, not as a client-side pool error, and the taxonomy must not lose them.
    for fault in [PoolFault::StatementTimeout, PoolFault::LockTimeout] {
        let failure = DbPoolFaultHarness::always(fault)
            .acquire("db.acquire")
            .expect_err("a cancelled statement is a failure");
        assert_eq!(
            failure.kind,
            DbFailureKind::Timeout,
            "{fault:?} must classify as Timeout",
        );
        assert!(failure.retryable, "{fault:?} is retryable");
    }

    // (3b) NEGATIVE / ADVERSARIAL. A SQLSTATE whose NAME contains "timeout" is not automatically a `Timeout`. The
    //      idle-in-transaction timeout (`25P03`) TERMINATES THE SESSION, and "too many clients" (`53300`) REFUSES the
    //      connection: both are evidence about the session, and the production taxonomy classifies them
    //      `DatabaseUnavailable` (`rust/core/db/src/error.rs:161-170`). A boundary that matched on the word "timeout"
    //      would call these `Timeout` and still pass step (3); this clause refuses that shortcut. They stay retryable
    //      — the work is fine, the session is not.
    for fault in [
        PoolFault::IdleInTransactionTimeout,
        PoolFault::ConnectionExhausted,
    ] {
        let failure = DbPoolFaultHarness::always(fault)
            .acquire("db.acquire")
            .expect_err("a session failure is a failure");
        assert_ne!(
            failure.kind,
            DbFailureKind::Timeout,
            "{fault:?} names a timeout but must not be classified as one",
        );
        assert_eq!(
            failure.kind,
            DbFailureKind::DatabaseUnavailable,
            "{fault:?} is a session/connection failure",
        );
        assert!(
            failure.retryable,
            "{fault:?} is retryable: the work is fine, the session is not",
        );
    }

    // (4) BOUNDED CONVERGENCE. Two timeouts then a connection: the production retry must converge to exactly one
    //     success, in exactly the scripted number of attempts, leaving no fault unspent.
    let converging = DbPoolFaultHarness::scripted(vec![
        PoolFault::PoolTimedOut,
        PoolFault::StatementTimeout,
        PoolFault::Ready,
    ]);
    retry(bounded_attempts(5), |_| async {
        converging.acquire("db.acquire")
    })
    .await
    .expect("the retry must converge once the pool recovers");
    assert_eq!(
        converging.attempts(),
        3,
        "{api}: convergence must consume exactly the timeouts then succeed",
    );
    assert_eq!(
        converging.successes(),
        1,
        "{api}: convergence must reach exactly one legal success",
    );

    // (5) BOUNDED FAILURE. A pool that times out forever must stop at the policy's ceiling and surface the Timeout —
    //     it must not retry without end.
    let exhausted = DbPoolFaultHarness::always(PoolFault::PoolTimedOut);
    let failure = retry(bounded_attempts(3), |_| async {
        exhausted.acquire("db.acquire")
    })
    .await
    .expect_err("a pool that never recovers must surface the timeout");
    assert_eq!(
        exhausted.attempts(),
        3,
        "{api}: the retry must be bounded by the policy, not by the pool",
    );
    assert_eq!(
        failure.kind,
        DbFailureKind::Timeout,
        "{api}: the surfaced failure must still be a Timeout",
    );

    // (5b) BOUNDED BY THE POLICY, NOT BY THE POOL RUNNING DRY. A script longer than the policy's ceiling must leave
    //      the excess faults unspent: that is the difference between "we stopped because the policy says so" and "we
    //      stopped because the pool happened to run out of script". Without this, a loop that ignored `attempts` and
    //      simply drained the script would still pass step (5).
    let overlong = DbPoolFaultHarness::scripted(vec![PoolFault::PoolTimedOut; 10]);
    let failure = retry(bounded_attempts(3), |_| async {
        overlong.acquire("db.acquire")
    })
    .await
    .expect_err("a pool that keeps timing out must surface the timeout");
    assert_eq!(
        failure.kind,
        DbFailureKind::Timeout,
        "{api}: the surfaced failure must still be a Timeout",
    );
    assert_eq!(
        overlong.attempts(),
        3,
        "{api}: the policy's ceiling, not the script's length, must bound the retry",
    );
    assert_eq!(
        overlong.remaining(),
        7,
        "{api}: the retry must stop at the ceiling and leave the rest of the script unspent",
    );

    // (6) NEGATIVE / REFUSAL. A non-timeout driver error must not be called a timeout and must not be retried: a
    //     constraint violation and a schema mismatch are both work failures, and waiting does not fix either. Without
    //     this clause a boundary that labelled everything Timeout would still pass step (4).
    for fault in [PoolFault::ConstraintViolation, PoolFault::SchemaMismatch] {
        let harness = DbPoolFaultHarness::always(fault);
        let failure = retry(bounded_attempts(5), |_| async {
            harness.acquire("db.acquire")
        })
        .await
        .expect_err("a work failure must surface");
        assert_ne!(
            failure.kind,
            DbFailureKind::Timeout,
            "{fault:?} must not be misclassified as a timeout",
        );
        assert!(!failure.retryable, "{fault:?} must not be retryable");
        assert_eq!(
            harness.attempts(),
            1,
            "{fault:?} must be attempted once: waiting does not fix it",
        );
    }

    // (6b) AND A DRIVER ERROR WITH NO TIMEOUT IN IT IS NOT PROMOTED. `RowNotFound` is neither retryable nor a Timeout.
    let no_rows = db::DbFailure::from_sqlx("db.acquire", &sqlx::Error::RowNotFound);
    assert_ne!(
        no_rows.kind,
        DbFailureKind::Timeout,
        "{api}: a non-timeout driver error must not be promoted to Timeout",
    );
    assert!(!no_rows.retryable);

    // (7) ADVERSARIAL: SIX CONCURRENT CALLERS, EACH BOUNDED. They meet at a barrier (so they overlap, rather than
    //     running one after another) and each drives its own scripted pool through the same production retry. Every
    //     caller must independently converge to exactly one success in exactly three attempts — no stampede, no lost
    //     caller, no unbounded retry.
    let parties = 6usize;
    let barrier = ConcurrencyBarrier::new(parties);
    let mut handles = Vec::with_capacity(parties);
    for _ in 0..parties {
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let harness = DbPoolFaultHarness::scripted(vec![
                PoolFault::PoolTimedOut,
                PoolFault::StatementTimeout,
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
        assert!(converged, "{api}: every concurrent caller must converge");
        assert_eq!(
            attempts, 3,
            "{api}: every concurrent caller must be bounded to the same attempts",
        );
        assert_eq!(
            successes, 1,
            "{api}: every concurrent caller must reach exactly one success",
        );
    }

    // (8) ADVERSARIAL: ONE POOL, SIX CALLERS, ONE CONNECTION. Clause (7) gives every caller its own pool, so it never
    //     sees contention for a single connection. Here the six share ONE `DbPoolFaultHarness` whose script holds
    //     exactly one `Ready` and whose fallback is a timeout. Under the barrier they race the same boundary, and the
    //     contract is convergence to ONE legal durable success: exactly one caller gets the connection, and every
    //     other caller keeps timing out and must still be retried to the policy ceiling. The latter is the load-bearing
    //     half — a `Timeout` production stopped treating as retryable would leave each loser at one attempt and fail
    //     here, where clause (7)'s private pools could not see it.
    let ceiling = 5u32;
    let shared = Arc::new(DbPoolFaultHarness::with_fallback(
        vec![
            PoolFault::PoolTimedOut,
            PoolFault::StatementTimeout,
            PoolFault::Ready,
        ],
        PoolFault::PoolTimedOut,
    ));
    let barrier = ConcurrencyBarrier::new(parties);
    let mut handles = Vec::with_capacity(parties);
    for _ in 0..parties {
        let barrier = barrier.clone();
        let shared = Arc::clone(&shared);
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            let mut my_attempts = 0u32;
            let outcome = retry(bounded_attempts(ceiling), |_| {
                my_attempts += 1;
                async { shared.acquire("db.acquire") }
            })
            .await;
            let kind = outcome.as_ref().err().map(|failure| failure.kind);
            (outcome.is_ok(), my_attempts, kind)
        }));
    }
    let mut winners = 0usize;
    let mut winner_attempts = 0u32;
    let mut total_attempts = 0u32;
    for handle in handles {
        let (won, attempts, kind) = handle.await.expect("a caller must not panic");
        total_attempts += attempts;
        if won {
            winners += 1;
            winner_attempts = attempts;
        } else {
            assert_eq!(
                kind,
                Some(DbFailureKind::Timeout),
                "{api}: a caller that does not get the connection must surface a Timeout",
            );
            assert_eq!(
                attempts, ceiling,
                "{api}: a losing caller's timeout must stay retryable to the policy ceiling",
            );
        }
    }
    assert_eq!(
        winners, 1,
        "{api}: six callers sharing one connection must converge to exactly one success",
    );
    assert_eq!(
        shared.successes(),
        1,
        "{api}: the pool must hand out its single connection exactly once",
    );
    assert_eq!(
        total_attempts,
        ceiling * (parties as u32 - 1) + winner_attempts,
        "{api}: the losers must each spend the full ceiling and the winner stop at its success",
    );
}
