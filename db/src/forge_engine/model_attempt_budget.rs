//! Durable model-attempt budget operations for the ForgeEngineDao facade.

use super::{ForgeEngineDao, ModelAttemptBudget, ModelAttemptReservation};
use crate::{DbFailure, DbResult};

impl ForgeEngineDao {
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
