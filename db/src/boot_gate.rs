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
//! the native luxesign surface and the storyboard trajectory table — and every name in it is recorded on
//! both DEV and PROD (the 260-276 set as of 2026-10-08, 292-293 as of 2026-10-11, and 294-295 as of
//! 2026-10-19 read-only ledger checks), so a healthy deploy passes
//! and an empty one is refused. A migration added later is not required until somebody adds it here,
//! deliberately: this is a floor, not a ceiling.
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
/// Every name here is recorded on BOTH targets — measured 2026-10-12: `pnpm db:migrations` lists 292 and 293 for
/// dev and prod (DEV 2026-10-11 09:40Z / 19:25Z, PROD 2026-10-11 11:57Z / 20:19Z) and `pnpm db:parity` is clean.
/// 294 and 295 are present on both targets too (read-only `schema_migration` ledger query, 2026-10-19).
/// 293 is listed because the code below it writes the columns it adds and no longer writes the key 292 defined:
/// a binary carrying this list must not serve a database that still holds 292's shape, and the fix is the one the
/// refusal message names — apply 293 to the target, then start. PROD met that requirement on 2026-10-11 20:19Z,
/// before the binary that writes turn-keyed rows was deployed there.
/// Add a name when the code being deployed cannot run without the file; do not add one merely because it is new.
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
    "292_storyboard_run_trajectory.sql",
    "293_run_trajectory_turn_identity.sql",
    // 294 (2026-10-18): the native Harness Evolution durable catalog, experiments and the one
    // active-pointer table. Code that proposes/adopts a harness revision writes these tables, so a
    // host without them must refuse to boot rather than fail on the first experiment.
    "294_forge_harness_evolution.sql",
    // 295 (2026-10-18): the correction ledger the Phase 3 learning loop writes and reads back
    // (corrections, repeat detection, experiment-outcome corrections). Applied to DEV 2026-10-18
    // 17:40Z and PROD by the Lead; code that reports learning corrections needs these rows.
    "295_forge_correction_ledger.sql",
];

/// Migrations whose FILE was edited after the ledger recorded an apply, and how the boot gate
/// reconciles them: the ledger checksum for these entries cannot equal the current file's hash, so
/// the gate verifies the SCHEMA those files promise instead (never silently accepts a bare name).
///
/// 295 is frozen: it was applied on DEV/PROD (ledger sha256:36a53a77…), then the file was rewritten
/// (now sha256:1ba60586…). Editing it again would rewrite history twice; migration
/// 296_forge_correction_ledger_reconcile.sql is the additive reconciliation record and
/// [`assert_reconciled_schemas`] is the check that a reconciled database really has the shape.
pub const RECONCILED_AFTER_APPLY: &[&str] = &["295_forge_correction_ledger.sql"];

/// The columns the reconciled correction-ledger schema must expose. Kept next to the exception so a
/// schema drift in the one checksum-exempt table fails the boot instead of slipping through.
pub const CORRECTION_LEDGER_REQUIRED_COLUMNS: &[&str] = &[
    "lane",
    "correction_key",
    "correction",
    "context",
    "root_cause",
    "spec_location",
    "permanent_rule_created",
    "evidence_refs",
    "experiment_id",
    "created_at",
];

/// Verify the SCHEMA promised by the checksum-exempt migrations in [`RECONCILED_AFTER_APPLY`].
///
/// This is the deliberate replacement for checksum verification on those entries: a ledger checksum
/// cannot be compared to a rewritten file, but the columns the running code writes must exist. A
/// missing column is a `SchemaMismatch` naming it and the boot refuses.
pub async fn assert_reconciled_schemas(database: &Database) -> DbResult<()> {
    let columns: Vec<(String,)> = sqlx::query_as(
        "select column_name from information_schema.columns \
         where table_schema = current_schema() and table_name = 'forge_correction_ledger'",
    )
    .fetch_all(database.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("db.boot_gate.reconciled_schema", &error))?;
    let present: std::collections::BTreeSet<&str> =
        columns.iter().map(|(name,)| name.as_str()).collect();
    let missing: Vec<&str> = CORRECTION_LEDGER_REQUIRED_COLUMNS
        .iter()
        .copied()
        .filter(|column| !present.contains(*column))
        .collect();
    if !missing.is_empty() {
        return Err(DbFailure::schema_mismatch(
            "db.boot_gate.reconciled_schema",
            format!(
                "{} is in RECONCILED_AFTER_APPLY, so its checksum is not verified; its schema must be, and                  forge_correction_ledger is missing column(s): {}. Apply 296_forge_correction_ledger_reconcile.sql                  on {} and start again.",
                RECONCILED_AFTER_APPLY.join(", "),
                missing.join(", "),
                database.target().as_str()
            ),
        ));
    }
    Ok(())
}

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
    if !missing.is_empty() {
        return Err(DbFailure::schema_mismatch(
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
        ));
    }
    // Checksum-exempt entries are verified by schema instead: 295 was rewritten after apply.
    assert_reconciled_schemas(database).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rewritten_migration_is_a_documented_schema_exception_not_a_silent_skip() {
        assert_eq!(
            RECONCILED_AFTER_APPLY,
            &["295_forge_correction_ledger.sql"],
            "295 is the only post-apply rewrite; add an entry only with a schema check"
        );
        assert!(
            REQUIRED_BOOT_MIGRATIONS.contains(&"295_forge_correction_ledger.sql"),
            "295 is still required: the exception is checksum->schema, not presence"
        );
        assert!(
            !REQUIRED_BOOT_MIGRATIONS.contains(&"296_forge_correction_ledger_reconcile.sql"),
            "296 is an additive reconciliation record, not a boot dependency"
        );
        assert!(
            CORRECTION_LEDGER_REQUIRED_COLUMNS.contains(&"correction_key")
                && CORRECTION_LEDGER_REQUIRED_COLUMNS.contains(&"experiment_id"),
            "the schema check covers the columns the ledger code reads and writes"
        );
    }

    #[test]
    #[ignore = "needs DATABASE_URL_DEV; read-only boot-gate evidence"]
    fn dev_smoke_reconciled_correction_ledger_schema_is_intact() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let database = runtime
            .block_on(Database::connect_target(crate::DbTarget::Dev))
            .expect("connect DEV");
        runtime
            .block_on(async {
                missing_required_migrations(&database).await?;
                assert_reconciled_schemas(&database).await
            })
            .expect("DEV boot gate accepts the reconciled ledger schema");
    }
}
