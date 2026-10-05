//! DB.MIGRATION — migrations apply sequentially (TST-DB-MIGRATION-002).
//!
//! Contract: filename order IS dependency order. The repository enumerates migrations by
//! sorted filename and applies them in that sequence; this test proves the sequence is
//! load-bearing on a database that starts EMPTY — the ordered prefix applies, and the same
//! file applied BEFORE its dependency is refused. The shared DEV database cannot prove
//! this: 240+ applied migrations would satisfy every dependency whatever order is used.
//!
//! The ordered chain is production truth, not a fixture: `001_initial_schema` lays the
//! base tables, `002_media` builds on them, `007_guide` creates `guide_item`, and
//! `009_guide_item_media` keys off both `guide_item` and `media`. Applying 009 before 007
//! is refused with `relation "guide_item" does not exist`.
//!
//! Level: L2 Persistence — a real disposable database on the DEV cluster, created and
//! dropped by this test. The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... DATABASE_URL_UNPOOLED=... cargo test --manifest-path Cargo.toml \
//!     -p test-harness --test db_migration__002__migrations_apply_sequentially -- --ignored
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

fn migration_sql(relative: &str) -> Result<String, String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::fs::read_to_string(
        std::path::Path::new(manifest_dir)
            .join("../db/migrations")
            .join(relative),
    )
    .map_err(|error| format!("migration file {relative} reads: {error}"))
}

/// The proof body returns its verdict instead of panicking, so the caller can drop the
/// disposable database FIRST — a failed assertion must never strand a database on DEV.
async fn proof(database: &Database, namespace: &str) -> Result<(), String> {
    // 1. In sequence: 001, 002, 007, then 009 — every step succeeds.
    for file in [
        "001_initial_schema.sql",
        "002_media.sql",
        "007_guide.sql",
        "009_guide_item_media.sql",
    ] {
        let sql = migration_sql(file)?;
        database
            .run_text(&sql)
            .await
            .map_err(|error| format!("{file} applies in sequence: {error}"))?;
        // Per-file session semantics (see TST-DB-MIGRATION-001): never carry a wreck
        // or a SET forward on the reused pooled connection.
        let _ = database.run_text("ROLLBACK").await;
        let _ = database.run_text("RESET ALL").await;
    }
    let landed = migration_probe::table_exists(database, "guide_item_media")
        .await
        .map_err(|error| format!("the catalogue reads: {error}"))?;
    if !landed {
        return Err("the ordered prefix lands guide_item_media".to_string());
    }

    // 2. Out of sequence: 009 BEFORE its dependency 007 is refused — the order is
    //    load-bearing, so a runner that applied files in any order could not pass.
    let schema = format!("tstseq_{}", namespace.replace('-', "_"));
    database
        .run_text(&format!("CREATE SCHEMA \"{schema}\""))
        .await
        .map_err(|error| format!("an isolated schema is created: {error}"))?;
    let scoped = format!(
        "SET search_path TO \"{schema}\";\n{}",
        migration_sql("009_guide_item_media.sql")?
    );
    if database.run_text(&scoped).await.is_ok() {
        return Err(
            "009 before 007 is refused — sequence is load-bearing, not incidental".to_string(),
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV + DATABASE_URL_UNPOOLED (a disposable DEV cluster); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-MIGRATION-002); the file and the assay use it.
async fn db_migration_002__migrations_apply_sequentially() {
    // 0. L2 boundary: an isolated disposable DEV-cluster target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the sequence proof runs only on an isolated DEV target"
    );
    db::disable_statement_timeout();

    // 1. Enumeration order is numeric order: the sequence production applies is the
    //    sequence the numbers declare. (Duplicate numbers are TST-DB-MIGRATION-005's
    //    subject; here the bar is only that no file sorts BEFORE an earlier number,
    //    which would silently reorder the chain.)
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let migrations_dir = std::path::Path::new(manifest_dir).join("../db/migrations");
    let mut files: Vec<String> = std::fs::read_dir(&migrations_dir)
        .expect("the migrations directory reads")
        .filter_map(|entry| {
            let name = entry.expect("a directory entry reads").file_name();
            let name = name.to_str().expect("a utf-8 filename").to_string();
            name.ends_with(".sql").then_some(name)
        })
        .collect();
    files.sort();
    let numbers: Vec<u32> = files
        .iter()
        .map(|file| {
            file.split('_')
                .next()
                .expect("a numeric prefix")
                .parse()
                .unwrap_or_else(|_| panic!("numeric prefix parses: {file}"))
        })
        .collect();
    for pair in numbers.windows(2) {
        assert!(
            pair[0] <= pair[1],
            "{HARNESS}: filename order is numeric order"
        );
    }

    // 2. A database that starts EMPTY; the verdict is collected before the drop so a
    //    red proof still leaves DEV as it found it.
    let db_name = migration_probe::create_fresh_db(dev.namespace())
        .await
        .expect("a disposable empty database is created");
    let database = migration_probe::connect_fresh_db(&db_name)
        .await
        .expect("the production Database type connects to the empty database");
    let outcome = proof(&database, dev.namespace()).await;
    drop(database);
    migration_probe::drop_fresh_db(&db_name)
        .await
        .expect("the disposable database is dropped");
    if let Err(report) = outcome {
        panic!("{HARNESS}: {report}");
    }
}
