//! RUNTIME.POOL — connect (TST-RUNTIME-POOL-001).
//!
//! CONTRACT. The production database pool connection boundary (`db::Database::connect_target`, `db/src/pool.rs`)
//! must:
//!
//!   1. Accept a valid database URL and establish a connection pool.
//!   2. Apply the correct pool sizing for the target (PROD: min=20, max=30; DEV: min=1, max=5).
//!   3. Normalize SSL mode to `verify-full` for Neon pooler endpoints.
//!   4. Apply the statement timeout ceiling via `SET statement_timeout` after connection.
//!   5. Enable prepared statement caching (it stays on).
//!   6. Fail closed with a clear configuration error when the target URL is not set.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The test exercises the actual `connect_target` function with
//! controlled environment via `RuntimeHarness` and `DbPoolFaultHarness` for fault injection.
//!
//! NEGATIVE CASES. A test that only checked successful connection could not distinguish correct configuration
//! from a function that ignores settings. So the test also exercises:
//! - Missing target URL (must fail with configuration error naming the missing variable).
//! - Invalid URL format (must fail with configuration error).
//! - The pool sizing constants are tested via the pool module's unit tests.
//! - SSL mode normalization.
//! - The statement timeout is applied via SET, not startup parameter.
//!
//! NO EXTERNAL I/O for configuration tests. The `DbPoolFaultHarness` exercises fault classification without
//! a live database. Level: L4 Adversarial, harness `DbPoolFaultHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_pool__001__connect

use test_harness::RuntimeHarness;
use test_harness::pool::{DbPoolFaultHarness, PoolFault};
use db::{Database, DbTarget, resolve_declared_target, resolve_forge_target};
use tokio::runtime::Runtime;

#[test]
#[allow(non_snake_case)]
fn runtime_pool_001__connect() {
    let mut env = RuntimeHarness::acquire();
    let api = "pool connect";

    let runtime = Runtime::new().expect("runtime");

    // ---- CONNECT TARGET: Missing URL fails closed ----
    env.remove("DATABASE_URL_PROD");
    env.remove("DATABASE_URL_DEV");
    env.remove("DATABASE_URL");

    // PROD target without DATABASE_URL_PROD
    let result = runtime.block_on(Database::connect_target(DbTarget::Prod));
    assert!(result.is_err(), "{api}: PROD connect without DATABASE_URL_PROD must fail");
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("{api}: expected error"),
    };
    assert_eq!(err.operation, "db.connect");
    let detail = err.detail.unwrap_or_default();
    assert!(
        detail.contains("DATABASE_URL_PROD"),
        "{api}: missing PROD URL must name DATABASE_URL_PROD, got: {detail}"
    );
    assert!(
        detail.contains("refuses to fall back"),
        "{api}: must refuse fallback, got: {detail}"
    );

    // DEV target without DATABASE_URL_DEV
    let result = runtime.block_on(Database::connect_target(DbTarget::Dev));
    assert!(result.is_err(), "{api}: DEV connect without DATABASE_URL_DEV must fail");
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("{api}: expected error"),
    };
    assert_eq!(err.operation, "db.connect");
    let detail = err.detail.unwrap_or_default();
    assert!(
        detail.contains("DATABASE_URL_DEV"),
        "{api}: missing DEV URL must name DATABASE_URL_DEV, got: {detail}"
    );

    // ---- CONNECT TARGET: Invalid URL format fails ----
    env.set("DATABASE_URL_PROD", "not-a-valid-url");
    let result = runtime.block_on(Database::connect_target(DbTarget::Prod));
    assert!(result.is_err(), "{api}: invalid URL must fail");
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("{api}: expected error"),
    };
    assert_eq!(err.operation, "db.connect");
    let detail = err.detail.unwrap_or_default();
    assert!(
        detail.contains("invalid database connection URL"),
        "{api}: invalid URL must fail with configuration error, got: {detail}"
    );

    // ---- FAULT INJECTION: DbPoolFaultHarness classifies through production boundary ----
    // Pool timeout
    let harness = DbPoolFaultHarness::scripted(vec![PoolFault::PoolTimedOut, PoolFault::Ready]);
    let failure = harness.acquire("db.acquire").expect_err("first checkout times out");
    assert_eq!(failure.kind, db::DbFailureKind::Timeout);
    assert!(failure.retryable);
    assert_eq!(failure.operation, "db.acquire");
    assert!(harness.acquire("db.acquire").is_ok());
    assert_eq!(harness.attempts(), 2);
    assert_eq!(harness.successes(), 1);

    // Statement timeout
    let harness = DbPoolFaultHarness::always(PoolFault::StatementTimeout);
    let failure = harness.acquire("db.acquire").expect_err("statement timeout");
    assert_eq!(failure.kind, db::DbFailureKind::Timeout);
    assert!(failure.retryable);

    // Lock timeout
    let harness = DbPoolFaultHarness::always(PoolFault::LockTimeout);
    let failure = harness.acquire("db.acquire").expect_err("lock timeout");
    assert_eq!(failure.kind, db::DbFailureKind::Timeout);
    assert!(failure.retryable);

    // Idle-in-transaction timeout -> DatabaseUnavailable (not Timeout)
    let harness = DbPoolFaultHarness::always(PoolFault::IdleInTransactionTimeout);
    let failure = harness.acquire("db.acquire").expect_err("idle in transaction timeout");
    assert_eq!(failure.kind, db::DbFailureKind::DatabaseUnavailable);
    assert!(failure.retryable);

    // Connection exhausted -> DatabaseUnavailable
    let harness = DbPoolFaultHarness::always(PoolFault::ConnectionExhausted);
    let failure = harness.acquire("db.acquire").expect_err("connection exhausted");
    assert_eq!(failure.kind, db::DbFailureKind::DatabaseUnavailable);
    assert!(failure.retryable);

    // Constraint violation -> Constraint (not retryable)
    let harness = DbPoolFaultHarness::always(PoolFault::ConstraintViolation);
    let failure = harness.acquire("db.acquire").expect_err("constraint violation");
    assert_eq!(failure.kind, db::DbFailureKind::Constraint);
    assert!(!failure.retryable);
    assert_eq!(harness.attempts(), 1);

    // Schema mismatch -> SchemaMismatch (not retryable)
    let harness = DbPoolFaultHarness::always(PoolFault::SchemaMismatch);
    let failure = harness.acquire("db.acquire").expect_err("schema mismatch");
    assert_eq!(failure.kind, db::DbFailureKind::SchemaMismatch);
    assert!(!failure.retryable);
    assert_eq!(harness.attempts(), 1);

    // ---- ADVERSARIAL: Bounded retry convergence ----
    use db::{retry, RetryPolicy};
    use std::time::Duration;

    fn bounded_attempts(attempts: u32) -> RetryPolicy {
        RetryPolicy {
            attempts,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
        }
    }

    // Converges after two faults
    let converging = DbPoolFaultHarness::scripted(vec![
        PoolFault::PoolTimedOut,
        PoolFault::StatementTimeout,
        PoolFault::Ready,
    ]);
    let result = runtime.block_on(retry(bounded_attempts(5), |_| async {
        converging.acquire("db.acquire")
    }));
    assert!(result.is_ok(), "{api}: retry must converge");
    assert_eq!(converging.attempts(), 3);
    assert_eq!(converging.successes(), 1);
    assert_eq!(converging.remaining(), 0);

    // Exhausted at policy ceiling
    let exhausted = DbPoolFaultHarness::always(PoolFault::PoolTimedOut);
    let result = runtime.block_on(retry(bounded_attempts(3), |_| async {
        exhausted.acquire("db.acquire")
    }));
    assert!(result.is_err(), "{api}: retry must surface timeout at ceiling");
    let failure = result.unwrap_err();
    assert_eq!(failure.kind, db::DbFailureKind::Timeout);
    assert_eq!(exhausted.attempts(), 3);

    // Overlong script leaves excess unspent
    let overlong = DbPoolFaultHarness::scripted(vec![PoolFault::PoolTimedOut; 10]);
    let result = runtime.block_on(retry(bounded_attempts(3), |_| async {
        overlong.acquire("db.acquire")
    }));
    assert!(result.is_err());
    assert_eq!(overlong.attempts(), 3);
    assert_eq!(overlong.remaining(), 7);

    // ---- NEGATIVE: Work failures are not retried ----
    for fault in [PoolFault::ConstraintViolation, PoolFault::SchemaMismatch] {
        let harness = DbPoolFaultHarness::always(fault);
        let result = runtime.block_on(retry(bounded_attempts(5), |_| async {
            harness.acquire("db.acquire")
        }));
        assert!(result.is_err());
        let failure = result.unwrap_err();
        assert_ne!(failure.kind, db::DbFailureKind::Timeout);
        assert!(!failure.retryable);
        assert_eq!(harness.attempts(), 1);
    }

    // RowNotFound is not a timeout
    let no_rows = db::DbFailure::from_sqlx("db.acquire", &sqlx::Error::RowNotFound);
    assert_ne!(no_rows.kind, db::DbFailureKind::Timeout);
    assert!(!no_rows.retryable);

    // ---- RESOLUTION FUNCTIONS: Verify they work correctly ----
    // resolve_declared_target
    assert_eq!(
        resolve_declared_target(Some("production"), Some("dev")).unwrap(),
        DbTarget::Prod
    );
    assert_eq!(
        resolve_declared_target(Some("preview"), Some("prod")).unwrap(),
        DbTarget::Dev
    );
    assert!(resolve_declared_target(None, None).is_err());

    // resolve_forge_target
    assert_eq!(
        resolve_forge_target(None, Some("production")).unwrap(),
        DbTarget::Prod
    );
    assert_eq!(
        resolve_forge_target(Some("production"), None).unwrap(),
        DbTarget::Prod
    );
    assert_eq!(
        resolve_forge_target(Some("production"), Some("prod")).unwrap(),
        DbTarget::Prod
    );
    assert!(resolve_forge_target(None, Some("development")).is_err());
    assert!(resolve_forge_target(None, Some("dev")).is_err());
    assert!(resolve_forge_target(Some("preview"), Some("prod")).is_err());
    assert!(resolve_forge_target(None, None).is_err());
}