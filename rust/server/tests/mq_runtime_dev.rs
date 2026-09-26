//! MQ-01 runtime proofs against the DEV database.
//!
//! Ignored by default; run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p server --test mq_runtime_dev -- --ignored
//!
//! Every test uses unique event/subscription IDs and deletes only rows it creates.
//! PROD is never selected.

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use db::{Database, DbTarget, DomainEventOutboxDao, OutboxDelivery, OutboxEventInput};
use serde_json::json;
use server::{MqRuntime, MqRuntimeConfig, MqSubscriber, MqSubscriberError};
use service::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceInfrastructure,
};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
struct TestSubscriber {
    id: String,
    routing_key: String,
    max_attempts: i32,
    retry_backoff_seconds: i32,
    fail: bool,
}

#[async_trait]
impl MqSubscriber for TestSubscriber {
    fn id(&self) -> &str {
        &self.id
    }

    fn routing_key(&self) -> &str {
        &self.routing_key
    }

    fn max_attempts(&self) -> i32 {
        self.max_attempts
    }

    fn retry_backoff_seconds(&self) -> i32 {
        self.retry_backoff_seconds
    }

    async fn handle(&self, _delivery: &OutboxDelivery) -> Result<(), MqSubscriberError> {
        if self.fail {
            return Err(MqSubscriberError::new(format!(
                "deliberate failure from {}",
                self.id
            )));
        }
        Ok(())
    }
}

fn infrastructure() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
}

async fn dev_db() -> Database {
    Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV must point to the DEV database")
}

fn subscriber(
    id: String,
    routing_key: String,
    max_attempts: i32,
    retry_backoff_seconds: i32,
    fail: bool,
) -> Arc<dyn MqSubscriber> {
    Arc::new(TestSubscriber {
        id,
        routing_key,
        max_attempts,
        retry_backoff_seconds,
        fail,
    })
}

async fn runtime(
    db: &Database,
    subscribers: Vec<Arc<dyn MqSubscriber>>,
    claim_limit: i64,
) -> MqRuntime {
    let outbox = DomainEventOutboxDao::new(db.clone());
    for item in &subscribers {
        outbox
            .register_subscription(
                item.id(),
                item.routing_key(),
                item.max_attempts(),
                item.retry_backoff_seconds(),
            )
            .await
            .unwrap();
    }

    MqRuntime::with_config(
        outbox,
        subscribers,
        infrastructure(),
        CancellationToken::new(),
        MqRuntimeConfig {
            worker_id: format!("mq-dev-test-{}", Uuid::new_v4()),
            poll_interval: Duration::from_secs(60),
            lease_duration: Duration::from_secs(30),
            claim_limit,
            max_concurrency: 4,
        },
    )
    .unwrap()
}

async fn append_event(db: &Database, event_id: &str, event_type: &str) {
    let outbox = DomainEventOutboxDao::new(db.clone());
    let mut tx = db.begin("mq.runtime.dev.append").await.unwrap();
    outbox
        .append_tx(
            &mut tx,
            &[OutboxEventInput {
                event_id: event_id.to_owned(),
                event_type: event_type.to_owned(),
                actor_app_user_id: None,
                aggregate_type: "MqRuntimeDevProof".into(),
                aggregate_id: format!("agg-{event_id}"),
                correlation_id: Some(format!("corr-{event_id}")),
                causation_id: Some(format!("cause-{event_id}")),
                occurred_at: Utc::now().to_rfc3339(),
                payload: json!({ "proof": true, "eventId": event_id }),
            }],
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

async fn cleanup(db: &Database, event_ids: &[String], subscription_ids: &[String]) {
    for event_id in event_ids {
        sqlx::query("delete from outbox_message where id=$1::uuid")
            .bind(event_id)
            .execute(db.pool())
            .await
            .unwrap();
    }
    for subscription_id in subscription_ids {
        sqlx::query("delete from mq_subscription where id=$1")
            .bind(subscription_id)
            .execute(db.pool())
            .await
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn mq_dev_fanout_isolates_failure_and_acknowledges_per_subscriber() {
    let db = dev_db().await;
    let tag = Uuid::new_v4().simple().to_string();
    let event_id = Uuid::new_v4().to_string();
    let routing = format!("mq.dev.fanout.{tag}");
    let failed_id = format!("mq-dev-fail-{tag}");
    let ok_id = format!("mq-dev-ok-{tag}");

    let runtime = runtime(
        &db,
        vec![
            subscriber(failed_id.clone(), routing.clone(), 1, 1, true),
            subscriber(ok_id.clone(), routing.clone(), 3, 1, false),
        ],
        20,
    )
    .await;
    append_event(&db, &event_id, &routing).await;

    runtime.dispatch_once().await.unwrap();

    let rows: Vec<(String, String, i32, Option<chrono::DateTime<Utc>>, Option<String>)> =
        sqlx::query_as(
            r#"
            select subscription_id, state, attempt_count, acknowledged_at, last_error
            from mq_delivery
            where message_id=$1::uuid
            order by subscription_id
            "#,
        )
        .bind(&event_id)
        .fetch_all(db.pool())
        .await
        .unwrap();

    cleanup(
        &db,
        std::slice::from_ref(&event_id),
        &[failed_id.clone(), ok_id.clone()],
    )
    .await;

    assert_eq!(rows.len(), 2);
    let failed = rows.iter().find(|row| row.0 == failed_id).unwrap();
    let ok = rows.iter().find(|row| row.0 == ok_id).unwrap();
    assert_eq!((failed.1.as_str(), failed.2), ("dead", 1));
    assert!(failed.3.is_none());
    assert!(failed
        .4
        .as_deref()
        .is_some_and(|value| value.contains("deliberate failure")));
    assert_eq!((ok.1.as_str(), ok.2), ("delivered", 1));
    assert!(ok.3.is_some());
    assert!(ok.4.is_none());
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn mq_dev_failed_delivery_retries_then_enters_dead_state() {
    let db = dev_db().await;
    let tag = Uuid::new_v4().simple().to_string();
    let event_id = Uuid::new_v4().to_string();
    let routing = format!("mq.dev.retry.{tag}");
    let subscription_id = format!("mq-dev-retry-{tag}");

    let runtime = runtime(
        &db,
        vec![subscriber(
            subscription_id.clone(),
            routing.clone(),
            2,
            0,
            true,
        )],
        20,
    )
    .await;
    append_event(&db, &event_id, &routing).await;

    runtime.dispatch_once().await.unwrap();
    let first: (String, i32, Option<String>) = sqlx::query_as(
        "select state, attempt_count, last_error from mq_delivery where message_id=$1::uuid and subscription_id=$2",
    )
    .bind(&event_id)
    .bind(&subscription_id)
    .fetch_one(db.pool())
    .await
    .unwrap();

    sqlx::query(
        "update mq_delivery set available_at=now()-interval '1 second' where message_id=$1::uuid and subscription_id=$2",
    )
    .bind(&event_id)
    .bind(&subscription_id)
    .execute(db.pool())
    .await
    .unwrap();

    runtime.dispatch_once().await.unwrap();
    let second: (String, i32, Option<String>) = sqlx::query_as(
        "select state, attempt_count, last_error from mq_delivery where message_id=$1::uuid and subscription_id=$2",
    )
    .bind(&event_id)
    .bind(&subscription_id)
    .fetch_one(db.pool())
    .await
    .unwrap();

    cleanup(
        &db,
        std::slice::from_ref(&event_id),
        std::slice::from_ref(&subscription_id),
    )
    .await;

    assert_eq!((first.0.as_str(), first.1), ("failed", 1));
    assert!(first.2.is_some());
    assert_eq!((second.0.as_str(), second.1), ("dead", 2));
    assert!(second.2.is_some());
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn mq_dev_stale_lease_is_reclaimed_and_attempt_count_advances() {
    let db = dev_db().await;
    let tag = Uuid::new_v4().simple().to_string();
    let event_id = Uuid::new_v4().to_string();
    let routing = format!("mq.dev.lease.{tag}");
    let subscription_id = format!("mq-dev-lease-{tag}");
    let outbox = DomainEventOutboxDao::new(db.clone());

    let runtime = runtime(
        &db,
        vec![subscriber(
            subscription_id.clone(),
            routing.clone(),
            3,
            1,
            false,
        )],
        20,
    )
    .await;
    append_event(&db, &event_id, &routing).await;

    let first_claim = outbox
        .claim_batch(
            "crashed-worker",
            std::slice::from_ref(&subscription_id),
            1,
            Utc::now() - ChronoDuration::seconds(1),
        )
        .await
        .unwrap();
    assert_eq!(first_claim.len(), 1);
    assert_eq!(first_claim[0].attempt_count, 1);

    runtime.dispatch_once().await.unwrap();
    let row: (String, i32, Option<String>) = sqlx::query_as(
        "select state, attempt_count, claimed_by from mq_delivery where message_id=$1::uuid and subscription_id=$2",
    )
    .bind(&event_id)
    .bind(&subscription_id)
    .fetch_one(db.pool())
    .await
    .unwrap();

    cleanup(
        &db,
        std::slice::from_ref(&event_id),
        std::slice::from_ref(&subscription_id),
    )
    .await;

    assert_eq!((row.0.as_str(), row.1), ("delivered", 2));
    assert!(row.2.is_none());
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn mq_dev_duplicate_event_append_is_idempotent() {
    let db = dev_db().await;
    let event_id = Uuid::new_v4().to_string();
    let routing = format!("mq.dev.duplicate.{}", Uuid::new_v4().simple());

    append_event(&db, &event_id, &routing).await;
    append_event(&db, &event_id, &routing).await;

    let count: i64 =
        sqlx::query_scalar("select count(*) from outbox_message where id=$1::uuid")
            .bind(&event_id)
            .fetch_one(db.pool())
            .await
            .unwrap();

    cleanup(&db, std::slice::from_ref(&event_id), &[]).await;
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn mq_dev_runtime_never_claims_subscription_it_does_not_own() {
    let db = dev_db().await;
    let tag = Uuid::new_v4().simple().to_string();
    let event_id = Uuid::new_v4().to_string();
    let routing = format!("mq.dev.scope.{tag}");
    let owned_id = format!("mq-dev-owned-{tag}");
    let foreign_id = format!("mq-dev-foreign-{tag}");
    let outbox = DomainEventOutboxDao::new(db.clone());

    let runtime = runtime(
        &db,
        vec![subscriber(
            owned_id.clone(),
            routing.clone(),
            3,
            1,
            false,
        )],
        20,
    )
    .await;
    outbox
        .register_subscription(&foreign_id, &routing, 3, 1)
        .await
        .unwrap();
    append_event(&db, &event_id, &routing).await;

    sqlx::query(
        "insert into mq_delivery (message_id, subscription_id) values ($1::uuid, $2) on conflict do nothing",
    )
    .bind(&event_id)
    .bind(&foreign_id)
    .execute(db.pool())
    .await
    .unwrap();

    runtime.dispatch_once().await.unwrap();

    let owned: (String, i32) = sqlx::query_as(
        "select state, attempt_count from mq_delivery where message_id=$1::uuid and subscription_id=$2",
    )
    .bind(&event_id)
    .bind(&owned_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let foreign: (String, i32) = sqlx::query_as(
        "select state, attempt_count from mq_delivery where message_id=$1::uuid and subscription_id=$2",
    )
    .bind(&event_id)
    .bind(&foreign_id)
    .fetch_one(db.pool())
    .await
    .unwrap();

    cleanup(
        &db,
        std::slice::from_ref(&event_id),
        &[owned_id.clone(), foreign_id.clone()],
    )
    .await;

    assert_eq!((owned.0.as_str(), owned.1), ("delivered", 1));
    assert_eq!((foreign.0.as_str(), foreign.1), ("pending", 0));
}
