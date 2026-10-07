//! RUNTIME.ENV — environment target resolution is deterministic across APP_ENV, EXECUTION_ENV, DATABASE_URL_PROD, DATABASE_URL_DEV, FORGE_STORE, and FORGE_ATTENDED (TST-RUNTIME-ENV-001).
//!
//! CONTRACT. The production database target resolution (`db::resolve_declared_target`, `db/src/pool.rs`) is the
//! single authority for which database a process connects to. It reads `VERCEL_ENV` (priority) then `APP_ENV`
//! and must:
//!
//!   1. Return `DbTarget::Prod` for `VERCEL_ENV=production` (regardless of APP_ENV).
//!   2. Return `DbTarget::Dev` for `VERCEL_ENV=preview` or `VERCEL_ENV=development`.
//!   3. Return `DbTarget::Prod` for `APP_ENV=production` or `APP_ENV=prod` (when VERCEL_ENV not set).
//!   4. Return `DbTarget::Dev` for `APP_ENV=development`, `dev`, `test`, or `testing`.
//!   5. Refuse (configuration error) when neither is set or when set to an unknown value.
//!   6. Be deterministic: the same inputs always produce the same output, with no external I/O.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The test exercises the actual `resolve_declared_target` function
//! with controlled environment variables via `RuntimeHarness`, not a re-implementation.
//!
//! NEGATIVE CASES. A test that only checked the happy paths could not distinguish correct resolution from a
//! function that defaults to DEV or PROD. So the test also exercises:
//! - Unknown `VERCEL_ENV` values fall through to `APP_ENV` (not default to PROD).
//! - Unknown `APP_ENV` values are refused (not default to DEV).
//! - Empty strings are treated as unset.
//! - Whitespace is trimmed.
//! - Case insensitivity.
//!
//! NO EXTERNAL I/O. `RuntimeHarness` scopes the process environment and restores it on drop. Level: L0 Pure,
//! harness `RuntimeHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_env__001__environment_target_resolution_is_deterministic_across_app_env_execution_env_database_url

use test_harness::RuntimeHarness;
use db::{resolve_declared_target, DbTarget};

#[test]
#[allow(non_snake_case)]
fn runtime_env_001__environment_target_resolution_is_deterministic_across_app_env_execution_env_database_url() {
    let mut env = RuntimeHarness::acquire();
    let api = "environment target resolution";

    // ---- VERCEL_ENV takes priority over APP_ENV ----
    // production
    assert_eq!(
        resolve_declared_target(Some("production"), Some("development")).unwrap(),
        DbTarget::Prod,
        "{api}: VERCEL_ENV=production must win over APP_ENV=development"
    );
    assert_eq!(
        resolve_declared_target(Some("production"), Some("dev")).unwrap(),
        DbTarget::Prod,
        "{api}: VERCEL_ENV=production must win over APP_ENV=dev"
    );
    assert_eq!(
        resolve_declared_target(Some("production"), None).unwrap(),
        DbTarget::Prod,
        "{api}: VERCEL_ENV=production alone must be PROD"
    );

    // preview -> DEV
    assert_eq!(
        resolve_declared_target(Some("preview"), Some("production")).unwrap(),
        DbTarget::Dev,
        "{api}: VERCEL_ENV=preview must be DEV even with APP_ENV=production"
    );
    assert_eq!(
        resolve_declared_target(Some("preview"), None).unwrap(),
        DbTarget::Dev,
        "{api}: VERCEL_ENV=preview alone must be DEV"
    );

    // development -> DEV
    assert_eq!(
        resolve_declared_target(Some("development"), Some("production")).unwrap(),
        DbTarget::Dev,
        "{api}: VERCEL_ENV=development must be DEV even with APP_ENV=production"
    );
    assert_eq!(
        resolve_declared_target(Some("development"), None).unwrap(),
        DbTarget::Dev,
        "{api}: VERCEL_ENV=development alone must be DEV"
    );

    // ---- APP_ENV when VERCEL_ENV not set ----
    // production variants
    assert_eq!(
        resolve_declared_target(None, Some("production")).unwrap(),
        DbTarget::Prod,
        "{api}: APP_ENV=production must be PROD"
    );
    assert_eq!(
        resolve_declared_target(None, Some("prod")).unwrap(),
        DbTarget::Prod,
        "{api}: APP_ENV=prod must be PROD"
    );
    assert_eq!(
        resolve_declared_target(None, Some("PRODUCTION")).unwrap(),
        DbTarget::Prod,
        "{api}: APP_ENV=PRODUCTION (uppercase) must be PROD"
    );
    assert_eq!(
        resolve_declared_target(None, Some("Prod")).unwrap(),
        DbTarget::Prod,
        "{api}: APP_ENV=Prod (mixed case) must be PROD"
    );

    // development variants
    assert_eq!(
        resolve_declared_target(None, Some("development")).unwrap(),
        DbTarget::Dev,
        "{api}: APP_ENV=development must be DEV"
    );
    assert_eq!(
        resolve_declared_target(None, Some("dev")).unwrap(),
        DbTarget::Dev,
        "{api}: APP_ENV=dev must be DEV"
    );
    assert_eq!(
        resolve_declared_target(None, Some("test")).unwrap(),
        DbTarget::Dev,
        "{api}: APP_ENV=test must be DEV"
    );
    assert_eq!(
        resolve_declared_target(None, Some("testing")).unwrap(),
        DbTarget::Dev,
        "{api}: APP_ENV=testing must be DEV"
    );
    assert_eq!(
        resolve_declared_target(None, Some("DEVELOPMENT")).unwrap(),
        DbTarget::Dev,
        "{api}: APP_ENV=DEVELOPMENT (uppercase) must be DEV"
    );

    // ---- NEGATIVE: Unknown values are refused ----
    // Unknown VERCEL_ENV falls through to APP_ENV
    assert_eq!(
        resolve_declared_target(Some("unknown"), Some("production")).unwrap(),
        DbTarget::Prod,
        "{api}: unknown VERCEL_ENV must fall through to APP_ENV=production"
    );
    assert_eq!(
        resolve_declared_target(Some("unknown"), Some("dev")).unwrap(),
        DbTarget::Dev,
        "{api}: unknown VERCEL_ENV must fall through to APP_ENV=dev"
    );

    // Unknown APP_ENV with no VERCEL_ENV is refused
    let err = resolve_declared_target(None, Some("staging")).unwrap_err();
    assert!(
        err.to_string().contains("database target is undeclared"),
        "{api}: unknown APP_ENV must be refused, got: {err}"
    );

    let err = resolve_declared_target(None, Some("local")).unwrap_err();
    assert!(
        err.to_string().contains("database target is undeclared"),
        "{api}: unknown APP_ENV must be refused, got: {err}"
    );

    // ---- NEGATIVE: Empty/whitespace treated as unset ----
    let err = resolve_declared_target(Some(""), None).unwrap_err();
    assert!(
        err.to_string().contains("database target is undeclared"),
        "{api}: empty VERCEL_ENV must be refused"
    );

    let err = resolve_declared_target(None, Some("")).unwrap_err();
    assert!(
        err.to_string().contains("database target is undeclared"),
        "{api}: empty APP_ENV must be refused"
    );

    let err = resolve_declared_target(Some("  "), Some("  ")).unwrap_err();
    assert!(
        err.to_string().contains("database target is undeclared"),
        "{api}: whitespace-only must be refused"
    );

    // ---- NEGATIVE: Both unset is refused ----
    let err = resolve_declared_target(None, None).unwrap_err();
    assert!(
        err.to_string().contains("database target is undeclared"),
        "{api}: both unset must be refused"
    );

    // ---- DETERMINISM: Same inputs always produce same outputs ----
    for _ in 0..10 {
        assert_eq!(
            resolve_declared_target(Some("production"), Some("dev")).unwrap(),
            DbTarget::Prod
        );
        assert_eq!(
            resolve_declared_target(None, Some("production")).unwrap(),
            DbTarget::Prod
        );
        assert_eq!(
            resolve_declared_target(None, Some("development")).unwrap(),
            DbTarget::Dev
        );
        assert!(resolve_declared_target(None, None).is_err());
    }

    // ---- EXECUTION_ENV and FORGE_STORE are NOT read by resolve_declared_target ----
    // These are read by other boundaries (Forge engine, vendor_session), not by the core resolution.
    // This test documents that they don't affect the core target resolution.
    env.set("EXECUTION_ENV", "PROD");
    env.set("FORGE_STORE", "memory");
    env.set("FORGE_ATTENDED", "1");
    env.set("DATABASE_URL_PROD", "postgres://prod/invalid");
    env.set("DATABASE_URL_DEV", "postgres://dev/invalid");

    // Core resolution unchanged
    assert_eq!(
        resolve_declared_target(Some("production"), None).unwrap(),
        DbTarget::Prod
    );
    assert_eq!(
        resolve_declared_target(None, Some("development")).unwrap(),
        DbTarget::Dev
    );
}