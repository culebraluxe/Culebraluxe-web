//! DB.MIGRATION — application won't start with unapplied required migration
//! (TST-DB-MIGRATION-007).
//!
//! Contract: starting the application against a database that is missing a required
//! migration must be REFUSED, not served. The proof runs against a database that starts
//! EMPTY — every migration is unapplied there — and connects through the production
//! [`db::Database`] type, the same handle the server boots with.
//!
//! The detection half of this contract exists in production and is exercised here:
//! `forge::engine::migration_guard::assess_migration_applied` reports a changed migration
//! file with no ledger row as unapplied, and `migration_applied_refusal` renders the
//! refusal. The enforcement half is `db::boot_gate::assert_boot_ready` — called by
//! `web/src/http_runtime.rs` before it binds a socket — so a database whose ledger does not
//! name every required migration is refused by name, and the refused process leaves an
//! `app_error` row behind through the capture framework.
//!
//! The proof runs against a database that starts EMPTY and then writes a ledger of its own:
//! refused with nothing applied, refused by name when one required row is missing, accepted
//! when every required file is recorded. That is what keeps the gate honest — a gate that
//! only ever refuses would pass a single-assertion test vacuously.
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

    // 3. THE REQUIREMENT, NOW ENFORCED: the boot gate refuses this database. `web/src/http_runtime.rs`
    //    calls `db::assert_boot_ready` before it binds a socket, so the same handle the server boots with
    //    can no longer be served a schema-less database. The refusal must name what is missing.
    let refusal = match db::assert_boot_ready(database).await {
        Ok(()) => {
            return Err(
                "the boot gate accepted a database with every required migration unapplied — \
                 a gate that never refuses is not a gate"
                    .to_string(),
            )
        }
        Err(failure) => failure,
    };
    if refusal.kind != db::DbFailureKind::SchemaMismatch {
        return Err(format!(
            "a boot refusal is a SchemaMismatch, not {:?}",
            refusal.kind
        ));
    }
    let refusal_text = refusal.to_string();
    if !refusal_text.contains("269_forge_work_queue.sql") {
        return Err(format!(
            "the refusal names an unapplied required migration: {refusal_text}"
        ));
    }
    if !refusal_text.contains("unapplied on dev") {
        return Err(format!(
            "the refusal names the target it refused (the gate reads db.target()): {refusal_text}"
        ));
    }

    // 4. DISCRIMINATION — the same database, one ledger row away from serving. A gate that only ever
    //    refuses would pass step 3 vacuously, so the ledger is built here from its own migration (the file
    //    production applies) and filled through the production DAO, with every required migration recorded
    //    EXCEPT the last: the gate must refuse, and must name exactly that one.
    let ledger_sql = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../db/migrations/144_schema_migration_ledger.sql"),
    )
    .map_err(|error| format!("the ledger migration reads: {error}"))?;
    database
        .run_text(&ledger_sql)
        .await
        .map_err(|error| format!("the ledger migration applies: {error}"))?;
    let ledger = SchemaMigrationDao::new(database.clone());
    let (last, recorded) = db::REQUIRED_BOOT_MIGRATIONS
        .split_last()
        .ok_or_else(|| "the required-migration list is not empty".to_string())?;
    let last: &str = *last;
    for name in recorded.iter().copied() {
        ledger
            .record(
                name,
                "sha256:db_migration__007",
                DbTarget::Dev,
                Some("db_migration__007 proof"),
            )
            .await
            .map_err(|error| format!("recording {name} reads: {error}"))?;
    }
    match db::assert_boot_ready(database).await {
        Ok(()) => {
            return Err(format!(
                "a required migration with no ledger row ({last}) was accepted — the gate is blind to a gap"
            ))
        }
        Err(failure) => {
            let text = failure.to_string();
            if !text.contains(last) {
                return Err(format!(
                    "the refusal names the one missing migration {last}: {text}"
                ));
            }
            if text.contains(recorded[0]) {
                return Err(format!(
                    "the refusal names only what is missing, not what is recorded ({0} is recorded): {text}",
                    recorded[0]
                ));
            }
        }
    }

    // 5. THE POSITIVE: record the last one and the same build boots. Together with step 3 this proves the
    //    gate discriminates — refused with nothing applied, refused by name when one row is missing,
    //    accepted when the ledger names every required file.
    ledger
        .record(
            last,
            "sha256:db_migration__007",
            DbTarget::Dev,
            Some("db_migration__007 proof"),
        )
        .await
        .map_err(|error| format!("recording {last} reads: {error}"))?;
    if let Err(failure) = db::assert_boot_ready(database).await {
        return Err(format!(
            "a database that records every required migration passes the gate: {failure}"
        ));
    }
    Ok(())
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
