//! The app-side tracing subscriber: one install per process, one contract for web and re-host.
//!
//! The HTTP server writes its events to **stdout** (Vercel captures both streams); the operator
//! `re-workflow` CLI writes to **stderr** so machine-readable command output on stdout stays clean.
//! Both use the same `EnvFilter` default (`info`) and the same `RUST_LOG_FORMAT=json` switch.
//!
//! Forge keeps its own copy of this contract in the Forge repo (its binaries also write stderr); the
//! app does not depend on Forge for logging, and Forge does not depend on this crate.

use std::io;

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Stdout,
    Stderr,
}

/// Install the process subscriber. Idempotent: `try_init` means a second caller in the same process
/// changes nothing and reports why on stderr instead of panicking.
pub fn init(destination: Destination) {
    match destination {
        Destination::Stdout => init_with(io::stdout),
        Destination::Stderr => init_with(io::stderr),
    }
}

pub fn init_stdout() {
    init(Destination::Stdout);
}

pub fn init_stderr() {
    init(Destination::Stderr);
}

fn init_with<W>(writer: W)
where
    W: for<'writer> fmt::MakeWriter<'writer> + Send + Sync + 'static,
{
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
                    .with_writer(writer),
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
                    .with_writer(writer),
            )
            .try_init()
    };

    if let Err(error) = result {
        eprintln!("app tracing subscriber not installed: {error}");
    }
}
