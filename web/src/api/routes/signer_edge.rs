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
        .map_err(|error| match &error {
            // An unusable signing LINK is a failed credential, the same as no link at all (`access_token` below answers
            // that with 401). It was a plain business error, so a forged or expired link read as 400 "bad request" —
            // wrong for a public edge whose only credential is the link, and inconsistent with its own missing-link answer.
            crate::service_support::CoreServiceError::Business { code, message }
                if matches!(*code, "SIGNER_ACCESS_INVALID" | "SIGNER_ACCESS_EXPIRED") =>
            {
                ApiError::unauthorized(*code, message.clone())
            }
            _ => ApiError::from(error),
        })
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

/// Where the request came from and what it came on: the proxy's forwarded address (its left-most entry is the
/// client), else the real-ip header, plus the browser's user agent. Both come from the HEADERS, which the edge reads,
/// and overwrite anything the body claimed.
fn client_details(headers: &HeaderMap) -> (Option<String>, Option<String>) {
    let text = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let ip = text("x-forwarded-for")
        .and_then(|list| list.split(',').next())
        .map(str::trim)
        .or_else(|| text("x-real-ip"))
        .map(|ip| ip.chars().take(64).collect::<String>());
    let user_agent = text("user-agent").map(|agent| agent.chars().take(300).collect::<String>());
    (ip, user_agent)
}

async fn edge_command(
    state: &ApiState,
    command_type: &str,
    mut body: serde_json::Value,
    headers: &HeaderMap,
    correlation_id: String,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let token = access_token(&body, &correlation_id)?;
    // Verify first: the recipient below comes from the token, so a forged
    // recipientId in the body cannot escalate — the service re-checks the
    // binding anyway, and this context carries the verified one.
    let session = edge_session(state, &token)
        .await
        .map_err(|error| error.with_correlation(correlation_id.clone()))?;
    let recipient_id = session.recipient.id.clone();
    if let Some(map) = body.as_object_mut() {
        map.insert("accessToken".into(), serde_json::Value::String(token));
        let (ip, user_agent) = client_details(headers);
        map.insert("ipAddress".into(), serde_json::json!(ip));
        map.insert("userAgent".into(), serde_json::json!(user_agent));
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

/// The document the verified recipient is being asked to sign — the PDF itself, served so the browser's own viewer
/// shows it.
///
/// A signer who cannot read the document cannot meaningfully consent to sign it. The link is the credential and already
/// sits in the page's own address (`/sign/:token`), so the document is a GET on the same link: an inline frame, a new
/// tab and a download all work natively, on a phone too, where a base64 blob in a frame would show only page one. The
/// link is verified first (the recipient and request come from the TOKEN), then the Vault's own signing door answers as
/// the recipient-bound actor — narrow action, SQL proof that the recipient belongs to an ISSUED, live envelope. A draft,
/// a voided envelope or an unknown recipient answers 404, not the bytes. `?download=1` asks for an attachment.
pub(super) async fn signer_document(
    State(state): State<ApiState>,
    Path(token): Path<String>,
    Query(query): Query<VaultDownloadQuery>,
) -> Result<Response, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let session = edge_session(&state, token.trim())
        .await
        .map_err(|error| error.with_correlation(correlation_id.clone()))?;
    let recipient_id = session.recipient.id.clone();
    let document = state
        .services()
        .vault()
        .signing_document_bytes(
            &session.signature_request_id,
            &recipient_id,
            &recipient_context(&recipient_id, &correlation_id),
        )
        .await
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id.clone()))?
        .ok_or_else(|| {
            ApiError::not_found(
                "SIGNING_DOCUMENT_UNAVAILABLE",
                "This signing request has no document to show.",
            )
            .with_correlation(correlation_id.clone())
        })?;
    let download = query.download.as_deref() == Some("1");
    vault_document_response(document, download)
        .map_err(|error| error.with_correlation(correlation_id))
}

/// The SEALED copy of the document, for a recipient of an envelope that has completed: the same verified link, the
/// Vault's own fourth door (`vault.signingSignedBytes`, SQL proof that the envelope is `completed`). Until then it
/// answers 404, so the signing page can offer the link only on its completion screen. Always an attachment: this is the
/// copy to keep.
pub(super) async fn signer_signed_copy(
    State(state): State<ApiState>,
    Path(token): Path<String>,
) -> Result<Response, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let session = edge_session(&state, token.trim())
        .await
        .map_err(|error| error.with_correlation(correlation_id.clone()))?;
    let recipient_id = session.recipient.id.clone();
    let document = state
        .services()
        .vault()
        .signing_signed_bytes(
            &session.signature_request_id,
            &recipient_id,
            &recipient_context(&recipient_id, &correlation_id),
        )
        .await
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id.clone()))?
        .ok_or_else(|| {
            ApiError::not_found(
                "SIGNED_COPY_UNAVAILABLE",
                "The signed copy is not ready yet.",
            )
            .with_correlation(correlation_id.clone())
        })?;
    vault_document_response(document, true).map_err(|error| error.with_correlation(correlation_id))
}

/// The signing page, drawn as it will be sealed: the verified recipient's own blocks on the original, with the pictures
/// they are about to sign with (posted as data URLs, validated exactly as a signature is), today's date, and only the
/// page(s) those blocks are on. Nothing is stored. It is a POST because the pictures are too large for an address.
pub(super) async fn signer_preview(
    State(state): State<ApiState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let token = access_token(&body, &correlation_id)?;
    let session = edge_session(&state, &token)
        .await
        .map_err(|error| error.with_correlation(correlation_id.clone()))?;
    crate::signer::validate_signature_value(&body)
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id.clone()))?;
    let recipient_id = session.recipient.id.clone();
    let original = state
        .services()
        .vault()
        .signing_document_bytes(
            &session.signature_request_id,
            &recipient_id,
            &recipient_context(&recipient_id, &correlation_id),
        )
        .await
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id.clone()))?
        .ok_or_else(|| {
            ApiError::not_found(
                "SIGNING_DOCUMENT_UNAVAILABLE",
                "This signing request has no document to show.",
            )
            .with_correlation(correlation_id.clone())
        })?;
    let text = |key: &str| body.get(key).and_then(serde_json::Value::as_str);
    let bytes = crate::document_sign::preview_pdf(
        &original.bytes,
        &session.fields,
        &session.recipient.name,
        text("image"),
        &chrono::Utc::now().to_rfc3339(),
    )
    .map_err(|error| ApiError::from(error).with_correlation(correlation_id.clone()))?;
    vault_document_response(
        model::VaultMediaBytes {
            bytes,
            filename: "signing-preview.pdf".into(),
            mime_type: "application/pdf".into(),
        },
        false,
    )
    .map_err(|error| error.with_correlation(correlation_id))
}

pub(super) async fn signer_open(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    edge_command(&state, "signer.open", body, &headers, correlation_id).await
}

pub(super) async fn signer_consent(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    edge_command(
        &state,
        "signer.acceptConsent",
        body,
        &headers,
        correlation_id,
    )
    .await
}

pub(super) async fn signer_field(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    edge_command(
        &state,
        "signer.completeField",
        body,
        &headers,
        correlation_id,
    )
    .await
}

pub(super) async fn signer_complete(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    edge_command(&state, "signer.complete", body, &headers, correlation_id).await
}

pub(super) async fn signer_decline(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    edge_command(&state, "signer.decline", body, &headers, correlation_id).await
}
