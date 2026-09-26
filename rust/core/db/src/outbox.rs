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
pub struct OutboxDelivery {
    pub delivery_id: String,
    pub event_id: String,
    pub subscription_id: String,
    pub event_type: String,
    pub actor_app_user_id: Option<String>,
    pub aggregate_type: Option<String>,
    pub aggregate_id: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub payload: Value,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub retry_backoff_seconds: i32,
    pub lease_until: Option<DateTime<Utc>>,
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
                insert into outbox_message (
                    id, event_type, aggregate_type, aggregate_id,
                    correlation_id, causation_id, actor_app_user_id,
                    occurred_at, payload
                )
                values (
                    $1::uuid,$2,$3,$4,$5,$6,$7,$8::timestamptz,$9
                )
                on conflict(id) do nothing
                "#,
            )
            .bind(&event.event_id)
            .bind(&event.event_type)
            .bind(&event.aggregate_type)
            .bind(&event.aggregate_id)
            .bind(event.correlation_id.as_deref())
            .bind(event.causation_id.as_deref())
            .bind(event.actor_app_user_id.as_deref())
            .bind(&event.occurred_at)
            .bind(&event.payload)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("outbox.append", &error))?;
        }
        Ok(())
    }

    pub async fn claim_batch(
        &self,
        worker_id: &str,
        limit: i64,
        lease_until: DateTime<Utc>,
    ) -> DbResult<Vec<OutboxDelivery>> {
        let mut tx = self.db.begin("outbox.claim_batch").await?;
        let result = async {
            sqlx::query(
                r#"
                insert into mq_delivery (message_id, subscription_id)
                select m.id, s.id
                from outbox_message m
                join mq_subscription s
                  on s.routing_key=m.event_type
                 and s.enabled
                on conflict(message_id, subscription_id) do nothing
                "#,
            )
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("outbox.materialize_deliveries", &error))?;

            let rows = sqlx::query_as::<_, OutboxDelivery>(
                r#"
                with candidate as (
                    select d.id
                    from mq_delivery d
                    where (
                            d.state in ('pending','failed')
                            and d.available_at <= now()
                          )
                       or (
                            d.state='claimed'
                            and d.lease_until <= now()
                          )
                    order by d.available_at, d.id
                    for update skip locked
                    limit $2
                ),
                claimed as (
                    update mq_delivery d
                    set state='claimed',
                        claimed_at=now(),
                        claimed_by=$1,
                        lease_until=$3,
                        attempt_count=d.attempt_count+1,
                        updated_at=now()
                    from candidate c
                    where d.id=c.id
                    returning d.id, d.message_id, d.subscription_id,
                              d.attempt_count, d.lease_until
                )
                select c.id::text as delivery_id,
                       m.id::text as event_id,
                       c.subscription_id,
                       m.event_type,
                       m.actor_app_user_id,
                       m.aggregate_type,
                       m.aggregate_id,
                       m.correlation_id,
                       m.causation_id,
                       m.occurred_at,
                       m.payload,
                       c.attempt_count,
                       s.max_attempts,
                       s.retry_backoff_seconds,
                       c.lease_until
                from claimed c
                join outbox_message m on m.id=c.message_id
                join mq_subscription s on s.id=c.subscription_id
                order by m.occurred_at, c.id
                "#,
            )
            .bind(worker_id)
            .bind(limit.clamp(1, 500))
            .bind(lease_until)
            .fetch_all(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("outbox.claim_deliveries", &error))?;
            Ok(rows)
        }
        .await;

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

    pub async fn mark_delivered(&self, event_id: &str, subscriber_id: &str) -> DbResult<()> {
        sqlx::query(
            r#"
            update mq_delivery
            set state='delivered',
                acknowledged_at=now(),
                lease_until=null,
                claimed_at=null,
                claimed_by=null,
                last_error=null,
                updated_at=now()
            where message_id=$1::uuid
              and subscription_id=$2
              and state='claimed'
            "#,
        )
        .bind(event_id)
        .bind(subscriber_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("outbox.mark_delivered", &error))?;
        Ok(())
    }

    pub async fn mark_failed(
        &self,
        delivery: &OutboxDelivery,
        error_message: &str,
    ) -> DbResult<bool> {
        let dead = delivery.attempt_count >= delivery.max_attempts;
        sqlx::query(
            r#"
            update mq_delivery
            set state=$3,
                last_error=$4,
                lease_until=null,
                claimed_at=null,
                claimed_by=null,
                available_at=case
                    when $3='dead' then available_at
                    else now() + make_interval(secs => $5)
                end,
                updated_at=now()
            where message_id=$1::uuid
              and subscription_id=$2
              and state='claimed'
            "#,
        )
        .bind(&delivery.event_id)
        .bind(&delivery.subscription_id)
        .bind(if dead { "dead" } else { "failed" })
        .bind(error_message.chars().take(2000).collect::<String>())
        .bind(delivery.retry_backoff_seconds)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("outbox.mark_failed", &error))?;
        Ok(dead)
    }

    pub async fn subscription_enabled(&self, subscriber_id: &str) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            select exists(
                select 1 from mq_subscription
                where id=$1 and enabled
            )
            "#,
        )
        .bind(subscriber_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("outbox.subscription_enabled", &error))
    }
}
