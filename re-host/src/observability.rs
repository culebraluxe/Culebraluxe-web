//! The app workflow host's tracing subscriber: one install per process, on **stderr**.
//!
//! The engine's turn, job and interrupt lines are `tracing` events. A `tracing` event with no subscriber
//! installed is dropped in silence — the failure this module exists to remove. It mirrors
//! `web/src/observability.rs` (same `EnvFilter` default, same `RUST_LOG_FORMAT=json` switch) with one deliberate
//! difference: the writer is stderr, because these lines were `eprintln!` before the engine spoke in events and a
//! run started from a script or a terminal reads them there. The CLI's own machine-readable output stays on
//! stdout, which is why the stream is chosen here rather than left to the default.
//!
//! `try_init` rather than `init`: the worker and the HTTP process can share one process (and every binary here can
//! be linked into another), so the second caller must not panic for finding a subscriber already installed.

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Install the subscriber. Idempotent: a second call in the same process changes nothing.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let json = std::env::var("RUST_LOG_FORMAT")
        .ok()
        .is_some_and(|value| value.eq_ignore_ascii_case("json"));

    let result = if json {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(true)
                    .with_thread_ids(true)
                    .with_target(true)
                    .with_writer(std::io::stderr),
            )
            .try_init()
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .with_thread_ids(true)
                    .with_thread_names(true)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_writer(std::io::stderr),
            )
            .try_init()
    };

    if let Err(error) = result {
        eprintln!("workflow tracing subscriber not installed: {error}");
    }
}
