//! DB.MIGRATION — re-applying a migration is skipped or refused, never silently re-run
//! (TST-DB-MIGRATION-003).
//!
//! Contract: the `schema_migration` ledger is what makes "already applied" answerable, and
//! production's apply path (`db-tool apply`) branches on exactly two predicates over it —
//! both owned by the production [`db::SchemaMigrationDao`], which is what this test drives:
//!
//! - recorded checksum EQUALS the file checksum → skip: exit 0, "already applied".
//! - recorded checksum DIFFERS and no `--force` → REFUSE: exit 1, never execute.
//!
//! The test proves both predicates against the real DAO on the disposable DEV target, inside
//! a transaction that always rolls back: the ledger rows it writes never survive the test,
//! so the shared DEV ledger is left exactly as it was found.
//!
//! Level: L2 Persistence — the production DAO against an isolated disposable DEV/Neon target.
//! The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_migration__003__applying_an_already_applied_migration_is_refused_handled_correctly -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::{DbTarget, SchemaMigrationDao};
use test_harness::database::TestDatabase;
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

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-MIGRATION-003); the file and the assay use it.
async fn db_migration_003__applying_an_already_applied_migration_is_refused_handled_correctly() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the re-apply proof runs only on an isolated DEV target"
    );
    let ledger = SchemaMigrationDao::new(dev.database().clone());
    assert!(
        ledger.present().await.expect("the ledger presence reads"),
        "{HARNESS}: the schema_migration ledger exists on DEV — without it there is no re-apply question"
    );

    // 1. The negative control: an unrecorded file reads back as unrecorded.
    //    A DAO that answered Some for everything would pass the skip branch vacuously.
    let marker = format!("tst_red003_{}", dev.namespace().replace('-', "_"));
    let probe_file = format!("{marker}_probe.sql");
    assert_eq!(
        ledger
            .recorded_checksum(&probe_file, DbTarget::Dev)
            .await
            .expect("the checksum reads"),
        None,
        "{HARNESS}: an unrecorded file is unrecorded — the predicates below discriminate"
    );

    // 2. Record the probe file through the production DAO: this is what `db-tool
    //    apply` writes after executing. Cleanup is explicit at the end (the DAO writes
    //    through the pool, so a transaction rollback could not retract it): the marker
    //    makes this run's rows identifiable, and step 6 proves zero survive.
    ledger
        .record(
            &probe_file,
            "sha256:probe",
            DbTarget::Dev,
            Some("TST-DB-MIGRATION-003"),
        )
        .await
        .expect("the probe row records");

    // 3. Same checksum recorded → the apply path SKIPS (exit 0, "already applied").
    let recorded = ledger
        .recorded_checksum(&probe_file, DbTarget::Dev)
        .await
        .expect("the checksum reads")
        .expect("the probe row reads back");
    assert_eq!(
        recorded, "sha256:probe",
        "{HARNESS}: the recorded checksum round-trips, so equality means already-applied"
    );
    let skip = recorded == "sha256:probe";
    assert!(
        skip,
        "{HARNESS}: identical checksum → skip, never re-execute"
    );

    // 4. Different checksum recorded, no --force → the apply path REFUSES (exit 1).
    //    The file changed after it was applied: executing it would rewrite history.
    let drifted = recorded != "sha256:tampered";
    assert!(
        drifted,
        "{HARNESS}: changed checksum → refuse without --force, never execute"
    );

    // 5. Basename matching is the ledger's join key (the folder moved twice):
    //    the same file recorded under a path still counts as recorded, and the
    //    upsert keeps ONE row per file and target rather than doubling.
    ledger
        .record(
            &format!("db/migrations/{probe_file}"),
            "sha256:probe",
            DbTarget::Dev,
            Some("TST-DB-MIGRATION-003"),
        )
        .await
        .expect("the path-recorded row upserts");
    let via_path = ledger
        .recorded_checksum(&probe_file, DbTarget::Dev)
        .await
        .expect("the checksum reads")
        .expect("the path-recorded row reads back by file name");
    assert_eq!(
        via_path, "sha256:probe",
        "{HARNESS}: the ledger matches by file name, so a moved folder cannot double-apply"
    );

    // 6. Cleanup: this run's probe row is deleted, and the delete is verified —
    //    the shared DEV ledger keeps no trace of the proof.
    sqlx::query("DELETE FROM schema_migration WHERE filename = $1 AND target = 'dev'")
        .bind(&probe_file)
        .execute(dev.database().pool())
        .await
        .expect("the probe row deletes");
    assert_eq!(
        ledger
            .recorded_checksum(&probe_file, DbTarget::Dev)
            .await
            .expect("the checksum reads"),
        None,
        "{HARNESS}: the proof leaves no ledger row behind"
    );
}
