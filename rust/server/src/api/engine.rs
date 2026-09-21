//! The workflow engine, served.
//!
//! WHY THIS EXISTS. The engine used to reach the application as a spawned binary: `rust-re-host.ts` ran `re-workflow`
//! once per operation. That means a process per call, a pool per process, and - because nothing else starts it - no
//! share of this application's authentication, audit, error capture or retry. As a service it is a normal part of the
//! server: the same internal key, the same identity resolution, the same `app_error` rows, the same pool.
//!
//! The verbs are exactly the ones the CLI had, so this is a transport change and not a redesign.

use super::context::resolve_engine_context;
use super::routes::{success_with_correlation, ApiSuccess};
use super::ApiState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;

use super::ApiError;

/// Engine operations return the workflow crate's error type, which is a `Display`. One conversion here rather than a
/// `.to_string()` at every call site, and the message that reaches the client is the engine's own diagnosis.
fn engine_failure(message: impl std::fmt::Display, correlation: String) -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "ENGINE_COMMAND_FAILED",
        message.to_string(),
        false,
    )
    .with_correlation(correlation)
}

/// The engine's worker threads.
///
/// THE THREADS ARE NOT DECORATION. The engine's store methods still call `block_on`, and calling `block_on` from a
/// thread that is already driving a runtime - which is exactly what a route handler is - panics with "Cannot start a
/// runtime from within a runtime". This was not theoretical: the first live call to these routes killed the request and
/// stranded the connection. A fresh thread has no runtime context, so `block_on` is legal there.
///
/// They are a POOL rather than a thread per command because a thread per command is unbounded: each engine command
/// occupies a thread for the whole transaction, and under a burst that is one OS thread per in-flight request. Four
/// reusable workers with a bounded queue is the same isolation with a ceiling, and it gives overload an answer -
/// `ENGINE_BUSY` with `retryable: true` - instead of letting the machine decide.
///
/// This is a bridge, not the destination. The destination is a store that does not block
/// (`docs/rust-resilience-status.md`, "Not yet done"); when that lands, the pool and `run_engine` collapse into a
/// plain `.await`. It is deliberately not the async conversion done halfway, because that refactor cannot compile
/// halfway.
struct EnginePool {
    jobs: std::sync::mpsc::SyncSender<Job>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

/// `FORGE_ENGINE_WORKERS` - how many engine commands may run at once. Default 4, which leaves the database pool
/// (`FORGE_DB_POOL_MAX`, default 5) a connection for ordinary reads rather than letting the engine take every one.
fn engine_workers() -> usize {
    std::env::var("FORGE_ENGINE_WORKERS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(4)
}

/// Enough queue to absorb a burst without pretending the engine can do more than it can.
const ENGINE_QUEUE: usize = 256;

fn engine_pool() -> &'static EnginePool {
    static POOL: std::sync::OnceLock<EnginePool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let (jobs, queue) = std::sync::mpsc::sync_channel::<Job>(ENGINE_QUEUE);
        let queue = std::sync::Arc::new(std::sync::Mutex::new(queue));
        for index in 0..engine_workers() {
            let queue = std::sync::Arc::clone(&queue);
            std::thread::Builder::new()
                .name(format!("engine-{index}"))
                .spawn(move || loop {
                    // The lock is held only to take the next job, never while running it.
                    let job = {
                        let guard = match queue.lock() {
                            Ok(guard) => guard,
                            Err(_) => return,
                        };
                        guard.recv()
                    };
                    match job {
                        Ok(job) => {
                            // A panic in the engine must cost one command, not one worker. Unwinding here would
                            // retire the thread permanently, so after a few panics the pool would be dead and every
                            // command would stall on a reply that never comes. The reply channel is dropped by the
                            // panic, which is what turns it into an error for the caller.
                            let outcome =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                            if outcome.is_err() {
                                eprintln!("engine worker caught a panic from an engine operation");
                            }
                        }
                        Err(_) => return,
                    }
                })
                .expect("engine worker thread");
        }
        EnginePool { jobs }
    })
}

/// Run one engine operation on an engine worker.
async fn run_engine<T, E, F>(operation: F, correlation: String) -> Result<T, ApiError>
where
    T: Send + 'static,
    E: std::fmt::Display + Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
{
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel::<Result<T, String>>();
    let job: Job = Box::new(move || {
        // Stringified here, on the worker, because the reply channel cannot carry an unnameable error type.
        let _ = reply_tx.send(operation().map_err(|error| error.to_string()));
    });

    if let Err(error) = engine_pool().jobs.try_send(job) {
        return Err(match error {
            std::sync::mpsc::TrySendError::Full(_) => ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "ENGINE_BUSY",
                "The workflow engine is at capacity; this command was not started.",
                true,
            )
            .with_correlation(correlation),
            std::sync::mpsc::TrySendError::Disconnected(_) => {
                engine_failure("The workflow engine workers are not running.", correlation)
            }
        });
    }

    match reply_rx.await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(message)) => Err(engine_failure(message, correlation)),
        Err(_) => Err(engine_failure(
            "The workflow engine dropped its reply.",
            correlation,
        )),
    }
}

#[derive(Debug, Deserialize)]
pub struct StartTransactionRequest {
    /// `deal` or `contract`, the same two subjects the CLI accepted.
    pub subject: String,
    pub id: String,
}

pub async fn start_transaction(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<StartTransactionRequest>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_engine_context(&state, &headers).await?;
    let correlation = resolved.correlation_id.clone();

    let result = run_engine(
        move || {
            forge::engine::re_runtime::start_residential_transaction(&request.subject, &request.id)
        },
        correlation,
    )
    .await?;

    // Structured, not the CLI's "instance=… started=…" string, so the host stops parsing text.
    let value = serde_json::json!({
        "instanceId": result.instance_id,
        "started": result.started,
    });
    Ok(success_with_correlation(value, &resolved.correlation_id))
}

#[derive(Debug, Deserialize)]
pub struct ReconcileTimerRequest {
    pub instance: String,
    #[serde(default)]
    pub node: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}

pub async fn reconcile_timer(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<ReconcileTimerRequest>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_engine_context(&state, &headers).await?;
    let correlation = resolved.correlation_id.clone();
    let node = request
        .node
        .clone()
        .unwrap_or_else(|| "closing_date_timer".into());

    let applied = run_engine(
        move || {
            if node == "closing_date_timer" {
                forge::engine::re_runtime::reconcile_closing_timer(
                    &request.instance,
                    request.date.as_deref(),
                )
                .map(|applied| applied.to_string())
            } else {
                forge::engine::re_runtime::reconcile_deadline_timer(
                    &request.instance,
                    &node,
                    request.date.as_deref(),
                )
                .map(|applied| applied.to_string())
            }
        },
        correlation,
    )
    .await?;
    let node = request
        .node
        .clone()
        .unwrap_or_else(|| "closing_date_timer".into());

    let value = serde_json::json!({ "node": node, "applied": applied });
    Ok(success_with_correlation(value, &resolved.correlation_id))
}

#[derive(Debug, Deserialize)]
pub struct CompleteTaskRequest {
    /// An application task id (`complete-task`) or an engine task id (`complete-engine-task`).
    pub task: String,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub transition: Option<String>,
    /// `application` (default) or `engine`.
    #[serde(default)]
    pub kind: Option<String>,
}

pub async fn complete_task(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CompleteTaskRequest>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_engine_context(&state, &headers).await?;
    let correlation = resolved.correlation_id.clone();
    let user = request.user.clone().unwrap_or_else(|| "system".into());

    let value = run_engine(
        move || {
            let transition = request.transition.as_deref();
            match request.kind.as_deref().unwrap_or("application") {
                "engine" => forge::engine::re_runtime::complete_engine_task(
                    &request.task,
                    &user,
                    transition,
                )
                .map(|()| serde_json::json!({ "completed": request.task })),
                _ => forge::engine::re_runtime::complete_workflow_task(
                    &request.task,
                    &user,
                    transition,
                )
                .map(|outcome| serde_json::json!({ "result": outcome.to_string() })),
            }
        },
        correlation,
    )
    .await?;
    Ok(success_with_correlation(value, &resolved.correlation_id))
}

#[derive(Debug, Deserialize)]
pub struct ReclaimRequest {
    #[serde(default)]
    pub batch: Option<usize>,
    #[serde(default)]
    pub instance: Option<String>,
}

pub async fn reclaim(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<ReclaimRequest>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_engine_context(&state, &headers).await?;
    let correlation = resolved.correlation_id.clone();

    let reclaimed = run_engine(
        move || match request.instance.as_deref() {
            Some(instance) => forge::engine::re_runtime::reclaim_stale_jobs_for_instance(instance),
            None => forge::engine::re_runtime::reclaim_stale_jobs(request.batch.unwrap_or(50)),
        },
        correlation,
    )
    .await?;

    let value = serde_json::json!({ "reclaimed": reclaimed });
    Ok(success_with_correlation(value, &resolved.correlation_id))
}
