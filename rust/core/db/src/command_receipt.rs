use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct CommandReceiptRow {
    pub command_id: String,
    pub outcome: String,
    pub aggregate_id: Option<String>,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub actor_app_user_id: Option<String>,
    pub command_type: Option<String>,
    pub request_fingerprint: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub aggregate_type: Option<String>,
    pub requested_at: Option<DateTime<Utc>>,
    pub result_payload: Option<Value>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone)]
pub struct CommandReceiptDao {
    db: Database,
}

impl CommandReceiptDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn find(&self, command_id: &str) -> DbResult<Option<CommandReceiptRow>> {
        sqlx::query_as::<_, CommandReceiptRow>(
            r#"
            select command_id, outcome, aggregate_id, message, created_at,
                   actor_app_user_id::text as actor_app_user_id,
                   command_type, request_fingerprint, correlation_id, causation_id, aggregate_type,
                   requested_at, result_payload, error_code, error_message
            from workflow_command_receipt
            where command_id=$1
            limit 1
            "#,
        )
        .bind(command_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("command_receipt.find", &error))
    }

    pub async fn find_tx(
        &self,
        tx: &mut DbTransaction,
        command_id: &str,
    ) -> DbResult<Option<CommandReceiptRow>> {
        sqlx::query_as::<_, CommandReceiptRow>(
            r#"
            select command_id, outcome, aggregate_id, message, created_at,
                   actor_app_user_id::text as actor_app_user_id,
                   command_type, request_fingerprint, correlation_id, causation_id, aggregate_type,
                   requested_at, result_payload, error_code, error_message
            from workflow_command_receipt
            where command_id=$1
            limit 1
            "#,
        )
        .bind(command_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("command_receipt.find_tx", &error))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn claim_tx(
        &self,
        tx: &mut DbTransaction,
        command_id: &str,
        command_type: &str,
        request_fingerprint: &str,
        actor_app_user_id: Option<&str>,
        aggregate_type: &str,
        aggregate_id: Option<&str>,
        correlation_id: Option<&str>,
        causation_id: Option<&str>,
        requested_at: &str,
    ) -> DbResult<bool> {
        let inserted = sqlx::query_scalar::<_, String>(
            r#"
            insert into workflow_command_receipt (
                command_id, outcome, aggregate_id, message, actor_app_user_id,
                command_type, correlation_id, causation_id, aggregate_type,
                requested_at, updated_at
            )
            values (
                $1, 'pending', $5, null, $3,
                $2, $6, $7, $4, $8::timestamptz, now()
            )
            on conflict(command_id) do nothing
            returning command_id
            "#,
        )
        .bind(command_id)
        .bind(command_type)
        .bind(request_fingerprint)
        .bind(actor_app_user_id)
        .bind(aggregate_type)
        .bind(aggregate_id)
        .bind(correlation_id)
        .bind(causation_id)
        .bind(requested_at)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("command_receipt.claim", &error))?;
        Ok(inserted.is_some())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn finalize_tx(
        &self,
        tx: &mut DbTransaction,
        command_id: &str,
        outcome: &str,
        aggregate_id: Option<&str>,
        message: Option<&str>,
        result_payload: Option<&Value>,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            update workflow_command_receipt
            set outcome=$2,
                aggregate_id=$3,
                message=$4,
                result_payload=$5,
                error_code=$6,
                error_message=$7,
                updated_at=now()
            where command_id=$1
            "#,
        )
        .bind(command_id)
        .bind(outcome)
        .bind(aggregate_id)
        .bind(message)
        .bind(result_payload)
        .bind(error_code)
        .bind(error_message)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("command_receipt.finalize", &error))?;

        if result.rows_affected() != 1 {
            return Err(DbFailure::schema_mismatch(
                "command_receipt.finalize",
                format!("No claimed receipt exists for command {command_id}"),
            ));
        }
        Ok(())
    }
}
