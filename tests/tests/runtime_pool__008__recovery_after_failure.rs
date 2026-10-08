//! RUNTIME.POOL — recovery after failure (TST-RUNTIME-POOL-008).
//!
//! CONTRACT. Every pool failure the taxonomy calls retryable (`DatabaseUnavailable`, `Timeout`) is a promise: the
//! work may be repeated, and when the pool heals the retry must converge to exactly one success — no more, no
//! fewer — consuming exactly the failures that came before it. Recovery is therefore not "the next call works";
//! it is convergence with an exact attempt count, an exact success count, and no fault left unspent. A pool that
//! never heals must instead stop at the policy's ceiling and surface the failure, and a failure the taxonomy calls
//! terminal must never be "recovered" by retrying it.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The faults are the driver's own errors (`PoolTimedOut`, I/O
//! drops, server SQLSTATEs) shaped through `sqlx::error::DatabaseError`, the same trait sqlx's Postgres driver
//! implements. `DbPoolFaultHarness` performs no classification of its own — it runs `DbFailure::from_sqlx` and
//! returns the production result. The retry loop is production `db::retry`.
//!
//! WHY ADVERSARIAL (L4). Recovery happens under load — every waiter retries at once when the pool heals — so one
//! caller recovering proves little. The last section runs six callers that meet at a barrier and each drives a
//! mixed failure script, asserting every one converges independently to exactly one success.
//!
//! NEGATIVE CASES. A test that only saw recovery could not tell it from a boundary that retries everything: a
//! non-retryable work failure (a constraint violation) must be attempted exactly once and must never "recover"
//! into a success by repetition, and a pool that never heals must surface its failure with zero successes. Without
//! these, a loop that ignored `retryable` and simply drained every script would still pass.
//!
//! NO EXTERNAL I/O. No socket is opened, no provider is called, no database is touched, and the harness cannot reach
//! the PRODUCTION database (it never connects at all). Level: L4 Adversarial, harness `DbPoolFaultHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_pool__008__recovery_after_failure

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-RUNTIME-POOL-008); the file and the assay use it.
async fn runtime_pool_008__recovery_after_failure() {
    let api = "recovery must converge to exactly one success with an exact attempt count, or stop at the ceiling";

    // (1) THE BASELINE. A healthy pool succeeds on the first checkout, so the convergences below can only be the
    //     recoveries'.
    let healthy = DbPoolFaultHarness::scripted(vec![PoolFault::Ready]);
    healthy
        .acquire("db.acquire")
        .expect("a healthy pool is a success");
    assert_eq!(healthy.attempts(), 1, "{api}: health needs no retry");
    assert_eq!(healthy.successes(), 1, "{api}: a healthy pool must succeed");

    // (2) RECOVERY ACROSS EVERY FAILURE FAMILY. A timeout, a drop, a terminated session and a cancelled
    //     statement, then a connection: the production retry must converge to exactly one success in exactly
    //     five attempts — every retryable family is recoverable, and recovery consumes exactly what came before.
    let healing = DbPoolFaultHarness::scripted(vec![
        PoolFault::PoolTimedOut,
        PoolFault::DroppedConnection,
        PoolFault::ServerShutdown,
        PoolFault::StatementTimeout,
        PoolFault::Ready,
    ]);
    retry(bounded_attempts(6), |_| async {
        healing.acquire("db.acquire")
    })
    .await
    .expect("the retry must converge once the pool heals");
    assert_eq!(
        healing.attempts(),
        5,
        "{api}: recovery must consume exactly the four failures then succeed",
    );
    assert_eq!(
        healing.successes(),
        1,
        "{api}: recovery must reach exactly one legal success — no more, no fewer",
    );
    assert_eq!(
        healing.remaining(),
        0,
        "{api}: recovery must leave no fault unspent",
    );

    // (3) THE FALLBACK HEALS TOO. Past the end of its script the pool takes the fallback, which is a connection
    //     unless the caller says otherwise: a one-fault script must converge in two attempts and then be healthy.
    let brief = DbPoolFaultHarness::scripted(vec![PoolFault::IdleInTransactionTimeout]);
    retry(bounded_attempts(5), |_| async {
        brief.acquire("db.acquire")
    })
    .await
    .expect("one fault then the healthy fallback must converge");
    assert_eq!(
        brief.attempts(),
        2,
        "{api}: one fault then health is two attempts"
    );
    assert_eq!(
        brief.successes(),
        1,
        "{api}: the fallback must hand out the success"
    );
    brief
        .acquire("db.acquire")
        .expect("the pool stays healthy after recovery");

    // (4) NO HEALING, NO RECOVERY. A pool that times out forever must stop at the policy's ceiling and surface the
    //     Timeout with zero successes — recovery is convergence, not hope.
    let down = DbPoolFaultHarness::always(PoolFault::PoolTimedOut);
    let failure = retry(bounded_attempts(3), |_| async {
        down.acquire("db.acquire")
    })
    .await
    .expect_err("a pool that never heals must surface its failure");
    assert_eq!(
        down.attempts(),
        3,
        "{api}: the retry must be bounded by the policy, not by the pool",
    );
    assert_eq!(
        down.successes(),
        0,
        "{api}: a pool that never heals must never smuggle a success",
    );
    assert_eq!(
        failure.kind,
        DbFailureKind::Timeout,
        "{api}: the surfaced failure must still be the Timeout",
    );

    // (5) NEGATIVE / REFUSAL. A terminal failure must never be "recovered" by repetition: a constraint violation
    //     is not retryable, so the retry must attempt it exactly once and surface it — a loop that ignored
    //     `retryable` and drained the script would attempt it five times and still fail this clause.
    let terminal = DbPoolFaultHarness::always(PoolFault::ConstraintViolation);
    let failure = retry(bounded_attempts(5), |_| async {
        terminal.acquire("db.acquire")
    })
    .await
    .expect_err("a terminal failure must surface");
    assert_eq!(
        failure.kind,
        DbFailureKind::Constraint,
        "{api}: the surfaced failure must still be the constraint",
    );
    assert!(
        !failure.retryable,
        "{api}: a constraint must not be retryable"
    );
    assert_eq!(
        terminal.attempts(),
        1,
        "{api}: a terminal failure must be attempted once: repetition is not recovery",
    );
    assert_eq!(
        terminal.successes(),
        0,
        "{api}: repetition must never turn a terminal failure into a success",
    );

    // (6) ADVERSARIAL: SIX CONCURRENT CALLERS EACH RECOVER INDEPENDENTLY. They meet at a barrier (so the failures
    //     overlap, the way a real outage lands on every waiter at once) and each drives its own mixed script
    //     through the same production retry. Every caller must independently converge to exactly one success in
    //     exactly four attempts — no lost caller, no double success, no unbounded retry.
    let parties = 6usize;
    let barrier = ConcurrencyBarrier::new(parties);
    let mut handles = Vec::with_capacity(parties);
    for _ in 0..parties {
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let harness = DbPoolFaultHarness::scripted(vec![
                PoolFault::ConnectionExhausted,
                PoolFault::DroppedConnection,
                PoolFault::StatementTimeout,
                PoolFault::Ready,
            ]);
            barrier.arrive_and_wait().await;
            let outcome = retry(bounded_attempts(6), |_| async {
                harness.acquire("db.acquire")
            })
            .await;
            (outcome.is_ok(), harness.attempts(), harness.successes())
        }));
    }
    for handle in handles {
        let (converged, attempts, successes) = handle.await.expect("a caller must not panic");
        assert!(converged, "{api}: every concurrent caller must recover");
        assert_eq!(
            attempts, 4,
            "{api}: every concurrent caller must consume exactly its script",
        );
        assert_eq!(
            successes, 1,
            "{api}: every concurrent caller must reach exactly one success",
        );
    }
}
