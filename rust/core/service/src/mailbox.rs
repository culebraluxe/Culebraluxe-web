use crate::{
    ServiceDispatchError, ServiceExecutionMode, ServiceExecutionPolicy, ServiceHealth,
    ServiceLifecycle, ServiceLifecycleError, ServiceStatus,
};
use async_trait::async_trait;
use serde_json::Value;
use std::{
    any::Any,
    collections::{HashSet, VecDeque},
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::{mpsc, oneshot, watch, Notify, OwnedSemaphorePermit, Semaphore};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

type WorkOutput = Box<dyn Any + Send>;
type WorkFuture = Pin<Box<dyn Future<Output = WorkOutput> + Send + 'static>>;

struct WorkItem {
    operation: String,
    partition_key: Option<String>,
    future: WorkFuture,
    respond_to: oneshot::Sender<Result<WorkOutput, ServiceDispatchError>>,
    queue_permit: Option<OwnedSemaphorePermit>,
}

enum ActorMessage {
    Work(WorkItem),
    Drain {
        respond_to: oneshot::Sender<()>,
    },
    #[cfg(test)]
    PanicForTest,
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
    queue_slots: Arc<Semaphore>,
    status: watch::Receiver<ServiceStatus>,
    cancel: CancellationToken,
    abort: CancellationToken,
    tracker: TaskTracker,
}

impl ServiceMailbox {
    pub fn spawn(
        domain: impl Into<Arc<str>>,
        config: ServiceMailboxConfig,
        parent_cancel: &CancellationToken,
    ) -> Result<Self, ServiceDispatchError> {
        if config.capacity == 0 || config.max_concurrency == 0 {
            return Err(ServiceDispatchError::infrastructure(
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
        // The mpsc channel alone is not enough to bound the mailbox: the actor moves
        // messages into its own pending scheduler queue. A permit stays with every
        // queued item until that item actually starts, so at most `capacity` tasks
        // can wait for execution no matter how quickly the actor drains mpsc.
        let queue_slots = Arc::new(Semaphore::new(config.capacity));
        let cancel = parent_cancel.child_token();
        let abort = CancellationToken::new();
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
            queue_slots: queue_slots.clone(),
            status: status_tx.clone(),
            cancel: cancel.clone(),
            abort: abort.clone(),
            tracker: tracker.clone(),
            max_concurrency: config.max_concurrency,
            active: 0,
            pending: VecDeque::new(),
            active_partitions: HashSet::new(),
            drain_waiters: Vec::new(),
            input_closed: false,
            stop_requested: false,
        };
        let supervisor_status = status_tx.clone();
        let domain_for_supervisor = domain.clone();
        let actor_handle = tokio::spawn(actor.run());
        tokio::spawn(async move {
            if let Err(error) = actor_handle.await {
                let _ = supervisor_status.send(ServiceStatus::Failed);
                tracing::error!(
                    target: "culebraluxe::service::lifecycle",
                    domain = %domain_for_supervisor,
                    %error,
                    "service mailbox actor terminated unexpectedly"
                );
            }
        });

        Ok(Self {
            domain,
            sender,
            accepting,
            queued,
            in_flight,
            idle_notify,
            queue_slots,
            status,
            cancel,
            abort,
            tracker,
        })
    }

    pub async fn submit_task<T, F>(
        &self,
        operation: impl Into<String>,
        policy: ServiceExecutionPolicy,
        payload: &Value,
        work: F,
    ) -> Result<T, ServiceDispatchError>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let operation = operation.into();
        if !self.accepting.load(Ordering::Acquire) {
            return Err(ServiceDispatchError::ServiceDraining(
                self.domain.to_string(),
            ));
        }

        let queue_permit = self
            .queue_slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ServiceDispatchError::ServiceDraining(self.domain.to_string()))?;
        // A drain may have begun while this sender was waiting for a queue slot.
        // Re-check after acquisition so work is never accepted across the drain fence.
        if !self.accepting.load(Ordering::Acquire) {
            return Err(ServiceDispatchError::ServiceDraining(
                self.domain.to_string(),
            ));
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
        let wrapped: WorkFuture = Box::pin(async move { Box::new(work.await) as WorkOutput });

        let (respond_to, response) = oneshot::channel();
        let permit = self
            .sender
            .reserve()
            .await
            .map_err(|_| ServiceDispatchError::ServiceStopped(self.domain.to_string()))?;
        if !self.accepting.load(Ordering::Acquire) {
            return Err(ServiceDispatchError::ServiceDraining(
                self.domain.to_string(),
            ));
        }
        self.queued.fetch_add(1, Ordering::Relaxed);
        permit.send(ActorMessage::Work(WorkItem {
            operation: operation.clone(),
            partition_key,
            future: wrapped,
            respond_to,
            queue_permit: Some(queue_permit),
        }));

        let boxed = response
            .await
            .map_err(|_| ServiceDispatchError::ServiceStopped(self.domain.to_string()))??;
        downcast_work(self.domain.as_ref(), &operation, boxed)
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
        self.submit_task(operation, policy, payload, work).await?
    }

    pub async fn drain(&self) -> Result<(), ServiceDispatchError> {
        self.refuse_new_work();

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
        // Wake submitters blocked on mailbox capacity and refuse them.
        self.queue_slots.close();
    }

    pub fn cancel(&self) {
        self.refuse_new_work();
        self.cancel.cancel();
    }

    /// Emergency shutdown for a service that did not finish during its graceful
    /// drain deadline. Accepted work is cancelled and all task resources are released.
    pub fn force_stop(&self) {
        self.cancel();
        self.abort.cancel();
    }

    pub fn child_token(&self) -> CancellationToken {
        self.cancel.child_token()
    }

    #[cfg(test)]
    async fn panic_actor_for_test(&self) {
        let _ = self.sender.send(ActorMessage::PanicForTest).await;
    }

    pub async fn wait_running(&self) -> Result<(), ServiceDispatchError> {
        let mut status = self.status.clone();
        loop {
            let current = *status.borrow_and_update();
            match current {
                ServiceStatus::Running => return Ok(()),
                ServiceStatus::Starting => {
                    status.changed().await.map_err(|_| {
                        ServiceDispatchError::ServiceStopped(self.domain.to_string())
                    })?;
                }
                ServiceStatus::Failed => {
                    self.tracker.close();
                    self.tracker.wait().await;
                    return Err(ServiceDispatchError::infrastructure(
                        "SERVICE_FAILED",
                        format!("Service {} failed while starting.", self.domain),
                        false,
                    ));
                }
                current => {
                    return Err(ServiceDispatchError::infrastructure(
                        "SERVICE_NOT_STARTABLE",
                        format!(
                            "Service {} cannot start from lifecycle state {:?}.",
                            self.domain, current
                        ),
                        false,
                    ));
                }
            }
        }
    }

    pub async fn wait_stopped(&self) -> Result<(), ServiceDispatchError> {
        let mut status = self.status.clone();
        loop {
            let current = *status.borrow_and_update();
            match current {
                ServiceStatus::Stopped => return Ok(()),
                ServiceStatus::Failed => {
                    return Err(ServiceDispatchError::infrastructure(
                        "SERVICE_FAILED",
                        format!("Service {} terminated in Failed state.", self.domain),
                        false,
                    ));
                }
                _ => {
                    status.changed().await.map_err(|_| {
                        ServiceDispatchError::ServiceStopped(self.domain.to_string())
                    })?;
                }
            }
        }
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
    queue_slots: Arc<Semaphore>,
    status: watch::Sender<ServiceStatus>,
    cancel: CancellationToken,
    abort: CancellationToken,
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
                        #[cfg(test)]
                        Some(ActorMessage::PanicForTest) => {
                            panic!("injected service mailbox actor panic");
                        }
                        None => {
                            if self.accepting.swap(false, Ordering::AcqRel) {
                                self.stop_requested = true;
                            }
                            self.input_closed = true;
                        }
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
        self.pending.push_back(work);
    }

    fn begin_drain(&mut self) {
        self.accepting.store(false, Ordering::Release);
        self.queue_slots.close();
        if !self.input_closed
            && !matches!(
                *self.status.borrow(),
                ServiceStatus::Draining
                    | ServiceStatus::Stopping
                    | ServiceStatus::Stopped
                    | ServiceStatus::Failed
            )
        {
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
        if let Some(partition) = completion.partition_key {
            self.active_partitions.remove(&partition);
        }
        self.idle_notify.notify_waiters();
    }

    fn schedule_ready(&mut self) {
        if self.active >= self.max_concurrency || self.pending.is_empty() {
            return;
        }

        while self.active < self.max_concurrency {
            let next = self.pending.iter().position(|work| {
                work.partition_key
                    .as_ref()
                    .is_none_or(|key| !self.active_partitions.contains(key))
            });
            let Some(index) = next else {
                break;
            };
            let Some(work) = self.pending.remove(index) else {
                break;
            };
            self.start(work);
        }
    }

    fn start(&mut self, mut work: WorkItem) {
        // Starting execution frees one waiting-mailbox slot. Concurrency remains
        // independently bounded by max_concurrency.
        drop(work.queue_permit.take());
        let partition_key = work.partition_key.clone();
        if let Some(key) = partition_key.as_ref() {
            self.active_partitions.insert(key.clone());
        }

        self.active += 1;
        self.queued.fetch_sub(1, Ordering::Relaxed);
        self.in_flight.fetch_add(1, Ordering::Relaxed);

        let domain = self.domain.clone();
        let completion = self.completion_tx.clone();
        let abort = self.abort.clone();
        let in_flight = self.in_flight.clone();
        let idle_notify = self.idle_notify.clone();
        self.tracker.spawn(async move {
            let result = run_work(domain, work.operation, work.future, abort).await;
            let _ = work.respond_to.send(result);
            in_flight.fetch_sub(1, Ordering::Relaxed);
            idle_notify.notify_waiters();
            let _ = completion.send(Completion { partition_key });
        });
    }
}

impl Drop for ServiceMailboxActor {
    fn drop(&mut self) {
        self.accepting.store(false, Ordering::Release);
        self.queue_slots.close();
        self.queued.store(0, Ordering::Release);
        self.abort.cancel();
        self.idle_notify.notify_waiters();
    }
}

async fn run_work(
    domain: Arc<str>,
    operation: String,
    work: WorkFuture,
    abort: CancellationToken,
) -> Result<WorkOutput, ServiceDispatchError> {
    let mut handle = tokio::spawn(work);
    let joined = tokio::select! {
        result = &mut handle => result,
        _ = abort.cancelled() => {
            handle.abort();
            handle.await
        }
    };
    match joined {
        Ok(result) => Ok(result),
        Err(error) => Err(ServiceDispatchError::OperationPanicked {
            domain: domain.to_string(),
            operation,
            message: error.to_string(),
        }),
    }
}

fn downcast_work<T: Send + 'static>(
    domain: &str,
    operation: &str,
    value: WorkOutput,
) -> Result<T, ServiceDispatchError> {
    value.downcast::<T>().map(|value| *value).map_err(|_| {
        ServiceDispatchError::infrastructure(
            "SERVICE_MAILBOX_TYPE_MISMATCH",
            format!("Service mailbox returned an unexpected task type for {domain}.{operation}."),
            false,
        )
    })
}

fn lifecycle_error(error: ServiceDispatchError) -> ServiceLifecycleError {
    ServiceLifecycleError::new("SERVICE_LIFECYCLE", error.to_string())
}

#[async_trait]
impl ServiceLifecycle for ServiceMailbox {
    async fn start(&self) -> Result<(), ServiceLifecycleError> {
        self.wait_running().await.map_err(lifecycle_error)
    }

    async fn drain(&self) -> Result<(), ServiceLifecycleError> {
        ServiceMailbox::drain(self).await.map_err(lifecycle_error)
    }

    async fn stop(&self) -> Result<(), ServiceLifecycleError> {
        ServiceMailbox::stop(self).await.map_err(lifecycle_error)
    }

    fn status(&self) -> ServiceStatus {
        ServiceMailbox::status(self)
    }

    fn health(&self) -> ServiceHealth {
        ServiceMailbox::health(self)
    }
}

#[cfg(test)]
mod tests;
