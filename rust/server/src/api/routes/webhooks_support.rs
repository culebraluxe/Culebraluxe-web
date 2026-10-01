//! Webhooks (WhatsApp, BoldSign), signature requests, the client diagnostic event, and the Support reads.

#[allow(unused_imports)]
use super::*;

pub(super) fn signature_service(
    state: &ApiState,
) -> Arc<crate::signature::SignatureService<db::SignatureDao>> {
    state.services().signature()
}

/// The provider's callback.
///
/// BoldSign authenticates by HMAC over the raw body, so this route deliberately does NOT require the internal API
/// key: `resolve_request_context` demands an application principal that a webhook cannot have, and the signature
/// check inside the service is the real gate. The context is a System actor with no principal — the same shape
/// `context.rs` builds before it resolves an identity.
pub(super) async fn record_app_diagnostic(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<AppDiagnosticEventBody>,
) -> Result<StatusCode, ApiError> {
    let _context = super::super::context::resolve_engine_context(&state, &headers).await?;
    super::super::error_capture::record_application(
        &body.kind,
        &body.operation,
        &body.message,
        &body.route,
        &body.level,
        body.code.as_deref(),
        body.meta,
    );
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn whatsapp_handshake(
    State(state): State<ApiState>,
    Query(query): Query<WhatsAppHandshakeQuery>,
) -> Result<Response, ApiError> {
    let service = state.services().whatsapp();
    match service.verify_handshake(
        query.mode.as_deref(),
        query.verify_token.as_deref(),
        query.challenge.as_deref(),
    ) {
        Ok(Some(challenge)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(challenge))
            .map_err(|error| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "WHATSAPP_RESPONSE_FAILED",
                    error.to_string(),
                    false,
                )
            }),
        Ok(None) => Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::from("forbidden"))
            .map_err(|error| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "WHATSAPP_RESPONSE_FAILED",
                    error.to_string(),
                    false,
                )
            }),
        Err(_) => Err(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "WHATSAPP_NOT_CONFIGURED",
            "WhatsApp webhook is not configured.",
            false,
        )),
    }
}

pub(super) async fn whatsapp_webhook(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<serde_json::Value>, ApiError> {
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|value| value.to_str().ok());

    let service = state.services().whatsapp();
    let result = service
        .handle_webhook(&body, signature)
        .await
        .map_err(|error| {
            if error == "WHATSAPP_SIGNATURE_INVALID" {
                ApiError::unauthorized("WHATSAPP_SIGNATURE_INVALID", "Invalid WhatsApp signature.")
            } else if error == "WHATSAPP_PAYLOAD_INVALID" {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "WHATSAPP_PAYLOAD_INVALID",
                    "Invalid WhatsApp payload.",
                    false,
                )
            } else if error.contains("not configured") {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "WHATSAPP_NOT_CONFIGURED",
                    "WhatsApp webhook is not configured.",
                    false,
                )
            } else {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "WHATSAPP_PROCESSING_FAILED",
                    "Webhook processing failed.",
                    true,
                )
            }
        })?;

    if result.retryable_failure {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "WHATSAPP_RETRYABLE_FAILURE",
            "Webhook processing is temporarily unavailable.",
            true,
        ));
    }

    Ok(Json(json!({
        "ok": true,
        "accepted": result.accepted,
        "relationshipProjected": result.relationship_projected,
        "outcomes": result.outcomes,
    })))
}

pub(super) async fn signature_webhook(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<ApiSuccess<domain::SignatureCommandResult>>, ApiError> {
    let correlation_id = headers
        .get("x-culebra-correlation-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let signature = headers
        .get("x-boldsign-signature")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .ok_or_else(|| {
            ApiError::unauthorized(
                "SIGNATURE_WEBHOOK_UNSIGNED",
                "Missing x-boldsign-signature header.",
            )
            .with_correlation(correlation_id.clone())
        })?;

    let context = service::ServiceContext {
        actor: service::ServiceActor {
            id: Some("boldsign-webhook".into()),
            kind: service::ServiceActorKind::System,
        },
        correlation_id: correlation_id.clone(),
        causation_id: None,
        principal: None,
    };

    let service = signature_service(&state);
    let value = service
        .handle_webhook(&body, &signature, &context)
        .await
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id.clone()))?;
    Ok(Json(ApiSuccess {
        ok: true,
        value,
        correlation_id,
    }))
}

pub(super) async fn signature_send(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<domain::SendSignatureRequest>,
) -> Result<Json<ApiSuccess<domain::SignatureCommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = signature_service(&state);
    let value = service
        .send(&request, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn signature_request(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::SignatureRequest>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = signature_service(&state);
    let value = service
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "SIGNATURE_REQUEST_NOT_FOUND",
                    format!("Signature request not found: {id}"),
                ),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

pub(super) async fn signature_refresh(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::SignatureCommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = signature_service(&state);
    let value = service
        .refresh_status(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn support_security_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::SupportSecurityStatus>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().support();
    let value = service
        .security_status(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn support_break_glass_readiness(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::SupportBreakGlassReadiness>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = break_glass_readiness(&state, &resolved).await?;
    Ok(success(value, &resolved))
}

/// Shared by `/v1` and the Support screens (`portal_bridge`).
pub(in super::super) async fn break_glass_readiness(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
) -> Result<domain::SupportBreakGlassReadiness, ApiError> {
    let app_user_id = std::env::var("AUTH_BREAK_GLASS_APP_USER_ID")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let secret_hash_configured = std::env::var("AUTH_BREAK_GLASS_SECRET_HASH")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());
    let enabled = std::env::var("AUTH_BREAK_GLASS_ENABLED")
        .ok()
        .is_some_and(|value| value.trim() == "true");
    let configured = app_user_id.is_some() && secret_hash_configured;

    let service = state.services().support();
    let value = service
        .break_glass_readiness(
            configured,
            enabled,
            app_user_id.as_deref(),
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), resolved))?;
    Ok(value)
}

pub(super) async fn support_system_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::SupportSystemHealth>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().support();
    let value = service
        .system_health(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn support_workflow_diagnostics(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::WorkflowDiagnosticsSnapshot>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().support();
    let value = service
        .workflow_diagnostics(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn support_workflow_detail(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::WorkflowDiagnosticsDetail>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().support();
    let value = service
        .workflow_detail(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "WORKFLOW_DIAGNOSTICS_NOT_FOUND",
                    format!("Workflow diagnostics instance not found: {id}"),
                ),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}
