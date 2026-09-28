//! The applied-migration ledger — `schema_migration`, created by migration 144.
//!
//! Before this ledger existed, applying a migration recorded NOTHING, so "what is applied where?" was
//! unanswerable, and that is how PROD went weeks without 116-122/138 while DEV went weeks without the Forge
//! dispatch columns. The ledger is authoritative from the 2026-09-10 baseline forward; pre-baseline history
//! is reported as unrecorded rather than claimed.
//!
//! ONE WRITER. `pnpm db:migrate` (the Rust `db-tool apply`) is what writes rows here. The retired
//! `scripts/apply-migration.mjs` wrote them first, and it is deleted — a second writer would mean two
//! answers to "what is applied", which is the drift this table exists to prevent.

use crate::{Database, DbFailure, DbResult, DbTarget};
use chrono::{DateTime, Utc};
use sqlx::Row;

/// One ledger row, normalized at the database boundary: the timestamp leaves the driver as an ISO-8601
/// string plus the `YYYY-MM-DD` the status report prints, so nothing above this needs to know how sqlx
/// hands a `timestamptz` over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationLedgerRow {
    pub filename: String,
    pub checksum: String,
    pub target: String,
    pub note: Option<String>,
    /// `YYYY-MM-DD`, the report's date column.
    pub applied_on: String,
    /// Full ISO-8601 timestamp, used for ordering.
    pub applied_at: String,
}

#[derive(Clone)]
pub struct SchemaMigrationDao {
    db: Database,
}

impl SchemaMigrationDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Is the ledger table there at all?
    ///
    /// Asked with `to_regclass` so a missing ledger is a sentence the operator can act on, rather than
    /// SQLSTATE 42P01 surfacing from inside a checksum query.
    pub async fn present(&self) -> DbResult<bool> {
        let present: Option<String> =
            sqlx::query_scalar("select to_regclass('public.schema_migration')::text")
                .fetch_one(self.db.pool())
                .await
                .map_err(|error| DbFailure::from_sqlx("schema_migration.present", &error))?;
        Ok(present.is_some())
    }

    /// Every row for one target, newest first.
    pub async fn rows(&self, target: DbTarget) -> DbResult<Vec<MigrationLedgerRow>> {
        let rows = sqlx::query(
            "select filename, checksum, target, applied_at, note from schema_migration \
             where target = $1 order by applied_at desc",
        )
        .bind(target.as_str())
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("schema_migration.rows", &error))?;

        let mut ledger = Vec::with_capacity(rows.len());
        for row in rows {
            let applied_at: DateTime<Utc> = row
                .try_get("applied_at")
                .map_err(|error| DbFailure::from_sqlx("schema_migration.rows", &error))?;
            ledger.push(MigrationLedgerRow {
                filename: row
                    .try_get("filename")
                    .map_err(|error| DbFailure::from_sqlx("schema_migration.rows", &error))?,
                checksum: row
                    .try_get("checksum")
                    .map_err(|error| DbFailure::from_sqlx("schema_migration.rows", &error))?,
                target: row
                    .try_get("target")
                    .map_err(|error| DbFailure::from_sqlx("schema_migration.rows", &error))?,
                note: row
                    .try_get("note")
                    .map_err(|error| DbFailure::from_sqlx("schema_migration.rows", &error))?,
                applied_on: applied_at.format("%Y-%m-%d").to_string(),
                applied_at: applied_at.to_rfc3339(),
            });
        }
        Ok(ledger)
    }

    /// The checksum already recorded for this file on this target, if the file is recorded at all.
    pub async fn recorded_checksum(
        &self,
        filename: &str,
        target: DbTarget,
    ) -> DbResult<Option<String>> {
        sqlx::query_scalar(
            "select checksum from schema_migration where filename = $1 and target = $2",
        )
        .bind(filename)
        .bind(target.as_str())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("schema_migration.recorded_checksum", &error))
    }

    /// Record an application, replacing the row for this file and target if there is one.
    ///
    /// The upsert mirrors the original script exactly, including `applied_at = now()`: a re-apply with
    /// `--force` is a new application and says so in the timestamp.
    pub async fn record(
        &self,
        filename: &str,
        checksum: &str,
        target: DbTarget,
        note: Option<&str>,
    ) -> DbResult<()> {
        sqlx::query(
            "insert into schema_migration (filename, checksum, target, note) \
             values ($1, $2, $3, $4) \
             on conflict (filename, target) \
             do update set checksum = excluded.checksum, applied_at = now(), note = excluded.note",
        )
        .bind(filename)
        .bind(checksum)
        .bind(target.as_str())
        .bind(note)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("schema_migration.record", &error))?;
        Ok(())
    }
}
