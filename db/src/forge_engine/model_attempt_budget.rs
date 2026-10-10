//! Durable model-attempt budget operations for the ForgeEngineDao facade.

use super::{
    ForgeEngineDao, ModelAttemptBudget, ModelAttemptReservation, ModelGenerationBudget,
    ModelGenerationReservation,
};
use crate::{DbFailure, DbResult};

impl ForgeEngineDao {
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
