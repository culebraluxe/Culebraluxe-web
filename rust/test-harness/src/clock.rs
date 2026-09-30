//! A deterministic clock for tests.
//!
//! Production time is read from the system clock, which moves on its own and cannot be asserted. A test that depends
//! on "now" is therefore flaky by construction: two runs a millisecond apart see different values. `TestClock` is the
//! harness's answer — a clock that only moves when the test moves it, so a timestamp is a fixture, not a coincidence.
//!
//! It is a plain value, not a `SystemTime` replacement: production code that reads `chrono::Utc::now()` is not
//! tricked by it. A boundary that wants to be testable takes its time as an argument or through a seam the harness
//! can drive; `TestClock` is what the test passes through that seam.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};

/// A clock whose value changes only when the test tells it to.
///
/// Cloning is cheap and shares the same instant: a fixture and the test that uses it advance the same clock. That is
/// deliberate — a single clock per test is what keeps a sequence of timestamps internally consistent.
#[derive(Debug, Clone)]
pub struct TestClock {
    now: Arc<Mutex<DateTime<Utc>>>,
    step: Duration,
}

impl Default for TestClock {
    fn default() -> Self {
        Self::epoch()
    }
}

impl TestClock {
    /// A clock stopped at `instant`, advancing one second per [`TestClock::tick`].
    pub fn fixed(instant: DateTime<Utc>) -> Self {
        Self {
            now: Arc::new(Mutex::new(instant)),
            step: Duration::seconds(1),
        }
    }

    /// A clock stopped at the Unix epoch (1970-01-01T00:00:00Z).
    pub fn epoch() -> Self {
        Self::at_unix_millis(0)
    }

    /// A clock stopped at a Unix timestamp in milliseconds.
    ///
    /// Panics only on a value chrono cannot represent, which is itself a test bug worth failing loudly on.
    pub fn at_unix_millis(millis: i64) -> Self {
        let instant = DateTime::from_timestamp_millis(millis)
            .unwrap_or_else(|| panic!("{millis}ms is not a representable UTC instant"));
        Self::fixed(instant)
    }

    /// The step [`TestClock::tick`] advances by. Defaults to one second.
    pub fn with_step(mut self, step: Duration) -> Self {
        self.step = step;
        self
    }

    /// The current instant.
    pub fn now(&self) -> DateTime<Utc> {
        *self
            .now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The current instant as a Unix timestamp in milliseconds.
    pub fn now_millis(&self) -> i64 {
        self.now().timestamp_millis()
    }

    /// Move the clock to an absolute instant.
    pub fn set(&self, instant: DateTime<Utc>) {
        *self
            .now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = instant;
    }

    /// Move the clock forward by `by` and return the new instant.
    pub fn advance(&self, by: Duration) -> DateTime<Utc> {
        let mut guard = self
            .now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard += by;
        *guard
    }

    /// Move the clock forward by `millis` (which may be negative) and return the new instant.
    pub fn advance_millis(&self, millis: i64) -> DateTime<Utc> {
        self.advance(Duration::milliseconds(millis))
    }

    /// Advance by the configured step and return the new instant.
    pub fn tick(&self) -> DateTime<Utc> {
        self.advance(self.step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_clock_does_not_move_on_its_own() {
        let clock = TestClock::at_unix_millis(1_700_000_000_000);
        assert_eq!(clock.now_millis(), 1_700_000_000_000);
        assert_eq!(clock.now_millis(), 1_700_000_000_000);
        assert_eq!(clock.now(), clock.now());
    }

    #[test]
    fn advancing_moves_the_clock_and_returns_the_new_instant() {
        let clock = TestClock::at_unix_millis(0);
        assert_eq!(clock.advance_millis(1_500).timestamp_millis(), 1_500);
        assert_eq!(clock.now_millis(), 1_500);
        assert_eq!(clock.tick().timestamp_millis(), 2_500);
    }

    #[test]
    fn a_clone_shares_the_same_instant() {
        let clock = TestClock::epoch();
        let alias = clock.clone();
        clock.advance_millis(42);
        assert_eq!(alias.now_millis(), 42, "the clone is the same clock");
    }
}
