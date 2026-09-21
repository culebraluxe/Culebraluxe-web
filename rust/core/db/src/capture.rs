//! Failure capture: the one place a database failure is announced.
//!
//! WHY THIS IS A HOOK AND NOT A WRITE. `core/db` cannot depend on `app_error` (that is the server's table and the
//! server's concern), and a capture that lives inside the pool would recurse into itself the moment the capture's own
//! insert failed. So the pool announces the failure and whoever owns the process decides what to do with it - the
//! server installs a sink that writes `app_error`, and the workflow engine gets that for free once it runs in the same
//! process, instead of needing its own copy of alerting.
//!
//! TWO RULES, both learned from the TypeScript gateway:
//!   1. CAPTURE IS BEST EFFORT. It must never change the outcome of the operation it observes.
//!   2. CAPTURE MUST NOT RECURSE. The sink's own database work runs under a thread-local guard, so a failure inside
//!      capture is dropped rather than announced again.

use std::cell::Cell;
use std::sync::OnceLock;

use crate::error::DbFailure;

/// What a process installs to receive failures. A plain function pointer: no allocation, no trait objects, and it
/// cannot capture state that would keep a connection open.
pub type DbFailureSink = fn(&DbFailure);

static SINK: OnceLock<DbFailureSink> = OnceLock::new();

thread_local! {
    /// Depth, not a bool: a sink may legitimately do database work that itself fails, and that inner failure must be
    /// swallowed without unhooking the outer one when it finishes.
    static IN_CAPTURE: Cell<u32> = const { Cell::new(0) };
}

/// Install the process's failure sink. First caller wins, and the return value says whether this call installed it,
/// because silently ignoring a second sink is how two processes end up disagreeing about where errors go.
pub fn on_failure(sink: DbFailureSink) -> bool {
    SINK.set(sink).is_ok()
}

pub fn has_sink() -> bool {
    SINK.get().is_some()
}

/// Announce a failure. Safe to call unconditionally: without a sink it does nothing, and inside a sink it is ignored.
pub(crate) fn notify(failure: &DbFailure) {
    let Some(sink) = SINK.get() else { return };
    let depth = IN_CAPTURE.with(|depth| {
        let current = depth.get();
        depth.set(current + 1);
        current
    });
    if depth == 0 {
        // The guard is released even if the sink panics, because a panicking sink must not poison every later failure.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sink(failure)));
        let _ = result;
    }
    IN_CAPTURE.with(|depth| depth.set(depth.get().saturating_sub(1)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DbFailureKind;

    thread_local! {
        static SEEN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    fn failure() -> DbFailure {
        DbFailure {
            kind: DbFailureKind::DatabaseUnavailable,
            operation: "test.select",
            incident_id: uuid::Uuid::nil(),
            code: None,
            detail: None,
            retryable: true,
        }
    }

    #[test]
    fn without_a_sink_nothing_happens() {
        // The whole workspace runs tests in one process, so the assertion is about behavior, not about the static.
        notify(&failure());
    }

    #[test]
    fn a_sink_runs_once_and_a_nested_capture_does_not() {
        let installed = on_failure(|failure| {
            SEEN.with(|seen| seen.borrow_mut().push(failure.operation.to_string()));
            // A sink whose own work fails must not announce the inner failure.
            notify(failure);
        });
        if installed {
            SEEN.with(|seen| seen.borrow_mut().clear());
            notify(&failure());
            SEEN.with(|seen| assert_eq!(seen.borrow().len(), 1, "recursion must be suppressed"));
        }
    }
}
