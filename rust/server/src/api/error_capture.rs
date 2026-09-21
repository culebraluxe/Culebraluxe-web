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

use db::{Database, DbFailure};
use std::sync::OnceLock;

static POOL: OnceLock<Database> = OnceLock::new();

/// Register the process's sink. Idempotent: a second call is ignored rather than replacing the first.
pub fn install(database: Database) -> bool {
    if POOL.set(database).is_err() {
        return false;
    }
    db::on_failure(sink)
}

/// Written with the same column list as `db/app-error.ts`, so both languages land in one place and one query reads
/// them. `route` carries the operation, `meta` carries the taxonomy that has no column of its own.
fn sink(failure: &DbFailure) {
    let Some(database) = POOL.get() else { return };
    let pool = database.pool().clone();
    // Copy everything the spawned thread needs BEFORE spawning it: `failure` is borrowed, and the capture must not
    // hold a reference into the operation that is already failing.
    let kind = format!("db:{:?}", failure.kind);
    let operation: &'static str = failure.operation;
    let incident_id = failure.incident_id.to_string();
    let code = failure.code.clone();
    let detail = failure.detail.clone();
    let retryable = failure.retryable;
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
        let _ = runtime.block_on(async move {
            sqlx::query(
                "insert into app_error (kind, operation, incident_id, code, message, retryable, route, level, meta)
                 values ($1, $2, $3::uuid, $4, $5, $6, $7, 'error', $8::jsonb)",
            )
            .bind(&kind)
            .bind(operation)
            .bind(&incident_id)
            .bind(code.as_deref())
            .bind(detail.as_deref().unwrap_or("database failure"))
            .bind(retryable)
            .bind("rust/db")
            .bind(meta)
            .execute(&pool)
            .await
        });
    });
}
