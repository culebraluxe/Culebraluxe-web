//! DB.SCHEMA — trigger/function referenced by code exists (TST-DB-SCHEMA-008).
//!
//! CONTRACT. A database routine that the production code names and invokes must
//! exist in the schema that code runs against. The Rust DAO layer calls database
//! functions by name (`select … from forge_dispatch_story($1)` in `db/src/forge_engine.rs`)
//! and depends on the triggers those routines fire; a routine that was renamed or
//! dropped on the database side is a runtime `42704` in production. This test turns
//! that class of drift into a named, local proof.
//!
//! Each routine is checked twice, so the list cannot rot into a hardcoded fiction:
//!   1. SOURCE — a repository file still names the routine, so a rename in code is
//!      caught here rather than only after the database was changed, and
//!   2. CATALOGUE — the isolated DEV target reports it present (`pg_proc` for a
//!      function, `pg_trigger` for a trigger).
//!
//! The test then exercises the subject rather than only reading static lists:
//! it creates a probe function inside a transaction, proves `pg_proc` sees it, rolls
//! the transaction back, and proves the committed catalogue no longer holds it. The
//! negative control asserts a fabricated routine name is absent from both catalogues,
//! so a query that answered "present" for everything — or one that ignored rollback —
//! cannot pass.
//!
//! Level: L2 Persistence — the production `Database` pool against an isolated
//! disposable DEV/Neon target. The harness refuses PRODUCTION before any socket is
//! opened, and the only write is a probe function rolled back inside the test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_schema__008__trigger_function_referenced_by_code_exists -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof
//! needs a disposable DEV database and the harness will never open a PRODUCTION one.

use std::path::{Path, PathBuf};

use db::DbTarget;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// The repository root, derived from this crate's manifest directory (`tests/`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests/ has a parent (the repository root)")
        .to_path_buf()
}

/// Functions the production code invokes by name: (routine, evidence path).
///
/// The evidence file must name the routine; that source check is what keeps the list
/// tied to code instead of to this test's memory of the schema.
const CODE_REFERENCED_FUNCTIONS: &[(&str, &str)] = &[
    // The dispatch trigger's function, named by `db/src/forge_engine.rs` in the
    // doc comment that explains the `42704` schema-mismatch path.
    ("agent_work_item_dispatch", "db/src/forge_engine.rs"),
    ("forge_begin_agent_work_run", "db/src/forge_engine.rs"),
    ("forge_claim_next_agent_work", "db/src/forge_engine.rs"),
    ("forge_claim_specific_agent_work", "db/src/forge_engine.rs"),
    ("forge_dispatch_story", "db/src/forge_engine.rs"),
    ("forge_finish_agent_work_run", "db/src/forge_engine.rs"),
    ("forge_hold_stale_work", "db/src/forge_control.rs"),
    ("forge_reconcile_dispatch_queue", "db/src/forge_engine.rs"),
    ("forge_record_tool_artifact", "db/src/forge_engine.rs"),
    ("forge_recover_stale_engine_claim", "db/src/forge_reset.rs"),
    (
        "forge_reject_agent_work_configuration",
        "db/src/forge_engine.rs",
    ),
    ("forge_requeue_stale_work", "db/src/forge_control.rs"),
    ("warehouse_promote_apple_contacts", "db/src/landing.rs"),
];

/// Triggers the code path depends on: (trigger, evidence path).
///
/// `storyboard_story_ready_dispatch` is the Ready-dispatch trigger the migration
/// suite installs; `forge_dispatch_story` restores the change into `Ready` that fires
/// it and returns `42704` if it is gone, so a deployment without it silently stops
/// queuing work.
const CODE_REFERENCED_TRIGGERS: &[(&str, &str)] = &[(
    "storyboard_story_ready_dispatch",
    "db/migrations/025_agent_work_queue.sql",
)];

/// Does the named evidence file still name the routine?
///
/// A missing file, or a file that no longer contains the name, fails the source half
/// of the contract; the caller records it as missing.
fn source_names(evidence: &str, routine: &str) -> bool {
    std::fs::read_to_string(repo_root().join(evidence))
        .map(|text| text.contains(routine))
        .unwrap_or(false)
}

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// Infrastructure, not the contract: `TestDatabase` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> TestDatabase {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match TestDatabase::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; the harness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-SCHEMA-008); the file and the assay use it.
async fn db_schema_008__trigger_function_referenced_by_code_exists() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the routine-existence proof runs only on an isolated DEV target"
    );
    let pool = dev.database().pool();

    // 1. Negative control: fabricated names are absent from both catalogues, so a
    //    lookup that answered "present" for everything could not pass this test.
    let fabricated_function: (i64,) =
        sqlx::query_as("select count(*) from pg_proc where proname = $1")
            .bind("tst_db_schema_008_no_such_function")
            .fetch_one(pool)
            .await
            .expect("count fabricated function");
    assert_eq!(
        fabricated_function.0, 0,
        "{HARNESS}: a fabricated function is absent — the pg_proc lookup discriminates"
    );

    let fabricated_trigger: (i64,) =
        sqlx::query_as("select count(*) from pg_trigger where tgname = $1")
            .bind("tst_db_schema_008_no_such_trigger")
            .fetch_one(pool)
            .await
            .expect("count fabricated trigger");
    assert_eq!(
        fabricated_trigger.0, 0,
        "{HARNESS}: a fabricated trigger is absent — the pg_trigger lookup discriminates"
    );

    // 2. Exercise the catalogue and the rollback contract together: a function that
    //    exists is visible, and one created inside a transaction that is rolled back
    //    is not. This is the positive control against which the fabricated-name
    //    negative control above is meaningful.
    let namespace = dev.namespace().replace('-', "_");
    let probe = format!("tst_db_schema_008_probe_{namespace}");
    let ddl = format!("create function {probe}() returns integer language sql as 'select 1'");

    let mut tx = dev.begin().await.expect("begin probe transaction");
    sqlx::query(sqlx::AssertSqlSafe(ddl))
        .execute(tx.connection())
        .await
        .expect("create probe function");
    let inside: (i64,) = sqlx::query_as("select count(*) from pg_proc where proname = $1")
        .bind(&probe)
        .fetch_one(tx.connection())
        .await
        .expect("probe visible inside its transaction");
    assert_eq!(
        inside.0, 1,
        "{HARNESS}: a function created in this transaction must be visible in pg_proc"
    );
    tx.rollback()
        .await
        .expect("roll back the probe transaction");

    let after: (i64,) = sqlx::query_as("select count(*) from pg_proc where proname = $1")
        .bind(&probe)
        .fetch_one(pool)
        .await
        .expect("probe absent after rollback");
    assert_eq!(
        after.0, 0,
        "{HARNESS}: rollback must remove the probe from the committed schema"
    );

    // 3. Every function the code references exists, and is still referenced by code.
    let mut missing: Vec<String> = Vec::new();
    for (routine, evidence) in CODE_REFERENCED_FUNCTIONS {
        if !source_names(evidence, routine) {
            missing.push(format!("{routine}: no longer named in {evidence}"));
            continue;
        }
        let present: (i64,) = sqlx::query_as("select count(*) from pg_proc where proname = $1")
            .bind(*routine)
            .fetch_one(pool)
            .await
            .expect("count referenced function");
        if present.0 == 0 {
            missing.push(format!(
                "{routine}: referenced by {evidence} but absent from pg_proc"
            ));
        }
    }

    // 4. Every trigger the code path depends on exists.
    for (trigger, evidence) in CODE_REFERENCED_TRIGGERS {
        if !source_names(evidence, trigger) {
            missing.push(format!("{trigger}: no longer named in {evidence}"));
            continue;
        }
        let present: (i64,) = sqlx::query_as("select count(*) from pg_trigger where tgname = $1")
            .bind(*trigger)
            .fetch_one(pool)
            .await
            .expect("count referenced trigger");
        if present.0 == 0 {
            missing.push(format!(
                "{trigger}: referenced by {evidence} but absent from pg_trigger"
            ));
        }
    }

    assert!(
        missing.is_empty(),
        "{HARNESS}: code-referenced database routines missing on DEV:\n  {}",
        missing.join("\n  ")
    );
}
