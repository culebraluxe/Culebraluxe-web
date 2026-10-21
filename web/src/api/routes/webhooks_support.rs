//! Webhooks (WhatsApp), signature requests, the client diagnostic event, and the Support reads.

#[allow(unused_imports)]
use super::*;

pub(super) fn signature_service(
    state: &ApiState,
) -> Arc<crate::signature::SignatureService<db::SignatureDao>> {
    state.services().signature()
}

/// Records a client diagnostic event through the error-capture sink.
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
        Err(error) => Err(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "WHATSAPP_NOT_CONFIGURED",
            error.to_string(),
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
        .map_err(|error| match error {
            CoreServiceError::Business { code, message } => match code {
                "WHATSAPP_SIGNATURE_INVALID" => ApiError::unauthorized(code, message),
                "WHATSAPP_PAYLOAD_INVALID" => {
                    ApiError::new(StatusCode::BAD_REQUEST, code, message, false)
                }
                "WHATSAPP_NOT_CONFIGURED" => ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    code,
                    "WhatsApp webhook is not configured.",
                    false,
                ),
                _ => ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, code, message, false),
            },
            // A `DbFailure` keeps its incident id and retryability through
            // `ApiError::from_db`; the response carries the original incident
            // instead of minting a second `rust:api` row for it.
            error => ApiError::from(error),
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

pub(super) async fn signature_send(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<model::SendSignatureRequest>,
) -> Result<Json<ApiSuccess<model::SignatureCommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = signature_service(&state);
    let value = service
        .send(&request, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn luxesign_request(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<model::SignatureRequest>>, ApiError> {
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
) -> Result<Json<ApiSuccess<model::SignatureCommandResult>>, ApiError> {
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
) -> Result<Json<ApiSuccess<model::SupportSecurityStatus>>, ApiError> {
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
) -> Result<Json<ApiSuccess<model::SupportBreakGlassReadiness>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = break_glass_readiness(&state, &resolved).await?;
    Ok(success(value, &resolved))
}

/// Shared by `/v1` and the Support screens (`portal_bridge`).
pub(in super::super) async fn break_glass_readiness(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
) -> Result<model::SupportBreakGlassReadiness, ApiError> {
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
) -> Result<Json<ApiSuccess<model::SupportSystemHealth>>, ApiError> {
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
) -> Result<Json<ApiSuccess<model::WorkflowDiagnosticsSnapshot>>, ApiError> {
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
) -> Result<Json<ApiSuccess<model::WorkflowDiagnosticsDetail>>, ApiError> {
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
