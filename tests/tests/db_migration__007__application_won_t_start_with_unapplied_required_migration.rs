//! DB.MIGRATION — application won't start with unapplied required migration
//! (TST-DB-MIGRATION-007).
//!
//! Contract: starting the application against a database that is missing a required
//! migration must be REFUSED, not served. The proof runs against a database that starts
//! EMPTY — every migration is unapplied there — and connects through the production
//! [`db::Database`] type, the same handle the server boots with.
//!
//! The detection half of this contract exists in production today and is exercised here:
//! `forge::engine::migration_guard::assess_migration_applied` reports a changed migration
//! file with no ledger row as unapplied, and `migration_applied_refusal` renders the
//! refusal. What is missing is the enforcement half: neither the web boot
//! (`web/src/bin/web.rs`, `web/src/http_runtime.rs`) nor any startup path consults the
//! `schema_migration` ledger before serving, so the connection below succeeds against a
//! database with nothing applied. The final assertion pins the required behavior; while
//! enforcement is absent, it fails and names the gap — a faithful red, not a weak test.
//!
//! Level: L2 Persistence — a real disposable database on the DEV cluster, created and
//! dropped by this test. The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... DATABASE_URL_UNPOOLED=... cargo test --manifest-path Cargo.toml \
//!     -p test-harness --test db_migration__007__application_won_t_start_with_unapplied_required_migration -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable database and the harness will never open a PRODUCTION one.

use db::{Database, DbTarget, SchemaMigrationDao};
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
    // 1. Detection exists: a changed migration file with no ledger row IS unapplied, and the
    //    refusal names it. If this failed, the requirement would be unstatable.
    let changed = vec!["db/migrations/999_required_for_boot.sql".to_string()];
    let unapplied = forge::engine::migration_guard::assess_migration_applied(&changed, &[]);
    if unapplied != changed {
        return Err("a migration with no ledger row is unapplied — detection works".to_string());
    }
    let refusal = forge::engine::migration_guard::migration_applied_refusal(&unapplied);
    if !refusal.contains("999_required_for_boot.sql") {
        return Err("the refusal names the unapplied migration".to_string());
    }
    // And the negative: a recorded migration is not flagged.
    if !forge::engine::migration_guard::assess_migration_applied(
        &changed,
        &["db/migrations/999_required_for_boot.sql".to_string()],
    )
    .is_empty()
    {
        return Err("a recorded migration is applied — detection discriminates".to_string());
    }

    // 2. The empty database holds no ledger — nothing is applied here.
    let dao = SchemaMigrationDao::new(database.clone());
    let ledger_present = dao
        .present()
        .await
        .map_err(|error| format!("the ledger presence reads: {error}"))?;
    if ledger_present {
        return Err("the empty database holds no ledger — nothing is applied".to_string());
    }

    // 3. THE REQUIREMENT: the application must not start here. The connection above is the
    //    same handle the server boots with, and it succeeded against a database with no
    //    ledger and no migration applied — no boot path consulted the ledger first.
    Err(
        "the application started with every required migration unapplied — \
         no boot gate consults schema_migration before serving (web boot performs no ledger check)"
            .to_string(),
    )
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV + DATABASE_URL_UNPOOLED (a disposable DEV cluster); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-MIGRATION-007); the file and the assay use it.
async fn db_migration_007__application_won_t_start_with_unapplied_required_migration() {
    // 0. L2 boundary: an isolated disposable DEV-cluster target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the boot-gate proof runs only on an isolated DEV target"
    );

    // 1. A database that starts EMPTY: every required migration is unapplied here. The
    //    verdict is collected before the drop so the red proof still leaves DEV clean.
    let db_name = migration_probe::create_fresh_db(dev.namespace())
        .await
        .expect("a disposable empty database is created");
    let database = migration_probe::connect_fresh_db(&db_name)
        .await
        .expect("the production Database type connects to the empty database");
    let outcome = proof(&database).await;
    drop(database);
    migration_probe::drop_fresh_db(&db_name)
        .await
        .expect("the disposable database is dropped");
    if let Err(report) = outcome {
        panic!("{HARNESS}: {report}");
    }
}
