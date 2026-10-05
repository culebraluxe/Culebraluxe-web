//! DB.MIGRATION — migrations apply cleanly from empty schema (TST-DB-MIGRATION-001).
//!
//! Contract: every migration file in `db/migrations/` applies without error on a database
//! that starts EMPTY, in filename order, through the same execution path production uses
//! (`Database::run_text` — one simple-protocol query per file, exactly what `db-tool apply`
//! runs). An empty database is the only honest baseline: the shared DEV database already
//! carries 240+ applied migrations, which would hide a file that only works because an
//! earlier era built its dependencies by hand.
//!
//! Level: L2 Persistence — a real disposable database on the DEV cluster, created and
//! dropped by this test. The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... DATABASE_URL_UNPOOLED=... cargo test --manifest-path Cargo.toml \
//!     -p test-harness --test db_migration__001__migrations_apply_cleanly_from_empty_schema -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable database and the harness will never open a PRODUCTION one.

use db::{Database, DbTarget};
use test_harness::database::TestDatabase;
use test_harness::migration_probe;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

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

/// The proof body returns its verdict instead of panicking, so the caller can drop the
/// disposable database FIRST — a failed assertion must never strand a database on DEV.
async fn proof(database: &Database) -> Result<(), String> {
    // 1. The negative control FIRST: the production execution path reports failure.
    //    A runner that swallowed errors would pass the replay below vacuously.
    if database
        .run_text("CREATE TABLE probe_broken (id uuid PRIMARY KEY, id text);")
        .await
        .is_ok()
    {
        return Err(
            "run_text refuses a broken statement, so a clean replay below means clean SQL"
                .to_string(),
        );
    }

    // 2. Every migration file, in the same order production enumerates, through run_text.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let migrations_dir = std::path::Path::new(manifest_dir).join("../db/migrations");
    let entries = std::fs::read_dir(&migrations_dir)
        .map_err(|error| format!("the migrations directory reads: {error}"))?;
    let mut files: Vec<String> = Vec::new();
    for entry in entries {
        let name = entry
            .map_err(|error| format!("a directory entry reads: {error}"))?
            .file_name();
        let name = name
            .to_str()
            .ok_or_else(|| "a utf-8 filename".to_string())?
            .to_string();
        if name.ends_with(".sql") {
            files.push(name);
        }
    }
    files.sort();
    if files.len() < 200 {
        return Err(format!(
            "the chain under test is the whole estate, found {} files",
            files.len()
        ));
    }

    let mut failures: Vec<String> = Vec::new();
    for file in &files {
        let sql = std::fs::read_to_string(migrations_dir.join(file))
            .map_err(|error| format!("a migration file reads: {error}"))?;
        if let Err(error) = database.run_text(&sql).await {
            failures.push(format!("{file}: {error}"));
        }
        // Per-file session semantics (the `psql -f` equivalence production relies on):
        // the pool reuses one connection, so a failed file's aborted transaction — or a
        // file's `SET` — must not leak into the next file. No migration leaves a real
        // transaction open on success (unbalanced `begin`s are plpgsql bodies), so ending
        // any wreck and resetting session state cannot discard committed work.
        let _ = database.run_text("ROLLBACK").await;
        let _ = database.run_text("RESET ALL").await;
    }
    if !failures.is_empty() {
        return Err(format!(
            "{} of {} migration files fail from an empty schema:\n  {}",
            failures.len(),
            files.len(),
            failures.join("\n  ")
        ));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV + DATABASE_URL_UNPOOLED (a disposable DEV cluster); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-MIGRATION-001); the file and the assay use it.
async fn db_migration_001__migrations_apply_cleanly_from_empty_schema() {
    // 0. L2 boundary: an isolated disposable DEV-cluster target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the empty-schema replay runs only on an isolated DEV target"
    );
    // Index builds on 241 files may legitimately take longer than a request statement:
    // the operator's own path (`db-tool`) lifts the ceiling for exactly this work.
    db::disable_statement_timeout();

    // 1. A database that starts EMPTY — no baseline, no carried-forward tables. The
    //    verdict is collected before the drop so a red replay still leaves DEV clean.
    let db_name = migration_probe::create_fresh_db(dev.namespace())
        .await
        .expect("a disposable empty database is created");
    let database = migration_probe::connect_fresh_db(&db_name)
        .await
        .expect("the production Database type connects to the empty database");
    assert_eq!(
        database.target(),
        DbTarget::Dev,
        "{HARNESS}: the replay handle is a DEV handle"
    );
    let outcome = proof(&database).await;
    drop(database);
    migration_probe::drop_fresh_db(&db_name)
        .await
        .expect("the disposable database is dropped");
    if let Err(report) = outcome {
        panic!("{HARNESS}: {report}");
    }
}
