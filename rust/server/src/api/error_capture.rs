//! The server's database-failure sink: Rust failures land in `app_error`, next to the TypeScript ones.
//!
//! WHY HERE AND NOT IN `core/db`: `app_error` is this application's table and this process's job is to write it.
//! `core/db` only announces; the composition root decides. That is also what makes the engine's failures visible
//! without giving the engine its own alerting - once it runs in this process, it inherits this sink.
//!
//! Two rules, and both are load-bearing:
//!   1. BEST EFFORT. A failed capture is dropped, never returned. Error reporting must not be able to fail the
//!      operation that was already failing.
//!   2. NO RECURSION. `core/db` runs the sink under a thread-local guard, so the `insert` below failing cannot
//!      announce another failure. Without that guard this file would be an infinite loop on a dead database.

use db::{AppErrorDao, Database, DbFailure};
use std::sync::OnceLock;

static POOL: OnceLock<Database> = OnceLock::new();

/// Register the process's sink. Idempotent: a second call is ignored rather than replacing the first.
pub fn install(database: Database) -> bool {
    if POOL.set(database).is_err() {
        return false;
    }
    let installed = db::on_failure(sink);
    // Panics are captured too, so "impossible" leaves a row instead of a line on a terminal nobody is watching.
    install_panic_hook();
    installed
}

/// Written with the same column list as `db/app-error.ts`, so both languages land in one place and one query reads
/// them. `route` carries the operation, `meta` carries the taxonomy that has no column of its own.
fn sink(failure: &DbFailure) {
    let Some(database) = POOL.get() else { return };
    let dao = AppErrorDao::new(database.clone());
    // Copy everything the spawned thread needs BEFORE spawning it: `failure` is borrowed, and the capture must not
    // hold a reference into the operation that is already failing.
    let failure = failure.clone();
    let kind = format!("db:{:?}", failure.kind);
    let meta = serde_json::json!({
        "kind": format!("{:?}", failure.kind),
        "detail": failure.detail,
        "retryable": failure.retryable,
        "source": "rust",
    })
    .to_string();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let Ok(runtime) = runtime else { return };
        let _ =
            runtime.block_on(async move { dao.record_db_failure(&failure, &kind, &meta).await });
    });
}

/// Record a failure that did not come from the database taxonomy - a panic, or a route returning a 5xx.
///
/// WHY THIS EXISTS. `sink` below only knows `DbFailure`, so before this every other way a Rust request could fail was
/// invisible: a panic printed to stderr and dropped the connection, and a handler returning a 500 returned a 500.
/// Neither wrote a row. The TypeScript path has captured its route failures for a long time; the Rust path now does too.
///
/// Same two rules as the sink: best effort, and no recursion. `kind` is the classification
/// (`rust:panic`, `rust:api`), `operation` is the route or operation name, `level` follows the same
/// info/warn/error/fatal vocabulary as `db/app-error.ts`.
pub fn record(
    kind: &str,
    operation: &str,
    message: &str,
    level: &str,
    stack: Option<&str>,
    meta: serde_json::Value,
) {
    let Some(database) = POOL.get() else { return };
    let dao = AppErrorDao::new(database.clone());
    let kind = kind.to_owned();
    let operation = operation.to_owned();
    let message = message.to_owned();
    let level = level.to_owned();
    let stack = stack.map(str::to_owned);
    let meta = meta.to_string();

    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        let _ = runtime.block_on(async move {
            dao.record_runtime_error(&kind, &operation, &message, &level, stack.as_deref(), &meta)
                .await
        });
    });
}

/// Capture panics.
///
/// A panic is the case this repository most needs recorded, because a panic is what "impossible" looks like at runtime:
/// the engine calling `block_on` inside the server's own runtime was impossible by construction, and when it happened
/// the only trace was a line on stderr in a terminal nobody was watching. The hook below writes it to `app_error` and
/// still prints it, so a panic is both visible live and queryable afterwards.
///
/// It cannot recurse: `record` spawns its own thread and swallows failure, so a panic caused by a dead database cannot
/// panic the reporter.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_owned());
        let message = if let Some(text) = info.payload().downcast_ref::<&str>() {
            (*text).to_owned()
        } else if let Some(text) = info.payload().downcast_ref::<String>() {
            text.clone()
        } else {
            "panic with a non-string payload".to_owned()
        };
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_owned();
        record(
            "rust:panic",
            &location,
            &message,
            "fatal",
            Some(&format!("{location}\nthread: {thread}")),
            serde_json::json!({ "location": location, "thread": thread, "source": "rust" }),
        );
    }));
}
