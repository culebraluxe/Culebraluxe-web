//! The reads behind `forge doctor` — the control plane in one command.
//!
//! Rust replacement for the gather half of `scripts/forge-doctor.ts`, which imported `legacy/db/*` (deleted
//! with the TypeScript application in `4cf98110`) and so exited `ERR_MODULE_NOT_FOUND`. The rendering half is
//! pure and lives in `forge::doctor_report`; the rules about agreement live in `forge::qa_consistency` and
//! `forge::roi`. This file is only the reading.
//!
//! READ-ONLY IS A HARD REQUIREMENT: no insert, no update, no claim. Every fact here is read from data that
//! already exists, and the reset tool is the writer for the same tables.
//!
//! Time arithmetic happens HERE, in SQL, and leaves as a number: an attempt's wall time arrives as minutes
//! and a claim's age can be measured from the timestamps this returns, so the pure rollups above never read a
//! clock that was not handed to them. That is the repository-boundary rule applied to time.

use crate::{Database, DbFailure, DbResult};
use sqlx::FromRow;

/// The one timestamp shape every field in this module leaves in.
const ISO_UTC: &str = "YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"";

/// An engine role turn still held. `at` is the row's creation and `updated_at` its last touch; the doctor
/// prefers the newer of the two when it measures how long a claim has been held, and keeps both so it can say
/// which it used.
#[derive(Debug, Clone, FromRow)]
pub struct EngineRunCardRow {
    pub story_id: String,
    pub status: String,
    pub attempts: i64,
    pub at: Option<String>,
    pub updated_at: Option<String>,
}

/// A work item that is not finished: the engine's queue.
#[derive(Debug, Clone, FromRow)]
pub struct EngineQueuedCardRow {
    pub story_id: String,
    pub title: Option<String>,
    pub state: String,
    pub since: Option<String>,
}

/// One finished attempt, with its wall time already converted to minutes.
#[derive(Debug, Clone, FromRow)]
pub struct RoiAttemptRow {
    pub kind: Option<String>,
    pub model_policy: Option<String>,
    pub state: String,
    pub wall_minutes: Option<f64>,
    pub result_status: Option<String>,
    pub cost_widgets: Option<f64>,
}

/// A story run, for the QA run/verdict agreement check.
#[derive(Debug, Clone, FromRow)]
pub struct QaRunRow {
    pub id: String,
    pub story_id: String,
    pub run_type: Option<String>,
    pub result_status: Option<String>,
    pub created_at: Option<String>,
}

/// The read-only control-plane reader `forge doctor` composes.
#[derive(Clone)]
pub struct ForgeDoctorDao {
    db: Database,
}

impl ForgeDoctorDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// The newest engine role turn per story — the engine ledger's own view of what it is holding.
    pub async fn engine_run_cards(&self, limit: i64) -> DbResult<Vec<EngineRunCardRow>> {
        let sql = format!(
            "select story_id, status, attempts,
                    to_char(at at time zone 'UTC', '{ISO_UTC}') as at,
                    to_char(updated_at at time zone 'UTC', '{ISO_UTC}') as updated_at
             from (
               select distinct on (e.story_id)
                 e.story_id,
                 e.status,
                 e.created_at as at,
                 e.updated_at,
                 (select count(*) from forge_engine_task_execution x where x.story_id = e.story_id) as attempts
               from forge_engine_task_execution e
               order by e.story_id, e.created_at desc
             ) latest
             order by latest.at desc
             limit $1"
        );
        sqlx::query_as::<_, EngineRunCardRow>(sqlx::AssertSqlSafe(sql))
            .bind(limit.clamp(1, 200))
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_doctor.engine_run_cards", &error))
    }

    /// The engine's queue: work items that are not finished.
    pub async fn engine_queued_cards(&self, limit: i64) -> DbResult<Vec<EngineQueuedCardRow>> {
        let sql = format!(
            "select w.story_id, coalesce(s.title, w.story_id) as title, w.state,
                    to_char(w.updated_at at time zone 'UTC', '{ISO_UTC}') as since
             from agent_work_item w
             left join storyboard_story s on s.id = w.story_id
             where w.story_id is not null
               and w.state not in ('Done', 'Error', 'Cancelled')
             order by w.updated_at desc
             limit $1"
        );
        sqlx::query_as::<_, EngineQueuedCardRow>(sqlx::AssertSqlSafe(sql))
            .bind(limit.clamp(1, 200))
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_doctor.engine_queued_cards", &error))
    }

    /// Finished attempts inside the window, newest first, with their wall time in minutes.
    ///
    /// "Finished" means `finished_at is not null`, which covers Done, Error and Cancelled alike: a rollup that
    /// only counted successes would report the cheap policy's failures as free. The join to the run is a LEFT
    /// join on purpose — an item with no run still counts as an attempt, and its cost is `None` rather than
    /// zero, because reporting a missing cost as 0 would make coverage look complete.
    pub async fn roi_attempts(&self, days: i64) -> DbResult<Vec<RoiAttemptRow>> {
        sqlx::query_as::<_, RoiAttemptRow>(
            "select w.kind, w.model_policy, w.state,
                    (extract(epoch from (w.finished_at - w.started_at)) / 60)::float8 as wall_minutes,
                    r.result_status,
                    r.cost_widgets::float8 as cost_widgets
             from agent_work_item w
             left join storyboard_story_run r on r.id = w.story_run_id
             where w.finished_at is not null
               and w.finished_at >= now() - ($1::text || ' days')::interval
             order by w.finished_at desc",
        )
        .bind(days.max(1).to_string())
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_doctor.roi_attempts", &error))
    }

    /// Story runs, newest first. The caller selects the QA-lane ones by `run_type` (one definition, in
    /// `forge::qa_consistency`) and takes the first QA run it sees per story as that story's latest.
    pub async fn story_runs(&self, limit: i64) -> DbResult<Vec<QaRunRow>> {
        let sql = format!(
            "select id::text as id, story_id, run_type, result_status,
                    to_char(created_at at time zone 'UTC', '{ISO_UTC}') as created_at
             from storyboard_story_run
             order by created_at desc, id desc
             limit $1"
        );
        sqlx::query_as::<_, QaRunRow>(sqlx::AssertSqlSafe(sql))
            .bind(limit.clamp(1, 500))
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_doctor.story_runs", &error))
    }

    /// The durable QA verdict recorded for a story, preferring the evidence of an ACTIVE process instance.
    /// `None` is "no verdict recorded" — never a FAIL.
    pub async fn qa_verdict(&self, story_id: &str) -> DbResult<Option<bool>> {
        sqlx::query_scalar::<_, Option<bool>>(
            "select e.qa_passed
             from forge_workflow_evidence e
             join process_instances pi on pi.id = e.process_instance_id
             where e.story_id = $1
             order by (pi.status = 'active') desc, e.updated_at desc
             limit 1",
        )
        .bind(story_id)
        .fetch_optional(self.db.pool())
        .await
        .map(|row| row.flatten())
        .map_err(|error| DbFailure::from_sqlx("forge_doctor.qa_verdict", &error))
    }

    /// The control plane in three numbers, counted rather than sampled.
    ///
    /// The retired reader inferred these from a 50-row page of cards, so its "instances" figure was really
    /// "instances, up to 50". A count is the fact the label claims, and the label is why this is a count:
    /// `open engine tasks` counts only stories whose LATEST turn is non-terminal AND not stale — a stale claim
    /// is abandoned, not running (active decision: abandoned-claim-is-not-running), and only the cleaner, not
    /// this read, is the authority on abandonment.
    pub async fn control_plane_counts(&self) -> DbResult<ControlPlaneCounts> {
        sqlx::query_as::<_, ControlPlaneCounts>(
            "select
               (select count(distinct e.story_id) from forge_engine_task_execution e) as instances,
               (select count(*) from (
                  select distinct on (e.story_id) e.story_id, e.status, e.updated_at
                  from forge_engine_task_execution e
                  order by e.story_id, e.created_at desc
                ) latest
                where latest.status not in ('completed', 'failed', 'interrupted')
                  and latest.updated_at >= now() - interval '15 minutes') as open_tasks,
               (select count(*) from agent_work_item
                where state not in ('Done', 'Error', 'Cancelled')) as open_work_items,
               -- NOT the same question as the line above: a `Ready` item is queued, not held. Summing the two
               -- is what made the doctor report held claims it did not have (see ControlPlaneCounts).
               (select count(*) from agent_work_item
                where state in ('Claimed', 'Running')) as claimed_work_items",
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_doctor.control_plane_counts", &error))
    }

    /// The oldest held claim across BOTH ledgers, with the ledger named and its age measured in SQL.
    ///
    /// The ledgers are separate on purpose (`forge_engine_task_execution` records role turns,
    /// `agent_work_item` enforces the per-story serial claim, not a system-wide lock, since 2026-09-29) and they can
    /// disagree, so the caller prints this figure only with its ledger attached. A claim whose age cannot be read is
    /// not a claim this reports.
    ///
    /// A STALE engine claim is excluded, not counted: a claim nobody has touched in 15 minutes is abandoned,
    /// and an abandoned claim is not a held one (active decision: abandoned-claim-is-not-running). The
    /// cleaner, not this read, is the authority on abandonment — so this read neither calls it held nor
    /// cleans it up.
    pub async fn oldest_claim(&self) -> DbResult<Option<ClaimRow>> {
        sqlx::query_as::<_, ClaimRow>(
            "select ledger, reference, age_ms from (
               select 'agent_work_item' as ledger,
                      id::text as reference,
                      (extract(epoch from (now() - coalesce(claimed_at, updated_at))) * 1000)::bigint as age_ms
               from agent_work_item
               where state in ('Claimed', 'Running', 'Paused')
               union all
               select 'forge_engine_task_execution' as ledger,
                      latest.story_id as reference,
                      (extract(epoch from (now() - coalesce(latest.updated_at, latest.created_at))) * 1000)::bigint as age_ms
               from (
                 select distinct on (e.story_id) e.story_id, e.status, e.updated_at, e.created_at
                 from forge_engine_task_execution e
                 order by e.story_id, e.created_at desc
               ) latest
               where latest.status not in ('completed', 'failed', 'interrupted')
                 and coalesce(latest.updated_at, latest.created_at) >= now() - interval '15 minutes'
             ) claims
             where age_ms is not null
             order by age_ms desc
             limit 1",
        )
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_doctor.oldest_claim", &error))
    }
}

/// The control plane's counts.
///
/// `open_work_items` is the QUEUE — every `agent_work_item` that is not terminal, which includes `Ready` — and
/// `claimed_work_items` is the subset actually HELD. They are separate fields because the doctor's
/// "active claims" line must not add the queue to the claims: a queued story is waiting for a claim, it is not
/// holding one. Measured 2026-09-29: the two were summed, so `forge doctor` printed `active claims: 8` on a
/// plane whose own next line said `oldest claim: none` (seven queued stories plus one stale item), and
/// AGENTS.md's "check nothing is in flight" gate reads that number.
#[derive(Debug, Clone, Copy, FromRow)]
pub struct ControlPlaneCounts {
    pub instances: i64,
    pub open_tasks: i64,
    pub open_work_items: i64,
    pub claimed_work_items: i64,
}

/// The oldest held claim: which ledger, which row, how old.
#[derive(Debug, Clone, FromRow)]
pub struct ClaimRow {
    pub ledger: String,
    pub reference: String,
    pub age_ms: Option<i64>,
}
