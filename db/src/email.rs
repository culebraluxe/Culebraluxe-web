use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use model::{
    EmailMessage, EmailMessageKind, EmailMessageStatus, EmailQueueResult, QueueEmailRequest,
};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct EmailRow {
    id: String,
    message_kind: String,
    recipient_email: String,
    template_key: String,
    template_payload: Value,
    dedupe_key: String,
    status: String,
    provider_message_id: Option<String>,
    attempt_count: i32,
    last_error: Option<String>,
    correlation_id: Option<String>,
    causation_id: Option<String>,
    queued_at: DateTime<Utc>,
    sent_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct EmailDao {
    db: Database,
}

impl EmailDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn get(&self, id: &str) -> DbResult<Option<EmailMessage>> {
        let row = sqlx::query_as::<_, EmailRow>(
            r#"
            select id::text as id, message_kind, recipient_email,
                   template_key, template_payload, dedupe_key, status,
                   provider_message_id, attempt_count, last_error,
                   correlation_id, causation_id, queued_at, sent_at, updated_at
              from email_message
             where id = $1::uuid
             limit 1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("email.get", &error))?;
        row.map(map_email).transpose()
    }

    pub async fn queue_tx(
        &self,
        tx: &mut DbTransaction,
        request: &QueueEmailRequest,
    ) -> DbResult<EmailQueueResult> {
        let inserted = sqlx::query_scalar::<_, String>(
            r#"
            insert into email_message (
                message_kind, recipient_email, template_key, template_payload,
                dedupe_key, status, correlation_id, causation_id
            )
            values ($1, $2, $3, $4, $5, 'queued', $6, $7)
            on conflict (dedupe_key) do nothing
            returning id::text
            "#,
        )
        .bind(request.message_kind.as_str())
        .bind(request.recipient_email.trim())
        .bind(request.template_key.trim())
        .bind(&request.template_payload)
        .bind(request.dedupe_key.trim())
        .bind(request.correlation_id.as_deref())
        .bind(request.causation_id.as_deref())
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("email.queue", &error))?;

        if let Some(message_id) = inserted {
            return Ok(EmailQueueResult {
                message_id,
                existing: false,
            });
        }

        let message_id = sqlx::query_scalar::<_, String>(
            "select id::text from email_message where dedupe_key = $1 limit 1",
        )
        .bind(request.dedupe_key.trim())
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("email.queue.existing", &error))?;

        Ok(EmailQueueResult {
            message_id,
            existing: true,
        })
    }

    pub async fn claim_for_delivery(&self, id: &str) -> DbResult<Option<EmailMessage>> {
        let row = sqlx::query_as::<_, EmailRow>(
            r#"
            update email_message
               set status = 'sending',
                   attempt_count = attempt_count + 1,
                   last_error = null
             where id = $1::uuid
               and status in ('queued','failed')
            returning id::text as id, message_kind, recipient_email,
                      template_key, template_payload, dedupe_key, status,
                      provider_message_id, attempt_count, last_error,
                      correlation_id, causation_id, queued_at, sent_at, updated_at
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("email.claim_delivery", &error))?;

        if let Some(row) = row {
            return map_email(row).map(Some);
        }

        self.get(id).await
    }

    pub async fn mark_sent(&self, id: &str, provider_message_id: Option<&str>) -> DbResult<()> {
        sqlx::query(
            r#"
            update email_message
               set status = 'sent',
                   provider_message_id = $2,
                   sent_at = coalesce(sent_at, now()),
                   last_error = null
             where id = $1::uuid
               and status in ('sending','sent')
            "#,
        )
        .bind(id)
        .bind(provider_message_id)
        .execute(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("email.mark_sent", &error))?;
        Ok(())
    }

    pub async fn mark_failed(&self, id: &str, error_message: &str, dead: bool) -> DbResult<()> {
        sqlx::query(
            r#"
            update email_message
               set status = $2,
                   last_error = $3
             where id = $1::uuid
               and status in ('sending','failed')
            "#,
        )
        .bind(id)
        .bind(if dead { "dead" } else { "failed" })
        .bind(error_message.chars().take(2000).collect::<String>())
        .execute(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("email.mark_failed", &error))?;
        Ok(())
    }
}

fn map_email(row: EmailRow) -> DbResult<EmailMessage> {
    Ok(EmailMessage {
        id: row.id,
        message_kind: EmailMessageKind::try_from(row.message_kind.as_str())
            .map_err(|error| DbFailure::schema_mismatch("email.map", error))?,
        recipient_email: row.recipient_email,
        template_key: row.template_key,
        template_payload: row.template_payload,
        dedupe_key: row.dedupe_key,
        status: EmailMessageStatus::try_from(row.status.as_str())
            .map_err(|error| DbFailure::schema_mismatch("email.map", error))?,
        provider_message_id: row.provider_message_id,
        attempt_count: row.attempt_count,
        last_error: row.last_error,
        correlation_id: row.correlation_id,
        causation_id: row.causation_id,
        queued_at: row.queued_at.to_rfc3339(),
        sent_at: row.sent_at.map(|value| value.to_rfc3339()),
        updated_at: row.updated_at.to_rfc3339(),
    })
}
