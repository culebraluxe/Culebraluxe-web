//! Read path for approved Forge assay plans and their immutable Story Run snapshots.
//! SQL remains inside the db crate; Forge cannot infer a plan from acceptance prose.

use crate::{Database, DbFailure, DbResult};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct AssayPlanSnapshotRow {
    pub story_run_id: String,
    pub story_id: String,
    pub assay_commands_snapshot: Option<String>,
    pub snapshot: Option<Value>,
}

#[derive(Debug, Clone, FromRow)]
pub struct AssayReceiptRow {
    pub id: String,
    pub story_id: String,
    pub story_run_id: Option<String>,
    pub verdict: Option<String>,
    pub detail: Option<Value>,
    pub sha: Option<String>,
    pub idempotency_key: String,
}

#[derive(Clone)]
pub struct ForgeAssayDao {
    db: Database,
}

impl ForgeAssayDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Read the run-frozen plan, never the mutable current Story plan.
    pub async fn plan_snapshot_for_run(
        &self,
        story_run_id: &str,
    ) -> DbResult<Option<AssayPlanSnapshotRow>> {
        sqlx::query_as::<_, AssayPlanSnapshotRow>(
            "select r.id::text as story_run_id, r.story_id,
                    r.assay_commands_snapshot, r.assay_plan_snapshot as snapshot
               from storyboard_story_run r
              where r.id=$1::uuid
              limit 1",
        )
        .bind(story_run_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_assay.plan_snapshot_for_run", &error))
    }

    /// Read a durable receipt by its deterministic measurement key for restart reconciliation.
    pub async fn receipt_for_key(&self, key: &str) -> DbResult<Option<AssayReceiptRow>> {
        sqlx::query_as::<_, AssayReceiptRow>(
            "select id::text, story_id, story_run_id::text, verdict, detail, sha, idempotency_key
               from forge_tool_artifact
              where idempotency_key=$1
              limit 1",
        )
        .bind(key)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_assay.receipt_for_key", &error))
    }
}
