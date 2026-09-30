//! A deterministic in-memory workflow engine for tests.
//!
//! The `WorkflowEngine` is the production state machine: definitions, instances, tokens, tasks and commands. It is
//! generic over its store, and production gives it the Neon store while a test gives it [`MemoryStore`] — the same
//! engine, the same transaction contract (a step commits all mutations or none), with no database. This module ties
//! the engine's clock to a [`TestClock`], so a test that advances time sees the engine observe exactly the instants
//! it advanced.
//!
//! The harness wraps the engine; it does not re-implement it. A test drives `start_process`, `apply_command` and the
//! rest through [`EngineHarness::engine`].

use workflow::{EngineOptions, MemoryStore, WorkflowEngine};

use crate::clock::TestClock;

/// The production engine on the in-memory store.
pub type TestEngine = WorkflowEngine<MemoryStore>;

/// A workflow engine whose `now` is a [`TestClock`].
pub struct EngineHarness {
    engine: TestEngine,
    clock: TestClock,
}

impl EngineHarness {
    /// An engine whose clock starts at `clock`'s current instant. Cloning `clock` afterwards advances the engine's
    /// time too, because the clock is shared.
    pub fn new(clock: TestClock) -> Self {
        let engine_clock = clock.clone();
        let engine = WorkflowEngine::new(
            MemoryStore::new(),
            EngineOptions {
                app: None,
                now: Box::new(move || engine_clock.now_millis()),
            },
        );
        Self { engine, clock }
    }

    /// An engine whose clock starts at a Unix timestamp in milliseconds.
    pub fn at_unix_millis(millis: i64) -> Self {
        Self::new(TestClock::at_unix_millis(millis))
    }

    /// The engine itself.
    pub fn engine(&self) -> &TestEngine {
        &self.engine
    }

    /// The in-memory store behind the engine.
    pub fn store(&self) -> &MemoryStore {
        self.engine.store()
    }

    /// The instant the engine's `now` currently returns.
    pub fn now_millis(&self) -> i64 {
        self.clock.now_millis()
    }

    /// The clock driving the engine.
    pub fn clock(&self) -> &TestClock {
        &self.clock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_reads_the_time_from_the_test_clock() {
        let harness = EngineHarness::at_unix_millis(1_000);
        assert_eq!(harness.now_millis(), 1_000);
        harness.clock().advance_millis(500);
        assert_eq!(harness.now_millis(), 1_500, "the engine sees advanced time");
    }

    #[test]
    fn two_engines_at_the_same_instant_agree() {
        let left = EngineHarness::at_unix_millis(42);
        let right = EngineHarness::at_unix_millis(42);
        assert_eq!(left.now_millis(), right.now_millis());
    }
}
