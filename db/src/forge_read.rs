//! The sanctioned READ path for the Forge control plane — the Rust home of the operator read tools.
//!
//! Rust replacement for `scripts/forge-read-tools.ts` (`forge:board`, `forge:story:show`) and the reads
//! behind `scripts/forge-batch-status.ts` (`forge:batch:status`). All three imported `legacy/db/*`, deleted
//! with the TypeScript application in `4cf98110`, so every one of them exited `ERR_MODULE_NOT_FOUND`: the
//! operator had a documented read path he could not run. Same shapes, same normalisation, one language.
//!
//! WHICH TABLES. The five views migration 191 declares are the sanctioned read shapes
//! (`forge_board_by_batch`, `forge_story_run_receipt`, `forge_open_holds`, `forge_story_findings`,
//! `forge_migration_ledger`), and this DAO selects from them — it does not re-derive a fact from a base
//! table, so a reader cannot invent a second interpretation of one. The queue, bench and board-status reads
//! the Cockpit has always had (`forge_batch`, `agent_work_item`, `storyboard_active_work`,
//! `storyboard_story`) are the exceptions the Cockpit screen itself makes, and they read columns, not
//! verdicts.
//!
//! READ-ONLY. There is no insert, no update and no claim in this file, and there must never be one: it is the
//! sibling an operator looks with before deciding to clean. `ForgeControlDao` and the reset tool are the
//! writers.
//!
//! NORMALISATION happens at this boundary, not above it (repository boundary rule): ids leave as strings,
//! every timestamp leaves as an ISO-8601 `…Z` string or `null`, and a hold reason that is blank leaves as
//! `unknown` rather than `""` — an empty string reads like "no problem", which is the opposite of a hold.
//! Timestamps are rendered by Postgres (`to_char(… at time zone 'UTC', …)`) rather than parsed in Rust, so
//! the output shape does not depend on the session's TimeZone setting — the guarantee the retired TypeScript
//! got for free from `Date.toISOString()`.

use crate::{Database, DbFailure, DbResult, ToolArtifactRow};
use model::{ForgeLiveNodeActivity, ForgeLiveRun, ForgeLiveSnapshot, ForgeLiveWorkItem};
use serde_json::Value;
use sqlx::FromRow;

/// The one timestamp shape every field in this module leaves in.
const ISO_UTC: &str = "YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"";

/// A batch of work, as `forge:board` and `forge:batch:status` both read it. `story_count`, `queued_count`
/// and `skipped_count` are the batch's own counts, repeated on every member row by the view, which is why
/// `board_batches` de-duplicates by id before returning.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeBatchRow {
    pub id: String,
    pub label: Option<String>,
    pub status: String,
    pub scheduled_for: Option<String>,
    pub fired_at: Option<String>,
    pub created_at: Option<String>,
    pub created_by: Option<String>,
    pub note: Option<String>,
    pub model_policy: Option<String>,
    pub story_count: i64,
    pub queued_count: i64,
    pub skipped_count: i64,
}

/// One (batch, story) membership row — the board's own grain.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeStoryBoardRow {
    pub batch_id: String,
    pub batch_label: Option<String>,
    pub batch_status: String,
    pub story_id: String,
    pub item_state: String,
    pub queued_at: Option<String>,
    pub error_text: Option<String>,
    pub story_title: Option<String>,
    pub story_status: Option<String>,
}

/// The newest run for a story: the receipt. `result_status` is the verdict word, `commit_hash` the commit it
/// happened on — both nullable, because a run that has not finished has neither.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeStoryReceiptRow {
    pub run_id: String,
    pub story_id: String,
    pub result_status: Option<String>,
    pub commit_hash: Option<String>,
    pub tests_summary: Option<String>,
    pub completion: Option<i32>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub created_at: Option<String>,
}

/// An unresolved hold: why a story is parked and where it would resume from.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeStoryHoldRow {
    pub hold_id: String,
    pub story_id: String,
    pub reason: Option<String>,
    pub originating_node: Option<String>,
    pub failure_class: Option<String>,
    pub resume_target: Option<String>,
    pub created_at: Option<String>,
}

/// An architect finding recorded for a story. The list-shaped columns arrive as JSON text and are parsed by
/// `parse_string_array`; a blob that is not a JSON array of strings becomes an empty list rather than failing
/// the read, because malformed evidence must not make the story it is attached to unreadable.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeStoryFindingRow {
    pub finding_row_id: String,
    pub finding_id: String,
    pub story_id: String,
    pub summary: Option<String>,
    pub required: Option<bool>,
    pub seams: Option<String>,
    pub hint: Option<String>,
    pub preconditions: Option<String>,
    pub classes: Option<String>,
    pub risks: Option<String>,
    pub created_at: Option<String>,
}

/// Where the story's declared change set sits in the migration ledger. A declared file with no ledger row
/// keeps `migration_id` null rather than disappearing — that absence is the finding.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeStoryMigrationRow {
    pub filename: String,
    pub migration_id: Option<String>,
    pub target: Option<String>,
    pub applied_at: Option<String>,
    pub migration_required: Option<bool>,
    pub dev_applied: Option<bool>,
    pub dev_verified: Option<bool>,
    pub prod_applied: Option<bool>,
    pub prod_verified: Option<bool>,
}

/// A work item the engine holds right now — the line between "in the table" and "running". Named for the
/// queue rather than the row (`ForgeAgentWorkRow` is the engine's own, claim-shaped row) so the two readers
/// cannot be confused for one another.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeQueueWorkRow {
    pub id: String,
    pub story_id: String,
    pub state: String,
    pub error_text: Option<String>,
    pub queued_at: Option<String>,
    pub updated_at: Option<String>,
    /// Retry accounting surfaces on the queue row (migration 277): a bounded queue must show its budget.
    pub attempts: Option<i32>,
    pub max_attempts: Option<i32>,
    /// The claim holder, named for the lease — `agent_work_item.claimed_by`. This is the fence the heartbeat
    /// checks, so the queue row's owner and the heartbeat's owner read as one fact.
    pub lease_owner: Option<String>,
    /// The generation that owner holds (migration 278). A heartbeat is fenced by (owner, generation), so a reader
    /// that wants to beat a claim has to carry both — a name alone is not an authority.
    pub claim_generation: i64,
    pub heartbeat_at: Option<String>,
    pub lease_expires_at: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct ForgeLiveWorkRow {
    work_item_id: String,
    story_id: String,
    title: String,
    state: String,
    kind: Option<String>,
    model_policy: Option<String>,
    claimed_by: Option<String>,
    error_text: Option<String>,
    queued_at: Option<String>,
    started_at: Option<String>,
    updated_at: Option<String>,
    finished_at: Option<String>,
    story_run_id: Option<String>,
}

impl ForgeLiveWorkRow {
    fn into_domain(self, status_bucket: Option<String>) -> ForgeLiveWorkItem {
        ForgeLiveWorkItem {
            work_item_id: self.work_item_id,
            story_id: self.story_id,
            title: self.title,
            state: self.state,
            status_bucket,
            kind: self.kind,
            model_policy: self.model_policy,
            claimed_by: self.claimed_by,
            error_text: self.error_text,
            queued_at: self.queued_at,
            started_at: self.started_at,
            updated_at: self.updated_at,
            finished_at: self.finished_at,
            story_run_id: self.story_run_id,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct ForgeLiveRunRow {
    id: String,
    story_id: String,
    run_type: Option<String>,
    run_phase: Option<String>,
    agent_runtime: Option<String>,
    model_used: Option<String>,
    result_status: Option<String>,
    started_at: Option<String>,
    ended_at: Option<String>,
    commit_hash: Option<String>,
    tests_summary: Option<String>,
    completion: Option<f64>,
    tokens_input: Option<i64>,
    tokens_output: Option<i64>,
    cost_usd: Option<f64>,
    cost_source: Option<String>,
    notes: Option<String>,
    evidence_detail: Option<String>,
    vendor_session_id: Option<String>,
}

impl From<ForgeLiveRunRow> for ForgeLiveRun {
    fn from(row: ForgeLiveRunRow) -> Self {
        Self {
            id: row.id,
            story_id: row.story_id,
            run_type: row.run_type,
            run_phase: row.run_phase,
            agent_runtime: row.agent_runtime,
            model_used: row.model_used,
            result_status: row.result_status,
            started_at: row.started_at,
            ended_at: row.ended_at,
            commit_hash: row.commit_hash,
            tests_summary: row.tests_summary,
            completion: row.completion,
            tokens_input: row.tokens_input,
            tokens_output: row.tokens_output,
            cost_usd: row.cost_usd,
            cost_source: row.cost_source,
            notes: row.notes,
            evidence_detail: row.evidence_detail,
            vendor_session_id: row.vendor_session_id,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct ForgeLiveNodeRow {
    process_instance_id: String,
    node_id: String,
    status: String,
    created_at: Option<String>,
    updated_at: Option<String>,
}

impl From<ForgeLiveNodeRow> for ForgeLiveNodeActivity {
    fn from(row: ForgeLiveNodeRow) -> Self {
        Self {
            process_instance_id: row.process_instance_id,
            node_id: row.node_id,
            status: row.status,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// A story the operator has selected as active work — the bench.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeBenchRow {
    pub story_id: String,
    pub work_order: i32,
    pub title: Option<String>,
    pub status: Option<String>,
}

/// `storyboard_story.id` with its board status, for the batch-status header counts.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeStoryStatusRow {
    pub id: String,
    pub status: String,
}

/// A story's whole picture, assembled from the five views.
#[derive(Debug, Clone)]
pub struct ForgeStoryShow {
    pub board: Vec<ForgeStoryBoardRow>,
    pub receipt: Option<ForgeStoryReceiptRow>,
    pub holds: Vec<ForgeStoryHoldRow>,
    pub findings: Vec<ForgeStoryFindingRow>,
    pub migrations: Vec<ForgeStoryMigrationRow>,
}

/// The five-view read path and the Cockpit's own reads. Construct with the process pool; never construct a
/// pool here.
#[derive(Clone)]
pub struct ForgeReadDao {
    db: Database,
}

impl ForgeReadDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Which database this read is against (`dev` / `prod`). The CLI prints it: an operator reading a board
    /// deserves to know which board it was.
    pub fn target(&self) -> String {
        self.db.declared_target().as_str().to_string()
    }

    /// The board: one row per (batch, story), de-duplicated to one row per batch with the batch's counts.
    pub async fn board_batches(&self) -> DbResult<Vec<ForgeBatchRow>> {
        let sql = format!(
            "select batch_id::text as id, batch_label as label, batch_status as status,
                    to_char(batch_scheduled_for at time zone 'UTC', '{ISO_UTC}') as scheduled_for,
                    to_char(batch_fired_at at time zone 'UTC', '{ISO_UTC}') as fired_at,
                    to_char(batch_created_at at time zone 'UTC', '{ISO_UTC}') as created_at,
                    batch_created_by::text as created_by, batch_note as note,
                    batch_model_policy as model_policy,
                    story_count, queued_count, skipped_count
             from forge_board_by_batch
             order by batch_created_at desc, batch_id"
        );
        let rows = sqlx::query_as::<_, ForgeBatchRow>(sqlx::AssertSqlSafe(sql))
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.board_batches", &error))?;
        Ok(dedupe_batches(rows))
    }

    /// The most recent batches, for `forge:batch:status`'s job stream.
    pub async fn recent_batches(&self, limit: i64) -> DbResult<Vec<ForgeBatchRow>> {
        let sql = format!("{} order by b.created_at desc limit $1", batch_select());
        sqlx::query_as::<_, ForgeBatchRow>(sqlx::AssertSqlSafe(sql))
            .bind(limit.max(1))
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.recent_batches", &error))
    }

    /// The batch things are being staged into: the newest `Staged` batch, if any. Read-only — the writer that
    /// creates one on first use lives on `ForgeControlDao`.
    pub async fn staging_batch(&self) -> DbResult<Option<ForgeBatchRow>> {
        let sql = format!(
            "{} where b.status = 'Staged' order by b.created_at desc limit 1",
            batch_select()
        );
        sqlx::query_as::<_, ForgeBatchRow>(sqlx::AssertSqlSafe(sql))
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.staging_batch", &error))
    }

    /// A story's membership in the board, newest batch first.
    pub async fn story_board(&self, story_id: &str) -> DbResult<Vec<ForgeStoryBoardRow>> {
        let sql = format!(
            "select batch_id::text as batch_id, batch_label, batch_status, story_id, item_state,
                    to_char(item_queued_at at time zone 'UTC', '{ISO_UTC}') as queued_at,
                    item_error_text as error_text, story_title, story_status
             from forge_board_by_batch
             where story_id = $1
             order by batch_created_at desc, batch_id"
        );
        sqlx::query_as::<_, ForgeStoryBoardRow>(sqlx::AssertSqlSafe(sql))
            .bind(story_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.story_board", &error))
    }

    /// A story's newest run — the receipt. `None` means no run has been recorded, which is a fact in itself:
    /// never render it as a verdict.
    pub async fn story_receipt(&self, story_id: &str) -> DbResult<Option<ForgeStoryReceiptRow>> {
        let sql = format!(
            "select run_id::text as run_id, story_id, result_status, commit_hash, tests_summary, completion,
                    to_char(started_at at time zone 'UTC', '{ISO_UTC}') as started_at,
                    to_char(ended_at at time zone 'UTC', '{ISO_UTC}') as ended_at,
                    to_char(created_at at time zone 'UTC', '{ISO_UTC}') as created_at
             from forge_story_run_receipt
             where story_id = $1"
        );
        sqlx::query_as::<_, ForgeStoryReceiptRow>(sqlx::AssertSqlSafe(sql))
            .bind(story_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.story_receipt", &error))
    }

    /// A story's unresolved holds. A blank reason is normalised to `unknown` here, at the boundary.
    pub async fn story_holds(&self, story_id: &str) -> DbResult<Vec<ForgeStoryHoldRow>> {
        let sql = format!(
            "select hold_id::text as hold_id, story_id, reason, originating_node, failure_class,
                    resume_target,
                    to_char(created_at at time zone 'UTC', '{ISO_UTC}') as created_at
             from forge_open_holds
             where story_id = $1
             order by created_at desc"
        );
        let mut rows = sqlx::query_as::<_, ForgeStoryHoldRow>(sqlx::AssertSqlSafe(sql))
            .bind(story_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.story_holds", &error))?;
        for row in &mut rows {
            row.reason = Some(normalize_reason(row.reason.as_deref()));
        }
        Ok(rows)
    }

    /// A story's architect findings, oldest first — the order the contract was written in.
    pub async fn story_findings(&self, story_id: &str) -> DbResult<Vec<ForgeStoryFindingRow>> {
        let sql = format!(
            "select finding_row_id::text as finding_row_id, finding_id, story_id, summary, required,
                    seams::text as seams, hint, preconditions::text as preconditions,
                    classes::text as classes, risks::text as risks,
                    to_char(created_at at time zone 'UTC', '{ISO_UTC}') as created_at
             from forge_story_findings
             where story_id = $1
             order by created_at asc, finding_id"
        );
        sqlx::query_as::<_, ForgeStoryFindingRow>(sqlx::AssertSqlSafe(sql))
            .bind(story_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.story_findings", &error))
    }

    /// Where the story's declared migration files sit in the ledger — DEV and PROD, applied and verified.
    pub async fn story_migrations(&self, story_id: &str) -> DbResult<Vec<ForgeStoryMigrationRow>> {
        let sql = format!(
            "select filename, migration_id::text as migration_id, target,
                    to_char(applied_at at time zone 'UTC', '{ISO_UTC}') as applied_at,
                    migration_required, dev_migration_applied as dev_applied,
                    dev_migration_verified as dev_verified,
                    prod_migration_applied as prod_applied,
                    prod_migration_verified as prod_verified
             from forge_migration_ledger
             where story_id = $1
             order by filename"
        );
        sqlx::query_as::<_, ForgeStoryMigrationRow>(sqlx::AssertSqlSafe(sql))
            .bind(story_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.story_migrations", &error))
    }

    /// A story's full picture, assembled from the five views.
    pub async fn story_show(&self, story_id: &str) -> DbResult<ForgeStoryShow> {
        Ok(ForgeStoryShow {
            board: self.story_board(story_id).await?,
            receipt: self.story_receipt(story_id).await?,
            holds: self.story_holds(story_id).await?,
            findings: self.story_findings(story_id).await?,
            migrations: self.story_migrations(story_id).await?,
        })
    }

    /// The engine's queue: the work items it currently holds. `Claimed`, `Running` and `Paused` are the
    /// states the Cockpit has always shown as in flight; a `Ready` item is in the table, not running.
    pub async fn active_agent_work(&self, limit: i64) -> DbResult<Vec<ForgeQueueWorkRow>> {
        let sql = format!(
            "select id::text as id, story_id, state, error_text,
                    to_char(queued_at at time zone 'UTC', '{ISO_UTC}') as queued_at,
                    to_char(updated_at at time zone 'UTC', '{ISO_UTC}') as updated_at,
                    attempts, max_attempts,
                    claimed_by as lease_owner,
                    claim_generation,
                    to_char(heartbeat_at at time zone 'UTC', '{ISO_UTC}') as heartbeat_at,
                    to_char(lease_expires_at at time zone 'UTC', '{ISO_UTC}') as lease_expires_at
             from agent_work_item
             where state in ('Claimed', 'Running', 'Paused')
             order by updated_at desc
             limit $1"
        );
        sqlx::query_as::<_, ForgeQueueWorkRow>(sqlx::AssertSqlSafe(sql))
            .bind(limit.clamp(1, 20))
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.active_agent_work", &error))
    }

    /// Parent ForgeService read model for the TECH Cockpit Work in Flight tab.
    /// Observation only: no claim, retry, settlement or Workflow transition is performed here.
    pub async fn live_snapshot(
        &self,
        selected_story_id: Option<&str>,
        limit: i64,
    ) -> DbResult<ForgeLiveSnapshot> {
        let active_work = self.live_work_items(limit).await?;
        let work_status = self.engine_work_status(limit).await?;
        let selected_story_id = selected_story_id
            .filter(|id| {
                active_work.iter().any(|item| item.story_id == *id)
                    || work_status.iter().any(|item| item.story_id == *id)
            })
            .map(str::to_owned)
            .or_else(|| active_work.first().map(|item| item.story_id.clone()))
            .or_else(|| work_status.first().map(|item| item.story_id.clone()));

        let (current_run, node_activity) = if let Some(story_id) = selected_story_id.as_deref() {
            (
                self.latest_live_run(story_id).await?,
                self.live_node_activity(story_id, 32).await?,
            )
        } else {
            (None, Vec::new())
        };

        Ok(ForgeLiveSnapshot {
            active_work,
            work_status,
            selected_story_id,
            current_run,
            node_activity,
        })
    }

    async fn live_work_items(&self, limit: i64) -> DbResult<Vec<ForgeLiveWorkItem>> {
        let sql = format!(
            "select w.id::text as work_item_id, w.story_id, coalesce(s.title,w.story_id) as title,
                    w.state,w.kind,w.model_policy,w.claimed_by,w.error_text,
                    to_char(w.queued_at at time zone 'UTC','{ISO_UTC}') as queued_at,
                    to_char(w.started_at at time zone 'UTC','{ISO_UTC}') as started_at,
                    to_char(w.updated_at at time zone 'UTC','{ISO_UTC}') as updated_at,
                    to_char(w.finished_at at time zone 'UTC','{ISO_UTC}') as finished_at,
                    w.story_run_id::text as story_run_id
             from agent_work_item w left join storyboard_story s on s.id=w.story_id
             where w.story_id is not null and w.state in ('Claimed','Running','Paused')
             order by w.updated_at desc limit $1"
        );
        let rows = sqlx::query_as::<_, ForgeLiveWorkRow>(sqlx::AssertSqlSafe(sql))
            .bind(limit.clamp(1, 50))
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.live_work_items", &error))?;
        Ok(rows.into_iter().map(|row| row.into_domain(None)).collect())
    }

    /// Engine Work Status is exactly Done / Error / Retry.
    /// Retry is not a stored state: it is Ready with an already-opened Story Run, the engine-fault clear path.
    async fn engine_work_status(&self, limit: i64) -> DbResult<Vec<ForgeLiveWorkItem>> {
        let sql = format!(
            "select w.id::text as work_item_id, w.story_id, coalesce(s.title,w.story_id) as title,
                    w.state,w.kind,w.model_policy,w.claimed_by,w.error_text,
                    to_char(w.queued_at at time zone 'UTC','{ISO_UTC}') as queued_at,
                    to_char(w.started_at at time zone 'UTC','{ISO_UTC}') as started_at,
                    to_char(w.updated_at at time zone 'UTC','{ISO_UTC}') as updated_at,
                    to_char(w.finished_at at time zone 'UTC','{ISO_UTC}') as finished_at,
                    w.story_run_id::text as story_run_id
             from agent_work_item w left join storyboard_story s on s.id=w.story_id
             where w.story_id is not null and (w.state in ('Done','Error') or (w.state='Ready' and w.story_run_id is not null))
             order by coalesce(w.finished_at,w.updated_at) desc limit $1"
        );
        let rows = sqlx::query_as::<_, ForgeLiveWorkRow>(sqlx::AssertSqlSafe(sql))
            .bind(limit.clamp(1, 50))
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.engine_work_status", &error))?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let bucket = engine_work_status_bucket(&row.state, row.story_run_id.as_deref())?;
                Some(row.into_domain(Some(bucket.to_string())))
            })
            .collect())
    }

    async fn latest_live_run(&self, story_id: &str) -> DbResult<Option<ForgeLiveRun>> {
        let sql = format!(
            "select r.id::text as id,r.story_id,r.run_type,r.run_phase,r.agent_runtime,r.model_used,r.result_status,
                    to_char(r.started_at at time zone 'UTC','{ISO_UTC}') as started_at,
                    to_char(r.ended_at at time zone 'UTC','{ISO_UTC}') as ended_at,
                    r.commit_hash,r.tests_summary,r.completion::float8 as completion,
                    r.tokens_input::bigint as tokens_input,r.tokens_output::bigint as tokens_output,
                    r.cost_usd::float8 as cost_usd,r.cost_source,r.notes,r.evidence_detail,v.session_id as vendor_session_id
             from storyboard_story_run r
             left join forge_vendor_session v on v.story_id=r.story_id and v.worker_id='opencode-v2'
             where r.story_id=$1 order by r.started_at desc nulls last,r.created_at desc,r.id desc limit 1"
        );
        sqlx::query_as::<_, ForgeLiveRunRow>(sqlx::AssertSqlSafe(sql))
            .bind(story_id)
            .fetch_optional(self.db.pool())
            .await
            .map(|row| row.map(Into::into))
            .map_err(|error| DbFailure::from_sqlx("forge_read.latest_live_run", &error))
    }

    async fn live_node_activity(
        &self,
        story_id: &str,
        limit: i64,
    ) -> DbResult<Vec<ForgeLiveNodeActivity>> {
        let sql = format!(
            "select process_instance_id::text as process_instance_id,node_id,status,
                    to_char(created_at at time zone 'UTC','{ISO_UTC}') as created_at,
                    to_char(updated_at at time zone 'UTC','{ISO_UTC}') as updated_at
             from forge_engine_task_execution where story_id=$1 and node_id is not null
             order by created_at desc limit $2"
        );
        sqlx::query_as::<_, ForgeLiveNodeRow>(sqlx::AssertSqlSafe(sql))
            .bind(story_id)
            .bind(limit.clamp(1, 100))
            .fetch_all(self.db.pool())
            .await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(|error| DbFailure::from_sqlx("forge_read.live_node_activity", &error))
    }
    /// The bench: the stories the operator selected as active work, in his order. Presence of a row in
    /// `storyboard_active_work` IS the active state — there is no flag to disagree with.
    pub async fn bench(&self) -> DbResult<Vec<ForgeBenchRow>> {
        sqlx::query_as::<_, ForgeBenchRow>(
            "select aw.story_id, aw.work_order, s.title, s.status
             from storyboard_active_work aw
             join storyboard_story s on s.id = aw.story_id
             order by aw.work_order, aw.story_id",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_read.bench", &error))
    }

    /// Every story's id and board status. Small enough to read whole, and reading it whole is what lets the
    /// caller count `Batched` / `Ready` without inventing a second interpretation of "status".
    pub async fn story_statuses(&self) -> DbResult<Vec<ForgeStoryStatusRow>> {
        sqlx::query_as::<_, ForgeStoryStatusRow>(
            "select id, status from storyboard_story order by id",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_read.story_statuses", &error))
    }

    /// The tool artifacts a set of stories recorded, newest first — one experiment's evidence in one read.
    ///
    /// The read lives here because the statement does: a caller that built its own `select … from
    /// forge_tool_artifact where story_id in (…)` was Forge holding a statement of its own, which
    /// `ARCH.BOUNDARY-005` refuses (the same reason `engine/observer.rs` lost its dead `INSERT` copy). The ids are
    /// **bound** (`= any($1::text[])`) rather than interpolated, so the `in` list cannot become a second spelling of
    /// an escaping rule either.
    ///
    /// READ ONLY. `ForgeEngineDao::record_tool_artifact` stays the one write of `forge_tool_artifact`.
    pub async fn tool_artifacts_for_stories(
        &self,
        story_ids: &[String],
        limit: i64,
    ) -> DbResult<Vec<ToolArtifactRow>> {
        if story_ids.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "select id::text as id, story_id, story_run_id::text as story_run_id, tool, kind, verdict, summary, sha,
                    to_char(created_at at time zone 'UTC', '{ISO_UTC}') as created_at
             from forge_tool_artifact
             where story_id = any($1::text[])
             order by created_at desc, id desc
             limit $2"
        );
        sqlx::query_as::<_, ToolArtifactRow>(sqlx::AssertSqlSafe(sql))
            .bind(story_ids)
            .bind(limit.clamp(1, 500))
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_read.tool_artifacts_for_stories", &error))
    }

    /// One ad-hoc read-only query, answered as JSON objects: the query a tool does not exist for yet.
    ///
    /// This is the home of `forge sql` (cli). It is here rather than in the CLI because a query outside
    /// `db` is not a thing this repository allows, and it is built the way a read tool should be:
    ///
    ///   * the statement is wrapped — `select row_to_json(t)::text from (<sql>) t limit $1` — so the answer comes
    ///     back as JSON with no per-query `FromRow` struct and no cast the caller has to guess at;
    ///   * it runs inside `begin read only`, so Postgres itself refuses a write even if the caller's own guard was
    ///     wrong; the transaction is rolled back and never committed, whatever the statement did;
    ///   * `limit` bounds what can be pulled into a terminal.
    ///
    /// The caller owns the *shape* guard (which statements are acceptable to name); this method owns the
    /// read-only guarantee. Both are needed: the guard gives a sentence, the transaction gives the ceiling.
    pub async fn read_only_rows(&self, sql: &str, limit: i64) -> DbResult<Vec<Value>> {
        let wrapped = format!("select row_to_json(t)::text as row from ({sql}) t limit $1");
        // A read-only transaction, not merely a read-shaped statement: the database enforces the promise, so a
        // guard that was wrong on the caller's side is refused here rather than obeyed.
        let mut tx = self.db.begin_read_only("forge_read.read_only_rows").await?;
        let outcome = async {
            sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(wrapped))
                .bind(limit)
                .fetch_all(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("forge_read.read_only_rows", &error))
        }
        .await;
        // Rolled back either way: a read must never leave a transaction holding a pooled connection.
        let _ = tx.rollback().await;
        let raw = outcome?;
        Ok(raw
            .into_iter()
            .map(|text| serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text)))
            .collect())
    }
}

/// Map the durable work row to the Cockpit's three Engine Work Status buckets.
pub fn engine_work_status_bucket(state: &str, story_run_id: Option<&str>) -> Option<&'static str> {
    match state {
        "Done" => Some("Done"),
        "Error" => Some("Error"),
        "Ready" if story_run_id.is_some_and(|id| !id.trim().is_empty()) => Some("Retry"),
        _ => None,
    }
}
/// The batch shape the Cockpit's job stream reads: the batch row plus its own item counts. One copy, so the
/// staging read and the recent-batches read can never disagree about what a batch looks like.
fn batch_select() -> String {
    format!(
        "select b.id::text as id, b.label, b.status,
                to_char(b.scheduled_for at time zone 'UTC', '{ISO_UTC}') as scheduled_for,
                to_char(b.fired_at at time zone 'UTC', '{ISO_UTC}') as fired_at,
                to_char(b.created_at at time zone 'UTC', '{ISO_UTC}') as created_at,
                b.created_by::text as created_by, b.note, b.model_policy,
                (select count(*) from forge_batch_item i where i.batch_id = b.id) as story_count,
                (select count(*) from forge_batch_item i where i.batch_id = b.id and i.state = 'Queued') as queued_count,
                (select count(*) from forge_batch_item i where i.batch_id = b.id and i.state = 'Skipped') as skipped_count
         from forge_batch b"
    )
}

/// One row per batch. The view repeats the batch's own counts on every member row, so without this the board
/// would print the same batch once per story in it.
pub fn dedupe_batches(rows: Vec<ForgeBatchRow>) -> Vec<ForgeBatchRow> {
    let mut seen = std::collections::HashSet::new();
    let mut batches = Vec::with_capacity(rows.len());
    for row in rows {
        if seen.insert(row.id.clone()) {
            batches.push(row);
        }
    }
    batches
}

/// A hold reason that is blank is `unknown`, never `""`. An operator reading an empty string reads "no
/// problem", which is the opposite of a hold.
pub fn normalize_reason(reason: Option<&str>) -> String {
    match reason.map(str::trim) {
        Some(text) if !text.is_empty() => text.to_string(),
        _ => "unknown".to_string(),
    }
}

/// The list-shaped evidence columns arrive as JSON text. A blob that is not a JSON array of strings is an
/// empty list, not a failed read: losing a whole story's picture over one malformed column would hide
/// exactly the story that needs looking at.
pub fn parse_string_array(raw: Option<&str>) -> Vec<String> {
    match raw {
        None => Vec::new(),
        Some(text) => serde_json::from_str::<Vec<String>>(text).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(id: &str, story_count: i64) -> ForgeBatchRow {
        ForgeBatchRow {
            id: id.to_string(),
            label: None,
            status: "Staged".to_string(),
            scheduled_for: None,
            fired_at: None,
            created_at: Some("2026-09-28T00:00:00.000Z".to_string()),
            created_by: None,
            note: None,
            model_policy: None,
            story_count,
            queued_count: 0,
            skipped_count: 0,
        }
    }

    #[test]
    fn the_board_shows_each_batch_once_even_though_the_view_repeats_its_counts_per_member() {
        let rows = vec![
            batch("b-1", 3),
            batch("b-1", 3),
            batch("b-1", 3),
            batch("b-2", 1),
        ];
        let batches = dedupe_batches(rows);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].id, "b-1");
        assert_eq!(batches[0].story_count, 3);
        assert_eq!(batches[1].id, "b-2");
    }

    #[test]
    fn a_blank_hold_reason_reads_as_unknown_never_as_an_empty_string() {
        assert_eq!(normalize_reason(None), "unknown");
        assert_eq!(normalize_reason(Some("")), "unknown");
        assert_eq!(normalize_reason(Some("   ")), "unknown");
        assert_eq!(
            normalize_reason(Some(" prod parity drift ")),
            "prod parity drift"
        );
    }

    #[test]
    fn a_malformed_evidence_blob_is_an_empty_list_not_a_failed_read() {
        assert!(parse_string_array(None).is_empty());
        assert!(parse_string_array(Some("not json")).is_empty());
        assert!(parse_string_array(Some(r#"{"not":"an array"}"#)).is_empty());
        assert_eq!(
            parse_string_array(Some(r#"["web/src/api/engine.rs"]"#)),
            vec!["web/src/api/engine.rs".to_string()]
        );
    }

    #[test]
    fn engine_work_status_distinguishes_retry_from_a_fresh_queue_item() {
        assert_eq!(engine_work_status_bucket("Done", None), Some("Done"));
        assert_eq!(engine_work_status_bucket("Error", None), Some("Error"));
        assert_eq!(
            engine_work_status_bucket("Ready", Some("run-1")),
            Some("Retry")
        );
        assert_eq!(engine_work_status_bucket("Ready", None), None);
        assert_eq!(engine_work_status_bucket("Claimed", Some("run-1")), None);
    }
    #[test]
    fn every_timestamp_in_the_batch_read_is_rendered_in_utc_iso_independent_of_the_session_timezone(
    ) {
        let sql = batch_select();
        let rendered = sql.matches("at time zone 'UTC'").count();
        assert_eq!(
            rendered, 3,
            "scheduled_for, fired_at and created_at must all be rendered in UTC"
        );
        assert!(
            sql.contains(ISO_UTC),
            "the one ISO shape is used for all three"
        );
        // The read is a constant: a bound parameter here would mean a caller-supplied fragment.
        assert!(!sql.contains('$'), "batch_select takes no parameters");
    }
}
