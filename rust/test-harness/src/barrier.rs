//! Concurrency barriers and helpers for L3/L4 tests.
//!
//! A concurrency contract is only exercised when the participants are actually concurrent. Two futures polled on one
//! task in sequence are not racing; they are a call graph drawn to look parallel. [`ConcurrencyBarrier::rendezvous`]
//! holds everyone at one point until all have arrived, which turns "started together" from a hope into a fact, and
//! [`rendezvous_map`] runs a closure for each participant and returns what each produced.

use std::sync::Arc;

use tokio::sync::Barrier;

/// A reusable rendezvous point for `parties` concurrent tasks.
#[derive(Debug, Clone)]
pub struct ConcurrencyBarrier {
    inner: Arc<Barrier>,
    parties: usize,
}

impl ConcurrencyBarrier {
    /// A barrier every participant reaches once. `parties` must be at least one.
    pub fn new(parties: usize) -> Self {
        assert!(parties >= 1, "a barrier needs at least one party");
        Self {
            inner: Arc::new(Barrier::new(parties)),
            parties,
        }
    }

    /// How many participants meet here.
    pub fn parties(&self) -> usize {
        self.parties
    }

    /// Block until `parties` callers have arrived.
    pub async fn arrive_and_wait(&self) {
        self.inner.wait().await;
    }
}

/// Run `body` for each of `parties` concurrent tasks, meeting at a shared barrier first.
///
/// The barrier is what makes the `parties` invocations overlap: none proceeds until all have started, so a test can
/// assert on the shared state one of them writes while another holds it.
pub async fn rendezvous_map<T, F>(parties: usize, body: F) -> Vec<T>
where
    T: Send + 'static,
    F: Fn(usize) -> T + Send + Sync + 'static,
{
    let barrier = ConcurrencyBarrier::new(parties);
    let body = Arc::new(body);
    let mut handles = Vec::with_capacity(parties);
    for index in 0..parties {
        let barrier = barrier.clone();
        let body = body.clone();
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            body(index)
        }));
    }
    let mut results = Vec::with_capacity(parties);
    for handle in handles {
        results.push(handle.await.expect("rendezvous task panicked"));
    }
    results
}

/// A one-shot gate: tasks wait on [`StartGate::wait`], and [`StartGate::open`] releases all of them at once.
///
/// It uses a `watch` channel rather than a bare `Notify` because a notification sent before a waiter starts waiting
/// would be lost; the channel's stored value means a waiter that arrives after `open` returns immediately.
#[derive(Debug, Clone)]
pub struct StartGate {
    inner: Arc<tokio::sync::watch::Sender<bool>>,
}

impl Default for StartGate {
    fn default() -> Self {
        Self::new()
    }
}

impl StartGate {
    pub fn new() -> Self {
        let (sender, _receiver) = tokio::sync::watch::channel(false);
        Self {
            inner: Arc::new(sender),
        }
    }

    /// Release every waiter, now and in the future.
    pub fn open(&self) {
        self.inner.send_replace(true);
    }

    /// Whether the gate has been opened.
    pub fn is_open(&self) -> bool {
        *self.inner.borrow()
    }

    /// Wait until [`StartGate::open`] is called (returning at once if it already has been).
    pub async fn wait(&self) {
        let mut receiver = self.inner.subscribe();
        if *receiver.borrow_and_update() {
            return;
        }
        while receiver.changed().await.is_ok() {
            if *receiver.borrow_and_update() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[tokio::test]
    async fn every_participant_is_released() {
        let mut results = rendezvous_map(4, |index| index).await;
        results.sort_unstable();
        assert_eq!(results, vec![0, 1, 2, 3]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn the_barrier_makes_the_participants_overlap() {
        // Each task waits at the barrier, then increments a counter. If they were serialised the peak would be 1;
        // because they meet first, all of them can be inside the increment region at once. The barrier guarantees
        // they all *start* together; the observed peak is at least 2 on a runtime with more than one worker.
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        rendezvous_map(4, {
            let concurrent = concurrent.clone();
            let peak = peak.clone();
            move |_| {
                let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(20));
                concurrent.fetch_sub(1, Ordering::SeqCst);
            }
        })
        .await;
        assert!(
            peak.load(Ordering::SeqCst) >= 2,
            "the barrier did not overlap the participants"
        );
    }

    #[tokio::test]
    async fn a_gate_releases_all_waiters() {
        let gate = StartGate::new();
        let mut handles = Vec::new();
        for _ in 0..3 {
            let gate = gate.clone();
            handles.push(tokio::spawn(async move {
                gate.wait().await;
                true
            }));
        }
        gate.open();
        for handle in handles {
            assert!(handle.await.unwrap());
        }
    }
}
