use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use db::{DbFailure, DomainEventOutboxDao, OutboxDelivery};
use service::{
    ServiceActor, ServiceActorKind, ServiceContext, ServiceDispatchError, ServiceHealth,
    ServiceInfrastructure, ServiceRuntime, ServiceStatus,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::{watch, Semaphore},
    task::JoinSet,
    time::{interval, MissedTickBehavior},
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use uuid::Uuid;

const MQ_PROOF_SUBSCRIPTION_ID: &str = "mq-proof";
const MQ_PROOF_ROUTING_KEY: &str = "mq.proof";

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct MqSubscriberError {
    message: String,
}

impl MqSubscriberError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[async_trait]
pub trait MqSubscriber: Send + Sync {
    fn id(&self) -> &str;
    fn routing_key(&self) -> &str;
    fn max_attempts(&self) -> i32;
    fn retry_backoff_seconds(&self) -> i32;

    async fn handle(&self, delivery: &OutboxDelivery) -> Result<(), MqSubscriberError>;
}

#[derive(Clone)]
pub struct MqSubscriberRegistry {
    entries: Arc<HashMap<String, Arc<dyn MqSubscriber>>>,
}

impl MqSubscriberRegistry {
    pub fn new(subscribers: Vec<Arc<dyn MqSubscriber>>) -> Result<Self, ServiceDispatchError> {
        let mut entries = HashMap::new();
        for subscriber in subscribers {
            let id = subscriber.id().trim();
            let routing_key = subscriber.routing_key().trim();
            if id.is_empty()
                || routing_key.is_empty()
                || subscriber.max_attempts() < 1
                || subscriber.retry_backoff_seconds() < 0
            {
                return Err(ServiceDispatchError::infrastructure(
                    "MQ_SUBSCRIBER_CONFIG_INVALID",
                    "MQ subscriber id/routing key must be non-empty, max_attempts >= 1, and retry backoff >= 0.",
                    false,
                ));
            }
            if entries.insert(id.to_owned(), subscriber).is_some() {
                return Err(ServiceDispatchError::infrastructure(
                    "MQ_SUBSCRIBER_DUPLICATE",
                    format!("MQ subscriber is registered more than once: {id}"),
                    false,
                ));
            }
        }
        Ok(Self {
            entries: Arc::new(entries),
        })
    }

    fn get(&self, subscriber_id: &str) -> Option<Arc<dyn MqSubscriber>> {
        self.entries.get(subscriber_id).cloned()
    }

    fn ids(&self) -> Vec<String> {
        let mut ids = self.entries.keys().cloned().collect::<Vec<_>>();
        ids.sort();
        ids
    }

    fn subscribers(&self) -> Vec<Arc<dyn MqSubscriber>> {
        let mut subscribers = self.entries.values().cloned().collect::<Vec<_>>();
        subscribers.sort_by(|a, b| a.id().cmp(b.id()));
        subscribers
    }
}

#[derive(Debug, Clone)]
pub struct MqRuntimeConfig {
    pub worker_id: String,
    pub poll_interval: Duration,
    pub lease_duration: Duration,
    pub claim_limit: i64,
    pub max_concurrency: usize,
}

impl Default for MqRuntimeConfig {
    fn default() -> Self {
        Self {
            worker_id: format!("rust-mq-{}", Uuid::new_v4()),
            poll_interval: Duration::from_secs(2),
            lease_duration: Duration::from_secs(300),
            claim_limit: 20,
            max_concurrency: 8,
        }
    }
}

#[derive(Clone)]
pub struct MqRuntime {
    outbox: DomainEventOutboxDao,
    registry: MqSubscriberRegistry,
    observer: ServiceRuntime,
    cancel: CancellationToken,
    tracker: TaskTracker,
    status_tx: watch::Sender<ServiceStatus>,
    status: watch::Receiver<ServiceStatus>,
    started: Arc<AtomicBool>,
    in_flight: Arc<AtomicUsize>,
    config: MqRuntimeConfig,
}

impl MqRuntime {
    pub fn new(
        outbox: DomainEventOutboxDao,
        subscribers: Vec<Arc<dyn MqSubscriber>>,
        infrastructure: ServiceInfrastructure,
        parent_cancel: CancellationToken,
    ) -> Result<Self, ServiceDispatchError> {
        Self::with_config(
            outbox,
            subscribers,
            infrastructure,
            parent_cancel,
            MqRuntimeConfig::default(),
        )
    }

    pub fn with_config(
        outbox: DomainEventOutboxDao,
        subscribers: Vec<Arc<dyn MqSubscriber>>,
        infrastructure: ServiceInfrastructure,
        parent_cancel: CancellationToken,
        config: MqRuntimeConfig,
    ) -> Result<Self, ServiceDispatchError> {
        if config.poll_interval.is_zero()
            || config.lease_duration.is_zero()
            || config.claim_limit < 1
            || config.max_concurrency < 1
        {
            return Err(ServiceDispatchError::infrastructure(
                "MQ_RUNTIME_CONFIG_INVALID",
                "MQ poll interval, lease duration, claim limit and max concurrency must be greater than zero.",
                false,
            ));
        }

        let registry = MqSubscriberRegistry::new(subscribers)?;
        let (status_tx, status) = watch::channel(ServiceStatus::Starting);
        Ok(Self {
            outbox,
            registry,
            observer: ServiceRuntime::new(infrastructure),
            cancel: parent_cancel.child_token(),
            tracker: TaskTracker::new(),
            status_tx,
            status,
            started: Arc::new(AtomicBool::new(false)),
            in_flight: Arc::new(AtomicUsize::new(0)),
            config,
        })
    }

    pub async fn start(&self) -> Result<(), ServiceDispatchError> {
        if self.started.swap(true, Ordering::AcqRel) {
            return self.wait_running().await;
        }

        for subscriber in self.registry.subscribers() {
            if let Err(error) = self
                .outbox
                .register_subscription(
                    subscriber.id(),
                    subscriber.routing_key(),
                    subscriber.max_attempts(),
                    subscriber.retry_backoff_seconds(),
                )
                .await
            {
                let dispatch = db_error("MQ_SUBSCRIPTION_REGISTER_FAILED", error);
                let _ = self.status_tx.send(ServiceStatus::Failed);
                self.observe_worker_failure(&dispatch).await;
                return Err(dispatch);
            }
        }

        let worker = self.clone();
        let supervisor = self.clone();
        self.tracker.spawn(async move {
            let outcome = tokio::spawn(async move { worker.run_loop().await }).await;
            match outcome {
                Ok(Ok(())) => {
                    let _ = supervisor.status_tx.send(ServiceStatus::Stopped);
                }
                Ok(Err(error)) => {
                    supervisor.observe_worker_failure(&error).await;
                    let _ = supervisor.status_tx.send(ServiceStatus::Failed);
                }
                Err(error) => {
                    let dispatch = ServiceDispatchError::OperationPanicked {
                        domain: "mq".into(),
                        operation: "worker".into(),
                        message: error.to_string(),
                    };
                    supervisor.observe_worker_failure(&dispatch).await;
                    let _ = supervisor.status_tx.send(ServiceStatus::Failed);
                }
            }
        });
        let _ = self.status_tx.send(ServiceStatus::Running);
        Ok(())
    }

    pub fn health(&self) -> ServiceHealth {
        ServiceHealth {
            status: *self.status.borrow(),
            accepting: *self.status.borrow() == ServiceStatus::Running && !self.cancel.is_cancelled(),
            queued: 0,
            in_flight: self.in_flight.load(Ordering::Relaxed),
        }
    }

    pub fn begin_shutdown(&self) {
        self.cancel.cancel();
    }

    pub async fn shutdown(&self) -> Result<(), ServiceDispatchError> {
        self.begin_shutdown();
        self.wait_stopped().await
    }

    pub async fn wait_running(&self) -> Result<(), ServiceDispatchError> {
        let mut status = self.status.clone();
        loop {
            match *status.borrow_and_update() {
                ServiceStatus::Running => return Ok(()),
                ServiceStatus::Starting => {
                    status.changed().await.map_err(|_| {
                        ServiceDispatchError::infrastructure(
                            "MQ_RUNTIME_STOPPED",
                            "MQ runtime status channel closed during startup.",
                            true,
                        )
                    })?;
                }
                ServiceStatus::Failed => {
                    return Err(ServiceDispatchError::infrastructure(
                        "MQ_RUNTIME_FAILED",
                        "MQ runtime failed while starting.",
                        false,
                    ));
                }
                current => {
                    return Err(ServiceDispatchError::infrastructure(
                        "MQ_RUNTIME_NOT_STARTABLE",
                        format!("MQ runtime cannot start from lifecycle state {current:?}."),
                        false,
                    ));
                }
            }
        }
    }

    pub async fn wait_stopped(&self) -> Result<(), ServiceDispatchError> {
        let mut status = self.status.clone();
        loop {
            match *status.borrow_and_update() {
                ServiceStatus::Stopped => {
                    self.tracker.close();
                    self.tracker.wait().await;
                    return Ok(());
                }
                ServiceStatus::Failed => {
                    self.tracker.close();
                    self.tracker.wait().await;
                    return Err(ServiceDispatchError::infrastructure(
                        "MQ_RUNTIME_FAILED",
                        "MQ runtime terminated in Failed state.",
                        false,
                    ));
                }
                _ => {
                    status.changed().await.map_err(|_| {
                        ServiceDispatchError::infrastructure(
                            "MQ_RUNTIME_STOPPED",
                            "MQ runtime status channel closed before shutdown completed.",
                            true,
                        )
                    })?;
                }
            }
        }
    }

    async fn run_loop(&self) -> Result<(), ServiceDispatchError> {
        let mut ticker = interval(self.config.poll_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => return Ok(()),
                _ = ticker.tick() => {
                    if let Err(error) = self.dispatch_once().await {
                        self.observe_worker_failure(&error).await;
                        if !error.retryable() {
                            return Err(error);
                        }
                    }
                }
            }
        }
    }

    async fn dispatch_once(&self) -> Result<(), ServiceDispatchError> {
        let lease = ChronoDuration::from_std(self.config.lease_duration).map_err(|error| {
            ServiceDispatchError::infrastructure(
                "MQ_LEASE_DURATION_INVALID",
                error.to_string(),
                false,
            )
        })?;
        let subscriber_ids = self.registry.ids();
        let deliveries = self
            .outbox
            .claim_batch(
                &self.config.worker_id,
                &subscriber_ids,
                self.config.claim_limit,
                Utc::now() + lease,
            )
            .await
            .map_err(|error| db_error("MQ_CLAIM_FAILED", error))?;

        if deliveries.is_empty() {
            return Ok(());
        }

        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrency));
        let mut jobs = JoinSet::new();
        for delivery in deliveries {
            let permit = semaphore.clone().acquire_owned().await.map_err(|_| {
                ServiceDispatchError::infrastructure(
                    "MQ_WORKER_CLOSED",
                    "MQ worker concurrency gate closed unexpectedly.",
                    true,
                )
            })?;
            let runtime = self.clone();
            jobs.spawn(async move {
                let _permit = permit;
                runtime.in_flight.fetch_add(1, Ordering::Relaxed);
                let result = runtime.process_delivery(delivery).await;
                runtime.in_flight.fetch_sub(1, Ordering::Relaxed);
                result
            });
        }

        let mut first_error = None;
        while let Some(result) = jobs.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
                Err(error) => {
                    let dispatch = ServiceDispatchError::OperationPanicked {
                        domain: "mq".into(),
                        operation: "delivery".into(),
                        message: error.to_string(),
                    };
                    if first_error.is_none() {
                        first_error = Some(dispatch);
                    }
                }
            }
        }

        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }

    async fn process_delivery(&self, delivery: OutboxDelivery) -> Result<(), ServiceDispatchError> {
        let Some(subscriber) = self.registry.get(&delivery.subscription_id) else {
            let error = ServiceDispatchError::infrastructure(
                "MQ_SUBSCRIBER_NOT_REGISTERED",
                format!(
                    "Claimed delivery {} for subscriber {} that is not owned by this runtime.",
                    delivery.delivery_id, delivery.subscription_id
                ),
                true,
            );
            self.fail_delivery(&delivery, &error).await?;
            return Ok(());
        };

        if subscriber.routing_key() != delivery.event_type {
            let error = ServiceDispatchError::infrastructure(
                "MQ_ROUTING_CONTRACT_MISMATCH",
                format!(
                    "Subscriber {} expects routing key {}, but delivery {} carries {}.",
                    subscriber.id(),
                    subscriber.routing_key(),
                    delivery.delivery_id,
                    delivery.event_type
                ),
                false,
            );
            self.fail_delivery(&delivery, &error).await?;
            return Ok(());
        }

        if let Err(error) = subscriber.handle(&delivery).await {
            let dispatch = ServiceDispatchError::infrastructure(
                "MQ_SUBSCRIBER_FAILED",
                format!("Subscriber {} failed: {error}", subscriber.id()),
                true,
            );
            self.fail_delivery(&delivery, &dispatch).await?;
            return Ok(());
        }

        self.outbox
            .mark_delivered(&delivery.event_id, &delivery.subscription_id)
            .await
            .map_err(|error| db_error("MQ_ACK_FAILED", error))?;

        tracing::info!(
            target: "culebraluxe::mq",
            worker_id = %self.config.worker_id,
            delivery_id = %delivery.delivery_id,
            event_id = %delivery.event_id,
            subscription_id = %delivery.subscription_id,
            attempt = delivery.attempt_count,
            "mq delivery acknowledged"
        );
        Ok(())
    }

    async fn fail_delivery(
        &self,
        delivery: &OutboxDelivery,
        error: &ServiceDispatchError,
    ) -> Result<(), ServiceDispatchError> {
        let dead = self
            .outbox
            .mark_failed(delivery, &error.to_string())
            .await
            .map_err(|db| db_error("MQ_FAILURE_PERSIST_FAILED", db))?;
        let context = delivery_context(delivery);
        let observed = ServiceDispatchError::infrastructure(
            error.code(),
            error.to_string(),
            !dead && error.retryable(),
        );
        self.observer
            .observe_dispatch_failure("mq", &delivery.subscription_id, &context, &observed)
            .await;

        if dead {
            tracing::error!(
                target: "culebraluxe::mq",
                worker_id = %self.config.worker_id,
                delivery_id = %delivery.delivery_id,
                event_id = %delivery.event_id,
                subscription_id = %delivery.subscription_id,
                attempt = delivery.attempt_count,
                "mq delivery exhausted retries and entered dead state"
            );
        } else {
            tracing::warn!(
                target: "culebraluxe::mq",
                worker_id = %self.config.worker_id,
                delivery_id = %delivery.delivery_id,
                event_id = %delivery.event_id,
                subscription_id = %delivery.subscription_id,
                attempt = delivery.attempt_count,
                "mq delivery failed and was scheduled for retry"
            );
        }
        Ok(())
    }

    async fn observe_worker_failure(&self, error: &ServiceDispatchError) {
        let context = ServiceContext {
            actor: ServiceActor {
                id: Some("rust-mq-worker".into()),
                kind: ServiceActorKind::System,
            },
            correlation_id: self.config.worker_id.clone(),
            causation_id: None,
            principal: None,
        };
        self.observer
            .observe_dispatch_failure("mq", "worker", &context, error)
            .await;
    }
}

fn db_error(code: &'static str, error: DbFailure) -> ServiceDispatchError {
    ServiceDispatchError::infrastructure(code, error.to_string(), error.retryable)
}

fn delivery_context(delivery: &OutboxDelivery) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("rust-mq-worker".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: delivery
            .correlation_id
            .clone()
            .unwrap_or_else(|| delivery.event_id.clone()),
        causation_id: delivery.causation_id.clone(),
        principal: None,
    }
}

#[derive(Clone)]
pub struct MqProofSubscriber {
    outbox: DomainEventOutboxDao,
}

impl MqProofSubscriber {
    pub fn new(outbox: DomainEventOutboxDao) -> Self {
        Self { outbox }
    }
}

#[async_trait]
impl MqSubscriber for MqProofSubscriber {
    fn id(&self) -> &str {
        MQ_PROOF_SUBSCRIPTION_ID
    }

    fn routing_key(&self) -> &str {
        MQ_PROOF_ROUTING_KEY
    }

    fn max_attempts(&self) -> i32 {
        3
    }

    fn retry_backoff_seconds(&self) -> i32 {
        1
    }

    async fn handle(&self, delivery: &OutboxDelivery) -> Result<(), MqSubscriberError> {
        self.outbox
            .record_proof_effect(delivery)
            .await
            .map_err(|error| MqSubscriberError::new(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSubscriber {
        id: &'static str,
        routing_key: &'static str,
    }

    #[async_trait]
    impl MqSubscriber for TestSubscriber {
        fn id(&self) -> &str {
            self.id
        }

        fn routing_key(&self) -> &str {
            self.routing_key
        }

        fn max_attempts(&self) -> i32 {
            3
        }

        fn retry_backoff_seconds(&self) -> i32 {
            1
        }

        async fn handle(&self, _delivery: &OutboxDelivery) -> Result<(), MqSubscriberError> {
            Ok(())
        }
    }

    #[test]
    fn subscriber_registry_rejects_duplicate_ids() {
        let result = MqSubscriberRegistry::new(vec![
            Arc::new(TestSubscriber {
                id: "same",
                routing_key: "one",
            }),
            Arc::new(TestSubscriber {
                id: "same",
                routing_key: "two",
            }),
        ]);
        assert!(matches!(
            result,
            Err(ServiceDispatchError::Operation { code, .. }) if code == "MQ_SUBSCRIBER_DUPLICATE"
        ));
    }

    #[test]
    fn subscriber_registry_scope_is_stable_and_sorted() {
        let registry = MqSubscriberRegistry::new(vec![
            Arc::new(TestSubscriber {
                id: "z-sub",
                routing_key: "z",
            }),
            Arc::new(TestSubscriber {
                id: "a-sub",
                routing_key: "a",
            }),
        ])
        .unwrap();
        assert_eq!(registry.ids(), vec!["a-sub".to_owned(), "z-sub".to_owned()]);
    }
}
