//! The ops signing desk's browser routes.
//!
//! The desk used to call the internal engine routes (`/v1/services/dispatch`, `/v1/commands/dispatch`,
//! `/v1/vault/documents`) straight from the browser. Those sit behind the internal key that only the trusted edge
//! holds, so every desk request was refused `INTERNAL_AUTH_REQUIRED` before any entitlement was looked at — ROOT
//! included. These routes take the browser's own session, like every other portal screen, and then run the same
//! gateway, harness and vault read the internal routes run.
//!
//! They are narrower than the internal routes on purpose: the luxesign domain and `luxesign.*` commands
//! only, so a signed-in browser is never handed the whole dispatcher.

use crate::api::routes::{service_dispatch_error, success, ApiSuccess};
use crate::api::ui_auth::resolve_portal_context;
use crate::api::{ApiError, ApiState};
use axum::{extract::State, http::HeaderMap, Json};
use model::VaultActorScope;
use serde_json::Value;
use services::{CommandRequest, CommandResult, ServiceEnvelope};

const SIGNING_DOMAIN: &str = "luxesign";
const SIGNING_COMMAND_PREFIX: &str = "luxesign.";

/// `luxesign.list` and `luxesign.get`: the desk's reads, through the same gateway the internal route uses.
pub(super) async fn signing_dispatch(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(envelope): Json<ServiceEnvelope>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if envelope.domain != SIGNING_DOMAIN {
        return Err(ApiError::forbidden(
            "SIGNING_DESK_SCOPE",
            "The signing desk only reads the luxesign service.",
        )
        .with_correlation(resolved.service.correlation_id.clone()));
    }
    let correlation_id = resolved.service.correlation_id.clone();
    let value = state
        .service_gateway()
        .dispatch(&envelope, &resolved.service)
        .await
        .map_err(|error| service_dispatch_error(error).with_correlation(correlation_id))?;
    Ok(success(value, &resolved))
}

/// The desk's commands (send, resend, void, remind): the same durable harness the internal route runs.
pub(super) async fn signing_command(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CommandRequest>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if !request.command_type.starts_with(SIGNING_COMMAND_PREFIX) {
        return Err(ApiError::forbidden(
            "SIGNING_DESK_SCOPE",
            "The signing desk only sends luxesign commands.",
        )
        .with_correlation(resolved.service.correlation_id.clone()));
    }
    let correlation_id = resolved.service.correlation_id.clone();
    let value = state
        .service_harness()
        .execute_command(&request, &resolved.service)
        .await
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id))?;
    Ok(success(value, &resolved))
}

/// The issued documents the desk can send for signature: the same Vault read `/v1/vault/documents` does.
pub(super) async fn signing_documents(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::IssuedDocumentListItem>>>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let scope = VaultActorScope {
        account_type: resolved.acting_user.account_type.clone(),
        person_id: resolved.acting_user.person_id.clone(),
    };
    let correlation_id = resolved.service.correlation_id.clone();
    let value = state
        .services()
        .vault()
        .list_issued_documents(Some(&scope), &resolved.service)
        .await
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id))?;
    Ok(success(value, &resolved))
}
