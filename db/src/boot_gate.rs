//! The boot gate — an application must not serve from a database whose required migrations are unapplied.
//!
//! WHY THIS EXISTS. `schema_migration` says what has been applied where, and nothing consulted it before
//! serving: `db_migration__007` (TST-DB-MIGRATION-007, measured 2026-10-08) connected the production
//! [`Database`] handle to a database with no schema at all and found the boot path content to serve it. Every
//! symptom of that state arrives as an outage with no named cause — a 500 on the first screen that touches a
//! missing column, an engine whose claims land in a table nobody created. A refusal at boot names the file
//! and the target instead, before a socket is opened.
//!
//! WHAT IT REQUIRES, AND WHY THE LIST IS CURATED RATHER THAN GENERATED. The ledger is authoritative only
//! from the 2026-09-10 baseline forward (`db_migration__004`): most files below that line were applied by
//! hand before the ledger existed, so "every file under `db/migrations/`" is not a set either live database
//! can satisfy, and a gate built on it would refuse to boot a healthy system. The list below is the tail the
//! running code actually depends on — Forge's work queue, worker heartbeat, runtime control, claim brake and
//! the native document-signing surface — and every name in it is recorded on both DEV and PROD as of
//! 2026-10-08, so a healthy deploy passes and an empty one is refused. A migration added later is not
//! required until somebody adds it here, deliberately: this is a floor, not a ceiling.
//!
//! `000_forge_base_schema.sql` is absent on purpose. It recreates the hand-built Forge base (see its own
//! header) and is idempotent, so it is a no-op on any database that already holds a ledger — and a database
//! whose ledger records 144 already has those tables, because 108-113 shipped long before it.
//!
//! The gate is pure: it reads the ledger, reports what is missing, and writes nothing. Skipping it is a
//! decision the booting process makes, out loud, in `web/src/http_runtime.rs` — not something this module
//! decides for it.

use crate::{Database, DbFailure, DbResult, SchemaMigrationDao};

/// The migrations whose absence means the booted code does not match the database it was pointed at.
///
/// Every name here is recorded on DEV and PROD (verified 2026-10-08). Add a name when the code being
/// deployed cannot run without the file; do not add one merely because it is new.
pub const REQUIRED_BOOT_MIGRATIONS: &[&str] = &[
    "260_docsign_native.sql",
    "261_docsign_entitlements.sql",
    "262_forge_agent_work_claim.sql",
    "263_forge_agent_work_settlement.sql",
    "264_forge_agent_work_begin.sql",
    "265_forge_dispatch_reconcile.sql",
    "266_forge_stale_recovery.sql",
    "267_forge_tool_artifact_write.sql",
    "268_forge_staged_item_follows_story_status.sql",
    "269_forge_work_queue.sql",
    "270_forge_work_queue_inlet.sql",
    "271_forge_work_queue_event_settle.sql",
    "272_forge_work_queue_event_payload.sql",
    "273_forge_worker_heartbeat.sql",
    "274_forge_runtime_control.sql",
    "275_forge_claim_story_brake.sql",
    "276_docsign_copy_and_reminders.sql",
];

/// The required migrations that hold no ledger row for this database's declared target, in list order.
///
/// An absent ledger is reported as EVERY required migration being unapplied, because that is what it means:
/// a database with no `schema_migration` has nothing applied that the ledger would have recorded.
pub async fn missing_required_migrations(database: &Database) -> DbResult<Vec<String>> {
    let ledger = SchemaMigrationDao::new(database.clone());
    if !ledger.present().await? {
        return Ok(REQUIRED_BOOT_MIGRATIONS
            .iter()
            .map(|name| (*name).to_string())
            .collect());
    }
    let target = database.target();
    let mut missing = Vec::new();
    for name in REQUIRED_BOOT_MIGRATIONS {
        if ledger.recorded_checksum(name, target).await?.is_none() {
            missing.push((*name).to_string());
        }
    }
    Ok(missing)
}

/// Refuse to start when a required migration is unapplied on this database's declared target.
///
/// `Ok(())` means the ledger names every required file — the application and the database agree about the
/// schema. `Err` is a [`DbFailure`] of kind `SchemaMismatch`, naming the target and each missing file, and it
/// is announced through `db::capture` from its constructor like every other `DbFailure`, so a refused boot
/// leaves a row behind rather than a line on a terminal nobody reads.
pub async fn assert_boot_ready(database: &Database) -> DbResult<()> {
    let target = database.target();
    let missing = missing_required_migrations(database).await?;
    if missing.is_empty() {
        return Ok(());
    }
    Err(DbFailure::schema_mismatch(
        "db.boot_gate",
        format!(
            "{} of {} required migration(s) are unapplied on {}: {}. The application refuses to serve a \
             database that does not match it; apply them with \
             `cargo run -p cli -- db-tool apply <file> {}` and start again.",
            missing.len(),
            REQUIRED_BOOT_MIGRATIONS.len(),
            target.as_str(),
            missing.join(", "),
            target.as_str()
        ),
    ))
}
