use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, Clone)]
pub struct OutboxEventInput {
    pub event_id: String,
    pub event_type: String,
    pub actor_app_user_id: Option<String>,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub occurred_at: String,
    pub payload: Value,
}

#[derive(Debug, Clone, FromRow)]
pub struct OutboxRecord {
    pub event_id: String,
    pub event_type: String,
    pub actor_app_user_id: Option<String>,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub payload: Value,
    pub status: String,
    pub attempts: i32,
    pub lease_until: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct DomainEventOutboxDao {
    db: Database,
}

impl DomainEventOutboxDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn append_tx(
        &self,
        tx: &mut DbTransaction,
        events: &[OutboxEventInput],
    ) -> DbResult<()> {
        for event in events {
            sqlx::query(
                r#"
                insert into domain_event_outbox (
                    event_id, event_type, actor_app_user_id,
                    aggregate_type, aggregate_id,
                    correlation_id, causation_id, occurred_at, payload
                )
                values ($1,$2,$3::uuid,$4,$5,$6,$7,$8::timestamptz,$9)
                on conflict(event_id) do nothing
                "#,
            )
            .bind(&event.event_id)
            .bind(&event.event_type)
            .bind(event.actor_app_user_id.as_deref())
            .bind(&event.aggregate_type)
            .bind(&event.aggregate_id)
            .bind(event.correlation_id.as_deref())
            .bind(event.causation_id.as_deref())
            .bind(&event.occurred_at)
            .bind(&event.payload)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("domain_event_outbox.append", &error))?;
        }
        Ok(())
    }

    pub async fn claim_batch(
        &self,
        worker_id: &str,
        limit: i64,
        lease_until: DateTime<Utc>,
    ) -> DbResult<Vec<OutboxRecord>> {
        let limit = limit.clamp(1, 500);
        let mut tx = self.db.begin("domain_event_outbox.claim").await?;
        let result = sqlx::query_as::<_, OutboxRecord>(
            r#"
            with candidate as (
                select event_id
                from domain_event_outbox
                where status in ('pending','failed','delivering')
                  and (next_attempt_at is null or next_attempt_at <= now())
                  and (lease_until is null or lease_until < now())
                order by occurred_at, event_id
                for update skip locked
                limit $2
            )
            update domain_event_outbox o
            set status='delivering',
                attempts=o.attempts+1,
                lease_until=$3,
                locked_by=$1,
                updated_at=now()
            from candidate c
            where o.event_id=c.event_id
            returning o.event_id, o.event_type,
                      o.actor_app_user_id::text as actor_app_user_id,
                      o.aggregate_type, o.aggregate_id,
                      o.correlation_id, o.causation_id, o.occurred_at,
                      o.payload, o.status, o.attempts, o.lease_until,
                      o.locked_by, o.next_attempt_at, o.last_error
            "#,
        )
        .bind(worker_id)
        .bind(limit)
        .bind(lease_until)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("domain_event_outbox.claim", &error));

        match result {
            Ok(rows) => {
                tx.commit().await?;
                Ok(rows)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn has_delivered(
        &self,
        event_id: &str,
        subscriber_id: &str,
    ) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            select exists(
                select 1
                from domain_event_delivery_receipt
                where event_id=$1 and subscriber_id=$2
            )
            "#,
        )
        .bind(event_id)
        .bind(subscriber_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("domain_event_outbox.has_delivered", &error))
    }

    pub async fn record_delivered(
        &self,
        event_id: &str,
        subscriber_id: &str,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into domain_event_delivery_receipt(event_id, subscriber_id)
            values($1,$2)
            on conflict(event_id, subscriber_id) do nothing
            "#,
        )
        .bind(event_id)
        .bind(subscriber_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("domain_event_outbox.record_delivered", &error))?;
        Ok(())
    }

    pub async fn mark_event_delivered(&self, event_id: &str) -> DbResult<()> {
        sqlx::query(
            r#"
            update domain_event_outbox
            set status='delivered', lease_until=null, locked_by=null,
                next_attempt_at=null, last_error=null, updated_at=now()
            where event_id=$1
            "#,
        )
        .bind(event_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("domain_event_outbox.mark_delivered", &error))?;
        Ok(())
    }

    pub async fn mark_failed(
        &self,
        event_id: &str,
        error_message: &str,
        next_attempt_at: Option<DateTime<Utc>>,
        dead_letter: bool,
    ) -> DbResult<()> {
        let status = if dead_letter { "dead_letter" } else { "failed" };
        sqlx::query(
            r#"
            update domain_event_outbox
            set status=$2, lease_until=null, locked_by=null,
                next_attempt_at=$3, last_error=$4, updated_at=now()
            where event_id=$1
            "#,
        )
        .bind(event_id)
        .bind(status)
        .bind(next_attempt_at)
        .bind(error_message)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("domain_event_outbox.mark_failed", &error))?;
        Ok(())
    }
}
