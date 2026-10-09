use crate::{Database, DbFailure, DbResult};
use sqlx::{FromRow, Row};

#[derive(Debug, Clone, FromRow)]
pub struct StaleAgentWorkRow {
    pub id: String,
    pub story_id: String,
    pub role: Option<String>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub story_run_id: Option<String>,
    pub updated_at: String,
    pub claimed_by: Option<String>,
    pub claim_generation: i64,
    pub state: String,
    pub heartbeat_at: Option<String>,
    pub lease_expires_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleRecoveryResult {
    Recovered,
    NoLongerStale,
    OwnershipChanged,
    TerminalPreserved,
    Conflict,
}

impl StaleRecoveryResult {
    pub(crate) fn from_db(value: &str) -> DbResult<Self> {
        match value {
            "recovered" => Ok(Self::Recovered),
            "no_longer_stale" => Ok(Self::NoLongerStale),
            "ownership_changed" => Ok(Self::OwnershipChanged),
            "terminal_preserved" => Ok(Self::TerminalPreserved),
            "conflict" => Ok(Self::Conflict),
            other => Err(DbFailure::schema_mismatch(
                "forge_control.stale_recovery.result",
                format!("unexpected database result {other:?}"),
            )),
        }
    }
}

/// The operator's brake and the fleet-wide ceiling — `forge_runtime_control`, one row (migration 274).
///
/// Read by the worker before it claims, because `forge_claim_story` (275) answers "no" to three different questions
/// (nothing eligible, paused, ceiling reached) and a pass that reports an idle queue while 500 rows wait is the
/// report that made this queue unoperable.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeRuntimeControlRow {
    pub paused: bool,
    /// The version the executor should be running, or `None` when nothing is pinned.
    pub desired_worker_sha: Option<String>,
    pub global_story_concurrency: i32,
    /// Who last wrote the row: a pause nobody signed cannot be asked about.
    pub updated_by: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct LearnStaleClaimRow {
    pub id: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct FlightFireResult {
    pub batch_id: String,
    pub queued: u64,
    pub stamped: u64,
    pub skipped: u64,
}

#[derive(Clone)]
pub struct ForgeControlDao {
    db: Database,
}

impl ForgeControlDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Claims a worker has walked away from: the one place the staleness rule is decided (`forge_reapable_claims`,
    /// migration 275), so a caller reads a list instead of re-deciding what "stale" means.
    ///
    /// Two reasons, and the second is the one this exists to add: the claim is older than `stale_after_minutes`
    /// (the original rule, kept for a claim whose owner never beats — a one-shot run, a test, a manual claim), **or**
    /// the worker holding it is `stale` in `forge_worker_health` (273), which is the worker's own word about itself.
    /// Before that second rule a live worker in a long model turn was indistinguishable from a dead one.
    pub async fn stale_agent_work(
        &self,
        stale_after_minutes: i64,
    ) -> DbResult<Vec<StaleAgentWorkRow>> {
        sqlx::query_as::<_, StaleAgentWorkRow>(
            "select id::text as id, story_id, role, attempts, max_attempts,
                    story_run_id::text as story_run_id, updated_at::text as updated_at,
                    claimed_by, claim_generation, state,
                    heartbeat_at::text as heartbeat_at, lease_expires_at::text as lease_expires_at
             from forge_reapable_claims($1::integer)",
        )
        .bind(stale_after_minutes.clamp(0, i32::MAX as i64) as i32)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.stale_agent_work", &error))
    }

    /// The operator's brake, the fleet ceiling and the pinned version — the row `forge_claim_story` obeys.
    ///
    /// `Ok(None)` means the row is absent, which 275 treats as a closed door; the worker reports it rather than
    /// assuming "not paused".
    pub async fn runtime_control(&self) -> DbResult<Option<ForgeRuntimeControlRow>> {
        sqlx::query_as::<_, ForgeRuntimeControlRow>(
            "select paused, desired_worker_sha, global_story_concurrency, updated_by
             from forge_runtime_control
             where id = 1",
        )
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.runtime_control", &error))
    }

    /// One resident-worker heartbeat (`forge_worker_beat`, migration 273). A beat that cannot be written is the
    /// caller's to report: a missed beat reads as `stale` in the view, which is the signal, so this returns the failure
    /// instead of swallowing it.
    #[allow(clippy::too_many_arguments)]
    pub async fn worker_beat(
        &self,
        worker_id: &str,
        host: &str,
        git_sha: &str,
        state: &str,
        running: i32,
        last_error: Option<&str>,
        started_at: chrono::DateTime<chrono::Utc>,
    ) -> DbResult<()> {
        sqlx::query("select forge_worker_beat($1, $2, $3, $4, $5, $6, $7)")
            .bind(worker_id)
            .bind(host)
            .bind(git_sha)
            .bind(state)
            .bind(running)
            .bind(last_error)
            .bind(started_at)
            .execute(self.db.pool())
            .await
            .map(|_| ())
            .map_err(|error| DbFailure::from_sqlx("forge_control.worker_beat", &error))
    }

    /// Apply stale recovery using the exact candidate snapshot selected by `forge_reapable_claims`.
    pub async fn recover_stale_work(
        &self,
        row: &StaleAgentWorkRow,
        stale_after_minutes: i64,
        reason: &str,
        failure_code: &str,
    ) -> DbResult<StaleRecoveryResult> {
        let result: String = sqlx::query_scalar(
            "select forge_recover_stale_work(
                 $1::uuid, $2, $3, $4, $5, $6::timestamptz, $7::timestamptz,
                 $8::timestamptz, $9, $10, $11)",
        )
        .bind(&row.id)
        .bind(&row.story_id)
        .bind(&row.claimed_by)
        .bind(row.claim_generation)
        .bind(&row.state)
        .bind(&row.updated_at)
        .bind(&row.heartbeat_at)
        .bind(&row.lease_expires_at)
        .bind(stale_after_minutes.clamp(1, i32::MAX as i64) as i32)
        .bind(reason)
        .bind(failure_code)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.recover_stale_work", &error))?;
        StaleRecoveryResult::from_db(&result)
    }

    /// Compatibility entry point for older callers. It still takes a fresh, explicit snapshot and delegates to the
    /// fenced database transition; runtime recovery passes its original candidate snapshot instead.
    pub async fn hold_stale_work(&self, id: &str, story_id: &str, reason: &str) -> DbResult<()> {
        let _ = self
            .recover_legacy_snapshot(id, story_id, reason, "HUMAN_DECISION_REQUIRED")
            .await?;
        Ok(())
    }

    /// Give a stale claim back to the queue — but only to a story that can still be worked.
    ///
    /// This is `forge_settlement_pair`'s rule (migration 263) pointing the other way: a requeue moves **both** rows
    /// or neither. The version that shipped on 2026-09-29 set the story back to `Ready` whatever the board said, so a
    /// claim left behind by a run whose work had already landed — or by a run a human had put on `Hold` — was
    /// requeued and the story was run a second time. That is the mirror image of the `forge:clean` strand, and it is
    /// why the board is read first.
    pub async fn requeue_stale_work(&self, id: &str, story_id: &str) -> DbResult<()> {
        let _ = self
            .recover_legacy_snapshot(
                id,
                story_id,
                "stale claim recovered",
                "HUMAN_DECISION_REQUIRED",
            )
            .await?;
        Ok(())
    }

    async fn recover_legacy_snapshot(
        &self,
        id: &str,
        story_id: &str,
        reason: &str,
        failure_code: &str,
    ) -> DbResult<StaleRecoveryResult> {
        let row = sqlx::query_as::<_, StaleAgentWorkRow>(
            "select id::text as id, story_id, role, attempts, max_attempts,
                    story_run_id::text as story_run_id, updated_at::text as updated_at,
                    claimed_by, claim_generation, state, heartbeat_at::text as heartbeat_at,
                    lease_expires_at::text as lease_expires_at
             from agent_work_item where id=$1::uuid and story_id=$2",
        )
        .bind(id)
        .bind(story_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.recovery_snapshot", &error))?;
        let Some(row) = row else {
            return Ok(StaleRecoveryResult::Conflict);
        };
        self.recover_stale_work(&row, 1, reason, failure_code).await
    }

    pub async fn due_flight_ids(&self) -> DbResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            "select id::text from forge_batch
             where status='Scheduled' and scheduled_for is not null and scheduled_for <= now()
             order by scheduled_for asc",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.due_flight_ids", &error))
    }

    pub async fn fire_flight(&self, batch_id: &str) -> DbResult<FlightFireResult> {
        let policy: Option<String> =
            sqlx::query_scalar("select model_policy from forge_batch where id=$1::uuid")
                .bind(batch_id)
                .fetch_optional(self.db.pool())
                .await
                .map_err(|error| DbFailure::from_sqlx("forge_control.flight_policy", &error))?
                .flatten();
        let policy = policy.unwrap_or_else(|| "cheap".into());

        let rows = sqlx::query(
            "select story_id, coalesce(kind,'normal') as kind
             from forge_batch_item where batch_id=$1::uuid and state='Staged' order by story_id",
        )
        .bind(batch_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.flight_members", &error))?;

        let mut queued = 0u64;
        let mut stamped = 0u64;
        let mut skipped = 0u64;
        for row in rows {
            let story: String = row.try_get("story_id").map_err(|error| {
                DbFailure::schema_mismatch("forge_control.flight_member.story", error.to_string())
            })?;
            let kind: String = row.try_get("kind").unwrap_or_else(|_| "normal".into());
            // ONE TRANSACTION PER MEMBER, AND THE ORDER INSIDE IT IS LOAD-BEARING (migration 268).
            //
            // The item is queued BEFORE the story leaves `Batched`. The obvious order -- the story moves, then its
            // item follows -- commits a state in which the story is `Ready` while the item this method just selected
            // is still `Staged`, and that is precisely the state 268's deferred constraint trigger reads as "the story
            // left the bench, withdraw its staged membership": the member would be deleted here, the following update
            // would match nothing, and this method would still count it as queued. So the item moves first (`Staged`
            // -> `Queued` writes `forge_batch_item`, and no trigger watches that table) and the story follows inside
            // the same commit, which is the state the rule is judged on. Both writes commit together or not at all, so
            // the intermediate order is invisible to every other reader and a crash cannot leave a member queued but
            // not ready -- nor the reverse, which is what the old autocommit sequence allowed.
            let member = async {
                let mut tx = self.db.begin("forge_control.fire_flight_member").await?;
                let result = async {
                    sqlx::query(
                        "update forge_batch_item set state='Queued', queued_at=now(), error_text=null
                         where batch_id=$1::uuid and story_id=$2",
                    )
                    .bind(batch_id)
                    .bind(&story)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("forge_control.flight_member.queued", &error)
                    })?;
                    sqlx::query(
                        "update storyboard_story set status='Ready', updated_at=now() where id=$1",
                    )
                    .bind(&story)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("forge_control.flight_member.story_ready", &error)
                    })?;
                    let affected = sqlx::query(
                        "update agent_work_item set kind=$2, model_policy=$3, updated_at=now()
                         where story_id=$1 and state='Ready'",
                    )
                    .bind(&story)
                    .bind(&kind)
                    .bind(&policy)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("forge_control.flight_member.routing", &error)
                    })?
                    .rows_affected();
                    Ok::<u64, DbFailure>(affected)
                }
                .await;
                match result {
                    Ok(affected) => {
                        tx.commit().await?;
                        Ok(affected)
                    }
                    Err(error) => {
                        let _ = tx.rollback().await;
                        Err(error)
                    }
                }
            }
            .await;

            match member {
                Ok(affected) => {
                    queued += 1;
                    stamped += affected;
                }
                Err(error) => {
                    skipped += 1;
                    sqlx::query(
                        "update forge_batch_item set state='Skipped', error_text=$3
                         where batch_id=$1::uuid and story_id=$2",
                    )
                    .bind(batch_id)
                    .bind(&story)
                    .bind(error.to_string().chars().take(2000).collect::<String>())
                    .execute(self.db.pool())
                    .await
                    .map_err(|sql_error| {
                        DbFailure::from_sqlx("forge_control.flight_member.skipped", &sql_error)
                    })?;
                }
            }
        }

        sqlx::query(
            "update forge_batch set status='Fired', fired_at=now() where id=$1::uuid and status<>'Fired'",
        )
        .bind(batch_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.flight_fired", &error))?;

        Ok(FlightFireResult {
            batch_id: batch_id.to_owned(),
            queued,
            stamped,
            skipped,
        })
    }

    // `next_ready_work` used to live here: a bare `select … where state='Ready' limit 1` that the worker dispatched
    // from without ever claiming. It is deleted rather than deprecated (2026-09-29) because the shape is the defect —
    // any caller of "find the next Ready row" is a caller that dispatches unowned work, and a second copy of that
    // selector is a second way back into the seam. Dispatch claims first: `ForgeEngineDao::claim_next_agent_work`.

    pub async fn stale_learn_claims(
        &self,
        stale_after_minutes: i64,
    ) -> DbResult<Vec<LearnStaleClaimRow>> {
        sqlx::query_as::<_, LearnStaleClaimRow>(
            "select id::text as id, updated_at::text as updated_at
             from agent_work_item
             where state in ('Claimed','Running','Paused')
               and updated_at < now() - ($1::text || ' minutes')::interval
             order by updated_at asc",
        )
        .bind(stale_after_minutes.max(0).to_string())
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.stale_learn_claims", &error))
    }

    pub async fn open_learn_pattern_keys(&self) -> DbResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            "select learn_pattern_key from agent_work_item
             where learn_pattern_key is not null and state in ('Ready','Claimed','Running','Paused')
             union
             select learn_pattern_key from forge_batch_item
             where learn_pattern_key is not null and state='Staged'",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.open_learn_pattern_keys", &error))
    }

    pub async fn ensure_staging_batch(&self) -> DbResult<String> {
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "select id::text from forge_batch where status='Staged' order by created_at desc limit 1",
        )
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.staging_batch.find", &error))?
        {
            return Ok(id);
        }
        sqlx::query_scalar::<_, String>(
            "insert into forge_batch(label,status,note)
             values('staging','Staged','built by Rust learn loop') returning id::text",
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.staging_batch.create", &error))
    }

    pub async fn create_learn_story(
        &self,
        id: &str,
        title: &str,
        priority: &str,
        notes: &str,
        goal: &str,
    ) -> DbResult<()> {
        sqlx::query(
            "insert into storyboard_story(id,workstream,title,priority,status,notes,goal,completion,rollup)
             values($1,'ENGINEERING',$2,$3,'Planned',$4,$5,0,true)
             on conflict(id) do nothing",
        )
        .bind(id)
        .bind(title)
        .bind(priority)
        .bind(notes)
        .bind(goal)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.create_learn_story", &error))?;
        Ok(())
    }

    pub async fn open_ready_learn_item(
        &self,
        story_id: &str,
        pattern_key: &str,
        instructions: &str,
    ) -> DbResult<()> {
        let mut tx = self.db.begin("forge_control.open_ready_learn_item").await?;
        let result = async {
            sqlx::query("update storyboard_story set status='Ready',updated_at=now() where id=$1")
                .bind(story_id)
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("forge_control.learn_story.ready", &error))?;
            sqlx::query(
                "update agent_work_item
                 set kind='learn',learn_pattern_key=$2,special_instructions=$3,updated_at=now()
                 where story_id=$1 and state='Ready'",
            )
            .bind(story_id)
            .bind(pattern_key)
            .bind(instructions)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_control.learn_item.ready", &error))?;
            Ok::<(), DbFailure>(())
        }
        .await;
        match result {
            Ok(()) => tx.commit().await,
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn stage_learn_item(
        &self,
        batch_id: &str,
        story_id: &str,
        pattern_key: &str,
    ) -> DbResult<()> {
        let mut tx = self.db.begin("forge_control.stage_learn_item").await?;
        let result = async {
            sqlx::query(
                "update storyboard_story set status='Batched',updated_at=now() where id=$1",
            )
            .bind(story_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_control.learn_story.batched", &error))?;
            sqlx::query(
                "insert into forge_batch_item(batch_id,story_id,state,kind,learn_pattern_key)
                 values($1::uuid,$2,'Staged','learn',$3)
                 on conflict(batch_id,story_id) do nothing",
            )
            .bind(batch_id)
            .bind(story_id)
            .bind(pattern_key)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_control.learn_item.staged", &error))?;
            Ok::<(), DbFailure>(())
        }
        .await;
        match result {
            Ok(()) => tx.commit().await,
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }
}

/// Wait for a `forge_work` NOTIFY on `url`, or until `timeout` elapses (which is `Ok`: a pass is due anyway).
///
/// `url` must be the DIRECT (non-pooler) endpoint: PgBouncer in transaction mode silently drops LISTEN. This opens its
/// own connection by design — a listener holds one for its whole life and must not be a pooled one — which is why it
/// lives here, beside the DAOs, and Forge never links a driver. A listener fault is an `Err`: notifications sent while
/// disconnected are lost, so the caller backs off and then runs a pass immediately.
pub async fn wait_for_forge_work(url: &str, timeout: std::time::Duration) -> DbResult<()> {
    let mut listener = sqlx::postgres::PgListener::connect(url)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_control.wait_for_forge_work.connect", &error)
        })?;
    listener.listen("forge_work").await.map_err(|error| {
        DbFailure::from_sqlx("forge_control.wait_for_forge_work.listen", &error)
    })?;
    match tokio::time::timeout(timeout, listener.recv()).await {
        Ok(Ok(_notification)) => Ok(()),
        Ok(Err(error)) => Err(DbFailure::from_sqlx(
            "forge_control.wait_for_forge_work.recv",
            &error,
        )),
        Err(_elapsed) => Ok(()),
    }
}
