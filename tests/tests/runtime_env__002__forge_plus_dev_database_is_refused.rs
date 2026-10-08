//! RUNTIME.ENV — Forge plus DEV database is refused (TST-RUNTIME-ENV-002).
//!
//! CONTRACT. Forge runs against PRODUCTION only (AGENTS.md: "Never run Forge against DEV"). The production
//! boundaries enforce this at three layers:
//!
//!   1. `resolve_forge_target` (`db/src/pool.rs`): refuses any declaration that resolves to DEV.
//!   2. `DbForgeStateWriter::connect_env` (`forge/src/engine/db_writer.rs`): refuses when `DATABASE_URL_PROD` is absent.
//!   3. `Database::connect_forge_from_env` (`db/src/pool.rs`): refuses when PROD is declared but `DATABASE_URL_PROD` is not configured.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The test exercises the actual Forge-target resolution, the
//! Forge state writer, and the Forge database pool with controlled environment variables via `RuntimeHarness`.
//!
//! NEGATIVE CASES. A test that only checked one layer could not distinguish a complete refusal from a
//! partial one that falls through. So the test also exercises:
//! - A declaration of `APP_ENV=development` with `DATABASE_URL_DEV` set (must be refused at layer 1).
//! - A declaration of `APP_ENV=production` with `DATABASE_URL_DEV` set but `DATABASE_URL_PROD` absent (must be refused at layer 2/3).
//! - A declaration of `VERCEL_ENV=preview` with `APP_ENV=production` (must be refused at layer 1 because VERCEL wins).
//! - The decoy `DATABASE_URL_DEV` must NOT satisfy any layer when PROD is declared but PROD URL is absent.
//!
//! NO EXTERNAL I/O. `RuntimeHarness` scopes the process environment; no connection is opened. Level: L0 Pure,
//! harness `RuntimeHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_env__002__forge_plus_dev_database_is_refused

use test_harness::RuntimeHarness;
use db::{resolve_forge_target, Database, DbTarget};
use forge::engine::db_writer::DbForgeStateWriter;
use tokio::runtime::Runtime;

#[test]
#[allow(non_snake_case)]
fn runtime_env_002__forge_plus_dev_database_is_refused() {
    let mut env = RuntimeHarness::acquire();
    let api = "Forge plus DEV database is refused";

    // ---- LAYER 1: resolve_forge_target refuses DEV declarations ----
    // APP_ENV=development
    assert!(
        resolve_forge_target(None, Some("development")).is_err(),
        "{api}: APP_ENV=development must be refused by resolve_forge_target"
    );
    let err = resolve_forge_target(None, Some("development")).unwrap_err();
    assert!(
        err.to_string().contains("PRODUCTION only"),
        "{api}: refusal must name PRODUCTION only rule, got: {err}"
    );

    // APP_ENV=dev
    assert!(
        resolve_forge_target(None, Some("dev")).is_err(),
        "{api}: APP_ENV=dev must be refused by resolve_forge_target"
    );

    // VERCEL_ENV=preview (wins over APP_ENV)
    assert!(
        resolve_forge_target(Some("preview"), Some("production")).is_err(),
        "{api}: VERCEL_ENV=preview must be refused even with APP_ENV=production"
    );

    // VERCEL_ENV=development
    assert!(
        resolve_forge_target(Some("development"), Some("production")).is_err(),
        "{api}: VERCEL_ENV=development must be refused even with APP_ENV=production"
    );

    // Silence is refused (by resolve_declared_target)
    assert!(
        resolve_forge_target(None, None).is_err(),
        "{api}: silence must be refused"
    );

    // ---- LAYER 2: DbForgeStateWriter::connect_env refuses when DATABASE_URL_PROD absent ----
    // Environment: PROD declared, DATABASE_URL_PROD absent, decoy DATABASE_URL_DEV present
    env.set("APP_ENV", "production");
    env.remove("VERCEL_ENV");
    env.remove("DATABASE_URL_PROD");
    env.set("DATABASE_URL_DEV", "postgres://dev-only.invalid/culebraluxe");
    env.remove("DATABASE_URL");

    let refusal = match DbForgeStateWriter::connect_env() {
        Ok(_) => panic!("{api}: Forge state writer must refuse without DATABASE_URL_PROD"),
        Err(msg) => msg,
    };
    assert!(
        refusal.contains("DATABASE_URL_PROD"),
        "{api}: refusal must name missing variable, got: {refusal}"
    );
    assert!(
        refusal.to_lowercase().contains("production"),
        "{api}: refusal must say writer is production-only, got: {refusal}"
    );

    // ---- LAYER 3: Database::connect_forge_from_env refuses before any socket ----
    let runtime = Runtime::new().expect("tokio runtime");
    let pool_failure = runtime
        .block_on(Database::connect_forge_from_env())
        .err()
        .expect("{api}: PROD Forge pool with no DATABASE_URL_PROD must be refused");
    assert_eq!(
        pool_failure.operation, "db.connect",
        "{api}: pool refusal is a configuration failure at connect time"
    );
    let detail = pool_failure.detail.unwrap_or_default();
    assert!(
        detail.contains("DATABASE_URL_PROD"),
        "{api}: pool refusal must name missing variable, got: {detail}"
    );
    assert!(
        detail.to_lowercase().contains("refuses to fall back"),
        "{api}: pool refusal must fail closed rather than fall back, got: {detail}"
    );

    // ---- CONTROL: Non-empty DATABASE_URL_PROD makes writer construct (no connection opened) ----
    env.set("DATABASE_URL_PROD", "postgres://placeholder.invalid/culebraluxe");
    assert!(
        DbForgeStateWriter::connect_env().is_ok(),
        "{api}: present DATABASE_URL_PROD must yield writer (no connection opened)"
    );

    // ---- NEGATIVE: DEV declaration with DATABASE_URL_DEV set is still refused ----
    // Reset environment to DEV with DEV URL
    env.set("APP_ENV", "development");
    env.remove("DATABASE_URL_PROD");
    env.set("DATABASE_URL_DEV", "postgres://dev.invalid/culebraluxe");

    assert!(
        resolve_forge_target(None, Some("development")).is_err(),
        "{api}: DEV declaration refused even with DATABASE_URL_DEV set"
    );

    // ---- NEGATIVE: VERCEL_ENV=preview with APP_ENV=prod still refused ----
    env.set("VERCEL_ENV", "preview");
    env.set("APP_ENV", "production");
    env.remove("DATABASE_URL_PROD");
    env.set("DATABASE_URL_DEV", "postgres://dev.invalid/culebraluxe");

    assert!(
        resolve_forge_target(Some("preview"), Some("production")).is_err(),
        "{api}: VERCEL_ENV=preview refused even with APP_ENV=production and DATABASE_URL_DEV"
    );
}