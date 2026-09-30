//! Deterministic fault injection for L1/L4 tests.
//!
//! A resilience contract — a retry, a fallback, a circuit breaker — is only exercised when something fails, and a
//! failure that depends on timing is not deterministic. `FaultInjector` turns failure into a script: the test says
//! which call fails and how, and the injector returns exactly that fault on exactly that call. The same script
//! always produces the same sequence, so a failing run can be replayed.
//!
//! The injector does not perform I/O and does not know what a "provider" or a "connection" is; it produces a value
//! the code under test acts on. That keeps it usable at any seam the production code exposes.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What should happen on a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// The call succeeds.
    None,
    /// The call fails with a code and a message.
    Error { code: String, message: String },
    /// The call panics with a message (for a boundary that must survive a collaborator panic, or prove it does not).
    Panic(String),
    /// The call is delayed before succeeding.
    Delay(Duration),
}

impl Fault {
    /// A failure with a code and message.
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
        }
    }

    /// A delay.
    pub fn delay(millis: u64) -> Self {
        Self::Delay(Duration::from_millis(millis))
    }

    /// Whether this fault is a failure (as opposed to a delay or success).
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Error { .. } | Self::Panic(_))
    }
}

/// A seeded, ordered script of faults.
#[derive(Clone)]
pub struct FaultInjector {
    script: Arc<Mutex<VecDeque<Fault>>>,
    fallback: Fault,
    fired: Arc<AtomicU64>,
}

impl FaultInjector {
    /// A scripted injector. Once the script is exhausted, every later call returns `fallback`.
    pub fn scripted(script: Vec<Fault>) -> Self {
        Self::with_fallback(script, Fault::None)
    }

    /// A scripted injector with an explicit value for calls past the end of the script.
    pub fn with_fallback(script: Vec<Fault>, fallback: Fault) -> Self {
        Self {
            script: Arc::new(Mutex::new(script.into())),
            fallback,
            fired: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Every call fails with the same fault.
    pub fn always(fault: Fault) -> Self {
        Self {
            script: Arc::new(Mutex::new(VecDeque::new())),
            fallback: fault,
            fired: Arc::new(AtomicU64::new(0)),
        }
    }

    /// No call fails.
    pub fn never() -> Self {
        Self::always(Fault::None)
    }

    /// The next fault in the script, advancing the injector.
    pub fn next_fault(&self) -> Fault {
        self.fired.fetch_add(1, Ordering::SeqCst);
        self.script
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop_front()
            .unwrap_or_else(|| self.fallback.clone())
    }

    /// The next fault, awaiting it first when it is a [`Fault::Delay`].
    pub async fn next_fault_async(&self) -> Fault {
        let fault = self.next_fault();
        if let Fault::Delay(duration) = fault {
            tokio::time::sleep(duration).await;
            return Fault::None;
        }
        fault
    }

    /// How many faults have been handed out.
    pub fn fired(&self) -> u64 {
        self.fired.load(Ordering::SeqCst)
    }

    /// How many scripted faults remain.
    pub fn remaining(&self) -> usize {
        self.script
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_is_returned_in_order() {
        let injector = FaultInjector::scripted(vec![
            Fault::error("E_TIMEOUT", "timed out"),
            Fault::None,
            Fault::Panic("boom".into()),
        ]);
        assert_eq!(
            injector.next_fault(),
            Fault::error("E_TIMEOUT", "timed out")
        );
        assert_eq!(injector.next_fault(), Fault::None);
        assert_eq!(injector.next_fault(), Fault::Panic("boom".into()));
        assert_eq!(
            injector.next_fault(),
            Fault::None,
            "past the end is the fallback"
        );
        assert_eq!(injector.fired(), 4);
        assert_eq!(injector.remaining(), 0);
    }

    #[test]
    fn an_always_injector_fails_every_call() {
        let injector = FaultInjector::always(Fault::error("E", "always"));
        for _ in 0..5 {
            assert!(injector.next_fault().is_failure());
        }
        assert_eq!(injector.fired(), 5);
    }

    #[test]
    fn the_same_script_is_reproducible() {
        let left = FaultInjector::scripted(vec![Fault::delay(1), Fault::error("E", "x")]);
        let right = FaultInjector::scripted(vec![Fault::delay(1), Fault::error("E", "x")]);
        assert_eq!(left.next_fault(), right.next_fault());
        assert_eq!(left.next_fault(), right.next_fault());
    }

    #[tokio::test]
    async fn a_delay_fault_is_awaited() {
        let injector = FaultInjector::scripted(vec![Fault::delay(10)]);
        let before = tokio::time::Instant::now();
        assert_eq!(injector.next_fault_async().await, Fault::None);
        assert!(tokio::time::Instant::now().duration_since(before) >= Duration::from_millis(10));
    }
}
