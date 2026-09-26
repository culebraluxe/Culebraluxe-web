use crate::{
    ServiceDispatchError, ServiceExecutionMode, ServiceExecutionPolicy, ServiceHealth, ServiceStatus,
};
use serde_json::Value;
use std::{
    collections::{HashSet, VecDeque},
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::{mpsc, oneshot, watch, Notify};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

type WorkFuture =
    Pin<Box<dyn Future<Output = Result<Value, ServiceDispatchError>> + Send + 'static>>;

struct WorkItem {
    operation: String,
    partition_key: Option<String>,
    future: WorkFuture,
    respond_to: oneshot::Sender<Result<Value, ServiceDispatchError>>,
}

enum ActorMessage {
    Work(WorkItem),
    Drain { respond_to: oneshot::Sender<()> },
}

struct Completion {
    partition_key: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceMailboxConfig {
    pub capacity: usize,
    pub max_concurrency: usize,
}

impl Default for ServiceMailboxConfig {
    fn default() -> Self {
        Self {
            capacity: 32,
            max_concurrency: 8,
        }
    }
}

#[derive(Clone)]
pub struct ServiceMailbox {
    domain: Arc<str>,
    sender: mpsc::Sender<ActorMessage>,
    accepting: Arc<AtomicBool>,
    queued: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    idle_notify: Arc<Notify>,
    status: watch::Receiver<ServiceStatus>,
    cancel: CancellationToken,
    tracker: TaskTracker,
}

impl ServiceMailbox {
    pub fn spawn(
        domain: impl Into<Arc<str>>,
        config: ServiceMailboxConfig,
        parent_cancel: &CancellationToken,
    ) -> Result<Self, ServiceDispatchError> {
        if config.capacity == 0 || config.max_concurrency == 0 {
            return Err(ServiceDispatchError::operation(
                "SERVICE_MAILBOX_CONFIG_INVALID",
                "Service mailbox capacity and max_concurrency must be greater than zero.",
                false,
            ));
        }

        let domain = domain.into();
        let (sender, receiver) = mpsc::channel(config.capacity);
        let (completion_tx, completion_rx) = mpsc::unbounded_channel();
        let (status_tx, status) = watch::channel(ServiceStatus::Starting);
        let accepting = Arc::new(AtomicBool::new(true));
        let queued = Arc::new(AtomicUsize::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let idle_notify = Arc::new(Notify::new());
        let cancel = parent_cancel.child_token();
        let tracker = TaskTracker::new();

        let actor = ServiceMailboxActor {
            domain: domain.clone(),
            receiver,
            completion_tx,
            completion_rx,
            accepting: accepting.clone(),
            queued: queued.clone(),
            in_flight: in_flight.clone(),
            idle_notify: idle_notify.clone(),
            status: status_tx,
            cancel: cancel.clone(),
            tracker: tracker.clone(),
            max_concurrency: config.max_concurrency,
            active: 0,
            pending: VecDeque::new(),
            active_partitions: HashSet::new(),
            drain_waiters: Vec::new(),
            input_closed: false,
            stop_requested: false,
        };
        tokio::spawn(actor.run());

        Ok(Self {
            domain,
            sender,
            accepting,
            queued,
            in_flight,
            idle_notify,
            status,
            cancel,
            tracker,
        })
    }

    pub async fn submit<F>(
        &self,
        operation: impl Into<String>,
        policy: ServiceExecutionPolicy,
        payload: &Value,
        work: F,
    ) -> Result<Value, ServiceDispatchError>
    where
        F: Future<Output = Result<Value, ServiceDispatchError>> + Send + 'static,
    {
        let operation = operation.into();
        if !self.accepting.load(Ordering::Acquire) {
            return Err(ServiceDispatchError::ServiceDraining(self.domain.to_string()));
        }

        if policy.mode == ServiceExecutionMode::Inline {
            self.in_flight.fetch_add(1, Ordering::Relaxed);
            let domain = self.domain.clone();
            let in_flight = self.in_flight.clone();
            let idle_notify = self.idle_notify.clone();
            let handle = self.tracker.spawn(async move {
                let result = run_work(domain, operation, Box::pin(work)).await;
                in_flight.fetch_sub(1, Ordering::Relaxed);
                idle_notify.notify_waiters();
                result
            });
            return handle
                .await
                .map_err(|error| ServiceDispatchError::OperationPanicked {
                    domain: self.domain.to_string(),
                    operation: "inline-dispatch".into(),
                    message: error.to_string(),
                })?;
        }

        let partition_key = if policy.mode == ServiceExecutionMode::Ordered {
            Some(policy.partition_key(payload).ok_or_else(|| {
                ServiceDispatchError::InvalidPayload {
                    domain: self.domain.to_string(),
                    operation: operation.clone(),
                    message: format!(
                        "ordered operation requires partition field '{}'",
                        policy.partition_by.as_deref().unwrap_or("<missing>")
                    ),
                }
            })?)
        } else {
            None
        };

        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(ActorMessage::Work(WorkItem {
                operation,
                partition_key,
                future: Box::pin(work),
                respond_to,
            }))
            .await
            .map_err(|_| ServiceDispatchError::ServiceStopped(self.domain.to_string()))?;

        response
            .await
            .map_err(|_| ServiceDispatchError::ServiceStopped(self.domain.to_string()))?
    }

    pub async fn drain(&self) -> Result<(), ServiceDispatchError> {
        self.accepting.store(false, Ordering::Release);

        match self.status() {
            ServiceStatus::Stopped => return Ok(()),
            ServiceStatus::Draining | ServiceStatus::Stopping => {
                self.wait_until_idle().await;
                return Ok(());
            }
            _ => {}
        }

        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(ActorMessage::Drain { respond_to })
            .await
            .map_err(|_| ServiceDispatchError::ServiceStopped(self.domain.to_string()))?;
        response
            .await
            .map_err(|_| ServiceDispatchError::ServiceStopped(self.domain.to_string()))?;
        self.wait_until_idle().await;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), ServiceDispatchError> {
        if self.status() == ServiceStatus::Stopped {
            return Ok(());
        }
        if !matches!(
            self.status(),
            ServiceStatus::Draining | ServiceStatus::Stopping
        ) {
            self.drain().await?;
        } else {
            self.wait_until_idle().await;
        }
        self.cancel.cancel();
        self.wait_stopped().await
    }

    pub fn refuse_new_work(&self) {
        self.accepting.store(false, Ordering::Release);
    }

    pub fn cancel(&self) {
        self.refuse_new_work();
        self.cancel.cancel();
    }

    pub fn child_token(&self) -> CancellationToken {
        self.cancel.child_token()
    }

    pub async fn wait_stopped(&self) -> Result<(), ServiceDispatchError> {
        let mut status = self.status.clone();
        while *status.borrow_and_update() != ServiceStatus::Stopped {
            status
                .changed()
                .await
                .map_err(|_| ServiceDispatchError::ServiceStopped(self.domain.to_string()))?;
        }
        Ok(())
    }

    pub fn status(&self) -> ServiceStatus {
        *self.status.borrow()
    }

    pub fn health(&self) -> ServiceHealth {
        ServiceHealth {
            status: self.status(),
            accepting: self.accepting.load(Ordering::Acquire),
            queued: self.queued.load(Ordering::Relaxed),
            in_flight: self.in_flight.load(Ordering::Relaxed),
        }
    }

    async fn wait_until_idle(&self) {
        loop {
            if self.queued.load(Ordering::Acquire) == 0
                && self.in_flight.load(Ordering::Acquire) == 0
            {
                return;
            }
            let notified = self.idle_notify.notified();
            if self.queued.load(Ordering::Acquire) == 0
                && self.in_flight.load(Ordering::Acquire) == 0
            {
                return;
            }
            notified.await;
        }
    }
}

struct ServiceMailboxActor {
    domain: Arc<str>,
    receiver: mpsc::Receiver<ActorMessage>,
    completion_tx: mpsc::UnboundedSender<Completion>,
    completion_rx: mpsc::UnboundedReceiver<Completion>,
    accepting: Arc<AtomicBool>,
    queued: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    idle_notify: Arc<Notify>,
    status: watch::Sender<ServiceStatus>,
    cancel: CancellationToken,
    tracker: TaskTracker,
    max_concurrency: usize,
    active: usize,
    pending: VecDeque<WorkItem>,
    active_partitions: HashSet<String>,
    drain_waiters: Vec<oneshot::Sender<()>>,
    input_closed: bool,
    stop_requested: bool,
}

impl ServiceMailboxActor {
    async fn run(mut self) {
        let _ = self.status.send(ServiceStatus::Running);

        loop {
            self.schedule_ready();

            if self.input_closed && self.active == 0 && self.pending.is_empty() {
                self.finish_drain();
                if self.stop_requested {
                    break;
                }
            }

            tokio::select! {
                _ = self.cancel.cancelled(), if !self.stop_requested => {
                    self.begin_stop();
                }
                Some(completion) = self.completion_rx.recv(), if self.active > 0 => {
                    self.complete(completion);
                }
                message = self.receiver.recv(), if !self.input_closed => {
                    match message {
                        Some(ActorMessage::Work(work)) => self.accept(work),
                        Some(ActorMessage::Drain { respond_to }) => {
                            self.begin_drain();
                            self.drain_waiters.push(respond_to);
                        }
                        None => self.begin_stop(),
                    }
                }
                else => {
                    if self.input_closed && self.active == 0 && self.pending.is_empty() {
                        self.finish_drain();
                        break;
                    }
                }
            }
        }

        let _ = self.status.send(ServiceStatus::Stopping);
        self.tracker.close();
        self.tracker.wait().await;
        let _ = self.status.send(ServiceStatus::Stopped);
        self.idle_notify.notify_waiters();
    }

    fn accept(&mut self, work: WorkItem) {
        self.queued.fetch_add(1, Ordering::Relaxed);
        self.pending.push_back(work);
    }

    fn begin_drain(&mut self) {
        if !self.input_closed {
            self.accepting.store(false, Ordering::Release);
            self.input_closed = true;
            self.receiver.close();
            let _ = self.status.send(ServiceStatus::Draining);
        }
    }

    fn begin_stop(&mut self) {
        self.stop_requested = true;
        self.begin_drain();
    }

    fn finish_drain(&mut self) {
        for waiter in self.drain_waiters.drain(..) {
            let _ = waiter.send(());
        }
        self.idle_notify.notify_waiters();
    }

    fn complete(&mut self, completion: Completion) {
        self.active = self.active.saturating_sub(1);
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
        if let Some(partition) = completion.partition_key {
            self.active_partitions.remove(&partition);
        }
        self.idle_notify.notify_waiters();
    }

    fn schedule_ready(&mut self) {
        if self.active >= self.max_concurrency || self.pending.is_empty() {
            return;
        }

        let candidates = self.pending.len();
        for _ in 0..candidates {
            if self.active >= self.max_concurrency {
                break;
            }
            let Some(work) = self.pending.pop_front() else {
                break;
            };
            let blocked = work
                .partition_key
                .as_ref()
                .is_some_and(|key| self.active_partitions.contains(key));
            if blocked {
                self.pending.push_back(work);
                continue;
            }
            self.start(work);
        }
    }

    fn start(&mut self, work: WorkItem) {
        let partition_key = work.partition_key.clone();
        if let Some(key) = partition_key.as_ref() {
            self.active_partitions.insert(key.clone());
        }

        self.active += 1;
        self.queued.fetch_sub(1, Ordering::Relaxed);
        self.in_flight.fetch_add(1, Ordering::Relaxed);

        let domain = self.domain.clone();
        let completion = self.completion_tx.clone();
        self.tracker.spawn(async move {
            let result = run_work(domain, work.operation, work.future).await;
            let _ = work.respond_to.send(result);
            let _ = completion.send(Completion { partition_key });
        });
    }
}

async fn run_work(
    domain: Arc<str>,
    operation: String,
    work: WorkFuture,
) -> Result<Value, ServiceDispatchError> {
    match tokio::spawn(work).await {
        Ok(result) => result,
        Err(error) => Err(ServiceDispatchError::OperationPanicked {
            domain: domain.to_string(),
            operation,
            message: error.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn ordered_work_serializes_one_partition_but_allows_other_partitions() {
        let root = CancellationToken::new();
        let mailbox = ServiceMailbox::spawn(
            "property",
            ServiceMailboxConfig {
                capacity: 8,
                max_concurrency: 2,
            },
            &root,
        )
        .unwrap();

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut jobs = Vec::new();

        for property_id in ["p1", "p1", "p2"] {
            let mailbox = mailbox.clone();
            let active = active.clone();
            let peak = peak.clone();
            let payload = json!({ "propertyId": property_id });
            jobs.push(tokio::spawn(async move {
                mailbox
                    .submit(
                        "property.save",
                        ServiceExecutionPolicy::ordered("propertyId"),
                        &payload,
                        async move {
                            let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                            peak.fetch_max(now, Ordering::SeqCst);
                            sleep(Duration::from_millis(25)).await;
                            active.fetch_sub(1, Ordering::SeqCst);
                            Ok(json!({ "ok": true }))
                        },
                    )
                    .await
                    .unwrap();
            }));
        }

        for job in jobs {
            job.await.unwrap();
        }

        assert_eq!(peak.load(Ordering::SeqCst), 2);
        mailbox.stop().await.unwrap();
    }

    #[tokio::test]
    async fn drain_refuses_new_work_and_waits_for_accepted_work() {
        let root = CancellationToken::new();
        let mailbox =
            ServiceMailbox::spawn("contract", ServiceMailboxConfig::default(), &root).unwrap();
        let (started_tx, started_rx) = oneshot::channel();

        let running = mailbox.clone();
        let accepted = tokio::spawn(async move {
            running
                .submit(
                    "contract.execute",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async move {
                        let _ = started_tx.send(());
                        sleep(Duration::from_millis(20)).await;
                        Ok(json!({ "executed": true }))
                    },
                )
                .await
        });

        started_rx.await.unwrap();
        mailbox.drain().await.unwrap();
        assert!(accepted.await.unwrap().is_ok());
        assert_eq!(mailbox.health().in_flight, 0);
        assert!(matches!(
            mailbox
                .submit(
                    "contract.execute",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async { Ok(json!({})) }
                )
                .await,
            Err(ServiceDispatchError::ServiceDraining(_))
        ));

        mailbox.cancel();
        mailbox.wait_stopped().await.unwrap();
    }

    #[tokio::test]
    async fn drain_tracks_inline_work_too() {
        let root = CancellationToken::new();
        let mailbox =
            ServiceMailbox::spawn("property", ServiceMailboxConfig::default(), &root).unwrap();
        let (started_tx, started_rx) = oneshot::channel();

        let running = mailbox.clone();
        let work = tokio::spawn(async move {
            running
                .submit(
                    "property.get",
                    ServiceExecutionPolicy::inline(),
                    &json!({}),
                    async move {
                        let _ = started_tx.send(());
                        sleep(Duration::from_millis(20)).await;
                        Ok(json!({ "id": "p1" }))
                    },
                )
                .await
        });

        started_rx.await.unwrap();
        mailbox.drain().await.unwrap();
        assert!(work.await.unwrap().is_ok());
        assert_eq!(mailbox.health().in_flight, 0);

        mailbox.cancel();
        mailbox.wait_stopped().await.unwrap();
    }
}
