//! RUNTIME.ENV — PROD Forge execution with missing DATABASE_URL_PROD is refused (TST-RUNTIME-ENV-004).
//!
//! CONTRACT. A Forge process runs against PRODUCTION only, and it writes its story state to the production database.
//! When such a run is declared against PROD but `DATABASE_URL_PROD` is absent, the production boundary refuses to
//! proceed — it does not fall back to `DATABASE_URL_DEV` or `DATABASE_URL`, and it does not silently run without a
//! writer. Two production-owned boundaries carry that rule and this test exercises both, exactly as the engine binary
//! does:
//!
//!   1. The Forge execution-target policy (`db::resolve_forge_target`, `db/src/pool.rs:584`): a declaration
//!      that resolves to PROD is accepted; one that resolves to DEV is refused ("Forge runs against PRODUCTION only").
//!   2. The Forge state writer (`forge::engine::db_writer::DbForgeStateWriter::connect_env`,
//!      `forge/src/engine/db_writer.rs:10`): with `DATABASE_URL_PROD` unset it returns the refusal
//!      "DATABASE_URL_PROD is not set; Forge writes story state in production only". The probe it sits on
//!      (`forge::engine::vendor_session::database_url`, `forge/src/engine/vendor_session.rs:46`) deliberately
//!      reads `DATABASE_URL_PROD` and nothing else.
//!   3. The production database pool (`db::Database::connect_forge_from_env`, `db/src/pool.rs:87`): with
//!      PROD declared and `DATABASE_URL_PROD` unset it fails closed in `connect_target` with
//!      "`DATABASE_URL_PROD` is not configured; Rust DB refuses to fall back to another environment" — before any
//!      socket is opened.
//!
//! NEGATIVE CASES. A test that only removed the variable and asserted one error could not tell this refusal from a
//! boundary that falls back to a development URL, or one that is broken for everything. So the environment also
//! carries a decoy `DATABASE_URL_DEV` (and no `DATABASE_URL`) while `DATABASE_URL_PROD` is absent: every boundary
//! must still refuse, which is what proves there is no fallback. An off-PROD declaration is refused by the target
//! policy as well. Finally, a non-empty placeholder `DATABASE_URL_PROD` makes the writer construct — with no
//! connection — which proves the refusal is keyed exactly to the missing variable and not to a blanket failure.
//! The writer's `connect_env` only probes presence; it never opens a socket, so the control case is I/O-free.
//!
//! NO EXTERNAL I/O. `RuntimeHarness` scopes the process environment and restores it on drop; no connection is opened
//! and the production database is never touched. Level: L0 Pure, harness `RuntimeHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test runtime_env__004__prod_forge_execution_with_missing_database_url_prod_is_refused

use db::{resolve_forge_target, Database, DbTarget};
use forge::engine::db_writer::DbForgeStateWriter;
use test_harness::RuntimeHarness;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-RUNTIME-ENV-004); the file and the assay use it.
fn runtime_env_004__prod_forge_execution_with_missing_database_url_prod_is_refused() {
    let mut env = RuntimeHarness::acquire();
    let api = "PROD Forge execution with missing DATABASE_URL_PROD must be refused";

    // (1) The Forge execution-target policy, pure and read from its arguments: PROD is a valid Forge target, and a
    //     declaration that resolves to DEV is refused. This is the precondition that makes the rest a *Forge* run.
    assert_eq!(
        resolve_forge_target(Some("production"), None)
            .expect("PROD is a valid Forge execution target"),
        DbTarget::Prod,
    );
    assert!(
        resolve_forge_target(Some("preview"), Some("development")).is_err(),
        "Forge execution must be refused off PROD, not silently run on DEV",
    );

    // (2) The environment the runtime boundary reads: PROD declared, DATABASE_URL_PROD absent, and a decoy
    //     development URL present. If the boundary fell back, the decoy would satisfy it.
    env.set("APP_ENV", "production");
    env.remove("VERCEL_ENV");
    env.remove("DATABASE_URL_PROD");
    env.set(
        "DATABASE_URL_DEV",
        "postgres://dev-only.invalid/culebraluxe",
    );
    env.remove("DATABASE_URL");

    // THE CONTRACT, layer one: the production Forge state writer refuses to construct.
    let refusal = match DbForgeStateWriter::connect_env() {
        Ok(_) => {
            panic!("a Forge run with no DATABASE_URL_PROD must have no production state writer")
        }
        Err(message) => message,
    };
    assert!(
        refusal.contains("DATABASE_URL_PROD"),
        "{api}: the refusal must name the missing variable, got: {refusal}",
    );
    assert!(
        refusal.to_lowercase().contains("production"),
        "{api}: the refusal must say the writer is production-only, got: {refusal}",
    );

    // THE CONTRACT, layer two: the production pool Forge may use fails closed before any socket is opened.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current-thread runtime is enough for a refusal that opens no socket");
    let pool_failure = runtime
        .block_on(Database::connect_forge_from_env())
        .err()
        .expect("a PROD Forge pool with no DATABASE_URL_PROD must be refused");
    assert_eq!(
        pool_failure.operation, "db.connect",
        "the pool refusal is a configuration failure at connect time",
    );
    let detail = pool_failure.detail.unwrap_or_default();
    assert!(
        detail.contains("DATABASE_URL_PROD"),
        "{api}: the pool refusal must name the missing variable, got: {detail}",
    );
    assert!(
        detail.to_lowercase().contains("refuses to fall back"),
        "{api}: the pool refusal must fail closed rather than fall back, got: {detail}",
    );

    // CONTROL, no I/O: a non-empty DATABASE_URL_PROD makes the writer construct. The writer only probes presence, so
    // this opens no connection — and it proves the refusal above is keyed to this exact variable, not a blanket error.
    env.set(
        "DATABASE_URL_PROD",
        "postgres://placeholder.invalid/culebraluxe",
    );
    assert!(
        DbForgeStateWriter::connect_env().is_ok(),
        "a present DATABASE_URL_PROD must yield a production state writer (no connection is opened)",
    );
}
