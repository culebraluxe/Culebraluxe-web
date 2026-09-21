//! The workflow engine, served.
//!
//! WHY THIS EXISTS. The engine used to reach the application as a spawned binary: `rust-re-host.ts` ran `re-workflow`
//! once per operation. That means a process per call, a pool per process, and - because nothing else starts it - no
//! share of this application's authentication, audit, error capture or retry. As a service it is a normal part of the
//! server: the same internal key, the same identity resolution, the same `app_error` rows, the same pool.
//!
//! The verbs are exactly the ones the CLI had, so this is a transport change and not a redesign.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;

use super::context::resolve_request_context;
use super::routes::{success, ApiSuccess};
use super::{ApiError, ApiState};

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
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();

    let result =
        forge::engine::re_runtime::start_residential_transaction(&request.subject, &request.id)
            .map_err(|error| engine_failure(error, correlation.clone()))?;

    // Structured, not the CLI's "instance=… started=…" string, so the host stops parsing text.
    let value = serde_json::json!({
        "instanceId": result.instance_id,
        "started": result.started,
    });
    Ok(success(value, &resolved))
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
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();
    let node = request.node.clone().unwrap_or_else(|| "closing_date_timer".into());

    let applied = if node == "closing_date_timer" {
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
    .map_err(|error| engine_failure(error, correlation.clone()))?;

    let value = serde_json::json!({ "node": node, "applied": applied });
    Ok(success(value, &resolved))
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
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();
    let user = request.user.clone().unwrap_or_else(|| "system".into());
    let transition = request.transition.as_deref();

    let value = match request.kind.as_deref().unwrap_or("application") {
        "engine" => {
            forge::engine::re_runtime::complete_engine_task(&request.task, &user, transition)
                .map_err(|error| engine_failure(error, correlation.clone()))?;
            serde_json::json!({ "completed": request.task })
        }
        _ => {
            let outcome = forge::engine::re_runtime::complete_workflow_task(
                &request.task,
                &user,
                transition,
            )
            .map_err(|error| engine_failure(error, correlation.clone()))?;
            serde_json::json!({ "result": outcome.to_string() })
        }
    };
    Ok(success(value, &resolved))
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
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();

    let reclaimed = match request.instance.as_deref() {
        Some(instance) => forge::engine::re_runtime::reclaim_stale_jobs_for_instance(instance),
        None => forge::engine::re_runtime::reclaim_stale_jobs(request.batch.unwrap_or(50)),
    }
    .map_err(|error| engine_failure(error, correlation.clone()))?;

    let value = serde_json::json!({ "reclaimed": reclaimed });
    Ok(success(value, &resolved))
}
