//! THE PORTAL'S OWN ADDRESSES — what the Yew portal calls (`rust/ui/src/app/api.rs`), answered here at the same paths
//! and in the same shapes the Next relays used, so the UI does not change. Each handler resolves the portal user
//! (`ui_auth`) and asks the services; nothing here decides policy.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use super::ui_auth::resolve_portal_context;
use super::{ApiError, ApiState};

pub fn router() -> Router<ApiState> {
    Router::new().route("/api/portal/rust-ui/entitlements", get(entitlements))
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
