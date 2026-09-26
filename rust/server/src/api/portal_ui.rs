//! THE PORTAL'S OWN ADDRESSES — what the Yew portal calls (`rust/ui/src/app/api.rs`), answered here at the same paths
//! and in the same shapes the Next relays used, so the UI does not change. Each handler resolves the portal user
//! (`ui_auth`) and asks the services; nothing here decides policy.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::context::ResolvedRequestContext;
use super::ui_auth::resolve_portal_context;
use super::{ApiError, ApiState};
use crate::service_support::CoreServiceError;

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/portal/rust-ui/entitlements", get(entitlements))
        .route(
            "/api/portal/rust-ui/cockpit",
            get(cockpit).post(cockpit_act),
        )
}

/// A service failure, tagged with the request's correlation id so the incident and the answer name the same request.
fn failed(resolved: &ResolvedRequestContext) -> impl Fn(CoreServiceError) -> ApiError + '_ {
    move |error| ApiError::from(error).with_correlation(resolved.service.correlation_id.clone())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortalEntitlements {
    account_type: String,
    security_level: String,
    is_root: bool,
    entitlement_codes: Vec<String>,
}

/// What the signed-in user may be offered: the shell reads it once and every screen asks it.
async fn entitlements(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<PortalEntitlements>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let level = resolved
        .service
        .principal
        .as_ref()
        .map(|p| p.level.clone())
        .unwrap_or_else(|| "GUEST".into());
    let user = resolved.acting_user;
    Ok(Json(PortalEntitlements {
        account_type: user.account_type,
        security_level: level,
        is_root: user.role_codes.iter().any(|role| role == "root"),
        entitlement_codes: user.entitlement_codes,
    }))
}

async fn cockpit_page(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
) -> Result<Json<Value>, ApiError> {
    let cockpit = state
        .services()
        .cockpit()
        .snapshot(&resolved.service)
        .await
        .map_err(failed(resolved))?;
    Ok(Json(json!({ "cockpit": cockpit })))
}

/// The Cockpit: KPIs, tasks, the featured deal, the pipeline and recent interactions, as `{ cockpit }`.
async fn cockpit(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    cockpit_page(&state, &resolved).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CockpitAction {
    action: Option<String>,
    task_id: Option<String>,
}

/// The Cockpit's one command, completing a task; answers the Cockpit as it now stands.
async fn cockpit_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CockpitAction>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if input.action.as_deref() != Some("completeTask") {
        return Err(ApiError::bad_request(
            "COCKPIT_ACTION_UNSUPPORTED",
            "Unsupported Cockpit action.",
        ));
    }
    let task = input
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let Some(task) = task else {
        return Err(ApiError::bad_request(
            "COCKPIT_TASK_REQUIRED",
            "taskId is required.",
        ));
    };
    state
        .services()
        .task()
        .complete(task, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    cockpit_page(&state, &resolved).await
}
