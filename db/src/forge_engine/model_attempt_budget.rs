//! Durable model-attempt budget operations for the ForgeEngineDao facade.

use super::{
    ForgeEngineDao, ModelAttemptBudget, ModelAttemptReservation, ModelGenerationBudget,
    ModelGenerationReservation,
};
use crate::{DbFailure, DbResult};

impl ForgeEngineDao {
    /// Settle one model attempt's usage exactly once. A later measured reading may upgrade an earlier
    /// explicit-unknown receipt; identical measured retries are no-ops and conflicting retries fail closed.
    pub async fn settle_model_generation_attempt_usage(
        &self,
        generation_id: &str,
        attempt_key: &str,
        story_run_id: &str,
        usage: Option<(&str, i64, i64, f64)>,
    ) -> DbResult<bool> {
        if let Some((session, input, output, cost)) = usage {
            if session.trim().is_empty()
                || input < 0
                || output < 0
                || !cost.is_finite()
                || cost < 0.0
            {
                return Err(DbFailure::schema_mismatch(
                    "forge_engine.model_attempt_usage",
                    "measured usage must have a session, nonnegative tokens, and finite nonnegative cost",
                ));
            }
        }
        let mut tx = self.db.pool().begin().await.map_err(|error| {
            DbFailure::from_sqlx("forge_engine.settle_model_usage.begin", &error)
        })?;
        let row = sqlx::query_as::<
            _,
            (
                String,
                bool,
                bool,
                Option<i64>,
                Option<i64>,
                Option<f64>,
                Option<String>,
            ),
        >(
            "select story_run_id::text, usage_settled, usage_known, usage_tokens_input,
                    usage_tokens_output, usage_cost_usd::float8, usage_session_id
               from forge_model_generation_attempt
              where generation_id=$1::uuid and attempt_key=$2
              for update",
        )
        .bind(generation_id)
        .bind(attempt_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.settle_model_usage.read", &error))?
        .ok_or_else(|| {
            DbFailure::schema_mismatch(
                "forge_engine.settle_model_usage.missing_attempt",
                format!("attempt {attempt_key} is missing from generation {generation_id}"),
            )
        })?;
        let (
            stored_run_id,
            settled,
            known,
            stored_input,
            stored_output,
            stored_cost,
            stored_session,
        ) = row;
        if stored_run_id != story_run_id {
            return Err(DbFailure::schema_mismatch(
                "forge_engine.settle_model_usage.run_identity",
                format!("attempt {attempt_key} belongs to run {stored_run_id}, not {story_run_id}"),
            ));
        }
        if settled && known {
            let Some((session, input, output, cost)) = usage else {
                tx.commit().await.map_err(|error| {
                    DbFailure::from_sqlx("forge_engine.settle_model_usage.commit", &error)
                })?;
                return Ok(false);
            };
            if stored_session.as_deref() == Some(session)
                && stored_input == Some(input)
                && stored_output == Some(output)
                && stored_cost.is_some_and(|stored| stored == cost)
            {
                tx.commit().await.map_err(|error| {
                    DbFailure::from_sqlx("forge_engine.settle_model_usage.commit", &error)
                })?;
                return Ok(false);
            }
            return Err(DbFailure::schema_mismatch(
                "forge_engine.settle_model_usage.conflict",
                format!("attempt {attempt_key} was already settled with different measured usage"),
            ));
        }
        if let Some((session, input, output, cost)) = usage {
            let input_i32 = i32::try_from(input).map_err(|_| {
                DbFailure::schema_mismatch(
                    "forge_engine.settle_model_usage.tokens",
                    "input token count exceeds run-total column range",
                )
            })?;
            let output_i32 = i32::try_from(output).map_err(|_| {
                DbFailure::schema_mismatch(
                    "forge_engine.settle_model_usage.tokens",
                    "output token count exceeds run-total column range",
                )
            })?;
            sqlx::query(
                "update forge_model_generation_attempt
                    set usage_settled=true, usage_known=true, usage_tokens_input=$3,
                        usage_tokens_output=$4, usage_cost_usd=$5::float8::numeric,
                        usage_session_id=$6, updated_at=now()
                  where generation_id=$1::uuid and attempt_key=$2",
            )
            .bind(generation_id)
            .bind(attempt_key)
            .bind(input)
            .bind(output)
            .bind(cost)
            .bind(session)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_engine.settle_model_usage.receipt", &error)
            })?;
            sqlx::query(
                "update storyboard_story_run
                    set tokens_input=coalesce(tokens_input, 0)+$2,
                        tokens_output=coalesce(tokens_output, 0)+$3,
                        cost_usd=coalesce(cost_usd, 0)+$4::float8::numeric,
                        cost_source=case when cost_source='widgets' then cost_source else 'vendor' end,
                        updated_at=now()
                  where id=$1::uuid",
            )
            .bind(story_run_id)
            .bind(input_i32)
            .bind(output_i32)
            .bind(cost)
            .execute(&mut *tx)
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.settle_model_usage.aggregate", &error))?;
            tx.commit().await.map_err(|error| {
                DbFailure::from_sqlx("forge_engine.settle_model_usage.commit", &error)
            })?;
            return Ok(true);
        }
        sqlx::query(
            "update forge_model_generation_attempt set usage_settled=true, usage_known=false,
                    usage_tokens_input=null, usage_tokens_output=null, usage_cost_usd=null,
                    usage_session_id=null, updated_at=now()
              where generation_id=$1::uuid and attempt_key=$2",
        )
        .bind(generation_id)
        .bind(attempt_key)
        .execute(&mut *tx)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.settle_model_usage.unknown", &error))?;
        tx.commit().await.map_err(|error| {
            DbFailure::from_sqlx("forge_engine.settle_model_usage.commit", &error)
        })?;
        Ok(false)
    }

    /// Persist one frozen allowance for a logical work generation. The caller passes the stable work-item
    /// UUID for scheduled work (or the Story Run UUID for an explicit direct run).
    pub async fn ensure_model_generation_budget(
        &self,
        generation_id: &str,
        story_id: &str,
        cap: i32,
    ) -> DbResult<ModelGenerationBudget> {
        sqlx::query(
            "insert into forge_model_generation_budget (generation_id, story_id, cap) \
             values ($1::uuid, $2, $3) on conflict (generation_id) do nothing",
        )
        .bind(generation_id)
        .bind(story_id)
        .bind(cap.clamp(1, 100))
        .execute(self.db.pool())
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.ensure_model_generation_budget", &error)
        })?;
        let budget = sqlx::query_as::<_, ModelGenerationBudget>(
            "select generation_id::text as generation_id, story_id, cap, used, uncertain \
             from forge_model_generation_budget where generation_id=$1::uuid",
        )
        .bind(generation_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.read_model_generation_budget", &error)
        })?;
        if budget.story_id != story_id {
            return Err(DbFailure::schema_mismatch(
                "forge_engine.model_generation_identity",
                format!(
                    "generation {generation_id} is already bound to story {}, not {story_id}",
                    budget.story_id
                ),
            ));
        }
        Ok(budget)
    }

    /// Atomically reserve one model launch from the shared logical-generation ledger.
    pub async fn reserve_model_generation_attempt(
        &self,
        generation_id: &str,
        story_id: &str,
        story_run_id: &str,
        cap: i32,
        task_id: &str,
        role_attempt: i32,
    ) -> DbResult<ModelGenerationReservation> {
        let mut tx = self.db.pool().begin().await.map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_generation.begin", &error)
        })?;
        sqlx::query(
            "insert into forge_model_generation_budget (generation_id, story_id, cap) \
             values ($1::uuid, $2, $3) on conflict (generation_id) do nothing",
        )
        .bind(generation_id)
        .bind(story_id)
        .bind(cap.clamp(1, 100))
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_generation.ensure", &error)
        })?;
        let budget = sqlx::query_as::<_, ModelGenerationBudget>(
            "select generation_id::text as generation_id, story_id, cap, used, uncertain \
             from forge_model_generation_budget where generation_id=$1::uuid for update",
        )
        .bind(generation_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_generation.lock", &error)
        })?;
        if budget.story_id != story_id {
            return Err(DbFailure::schema_mismatch(
                "forge_engine.model_generation_identity",
                format!(
                    "generation {generation_id} is already bound to story {}, not {story_id}",
                    budget.story_id
                ),
            ));
        }
        let attempt_key = format!("{task_id}:role-attempt:{role_attempt}");
        let duplicate = sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from forge_model_generation_attempt \
             where generation_id=$1::uuid and attempt_key=$2)",
        )
        .bind(generation_id)
        .bind(&attempt_key)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_generation.duplicate", &error)
        })?;
        if duplicate || budget.uncertain || budget.used >= budget.cap {
            tx.commit().await.map_err(|error| {
                DbFailure::from_sqlx("forge_engine.reserve_model_generation.commit", &error)
            })?;
            return Ok(ModelGenerationReservation {
                budget,
                attempt_key,
                authorized: false,
                duplicate,
            });
        }
        sqlx::query(
            "insert into forge_model_generation_attempt \
             (generation_id, story_run_id, attempt_key, task_id, role_attempt, status) \
             values ($1::uuid, $2::uuid, $3, $4, $5, 'authorized')",
        )
        .bind(generation_id)
        .bind(story_run_id)
        .bind(&attempt_key)
        .bind(task_id)
        .bind(role_attempt)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_generation.insert", &error)
        })?;
        let budget = sqlx::query_as::<_, ModelGenerationBudget>(
            "update forge_model_generation_budget set used=used+1, updated_at=now() \
             where generation_id=$1::uuid \
             returning generation_id::text as generation_id, story_id, cap, used, uncertain",
        )
        .bind(generation_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_generation.increment", &error)
        })?;
        tx.commit().await.map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_generation.commit", &error)
        })?;
        Ok(ModelGenerationReservation {
            budget,
            attempt_key,
            authorized: true,
            duplicate: false,
        })
    }

    pub async fn mark_model_generation_attempt(
        &self,
        generation_id: &str,
        attempt_key: &str,
        status: &str,
        detail: Option<&str>,
    ) -> DbResult<bool> {
        sqlx::query(
            "update forge_model_generation_attempt set status=$3, detail=$4, updated_at=now() \
             where generation_id=$1::uuid and attempt_key=$2",
        )
        .bind(generation_id)
        .bind(attempt_key)
        .bind(status)
        .bind(detail)
        .execute(self.db.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|error| DbFailure::from_sqlx("forge_engine.mark_model_generation_attempt", &error))
    }

    /// Persist the generation's resolved model-attempt cap once. Later
    /// environment changes and resume calls cannot alter it.
    pub async fn ensure_model_attempt_budget(
        &self,
        run_id: &str,
        cap: i32,
    ) -> DbResult<ModelAttemptBudget> {
        let cap = cap.clamp(1, 100);
        sqlx::query(
            "insert into forge_model_attempt_budget (story_run_id, cap) values ($1::uuid, $2) \
             on conflict (story_run_id) do nothing",
        )
        .bind(run_id)
        .bind(cap)
        .execute(self.db.pool())
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.ensure_model_attempt_budget", &error)
        })?;
        sqlx::query_as::<_, ModelAttemptBudget>(
            "select story_run_id::text as story_run_id, cap, used from forge_model_attempt_budget \
             where story_run_id = $1::uuid",
        )
        .bind(run_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.read_model_attempt_budget", &error))
    }

    /// Atomically reserve exactly one attempt. A duplicate identity or a full
    /// budget is refused; neither condition authorizes a harness call.
    pub async fn reserve_model_attempt(
        &self,
        run_id: &str,
        cap: i32,
        task_id: &str,
        role_attempt: i32,
    ) -> DbResult<ModelAttemptReservation> {
        let mut tx = self.db.pool().begin().await.map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_attempt.begin", &error)
        })?;
        sqlx::query(
            "insert into forge_model_attempt_budget (story_run_id, cap) values ($1::uuid, $2) \
             on conflict (story_run_id) do nothing",
        )
        .bind(run_id)
        .bind(cap.clamp(1, 100))
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_attempt.budget", &error)
        })?;
        let budget = sqlx::query_as::<_, ModelAttemptBudget>(
            "select story_run_id::text as story_run_id, cap, used from forge_model_attempt_budget \
             where story_run_id = $1::uuid for update",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.reserve_model_attempt.lock", &error))?;
        let attempt_key = format!("{task_id}:role-attempt:{role_attempt}");
        let duplicate = sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from forge_model_attempt \
             where story_run_id=$1::uuid and attempt_key=$2)",
        )
        .bind(run_id)
        .bind(&attempt_key)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_attempt.duplicate", &error)
        })?;
        if duplicate || budget.used >= budget.cap {
            tx.commit().await.map_err(|error| {
                DbFailure::from_sqlx("forge_engine.reserve_model_attempt.commit", &error)
            })?;
            return Ok(ModelAttemptReservation {
                budget,
                attempt_key,
                authorized: false,
                duplicate,
            });
        }
        sqlx::query(
            "insert into forge_model_attempt \
             (story_run_id, attempt_key, task_id, role_attempt, status) \
             values ($1::uuid, $2, $3, $4, 'authorized')",
        )
        .bind(run_id)
        .bind(&attempt_key)
        .bind(task_id)
        .bind(role_attempt)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_attempt.insert", &error)
        })?;
        let budget = sqlx::query_as::<_, ModelAttemptBudget>(
            "update forge_model_attempt_budget set used=used+1, updated_at=now() \
             where story_run_id=$1::uuid returning story_run_id::text as story_run_id, cap, used",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_attempt.increment", &error)
        })?;
        tx.commit().await.map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reserve_model_attempt.commit", &error)
        })?;
        Ok(ModelAttemptReservation {
            budget,
            attempt_key,
            authorized: true,
            duplicate: false,
        })
    }

    pub async fn mark_model_attempt(
        &self,
        run_id: &str,
        attempt_key: &str,
        status: &str,
        detail: Option<&str>,
    ) -> DbResult<bool> {
        sqlx::query(
            "update forge_model_attempt set status=$3, detail=$4, updated_at=now() \
             where story_run_id=$1::uuid and attempt_key=$2",
        )
        .bind(run_id)
        .bind(attempt_key)
        .bind(status)
        .bind(detail)
        .execute(self.db.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|error| DbFailure::from_sqlx("forge_engine.mark_model_attempt", &error))
    }
}
