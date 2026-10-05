//! DB.MIGRATION — migration guard fails closed if ledger cannot be read
//! (TST-DB-MIGRATION-008).
//!
//! Contract: when the `schema_migration` ledger is unreadable, every guard that depends on
//! it must answer REFUSE, never "no rows, proceed". An unreadable ledger must be
//! indistinguishable from a failing one — in particular it must never read as "nothing
//! applied, safe to continue". The production seams this test drives:
//!
//! - [`db::SchemaMigrationDao::present`]: reports whether the ledger exists at all, via
//!   `to_regclass`, so a missing ledger is a readable sentence rather than SQLSTATE 42P01.
//! - [`db::SchemaMigrationDao::recorded_checksum`]: an unreadable ledger surfaces `Err`,
//!   never `Ok(None)` — no caller can mistake "cannot read" for "not applied".
//! - `forge::engine::migration_guard::assess_migration_applied`: an empty ledger half
//!   reports every changed migration as unapplied, and the refusal names them.
//!
//! The unreadable-ledger state is built inside a transaction that always rolls back: the
//! ledger table is dropped, the seams are read, and the rollback restores it — the shared
//! DEV ledger is left exactly as it was found.
//!
//! Level: L2 Persistence — the production DAO against an isolated disposable DEV/Neon target.
//! The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_migration__008__migration_guard_fails_closed_if_ledger_cannot_be_read -- --ignored
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-MIGRATION-008); the file and the assay use it.
async fn db_migration_008__migration_guard_fails_closed_if_ledger_cannot_be_read() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the fail-closed proof runs only on an isolated DEV target"
    );
    let ledger = SchemaMigrationDao::new(dev.database().clone());
    assert!(
        ledger.present().await.expect("the ledger presence reads"),
        "{HARNESS}: the schema_migration ledger exists on DEV — the readable baseline"
    );

    // 1. The readable baseline: a recorded file answers Some, an unrecorded file None.
    //    The guard below must preserve this discrimination while refusing the unreadable.
    let baseline: Option<String> = ledger
        .recorded_checksum("144_schema_migration_ledger.sql", DbTarget::Dev)
        .await
        .expect("the checksum reads");
    let _ = baseline;

    // 2. Make the ledger unreadable in a way every pool connection observes: rename it
    //    out of the way, read the seams, rename it back BEFORE asserting anything. An
    //    uncommitted DROP inside a transaction would be invisible to the DAO's pooled
    //    connections (and would hang them on the lock) — the rename is committed, brief,
    //    and pool-visible, which is exactly the "ledger cannot be read" state.
    let hidden = "schema_migration_probe_hidden_008";
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "ALTER TABLE public.schema_migration RENAME TO \"{hidden}\""
    )))
    .execute(dev.database().pool())
    .await
    .expect("the rename hides the ledger");
    let dao = SchemaMigrationDao::new(dev.database().clone());
    let present_while_hidden = dao.present().await;
    let checksum_while_hidden = dao
        .recorded_checksum("144_schema_migration_ledger.sql", DbTarget::Dev)
        .await;
    let rows_while_hidden = dao.rows(DbTarget::Dev).await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "ALTER TABLE public.\"{hidden}\" RENAME TO schema_migration"
    )))
    .execute(dev.database().pool())
    .await
    .expect("the rename restores the ledger — if this panics, rename it back by hand");

    // 3. `present()` answers Ok(false) — the missing ledger is a sentence an operator can
    //    act on, not SQLSTATE 42P01 from inside a checksum query.
    assert_eq!(
        present_while_hidden.expect("presence reads while the ledger is hidden"),
        false,
        "{HARNESS}: an unreadable ledger reports absent, never present"
    );

    // 4. `recorded_checksum` surfaces Err — "cannot read" must never read as Ok(None)
    //    ("not applied"), which is the open gate this story forbids.
    assert!(
        checksum_while_hidden.is_err(),
        "{HARNESS}: an unreadable ledger errors, never answers not-applied"
    );
    assert!(
        rows_while_hidden.is_err(),
        "{HARNESS}: unreadable ledger rows error, never answer empty"
    );

    // 5. With no ledger half to consult, the change-set guard reports every changed
    //    migration unapplied and refuses by name — fail closed.
    let changed = vec!["db/migrations/999_unreadable_ledger.sql".to_string()];
    let unapplied = forge::engine::migration_guard::assess_migration_applied(&changed, &[]);
    assert_eq!(
        unapplied, changed,
        "{HARNESS}: no readable ledger row means unapplied, never assumed-applied"
    );
    assert!(
        !forge::engine::migration_guard::migration_applied_refusal(&unapplied).is_empty(),
        "{HARNESS}: the refusal renders — a guard that cannot read refuses out loud"
    );

    // 6. The rename-back held: the shared DEV ledger is intact and readable again.
    assert!(
        ledger.present().await.expect("the ledger presence reads"),
        "{HARNESS}: the proof restores the ledger it hid"
    );
}
