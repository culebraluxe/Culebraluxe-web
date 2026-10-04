//! The public signer edge: signing-link credentialed, identity-free.
//!
//! An external signer holds no portal account, so `resolve_request_context`
//! has nothing to resolve. The access token IS the credential: every handler
//! verifies it through `signer.session` first, derives the recipient-bound
//! actor (`signature-recipient:<id>`) from the VERIFIED token — never from
//! request JSON — and runs mutations through the durable command dispatcher
//! with that context. Casbin admits exactly the six signer ops for the
//! `document-sign-edge` actor; everything else stays refused.

#[allow(unused_imports)]
use super::*;

use services::{ServiceActor, ServiceActorKind};

use crate::signer::DOCSIGN_EDGE_ACTOR;

async fn edge_session(
    state: &ApiState,
    access_token: &str,
) -> Result<model::SignerSession, ApiError> {
    state
        .services()
        .signer()
        .session(access_token, &edge_context("signer.session"))
        .await
        .map_err(ApiError::from)
}

/// The edge context: a System actor with no principal. Narrower than the
/// caller — the recipient binding is filled in only after the token verifies.
fn edge_context(correlation_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(DOCSIGN_EDGE_ACTOR.into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: None,
    }
}

fn recipient_context(recipient_id: &str, correlation_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(format!("signature-recipient:{recipient_id}")),
            kind: ServiceActorKind::System,
        },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: None,
    }
}

fn access_token(body: &serde_json::Value, correlation_id: &str) -> Result<String, ApiError> {
    body.get("accessToken")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            ApiError::unauthorized("SIGNER_ACCESS_INVALID", "A signing link is required.")
                .with_correlation(correlation_id.to_owned())
        })
}

async fn edge_command(
    state: &ApiState,
    command_type: &str,
    mut body: serde_json::Value,
    correlation_id: String,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let token = access_token(&body, &correlation_id)?;
    // Verify first: the recipient below comes from the token, so a forged
    // recipientId in the body cannot escalate — the service re-checks the
    // binding anyway, and this context carries the verified one.
    let session = edge_session(state, &token).await.map_err(|error| {
        error.with_correlation(correlation_id.clone())
    })?;
    let recipient_id = session.recipient.id.clone();
    if let Some(map) = body.as_object_mut() {
        map.insert(
            "accessToken".into(),
            serde_json::Value::String(token),
        );
    }
    let request = CommandRequest {
        command_id: uuid::Uuid::new_v4().to_string(),
        command_type: command_type.into(),
        aggregate_type: "signature_recipient".into(),
        aggregate_id: Some(recipient_id.clone()),
        requested_at: chrono::Utc::now().to_rfc3339(),
        input: body.as_object().cloned().unwrap_or_default(),
    };
    // The envelope already carries recipientId for scheduling; enforce the
    // verified one so a forged body cannot address another recipient's lane.
    let context = recipient_context(&recipient_id, &correlation_id);
    let value = state
        .service_harness()
        .execute_command(&request, &context)
        .await
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id.clone()))?;
    Ok(success_with_correlation(value, &correlation_id))
}

pub(super) async fn signer_session(
    State(state): State<ApiState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<model::SignerSession>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let token = access_token(&body, &correlation_id)?;
    let value = edge_session(&state, &token)
        .await
        .map_err(|error| error.with_correlation(correlation_id.clone()))?;
    Ok(success_with_correlation(value, &correlation_id))
}

pub(super) async fn signer_open(
    State(state): State<ApiState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    edge_command(&state, "signer.open", body, correlation_id).await
}

pub(super) async fn signer_consent(
    State(state): State<ApiState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    edge_command(&state, "signer.acceptConsent", body, correlation_id).await
}

pub(super) async fn signer_field(
    State(state): State<ApiState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    edge_command(&state, "signer.completeField", body, correlation_id).await
}

pub(super) async fn signer_complete(
    State(state): State<ApiState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    edge_command(&state, "signer.complete", body, correlation_id).await
}

pub(super) async fn signer_decline(
    State(state): State<ApiState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    edge_command(&state, "signer.decline", body, correlation_id).await
}
