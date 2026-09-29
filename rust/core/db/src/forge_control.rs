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

    pub async fn stale_agent_work(
        &self,
        stale_after_minutes: i64,
    ) -> DbResult<Vec<StaleAgentWorkRow>> {
        sqlx::query_as::<_, StaleAgentWorkRow>(
            "select id::text as id, story_id, role, attempts, max_attempts, story_run_id::text as story_run_id, updated_at::text as updated_at
             from agent_work_item
             where state in ('Claimed','Running','Paused')
               and updated_at < now() - ($1::text || ' minutes')::interval
             order by updated_at asc",
        )
        .bind(stale_after_minutes.max(0).to_string())
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.stale_agent_work", &error))
    }

    pub async fn interrupt_story_run(
        &self,
        run_id: &str,
        failure_code: &str,
        reason: &str,
    ) -> DbResult<()> {
        sqlx::query(
            "update storyboard_story_run
             set ended_at=now(), result_status='Interrupted', failure_code=$2,
                 notes=case when notes is null or notes='' then $3 else notes || E'\\n' || $3 end,
                 updated_at=now()
             where id=$1::uuid and ended_at is null",
        )
        .bind(run_id)
        .bind(failure_code)
        .bind(reason)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_control.interrupt_story_run", &error))?;
        Ok(())
    }

    pub async fn hold_stale_work(&self, id: &str, story_id: &str, reason: &str) -> DbResult<()> {
        let mut tx = self.db.begin("forge_control.hold_stale_work").await?;
        let result = async {
            sqlx::query(
                "update agent_work_item
                 set state='Error', error_text=$2, finished_at=now(), updated_at=now()
                 where id=$1::uuid and state in ('Claimed','Running','Paused')",
            )
            .bind(id)
            .bind(reason)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_control.hold_stale_work.item", &error))?;
            sqlx::query(
                "update storyboard_story set status='Hold', completed_at=null, updated_at=now() where id=$1",
            )
            .bind(story_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_control.hold_stale_work.story", &error))?;
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

    /// Give a stale claim back to the queue — but only to a story that can still be worked.
    ///
    /// This is `db::settlement_pair`'s rule pointing the other way: a requeue moves **both** rows or neither. The
    /// version that shipped on 2026-09-29 set the story back to `Ready` whatever the board said, so a claim left
    /// behind by a run whose work had already landed — or by a run a human had put on `Hold` — was requeued and the
    /// story was run a second time. That is the mirror image of the `forge:clean` strand, and it is why the board is
    /// read here first.
    pub async fn requeue_stale_work(&self, id: &str, story_id: &str) -> DbResult<()> {
        let mut tx = self.db.begin("forge_control.requeue_stale_work").await?;
        let result = async {
            let board: Option<String> = sqlx::query_scalar(
                "select s.status
                   from agent_work_item i
                   join storyboard_story s on s.id = i.story_id
                  where i.id=$1::uuid and i.state in ('Claimed','Running','Paused')
                  for update of i",
            )
            .bind(id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_control.requeue_stale_work.read", &error)
            })?;
            let Some(board) = board else {
                return Ok::<(), DbFailure>(());
            };
            match board.as_str() {
                // The work landed. A stale row is bookkeeping, not a reason to run the story again.
                "Complete" => {
                    sqlx::query(
                        "update agent_work_item
                            set state='Done', error_text=null, finished_at=now(), updated_at=now()
                          where id=$1::uuid and state in ('Claimed','Running','Paused')",
                    )
                    .bind(id)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("forge_control.requeue_stale_work.completed", &error)
                    })?;
                }
                // A human gate owns the story: do not reopen it, and do not leave a claim hanging on it either.
                "Hold" => {
                    sqlx::query(
                        "update agent_work_item
                            set state='Error', error_text='stale claim on a story a human holds',
                                finished_at=now(), updated_at=now()
                          where id=$1::uuid and state in ('Claimed','Running','Paused')",
                    )
                    .bind(id)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("forge_control.requeue_stale_work.held", &error)
                    })?;
                }
                // A run the board still expects: retry it, and move the story with it.
                _ => {
                    sqlx::query(
                        "update agent_work_item
                         set state='Ready', queued_at=now(), claimed_at=null, claimed_by=null,
                             started_at=null, finished_at=null, error_text=null, runtime_adapter=null,
                             external_run_id=null, updated_at=now()
                         where id=$1::uuid and state in ('Claimed','Running','Paused')",
                    )
                    .bind(id)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("forge_control.requeue_stale_work.item", &error)
                    })?;
                    sqlx::query(
                        "update storyboard_story set status='Ready', completed_at=null, updated_at=now() where id=$1",
                    )
                    .bind(story_id)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("forge_control.requeue_stale_work.story", &error)
                    })?;
                }
            }
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
            let member = async {
                sqlx::query(
                    "update storyboard_story set status='Ready', updated_at=now() where id=$1",
                )
                .bind(&story)
                .execute(self.db.pool())
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx("forge_control.flight_member.story_ready", &error)
                })?;
                sqlx::query(
                    "update forge_batch_item set state='Queued', queued_at=now(), error_text=null
                     where batch_id=$1::uuid and story_id=$2",
                )
                .bind(batch_id)
                .bind(&story)
                .execute(self.db.pool())
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx("forge_control.flight_member.queued", &error)
                })?;
                let affected = sqlx::query(
                    "update agent_work_item set kind=$2, model_policy=$3, updated_at=now()
                     where story_id=$1 and state='Ready'",
                )
                .bind(&story)
                .bind(&kind)
                .bind(&policy)
                .execute(self.db.pool())
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx("forge_control.flight_member.routing", &error)
                })?
                .rows_affected();
                Ok::<u64, DbFailure>(affected)
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
