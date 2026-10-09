//! The Forge reset / recover / clean writer.
//!
//! Rust replacement for `scripts/forge-story-reset.ts` + `scripts/forge-story-reset-config.ts`, which import
//! `legacy/db/*` (deleted with the TypeScript application in `4cf98110`) and so exited
//! `ERR_MODULE_NOT_FOUND`. `pnpm forge:clean` is named in the handbook as pre-run hygiene and could not run.
//!
//! THREE MODES, and they are not interchangeable:
//!
//!   reset   — ABORT the story's active engine instance, obsolete its open tasks, cancel its open work items,
//!             interrupt the engine claims it just killed, and return the story to `Planned`. The JUNK step is
//!             the engine-claim one: without it a reset leaves every in-flight `forge_engine_task_execution`
//!             row in `claimed` forever, and the next run has to be read against a control plane still holding
//!             the previous run's claims (15 accumulated in one afternoon: 9 lead_pre, 4 architect, 2
//!             fast_smith, hours stale). A story reset that leaves claims behind is not a reset.
//!   recover — RELEASE stale claims so an existing instance RESUMES, for a partial run whose worker died.
//!   clean   — control-plane hygiene across every story: stale open work items, stale engine claims through
//!             the SAME recovery path the engine uses, stale instances, tasks under terminal instances, and a
//!             mop-up for any claim still open on an instance that is no longer running.
//!
//! SAFETY, and this is the whole reason `clean` can be run on a shared control plane: only claims OLDER THAN
//! the caller's stale window are touched, so a live peer survives untouched. The post-condition is printed
//! every time, because a sweep you cannot read the result of is a sweep you will run twice and trust neither
//! time.
//!
//! THE DATABASE TARGET IS NOT A CHOICE this layer makes: the pool decides it, and the CLI refuses anything but
//! PROD plus an explicit `--force`. See `cli/src/forge/reset.rs`.

use crate::{Database, DbFailure, DbResult, StaleRecoveryResult};
use sqlx::FromRow;

/// How much one mode changed, in the order it was printed.
#[derive(Debug, Clone, Default)]
pub struct ResetReport {
    /// `(step label, rows touched)`, in the order the steps ran.
    pub steps: Vec<(&'static str, u64)>,
    /// The post-condition: what is still open afterwards.
    pub remaining: RemainingCounts,
}

/// The post-condition counters, story-scoped or control-plane wide.
#[derive(Debug, Clone, Copy, Default, FromRow)]
pub struct RemainingCounts {
    pub instances: i64,
    pub open_tasks: i64,
    pub open_work_items: i64,
    pub active_engine_claims: i64,
}

/// The one writer for reset / recover / clean.
#[derive(Clone)]
pub struct ForgeResetDao {
    db: Database,
}

impl ForgeResetDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// `reset`: make the story runnable from scratch, and close everything the last attempt left open.
    pub async fn reset_story(&self, story_id: &str) -> DbResult<ResetReport> {
        let mut steps = Vec::new();

        // 1. Abort any non-terminal engine instance for the story.
        let instances = self
            .execute(
                "update process_instances set status='aborted', updated_at=now()
                 where subject_id=$1 and status in ('active','running','reserved','suspended')",
                story_id,
            )
            .await?;
        steps.push(("aborted engine instances", instances));

        // 2. Obsolete open tasks under those instances.
        let tasks = self
            .execute(
                "update tasks set status='obsolete', updated_at=now()
                 where status in ('created','ready','reserved','in_progress')
                   and process_instance_id in (select id from process_instances where subject_id=$1)",
                story_id,
            )
            .await?;
        steps.push(("obsoleted open tasks", tasks));

        // 3. Cancel open work items for the story.
        let items = self
            .execute(
                "update agent_work_item set state='Cancelled', claimed_by=null, started_at=null,
                        finished_at=now(), updated_at=now()
                 where story_id=$1 and state in ('Ready','Claimed','Running')",
                story_id,
            )
            .await?;
        steps.push(("cancelled open work items", items));

        // 4. Close the engine task executions the reset just killed — see the module header.
        let claims = self
            .execute(
                "update forge_engine_task_execution
                    set status='interrupted', last_error='story reset', updated_at=now()
                  where status in ('claimed','running')
                    and process_instance_id in (select id from process_instances where subject_id=$1)",
                story_id,
            )
            .await?;
        steps.push(("interrupted engine task executions", claims));

        // 5. Return the story to Planned.
        let status = self
            .execute(
                "update storyboard_story set status='Planned', completion=0, forge_repair_attempts=0,
                        forge_replan_attempts=0, updated_at=now()
                 where id=$1",
                story_id,
            )
            .await?;
        steps.push(("story returned to Planned", status));

        Ok(ResetReport {
            steps,
            remaining: self.remaining_for_story(story_id).await?,
        })
    }

    /// `recover`: release the story's stale claims so its existing instance RESUMES, rather than starting over.
    pub async fn recover_story(&self, story_id: &str) -> DbResult<ResetReport> {
        let mut steps = Vec::new();

        let tasks = self
            .execute(
                "update tasks set status='ready', assignee=null, claimed_at=null, updated_at=now()
                 where status in ('reserved','in_progress')
                   and process_instance_id in (
                     select id from process_instances
                     where subject_id=$1 and status in ('active','running')
                   )",
                story_id,
            )
            .await?;
        steps.push(("released tasks to ready", tasks));

        let items = self.recover_story_stale_work(story_id, 15).await?;
        steps.push(("recovered stale running work items", items));

        Ok(ResetReport {
            steps,
            remaining: self.remaining_for_story(story_id).await?,
        })
    }

    /// `clean`: pre-test hygiene for the control plane, not for one story's chain. Only rows that have not
    /// moved since the stale cutoff are touched, so a live peer survives.
    pub async fn clean(&self, stale_minutes: i64) -> DbResult<ResetReport> {
        let stale_minutes = stale_minutes.max(1);
        let mut steps = Vec::new();

        // a. Open work items that have not moved since the cutoff — **and the stories that were still expecting
        //    them**. Cancelling the item alone leaves `Ready` beside `Cancelled`, and nothing dispatches that again:
        //    the claim consults both authorities, and the Ready trigger fires only on a *change* of board status.
        //    This is the writer whose output migration 258 had to repair, so it now moves the pair or neither half.
        let (items, held) = self.cancel_stale_open_items(stale_minutes).await?;
        steps.push(("cancelled stale open work items", items));
        if held > 0 {
            steps.push(("held stories whose stale work was cancelled", held));
        }

        // b. Stale engine claims, through the SAME recovery the engine itself uses — one implementation of
        //    "recovered", so the sweep cannot drift from what recovery means.
        let (recovered, skipped) = self.recover_stale_engine_claims(stale_minutes, 500).await?;
        steps.push(("interrupted stale engine claims", recovered));
        if skipped > 0 {
            steps.push(("engine claims skipped (instance not lockable)", skipped));
        }

        // c. Instances that have not moved since the cutoff.
        let instances = self
            .execute_with_stale_minutes(
                "update process_instances set status='aborted', updated_at=now()
                  where status in ('active','running','reserved','suspended')
                    and coalesce(updated_at, created_at) <= now() - ($1::text || ' minutes')::interval",
                stale_minutes,
            )
            .await?;
        steps.push(("aborted stale engine instances", instances));

        // d. Open workflow rows under instances that are now terminal.
        let tasks = self
            .execute_plain(
                "update tasks set status='obsolete', updated_at=now()
                  where status in ('created','ready','reserved','in_progress')
                    and process_instance_id in (
                      select id from process_instances
                      where status not in ('active','running','reserved','suspended')
                    )",
            )
            .await?;
        steps.push(("obsoleted open tasks under terminal instances", tasks));

        // Unmatched/legacy engine rows are left visible for operator review. A broad mop-up here used to interrupt
        // executions after their item authority had changed, bypassing the exact-generation recovery fence.

        Ok(ResetReport {
            steps,
            remaining: self.remaining_control_plane().await?,
        })
    }

    /// Recover stale `claimed` / `running` engine claims — the engine's OWN recovery path, in Rust.
    ///
    /// Returns `(recovered, skipped)`. Idempotent and observable: a row that recovered is one this returned.
    ///
    /// `RETURNING task_id` IS LOAD-BEARING. The port of this function must keep the row count: the first
    /// version of the check read an UPDATE that returned nothing, so every recovery reported a false CAS miss
    /// and returned early — skipping the work-item release below while the row itself was quietly interrupted.
    /// Observed live on 2026-09-13: a clean sweep reported "0 recovered, 15 skipped" moments after stamping all
    /// fifteen rows 'stale claim recovered'. A sweep whose result cannot be trusted is worse than no sweep,
    /// because the operator stops looking.
    pub async fn recover_stale_engine_claims(
        &self,
        stale_minutes: i64,
        limit: i64,
    ) -> DbResult<(u64, u64)> {
        #[derive(FromRow)]
        struct StaleClaim {
            task_id: String,
            work_item_id: String,
            story_id: String,
            worker_id: String,
            claim_generation: i64,
            item_state: String,
            item_updated_at: String,
            item_heartbeat_at: Option<String>,
            item_lease_expires_at: Option<String>,
            execution_status: String,
            heartbeat_at: String,
        }

        // Candidates first (not under lock), newest heartbeat last so the oldest is handled first.
        let stale = sqlx::query_as::<_, StaleClaim>(
            "select e.task_id::text as task_id, e.work_item_id::text as work_item_id,
                    i.story_id, i.claimed_by as worker_id, i.claim_generation,
                    i.state as item_state, i.updated_at::text as item_updated_at,
                    i.heartbeat_at::text as item_heartbeat_at,
                    i.lease_expires_at::text as item_lease_expires_at,
                    e.status as execution_status, e.heartbeat_at::text as heartbeat_at
             from forge_engine_task_execution e join agent_work_item i on i.id=e.work_item_id
             where e.status in ('claimed', 'running')
               and e.claim_generation=i.claim_generation and e.worker_id=i.claimed_by
               and i.state in ('Claimed','Running','Paused')
               and e.heartbeat_at <= now() - ($1::text || ' minutes')::interval
             order by e.heartbeat_at asc
             limit $2",
        )
        .bind(stale_minutes.max(1).to_string())
        .bind(limit.clamp(1, 5_000))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_reset.stale_claims.list", &error))?;

        let mut recovered = 0u64;
        let mut skipped = 0u64;
        // Each recovery is its own transaction. Migration 279 locks and validates item, story and exact execution.
        for claim in stale {
            let result: String = sqlx::query_scalar(
                "select forge_recover_stale_work($1::uuid,$2,$3,$4,$5,$6::timestamptz,$7::timestamptz,$8::timestamptz,$9,$10,$11,$12::uuid,$13,$14::timestamptz)",
            )
            .bind(&claim.work_item_id)
            .bind(&claim.story_id)
            .bind(&claim.worker_id)
            .bind(claim.claim_generation)
            .bind(&claim.item_state)
            .bind(&claim.item_updated_at)
            .bind(&claim.item_heartbeat_at)
            .bind(&claim.item_lease_expires_at)
            .bind(stale_minutes.max(1) as i32)
            .bind("stale engine claim recovered")
            .bind("ENGINE_RUNTIME_INTERRUPTED")
            .bind(&claim.task_id)
            .bind(&claim.execution_status)
            .bind(&claim.heartbeat_at)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_reset.recover_stale_claim", &error))?;
            if matches!(
                StaleRecoveryResult::from_db(&result)?,
                StaleRecoveryResult::Recovered | StaleRecoveryResult::TerminalPreserved
            ) {
                recovered += 1;
            } else {
                skipped += 1;
            }
        }
        Ok((recovered, skipped))
    }

    /// What is still open for one story, after a reset or a recover. Printed every time: a sweep you cannot
    /// read the result of is a sweep you will run twice and trust neither time.
    async fn remaining_for_story(&self, story_id: &str) -> DbResult<RemainingCounts> {
        sqlx::query_as::<_, RemainingCounts>(
            "select
               (select count(*) from process_instances
                 where subject_id=$1 and status in ('active','running','reserved','suspended')) as instances,
               (select count(*) from tasks
                 where status in ('created','ready','reserved','in_progress')
                   and process_instance_id in (select id from process_instances where subject_id=$1)) as open_tasks,
               (select count(*) from agent_work_item
                 where story_id=$1 and state in ('Ready','Claimed','Running','Paused')) as open_work_items,
               (select count(*) from forge_engine_task_execution
                 where status in ('claimed','running')
                   and process_instance_id in (select id from process_instances where subject_id=$1)) as active_engine_claims",
        )
        .bind(story_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_reset.remaining_for_story", &error))
    }

    async fn recover_story_stale_work(&self, story_id: &str, stale_minutes: i64) -> DbResult<u64> {
        let candidates = sqlx::query_as::<_, crate::StaleAgentWorkRow>(
            "select id::text as id, story_id, role, attempts, max_attempts,
                    story_run_id::text as story_run_id, updated_at::text as updated_at,
                    claimed_by, claim_generation, state, heartbeat_at::text as heartbeat_at,
                    lease_expires_at::text as lease_expires_at
               from forge_reapable_claims($1::integer) where story_id=$2",
        )
        .bind(stale_minutes as i32)
        .bind(story_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_reset.recover_story.stale_list", &error))?;
        let mut recovered = 0u64;
        for row in candidates {
            let result: String = sqlx::query_scalar(
                "select forge_recover_stale_work($1::uuid,$2,$3,$4,$5,$6::timestamptz,$7::timestamptz,$8::timestamptz,$9,$10,$11)",
            ).bind(&row.id).bind(&row.story_id).bind(&row.claimed_by)
             .bind(row.claim_generation).bind(&row.state).bind(&row.updated_at)
             .bind(&row.heartbeat_at).bind(&row.lease_expires_at)
             .bind(stale_minutes as i32).bind("stale story claim recovered")
             .bind("ENGINE_RUNTIME_INTERRUPTED").fetch_one(self.db.pool()).await
             .map_err(|error| DbFailure::from_sqlx("forge_reset.recover_story.stale_item", &error))?;
            if matches!(
                StaleRecoveryResult::from_db(&result)?,
                StaleRecoveryResult::Recovered | StaleRecoveryResult::TerminalPreserved
            ) {
                recovered += 1;
            }
        }
        Ok(recovered)
    }

    /// What is still open across the whole control plane, after a `clean`.
    async fn remaining_control_plane(&self) -> DbResult<RemainingCounts> {
        sqlx::query_as::<_, RemainingCounts>(
            "select
               (select count(*) from process_instances
                 where status in ('active','running','reserved','suspended')) as instances,
               (select count(*) from tasks
                 where status in ('created','ready','reserved','in_progress')) as open_tasks,
               (select count(*) from agent_work_item
                 where state in ('Ready','Claimed','Running','Paused')) as open_work_items,
               (select count(*) from forge_engine_task_execution
                 where status in ('claimed','running')) as active_engine_claims",
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_reset.remaining_control_plane", &error))
    }

    /// One update, bound to the story, returning how many rows it touched.
    async fn execute(&self, sql: &'static str, story_id: &str) -> DbResult<u64> {
        sqlx::query(sql)
            .bind(story_id)
            .execute(self.db.pool())
            .await
            .map(|result| result.rows_affected())
            .map_err(|error| DbFailure::from_sqlx("forge_reset.execute", &error))
    }

    /// Snapshot stale open items, then cancel each under a generation/state/timestamp CAS and lock item before story.
    async fn cancel_stale_open_items(&self, stale_minutes: i64) -> DbResult<(u64, u64)> {
        #[derive(FromRow)]
        struct Candidate {
            id: String,
            story_id: String,
            claimed_by: Option<String>,
            claim_generation: i64,
            state: String,
            updated_at: String,
            heartbeat_at: Option<String>,
            lease_expires_at: Option<String>,
        }
        let rows = sqlx::query_as::<_, Candidate>(
            "select id::text as id, story_id, claimed_by, claim_generation, state,
                    updated_at::text as updated_at, heartbeat_at::text as heartbeat_at,
                    lease_expires_at::text as lease_expires_at
               from agent_work_item where state in ('Ready','Claimed','Running','Paused')
                and coalesce(updated_at,created_at) <= now()-($1::text || ' minutes')::interval
               order by updated_at asc",
        )
        .bind(stale_minutes.max(1).to_string())
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_reset.clean.open_work_items.list", &error))?;
        let mut cancelled = 0u64;
        let mut held = 0u64;
        for row in rows {
            let result: String = sqlx::query_scalar(
                "select forge_cancel_stale_open_work($1::uuid,$2,$3,$4,$5,$6::timestamptz,$7::timestamptz,$8::timestamptz,$9)",
            ).bind(&row.id).bind(&row.story_id).bind(&row.claimed_by)
             .bind(row.claim_generation).bind(&row.state).bind(&row.updated_at)
             .bind(&row.heartbeat_at).bind(&row.lease_expires_at)
             .bind(stale_minutes.max(1) as i32).fetch_one(self.db.pool()).await
             .map_err(|error| DbFailure::from_sqlx("forge_reset.clean.open_work_items.recover", &error))?;
            if result == "recovered" {
                cancelled += 1;
                held += 1;
            }
        }
        Ok((cancelled, held))
    }

    async fn execute_with_stale_minutes(&self, sql: &'static str, minutes: i64) -> DbResult<u64> {
        sqlx::query(sql)
            .bind(minutes.max(1).to_string())
            .execute(self.db.pool())
            .await
            .map(|result| result.rows_affected())
            .map_err(|error| DbFailure::from_sqlx("forge_reset.execute", &error))
    }

    /// One update with no parameter at all — the control-plane-wide sweeps.
    async fn execute_plain(&self, sql: &'static str) -> DbResult<u64> {
        sqlx::query(sql)
            .execute(self.db.pool())
            .await
            .map(|result| result.rows_affected())
            .map_err(|error| DbFailure::from_sqlx("forge_reset.execute", &error))
    }
}
