//! RUNTIME.POOL — exhausted pool (TST-RUNTIME-POOL-002).
//!
//! CONTRACT. An exhausted pool has no connection to give, and it says so in exactly two ways: the server refuses
//! the checkout (`53300`, "too many clients", `DatabaseUnavailable`) or the client-side acquire deadline passes
//! first (`sqlx::Error::PoolTimedOut`, `Timeout`). Both are transient — the WORK is fine, only this checkout
//! failed — so both must be `retryable`, and the production retry (`db::retry`, `db/src/retry.rs:57`) must treat
//! exhaustion as bounded waiting: it converges when a slot frees and stops at the policy's attempt ceiling when
//! none does. "Exhausted" is therefore not "the call fails" and it is not "the call waits forever": it is a
//! classified, retryable failure with zero successes until a checkout actually hands out a connection.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. `53300` is shaped through `sqlx::error::DatabaseError`, the same
//! trait sqlx's Postgres driver implements, and `PoolTimedOut` is the exact value sqlx builds. `DbPoolFaultHarness`
//! performs no classification of its own — it runs `DbFailure::from_sqlx` and returns the production result. The
//! retry loop is production `db::retry`.
//!
//! WHY ADVERSARIAL (L4). The failure of an exhaustion policy is not that one caller waits; it is that a saturated
//! pool either stampedes (every waiter retries without bound, deepening the pile-up) or strands callers. The last
//! section parks six callers at a barrier against pools that never free a slot and asserts every one stops at the
//! same ceiling with zero successes — bounded waiting under saturation, not a stampede and not a hang.
//!
//! NEGATIVE CASES. A test that only saw exhaustion could not tell the classification from a boundary that calls
//! every driver error exhaustion and retries everything. So a constraint violation (`23505`) is driven through the
//! same boundary and must be neither `DatabaseUnavailable` nor `Timeout`, must not be retryable, and must be
//! attempted exactly once — waiting does not free a constraint. And the healthy path is asserted first, so a pool
//! that never hands out a connection cannot pass.
//!
//! NO EXTERNAL I/O. No socket is opened, no provider is called, no database is touched, and the harness cannot reach
//! the PRODUCTION database (it never connects at all). Level: L4 Adversarial, harness `DbPoolFaultHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_pool__002__exhausted_pool

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-RUNTIME-POOL-002); the file and the assay use it.
async fn runtime_pool_002__exhausted_pool() {
    let api = "an exhausted pool must be a classified, retryable failure with no success until a slot frees";

    // (1) THE BASELINE. A pool with a free slot hands out a connection, so the failures below can only be the
    //     faults'. A pool that never succeeds must not be able to pass this test.
    let healthy = DbPoolFaultHarness::scripted(vec![PoolFault::Ready]);
    healthy
        .acquire("db.acquire")
        .expect("a free slot is a connection");
    assert_eq!(healthy.successes(), 1, "{api}: a free slot must succeed");

    // (2) THE SERVER SAYS "NO FREE SLOTS": 53300 IS A RETRYABLE CONNECTION FAILURE, AND IT KEEPS THE LABEL.
    let refused = DbPoolFaultHarness::always(PoolFault::ConnectionExhausted)
        .acquire("db.acquire")
        .expect_err("an exhausted server refuses the checkout");
    assert_eq!(
        refused.kind,
        DbFailureKind::DatabaseUnavailable,
        "{api}: 53300 (too many clients) is evidence about the pool, not the work",
    );
    assert!(
        refused.retryable,
        "{api}: exhaustion is transient and must be retryable",
    );
    assert_eq!(
        refused.operation, "db.acquire",
        "{api}: the failure must name the operation that found no slot",
    );
    assert_eq!(
        refused.code.as_deref(),
        Some("53300"),
        "{api}: the sqlstate must travel with the failure",
    );

    // (3) THE CLIENT STOPS WAITING FIRST: THE ACQUIRE TIMEOUT IS THE DRIVER'S OWN ERROR, AND IT IS RETRYABLE.
    let driver = PoolFault::PoolTimedOut
        .driver_error()
        .expect("an acquire timeout is an error");
    assert_eq!(
        driver.to_string(),
        "pool timed out while waiting for an open connection",
        "{api}: the acquire timeout must be sqlx's own `PoolTimedOut`",
    );
    let timed_out = db::DbFailure::from_sqlx("db.acquire", &driver);
    assert_eq!(
        timed_out.kind,
        DbFailureKind::Timeout,
        "{api}: a pool-acquire timeout must classify as Timeout",
    );
    assert!(
        timed_out.retryable,
        "{api}: a Timeout is transient and must be retryable",
    );

    // (4) TOTAL EXHAUSTION IS BOUNDED, FOR BOTH SPELLINGS. A pool that never frees a slot must stop at the
    //     policy's ceiling — whether the server keeps refusing or the client keeps timing out — with zero
    //     successes and the exhaustion kind surfaced, not a hang and not a phantom success.
    for fault in [PoolFault::ConnectionExhausted, PoolFault::PoolTimedOut] {
        let exhausted = DbPoolFaultHarness::always(fault);
        let failure = retry(bounded_attempts(3), |_| async {
            exhausted.acquire("db.acquire")
        })
        .await
        .expect_err("a pool with no free slot must surface the exhaustion");
        assert_eq!(
            exhausted.attempts(),
            3,
            "{fault:?}: the retry must be bounded by the policy, not by the pool",
        );
        assert_eq!(
            exhausted.successes(),
            0,
            "{fault:?}: exhaustion must never smuggle a success",
        );
        assert!(
            matches!(
                failure.kind,
                DbFailureKind::DatabaseUnavailable | DbFailureKind::Timeout
            ),
            "{fault:?}: the surfaced failure must still be the exhaustion, got {:?}",
            failure.kind,
        );
    }

    // (4b) BOUNDED BY THE POLICY, NOT BY THE SCRIPT RUNNING DRY. A script longer than the ceiling must leave the
    //      excess faults unspent: that is the difference between "we stopped because the policy says so" and "we
    //      stopped because the pool happened to run out of script".
    let overlong = DbPoolFaultHarness::scripted(vec![PoolFault::ConnectionExhausted; 10]);
    retry(bounded_attempts(3), |_| async {
        overlong.acquire("db.acquire")
    })
    .await
    .expect_err("a pool that stays exhausted must surface it");
    assert_eq!(
        overlong.attempts(),
        3,
        "{api}: the policy's ceiling, not the script's length, must bound the wait",
    );
    assert_eq!(
        overlong.remaining(),
        7,
        "{api}: the wait must stop at the ceiling and leave the rest unspent",
    );

    // (5) A SLOT FREES: CONVERGENCE CONSUMES EXACTLY THE EXHAUSTION, THEN SUCCEEDS EXACTLY ONCE. Two refusals and
    //     a timeout, then a connection: the retry must converge to exactly one success in exactly four attempts,
    //     leaving no fault unspent.
    let freeing = DbPoolFaultHarness::scripted(vec![
        PoolFault::ConnectionExhausted,
        PoolFault::PoolTimedOut,
        PoolFault::ConnectionExhausted,
        PoolFault::Ready,
    ]);
    retry(bounded_attempts(5), |_| async {
        freeing.acquire("db.acquire")
    })
    .await
    .expect("the retry must converge once a slot frees");
    assert_eq!(
        freeing.attempts(),
        4,
        "{api}: convergence must consume exactly the exhaustion then succeed",
    );
    assert_eq!(
        freeing.successes(),
        1,
        "{api}: convergence must reach exactly one legal success",
    );

    // (6) NEGATIVE / REFUSAL. A work failure is not exhaustion: a constraint violation must be neither
    //     `DatabaseUnavailable` nor `Timeout`, must not be retryable, and must be attempted exactly once —
    //     waiting for a slot does not fix the work. Without this clause a boundary that labelled everything
    //     exhaustion would still pass step (4).
    let harness = DbPoolFaultHarness::always(PoolFault::ConstraintViolation);
    let failure = retry(bounded_attempts(5), |_| async {
        harness.acquire("db.acquire")
    })
    .await
    .expect_err("a work failure must surface");
    assert_eq!(
        failure.kind,
        DbFailureKind::Constraint,
        "{api}: a constraint violation is work evidence, not pool evidence",
    );
    assert!(
        !failure.retryable,
        "{api}: a constraint must not be retryable"
    );
    assert_eq!(
        harness.attempts(),
        1,
        "{api}: a work failure must be attempted once: waiting does not fix it",
    );

    // (7) ADVERSARIAL: SIX CALLERS SATURATE AT ONCE, EACH BOUNDED, NONE SUCCEEDS. They meet at a barrier (so they
    //     overlap, rather than queueing one after another) against pools that never free a slot. Every caller must
    //     independently stop at the same ceiling with zero successes — bounded waiting under saturation, no
    //     stampede, no hang, no phantom success.
    let parties = 6usize;
    let barrier = ConcurrencyBarrier::new(parties);
    let mut handles = Vec::with_capacity(parties);
    for index in 0..parties {
        let barrier = barrier.clone();
        // Alternate the two spellings of exhaustion across callers, so the bound holds for both at once.
        let fault = if index % 2 == 0 {
            PoolFault::ConnectionExhausted
        } else {
            PoolFault::PoolTimedOut
        };
        handles.push(tokio::spawn(async move {
            let harness = DbPoolFaultHarness::always(fault);
            barrier.arrive_and_wait().await;
            let outcome = retry(bounded_attempts(3), |_| async {
                harness.acquire("db.acquire")
            })
            .await;
            (
                outcome.is_err(),
                harness.attempts(),
                harness.successes(),
                fault,
            )
        }));
    }
    for handle in handles {
        let (failed, attempts, successes, fault) = handle.await.expect("a caller must not panic");
        assert!(
            failed,
            "{fault:?}: a saturated caller must surface the exhaustion"
        );
        assert_eq!(
            attempts, 3,
            "{fault:?}: every saturated caller must be bounded to the same ceiling",
        );
        assert_eq!(
            successes, 0,
            "{fault:?}: no saturated caller may smuggle a success",
        );
    }
}
