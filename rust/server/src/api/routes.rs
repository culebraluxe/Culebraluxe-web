use super::context::{
    asserted_identity_context, resolve_public_guest_context, resolve_request_context,
    ResolvedRequestContext,
};
use super::{diagnostics, engine, ApiError, ApiState};
use crate::service_support::CoreServiceError;
use crate::vault::VaultArtifactPort;
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use domain::{
    AttachPropertyVideoRequest, ClientAdminPageRequest, ClientDirectoryPageRequest,
    ClientHistoryRequest, GetCommsPanelRequest, GetCommsTimelineRequest, SearchPeopleRequest,
    UploadPropertyMediaRequest, VaultActorScope, VaultArtifactFailure, VaultCommandOutcome,
    VaultRenderRequest, VaultRenderedArtifact, MAX_MEDIA_UPLOAD_BYTES,
};
use integrations::boldsign::{BoldSignConfig, BoldSignSignatureProvider};
use integrations::mux::{MuxClient, MuxConfig};
use serde::{Deserialize, Serialize};
use serde_json::json;
use service::{
    CommandRequest, CommandResult, OperationKind, ServiceContext, ServiceControlCommand,
    ServiceControlResult, ServiceDispatchError, ServiceEnvelope, SignatureProvider,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiSuccess<T> {
    ok: bool,
    value: T,
    correlation_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    ok: bool,
    service: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadyResponse {
    ok: bool,
    database_target: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WhoAmI {
    app_user_id: String,
    display_name: String,
    email: Option<String>,
    account_type: String,
    role_codes: Vec<String>,
    authority_codes: Vec<String>,
    entitlement_codes: Vec<String>,
    person_id: Option<String>,
    security_level: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GuestCodeRequestBody {
    email: String,
    /// The visitor's address as the website saw it, for the per-IP limit.
    client_ip: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GuestCodeVerifyBody {
    email: String,
    code: String,
}

/// What the provider says about the proved identity; the identity itself arrives in the edge's identity headers.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GuestProvisionBody {
    email: Option<String>,
    #[serde(default)]
    email_verified: bool,
    display_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuestCodeSent {
    sent: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuestCodeVerified {
    email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetRoleEntitlementBody {
    role_code: String,
    action: String,
    granted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetUserPrimaryRoleBody {
    app_user_id: String,
    role_code: String,
}

#[derive(Debug, Deserialize)]
struct PeopleSearchQuery {
    #[serde(default)]
    query: String,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientsQuery {
    view: Option<String>,
    search: Option<String>,
    status: Option<String>,
    role: Option<String>,
    sort: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientHistoryQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    recent: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ClientPageResponse {
    Directory(domain::ClientsPageResult),
    Admin(domain::ClientAdminPageResult),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommsPanelQuery {
    moment_limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommsTimelineQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ActivityQuery {
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssuesQuery {
    scope: Option<String>,
    state: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelationshipReviewQuery {
    review_state: Option<String>,
    search: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelationshipActionBody {
    action: String,
    id: Option<String>,
    person_id: Option<String>,
    confirm: Option<bool>,
    source: Option<String>,
    review_state: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TechCockpitQuery {
    selected: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppDiagnosticEventBody {
    kind: String,
    operation: String,
    message: String,
    route: String,
    level: String,
    code: Option<String>,
    #[serde(default)]
    meta: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct WhatsAppHandshakeQuery {
    #[serde(rename = "hub.mode")]
    mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    challenge: Option<String>,
}

/// The P&L's period. Both ends are required: see the handler for why there is no default.
#[derive(Debug, Deserialize)]
struct AccountingPnlQuery {
    from: String,
    to: String,
}

/// The expense a caller is recording. The optional associations are `Option` because the form sends them only when the
/// expense belongs to something.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateExpenseBody {
    vendor: String,
    category: String,
    /// A decimal as a STRING, so the digits the operator typed reach Postgres unchanged. A JSON number here would have
    /// become a float and lost the cent.
    amount: String,
    expense_on: String,
    #[serde(default)]
    memo: Option<String>,
    #[serde(default)]
    deal_id: Option<String>,
    #[serde(default)]
    property_id: Option<String>,
    #[serde(default)]
    person_id: Option<String>,
}

/// The receivable a caller is recording.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateReceivableBody {
    #[serde(default)]
    reference: Option<String>,
    description: String,
    #[serde(default)]
    category: String,
    amount: String,
    issued_on: String,
    #[serde(default)]
    due_on: Option<String>,
    #[serde(default)]
    deal_id: Option<String>,
    #[serde(default)]
    property_id: Option<String>,
    #[serde(default)]
    person_id: Option<String>,
}

/// The date a receivable was paid.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkReceivablePaidBody {
    paid_on: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFormBody {
    template_id: String,
    template_version: i32,
    deal_id: Option<String>,
    person_id: Option<String>,
    property_id: Option<String>,
    #[serde(default)]
    field_values: BTreeMap<String, String>,
    #[serde(default)]
    sections: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFormBody {
    field_values: Option<BTreeMap<String, String>>,
    sections: Option<BTreeMap<String, String>>,
    status: Option<String>,
    contract_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectBody {
    id: String,
    name: String,
    owner: Option<String>,
    description: Option<String>,
    areas: Option<Vec<String>>,
    starts_at: Option<String>,
    ends_at: Option<String>,
    project_type: Option<String>,
    playbook_id: Option<String>,
    playbook_version: Option<i32>,
    person_id: Option<String>,
    property_id: Option<String>,
    contract_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProjectBody {
    name: Option<String>,
    owner: Option<String>,
    status: Option<String>,
    description: Option<String>,
    areas: Option<Vec<String>>,
    project_type: Option<String>,
    playbook_id: Option<String>,
    playbook_version: Option<i32>,
    person_id: Option<String>,
    property_id: Option<String>,
    contract_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PropertyAdminQuery {
    #[serde(default)]
    search: String,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePropertyAdminBody {
    name: String,
    property_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavePropertyAdminBody {
    name: String,
    slug: Option<String>,
    status: String,
    featured: bool,
    is_active_listing: bool,
    is_published: bool,
    property_type: Option<String>,
    has_ocean_view: bool,
    has_bay_view: bool,
    has_beach_view: bool,
    has_harbor_view: bool,
    has_island_view: bool,
    has_mountain_view: bool,
    has_sunrise_view: bool,
    has_sunset_view: bool,
    has_water_access: bool,
    has_beach_access: bool,
    has_pool: bool,
    has_generator: bool,
    has_solar: bool,
    is_furnished: bool,
    is_gated: bool,
    list_price: Option<String>,
    original_list_price: Option<String>,
    location: Option<String>,
    address_line1: Option<String>,
    street_number: Option<String>,
    street_name: Option<String>,
    unit_number: Option<String>,
    city: Option<String>,
    state_or_province: Option<String>,
    neighborhood: Option<String>,
    postal_code: Option<String>,
    country: Option<String>,
    iso_country_code: Option<String>,
    latitude: Option<String>,
    longitude: Option<String>,
    bedrooms: Option<String>,
    bathrooms: Option<String>,
    bathrooms_full: Option<String>,
    bathrooms_half: Option<String>,
    square_feet: Option<String>,
    lot_size: Option<String>,
    lot_size_units: Option<String>,
    lot_size_acres: Option<String>,
    lot_size_sqft: Option<String>,
    road_frontage_feet: Option<String>,
    road_surface_type: Option<String>,
    lot_description: Option<String>,
    utilities_notes: Option<String>,
    catastro_number: Option<String>,
    buildability: Option<String>,
    slope_description: Option<String>,
    pool_potential: Option<String>,
    road_adjacency: Option<String>,
    utilities_availability: Option<String>,
    hoa_status: Option<String>,
    view_description: Option<String>,
    year_built: Option<String>,
    stories: Option<String>,
    parking_spaces: Option<String>,
    short_description: Option<String>,
    editorial_description: Option<String>,
    public_remarks: Option<String>,
    seo_title: Option<String>,
    seo_description: Option<String>,
    hero_title: Option<String>,
    tagline: Option<String>,
    architecture_notes: Option<String>,
    amenities_notes: Option<String>,
    lifestyle_notes: Option<String>,
    listing_agent_name: Option<String>,
    listing_agent_email: Option<String>,
    listing_agent_phone: Option<String>,
    listing_office: Option<String>,
    legal_owner_name: Option<String>,
    listing_identifier: Option<String>,
    registry_entry: Option<String>,
    finca_number: Option<String>,
    registry_section: Option<String>,
    seller_person_id: Option<String>,
    archived: bool,
    #[serde(default)]
    stellar: domain::PropertyStellarDetails,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachPropertyVideoBody {
    role: String,
    mux_asset_id: String,
    mux_playback_id: String,
    duration_seconds: Option<String>,
    aspect_ratio: Option<String>,
    caption: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePropertyVideoUploadBody {
    cors_origin: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinalizePropertyVideoUploadBody {
    role: String,
    caption: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePersonAdminBody {
    display_name: String,
    status: String,
    company: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWbsBody {
    id: String,
    title: String,
    notes: Option<String>,
    category: String,
    project_id: Option<String>,
    parent_id: Option<String>,
    due_at: Option<String>,
    owner: Option<String>,
    order: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateWbsBody {
    title: Option<String>,
    notes: Option<String>,
    status: Option<String>,
    due_at: Option<Option<String>>,
    owner: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
struct AppleReminderBody {
    alert: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BindVaultFormContractBody {
    contract_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RouteProjectWorkBody {
    title: String,
    notes: String,
    status: String,
    due_at: Option<String>,
    owner: Option<String>,
    destination: String,
    start_at: Option<String>,
    end_at: Option<String>,
    location: Option<String>,
    alert: Option<bool>,
}

struct UnavailableVaultArtifactPort;

#[async_trait]
impl VaultArtifactPort for UnavailableVaultArtifactPort {
    async fn render_issued_document(
        &self,
        _request: VaultRenderRequest,
    ) -> Result<VaultRenderedArtifact, VaultArtifactFailure> {
        Err(VaultArtifactFailure {
            outcome: VaultCommandOutcome::PreconditionFailure,
            message: "Vault artifact renderer is not configured on this transport.".into(),
        })
    }
}

/// Signature (BoldSign) — the HTTP attachment for the service in `rust/server/src/signature`.
///
/// The service has been complete in Rust for a while: send, get, refresh, cancel, decline, reconcile, and webhook
/// handling all exist, and the BoldSign adapter behind them verifies the provider's HMAC. What was missing was this
/// transport, which is why `docs/rust-parity-ledger.md` recorded the `signature` capability as "built, 0 routes"
/// while the production webhook still ran through TypeScript.
///
/// The provider is built from the environment per request, like every other integration edge here. A host with no
/// BoldSign configuration answers with a business failure instead of refusing to boot, because this transport also
/// runs in environments that never sign a document.
fn mux_video() -> Result<MuxClient, ApiError> {
    let config = MuxConfig::from_env().map_err(|message| {
        ApiError::from(CoreServiceError::business("MUX_NOT_CONFIGURED", message))
    })?;
    MuxClient::new(config).map_err(|error| {
        ApiError::from(CoreServiceError::business(
            "MUX_NOT_CONFIGURED",
            error.message,
        ))
    })
}

fn bold_sign(state: &ApiState) -> Result<Arc<dyn SignatureProvider>, ApiError> {
    let config = BoldSignConfig::from_env().map_err(|message| {
        ApiError::from(CoreServiceError::business(
            "SIGNATURE_NOT_CONFIGURED",
            message,
        ))
    })?;
    let provider =
        BoldSignSignatureProvider::new(state.db().clone(), config).map_err(|message| {
            ApiError::from(CoreServiceError::business(
                "SIGNATURE_NOT_CONFIGURED",
                message,
            ))
        })?;
    Ok(Arc::new(provider))
}

/// The provider's callback.
///
/// BoldSign authenticates by HMAC over the raw body, so this route deliberately does NOT require the internal API
/// key: `resolve_request_context` demands an application principal that a webhook cannot have, and the signature
/// check inside the service is the real gate. The context is a System actor with no principal — the same shape
/// `context.rs` builds before it resolves an identity.
async fn record_app_diagnostic(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<AppDiagnosticEventBody>,
) -> Result<StatusCode, ApiError> {
    let _context = super::context::resolve_engine_context(&state, &headers).await?;
    let dao = db::AppErrorDao::new(state.db().clone());
    let meta = body.meta.to_string();
    let _ = dao
        .record_application_event(
            &body.kind,
            &body.operation,
            &body.message,
            &body.route,
            &body.level,
            body.code.as_deref(),
            &meta,
        )
        .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn whatsapp_handshake(
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

async fn whatsapp_webhook(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<serde_json::Value>, ApiError> {
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|value| value.to_str().ok());

    let mut service = state.services().whatsapp();
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

async fn signature_webhook(
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

    let mut service = state.services().signature(bold_sign(&state)?);
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

async fn signature_send(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<domain::SendSignatureRequest>,
) -> Result<Json<ApiSuccess<domain::SignatureCommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().signature(bold_sign(&state)?);
    let value = service
        .send(&request, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn signature_request(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::SignatureRequest>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().signature(bold_sign(&state)?);
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

async fn signature_refresh(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::SignatureCommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().signature(bold_sign(&state)?);
    let value = service
        .refresh_status(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn support_security_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::SupportSecurityStatus>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().support();
    let value = service
        .security_status(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn support_break_glass_readiness(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::SupportBreakGlassReadiness>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
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

    let mut service = state.services().support();
    let value = service
        .break_glass_readiness(
            configured,
            enabled,
            app_user_id.as_deref(),
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn support_system_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::SupportSystemHealth>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().support();
    let value = service
        .system_health(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn support_workflow_diagnostics(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::WorkflowDiagnosticsSnapshot>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().support();
    let value = service
        .workflow_diagnostics(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn support_workflow_detail(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::WorkflowDiagnosticsDetail>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().support();
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

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/v1/whoami", get(whoami))
        // ABSTRACT SERVICE INGRESS. The transport resolves identity/context; callers supply only
        // domain + operation + payload. Typed services remain authoritative underneath.
        .route("/v1/services", get(service_catalog))
        .route("/v1/services/health", get(service_health))
        .route("/v1/services/kernel/health", get(service_kernel_health))
        .route("/v1/services/runtime/health", get(service_runtime_health))
        .route("/v1/services/{domain}/control", post(service_control))
        .route("/v1/services/dispatch", post(service_dispatch))
        .route("/v1/commands/dispatch", post(command_dispatch))
        // THE LOGIN SEAM'S QUESTION, as opposed to whoami's. Auth.js has proved a Google subject and nobody
        // knows yet whether it maps to an active application user; this answers known / unmapped / inactive.
        .route("/v1/security/identity", get(security_identity))
        .route("/v1/diagnostics/app-error", post(record_app_diagnostic))
        .route(
            "/api/integrations/whatsapp/webhook",
            get(whatsapp_handshake).post(whatsapp_webhook),
        )
        .route("/v1/security/guests", post(provision_guest))
        .route("/v1/security/guest-code", post(request_guest_code))
        .route("/v1/security/guest-code/verify", post(verify_guest_code))
        // THE SINGLE DECISION SURFACE: the TypeScript kernel asks here rather than holding its own copy of the
        // rule, so "may this principal do this" is answered in one place.
        .route("/v1/security/authorize", post(authorize_action))
        // The same decision for the anonymous public site, which has no principal to decide for. What it may
        // reach is the policy's named public-read list, not this route's business.
        .route(
            "/v1/security/authorize/public",
            post(authorize_public_action),
        )
        .route(
            "/v1/security/role-entitlements",
            get(role_entitlements).put(set_role_entitlement),
        )
        .route(
            "/v1/security/users",
            get(security_users).put(set_user_primary_role),
        )
        .route("/v1/cockpit", get(cockpit))
        .route("/v1/tech/cockpit", get(tech_cockpit).post(tech_command))
        .route("/v1/support/security-status", get(support_security_status))
        .route(
            "/v1/support/break-glass-readiness",
            get(support_break_glass_readiness),
        )
        .route("/v1/support/system-health", get(support_system_health))
        .route(
            "/v1/support/workflow-diagnostics",
            get(support_workflow_diagnostics),
        )
        .route(
            "/v1/support/workflow-diagnostics/{id}",
            get(support_workflow_detail),
        )
        .route("/v1/workflows", get(workflows))
        .route("/v1/workflows/{id}", get(workflow_detail))
        .route("/v1/flight-recorder/{id}", get(flight_recorder))
        .route("/v1/projects", get(projects).post(create_project))
        .route("/v1/projects/{id}", get(project).patch(update_project))
        .route("/v1/wbs", post(create_wbs_item))
        .route("/v1/wbs/project-items", get(wbs_project_items))
        .route("/v1/wbs/{id}", get(wbs_item).patch(update_wbs_item))
        .route("/v1/tasks/{id}/complete", post(complete_task))
        .route("/v1/wbs/{id}/apple-reminder", post(queue_apple_reminder))
        .route("/v1/wbs/{id}/route", post(route_project_work))
        .route("/v1/clients", get(clients))
        .route("/v1/clients/agents", get(client_agents))
        .route("/v1/clients/{person_id}/history", get(client_history))
        .route("/v1/clients/{person_id}", get(client_detail))
        .route("/v1/people/search", get(search_people))
        .route("/v1/people/{id}", get(person).patch(update_person_admin))
        .route("/v1/people/{id}/properties", get(properties_for_person))
        .route(
            "/v1/properties/admin",
            get(property_admin_page).post(create_property_admin),
        )
        .route(
            "/v1/properties/{id}/admin",
            get(property_admin_detail).patch(save_property_admin),
        )
        .route("/v1/properties/{id}", get(property))
        .route("/v1/media/{id}", get(private_media))
        .route(
            "/v1/media/upload",
            post(upload_standalone_media)
                .layer(DefaultBodyLimit::max(MAX_MEDIA_UPLOAD_BYTES + 1024 * 1024)),
        )
        .route(
            "/v1/properties/{id}/media",
            get(property_media)
                .post(upload_property_media)
                .layer(DefaultBodyLimit::max(MAX_MEDIA_UPLOAD_BYTES + 1024 * 1024)),
        )
        .route("/v1/properties/{id}/video", post(attach_property_video))
        // The chunked upload, for the photographs too large for one request. The per-chunk limit is deliberately a
        // few megabytes rather than the whole-file limit: a chunk is a fragment by definition, and the gateway
        // refuses anything past ~4.5 MB anyway.
        .route(
            "/v1/properties/{id}/media/uploads",
            post(begin_media_upload),
        )
        .route(
            "/v1/properties/{id}/media/uploads/{upload_id}/chunks/{index}",
            post(stage_media_chunk).layer(DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route(
            "/v1/properties/{id}/media/uploads/{upload_id}/complete",
            post(complete_media_upload),
        )
        .route(
            "/v1/properties/{id}/video-uploads",
            post(create_property_video_upload),
        )
        .route(
            "/v1/properties/{id}/video-uploads/{upload_id}/finalize",
            post(finalize_property_video_upload),
        )
        .route("/v1/deals", get(deals).post(create_deal))
        .route("/v1/deals/{id}", get(deal_workspace))
        .route("/v1/deals/{id}/commands", post(deal_workspace_command))
        .route("/v1/contracts", get(contracts))
        .route("/v1/contracts/{id}", get(contract))
        .route(
            "/v1/process-instances/{id}/contracts",
            get(contracts_for_process_instance),
        )
        .route("/v1/forms", get(forms).post(create_form))
        .route("/v1/forms/deal-facts/{deal_id}", get(form_deal_facts))
        .route("/v1/forms/{id}/signers", get(form_signers))
        .route("/v1/forms/{id}/issued-document", get(form_issued_document))
        .route("/v1/forms/{id}", get(form).patch(update_form))
        .route("/v1/comms/{person_id}/panel", get(comms_panel))
        .route("/v1/comms/{person_id}/timeline", get(comms_timeline))
        .route("/v1/activity", get(activity))
        .route("/v1/issues", get(issues))
        .route(
            "/v1/relationship-evidence/review",
            get(relationship_evidence_review),
        )
        .route(
            "/v1/relationship-evidence/actions",
            post(relationship_evidence_action),
        )
        // Accounting V1: the two canonical tables, read as lists and as the projections over them, plus the three
        // commands. The P&L takes its period from the query string — the range is the caller's, and a route that invented
        // one would be the reason a filter could not be honoured.
        .route("/v1/accounting/dashboard", get(accounting_dashboard))
        .route(
            "/v1/accounting/receivables",
            get(accounting_receivables).post(create_receivable),
        )
        .route(
            "/v1/accounting/receivables/{id}/paid",
            post(mark_receivable_paid),
        )
        .route(
            "/v1/accounting/expenses",
            get(accounting_expenses).post(create_expense),
        )
        .route(
            "/v1/accounting/expense-categories",
            get(accounting_expense_categories),
        )
        .route("/v1/accounting/pnl", get(accounting_pnl))
        .route(
            "/v1/calendar",
            get(calendar).post(create_apple_calendar_event),
        )
        .route("/v1/vault/documents", get(vault_documents))
        .route("/v1/vault/documents/{id}", get(vault_document))
        .route(
            "/v1/vault/deals/{id}/documents",
            get(vault_documents_by_deal),
        )
        .route("/v1/vault/forms/{id}/contract", get(vault_form_contract))
        .route(
            "/v1/vault/forms/{id}/bind-contract",
            post(vault_bind_form_contract),
        )
        .route(
            "/v1/vault/contracts/{contract_id}/templates/{template_id}/prior",
            get(vault_prior_contract_document),
        )
        .route(
            "/v1/vault/public-listing-documents/{id}",
            get(vault_public_listing_document_bytes),
        )
        .route("/v1/public/listing-copy", get(public_listing_copy))
        // THE PUBLIC INVENTORY, served by the Rust service the way every other read is. The site's own thin proxy
        // shapes this for the buyers grid, so the grid stops reaching into the database itself.
        .route("/v1/public/listings", get(public_listings))
        // ONE PROPERTY, by any key that names it: slug, name, or id. The property page reads this.
        .route("/v1/public/property", get(public_property))
        // ONE PHOTOGRAPH'S BYTES. The web copy when there is one, the original otherwise — the site never sends a
        // 13 MB original through a 4.5 MB gateway, and never reads Postgres itself to find out.
        .route("/v1/public/media/{id}", get(public_media))
        // THE PROPERTY PAGE'S STRIP, and the sitemap's list. Both by the same rule as the inventory.
        .route("/v1/public/similar", get(public_similar))
        .route("/v1/public/slugs", get(public_slugs))
        .route("/v1/public/guide", get(public_guide))
        .route(
            "/v1/public/marketing-content",
            get(public_marketing_content),
        )
        .route("/v1/website-intake", post(submit_website_intake))
        .route("/v1/catchup/leads", post(submit_catchup_lead))
        .route("/v1/website-intake/{id}/notify", post(notify_website_lead))
        .route(
            "/v1/vault/document-bytes/{id}",
            get(vault_private_document_bytes),
        )
        // The workflow engine, served. Same verbs the re-workflow CLI accepted, now behind the internal key and the
        // same identity resolution as every other route, so an engine command is a first-class part of this server
        // instead of a spawned process with its own pool and no error capture.
        .route("/v1/diagnostics/db", get(diagnostics::db_metrics))
        .route("/v1/engine/transactions", post(engine::start_transaction))
        .route("/v1/engine/timers/reconcile", post(engine::reconcile_timer))
        .route("/v1/engine/tasks/complete", post(engine::complete_task))
        .route("/v1/engine/reclaim", post(engine::reclaim))
        // Signature (BoldSign). Mirrors the production endpoints the TypeScript path already serves, so the webhook
        // and the operator actions can be pointed at Rust without changing a client contract.
        .route("/v1/signature/requests", post(signature_send))
        .route("/v1/signature/requests/{id}", get(signature_request))
        .route(
            "/v1/signature/requests/{id}/refresh",
            post(signature_refresh),
        )
        .route(
            "/api/integrations/boldsign/webhook",
            post(signature_webhook),
        )
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "culebraluxe-rust",
    })
}

async fn ready(State(state): State<ApiState>) -> Result<Json<ReadyResponse>, ApiError> {
    state.db().ping().await.map_err(ApiError::from_db)?;
    Ok(Json(ReadyResponse {
        ok: true,
        database_target: state.db().target().as_str().to_owned(),
    }))
}

async fn role_entitlements(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::security::RoleEntitlements>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state
        .services()
        .security()
        .list_role_entitlements(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn set_role_entitlement(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<SetRoleEntitlementBody>,
) -> Result<Json<ApiSuccess<Vec<domain::security::RoleEntitlements>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut security = state.services().security();
    security
        .set_role_entitlement(
            &body.role_code,
            &body.action,
            body.granted,
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    let value = security
        .list_role_entitlements(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn security_users(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::security::SecurityUserRoles>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state
        .services()
        .security()
        .list_security_users(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn set_user_primary_role(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<SetUserPrimaryRoleBody>,
) -> Result<Json<ApiSuccess<Vec<domain::security::SecurityUserRoles>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut security = state.services().security();
    security
        .set_user_primary_role(&body.app_user_id, &body.role_code, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    let value = security
        .list_security_users(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// One identity as the login seam consumes it: the mapped actor, or why there is none.
///
/// `kind` is the whole point of the endpoint. "unmapped" and "inactive" are different answers and the login page
/// says different things about them; collapsing them into a 401 would throw away the distinction the resolver
/// already computes.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentityResolutionResponse {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    acting_user: Option<IdentityActingUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_level: Option<String>,
}

/// The actor fields the application needs, named as the TypeScript boundary names them.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentityActingUser {
    app_user_id: String,
    display_name: String,
    email: Option<String>,
    account_type: String,
    role_codes: Vec<String>,
    authority_codes: Vec<String>,
    entitlement_codes: Vec<String>,
    person_id: Option<String>,
}

impl From<domain::ActingUser> for IdentityActingUser {
    fn from(actor: domain::ActingUser) -> Self {
        Self {
            app_user_id: actor.app_user_id,
            display_name: actor.display_name,
            email: actor.email,
            account_type: actor.account_type,
            role_codes: actor.role_codes,
            authority_codes: actor.authority_codes,
            entitlement_codes: actor.entitlement_codes,
            person_id: actor.person_id,
        }
    }
}

/// Resolve an ASSERTED provider identity — the login seam's question, not "who am I".
///
/// Auth.js proves the Google subject; this decides the application mapping, in the one place that owns it. The
/// caller asserts a subject and holds no principal (see `asserted_identity_context`), which is why an unmapped
/// or inactive answer is a normal 200 response rather than an error: nothing went wrong, the answer is "no".
async fn security_identity(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<IdentityResolutionResponse>>, ApiError> {
    let (provider, provider_subject, context) = asserted_identity_context(&state, &headers)?;
    let resolution = state
        .services()
        .security()
        .resolve_identity(&provider, &provider_subject, &context)
        .await
        .map_err(ApiError::from)?;

    let value = identity_response(resolution);

    // A system context has no acting user, so the envelope carries the correlation id rather than a resolved
    // request context: inventing an actor here would be worse than admitting there is not one.
    Ok(success_with_correlation(value, &context.correlation_id))
}

fn identity_response(resolution: domain::SecurityIdentityResolution) -> IdentityResolutionResponse {
    match resolution {
        domain::SecurityIdentityResolution::Known(principal) => IdentityResolutionResponse {
            kind: "known",
            acting_user: Some(principal.acting_user.into()),
            security_level: Some(principal.level.as_str().to_owned()),
        },
        domain::SecurityIdentityResolution::Unmapped => IdentityResolutionResponse {
            kind: "unmapped",
            acting_user: None,
            security_level: None,
        },
        domain::SecurityIdentityResolution::Inactive => IdentityResolutionResponse {
            kind: "inactive",
            acting_user: None,
            security_level: None,
        },
    }
}

/// Email a guest a sign-in code. The public website's door: its server holds the internal key.
async fn request_guest_code(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<GuestCodeRequestBody>,
) -> Result<Json<ApiSuccess<GuestCodeSent>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    state
        .services()
        .guest_sign_in()
        .request_code(&body.email, body.client_ip.as_deref(), &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(
        GuestCodeSent { sent: true },
        &context.correlation_id,
    ))
}

/// Check a guest's code. On success the guest exists, and the answer is the email to sign in as (the `email-code`
/// identity's subject).
async fn verify_guest_code(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<GuestCodeVerifyBody>,
) -> Result<Json<ApiSuccess<GuestCodeVerified>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let email = state
        .services()
        .guest_sign_in()
        .verify_code(&body.email, &body.code, &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(
        GuestCodeVerified { email },
        &context.correlation_id,
    ))
}

/// Provision the external guest behind an identity the edge has proved (Google, email code), then resolve it exactly
/// as `/v1/security/identity` does. A staff identity that already maps is simply resolved; provisioning only ever
/// creates EXTERNAL guests.
async fn provision_guest(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<GuestProvisionBody>,
) -> Result<Json<ApiSuccess<IdentityResolutionResponse>>, ApiError> {
    let (provider, provider_subject, context) = asserted_identity_context(&state, &headers)?;
    let mut security = state.services().security();
    let resolution = match security
        .resolve_identity(&provider, &provider_subject, &context)
        .await
        .map_err(ApiError::from)?
    {
        domain::SecurityIdentityResolution::Unmapped => {
            state
                .services()
                .guest_sign_in()
                .provision(
                    domain::security::GuestClaim {
                        provider: provider.clone(),
                        subject: provider_subject.clone(),
                        email: body.email,
                        email_verified: body.email_verified,
                        display_name: body.display_name,
                    },
                    &context,
                )
                .await
                .map_err(ApiError::from)?;
            security
                .resolve_identity(&provider, &provider_subject, &context)
                .await
                .map_err(ApiError::from)?
        }
        known_or_inactive => known_or_inactive,
    };
    Ok(success_with_correlation(
        identity_response(resolution),
        &context.correlation_id,
    ))
}

/// The decision itself, shared by the identified and the anonymous doors.
///
/// The two doors differ ONLY in how the caller is resolved (a signed-in principal, or the public website), so the
/// question and its validation live here once. A second copy for the public path is how the public path would end up
/// accepting an action the catalog does not define.
async fn authorize_decision(
    state: &ApiState,
    body: &AuthorizeBody,
    context: &ServiceContext,
) -> Result<AuthorizeResponse, ApiError> {
    let Some((action, catalog_kind)) = crate::security::catalog_action(&body.action) else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "SECURITY_ACTION_UNKNOWN",
            format!("The action catalog does not contain {}.", body.action),
            false,
        ));
    };

    // Rust's catalog owns the operation kind. The caller names only the
    // action; it cannot reinterpret a command as a query (or vice versa).
    let kind = match catalog_kind {
        "query" => OperationKind::Query,
        "command" => OperationKind::Command,
        other => {
            return Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SECURITY_CATALOG_KIND_INVALID",
                format!("The action catalog contains an invalid kind for {action}: {other}."),
                false,
            ));
        }
    };

    let decision = state
        .services()
        .security()
        .decide(action, kind, context)
        .await
        .map_err(|error| ApiError::from(error))?;

    Ok(AuthorizeResponse {
        allowed: decision.allowed,
        reason: decision.reason,
        policy_id: decision.policy_id,
        mode: decision.mode,
    })
}

/// THE ONE DECISION SURFACE.
///
/// The TypeScript kernel asks here instead of carrying its own copy of the rule, so "may this principal do this?"
/// has a single answer — including the ROOT-only rule for `security.entitlement.manage` and `security.role.manage`,
/// which the TypeScript copy did not carry and therefore disagreed with this one by construction.
///
/// THE CALLER NAMES ONLY AN ACTION, and it must be one the catalog knows. Domain and operation are derived from it
/// inside the service (see `SecurityService::decide`), because the policy keys rules on those fields and a caller
/// that could rename them could dodge the `contract.execute` level floor. A command asked about as a query is
/// refused rather than answered: the kind is part of the question.
async fn authorize_action(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<AuthorizeBody>,
) -> Result<Json<ApiSuccess<AuthorizeResponse>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let decision = authorize_decision(&state, &body, &resolved.service)
        .await
        .map_err(|error| correlate(error, &resolved))?;

    Ok(success(decision, &resolved))
}

/// The same decision, asked by the ANONYMOUS PUBLIC SITE.
///
/// The public site reads published listings for visitors who have no session, so it has no principal to decide for —
/// and it cannot use the identified door, which requires one. This door resolves the caller to the `public-website`
/// system actor instead (`resolve_public_guest_context`, the same actor the vault's public document route uses) and
/// refuses identity headers outright: a caller either IS the public website or IS somebody, never an ambiguous
/// mixture that could be read either way by whichever rule matched first.
///
/// WHAT IT CAN REACH IS THE POLICY'S BUSINESS, NOT THIS HANDLER'S: the public actor is allowed the operations named
/// in the Rust `PUBLIC_READ_ACTIONS` list, and queries only. This door only says who is asking.
async fn authorize_public_action(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<AuthorizeBody>,
) -> Result<Json<ApiSuccess<AuthorizeResponse>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let correlation_id = context.correlation_id.clone();
    let decision = authorize_decision(&state, &body, &context)
        .await
        .map_err(|error| error.with_correlation(correlation_id.clone()))?;

    // No acting user to hand to `success`, so the correlation id is passed explicitly rather than invented.
    Ok(success_with_correlation(decision, &correlation_id))
}

/// One action to decide. Rust derives its operation kind from the catalog.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthorizeBody {
    action: String,
    // Compatibility input only. Older edges may still send it during a rolling
    // deploy, but it is ignored; the Rust action catalog is authoritative.
    #[serde(default, rename = "kind")]
    _legacy_kind: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizeResponse {
    allowed: bool,
    reason: String,
    policy_id: String,
    mode: &'static str,
}

/// THE ONE DECISION SURFACE.
///

async fn service_catalog(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<service::ServiceDescriptor>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state.service_gateway().descriptors();
    Ok(success(value, &resolved))
}

async fn service_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<std::collections::BTreeMap<String, service::ServiceHealth>>>, ApiError>
{
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state.service_kernel().registry().health();
    Ok(success(value, &resolved))
}

async fn service_kernel_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<crate::ServiceKernelHealth>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state.service_harness().health();
    Ok(success(value, &resolved))
}

async fn service_runtime_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<crate::ServiceHarnessHealth>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    Ok(success(state.service_harness().runtime_health(), &resolved))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ServiceControlBody {
    command: ServiceControlCommand,
}

async fn service_control(
    State(state): State<ApiState>,
    Path(domain): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ServiceControlBody>,
) -> Result<Json<ApiSuccess<ServiceControlResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let authorization = authorize_decision(
        &state,
        &AuthorizeBody {
            action: "tech.operate".into(),
            _legacy_kind: None,
        },
        &resolved.service,
    )
    .await
    .map_err(|error| correlate(error, &resolved))?;
    if !authorization.allowed {
        return Err(ApiError::forbidden(
            "SERVICE_CONTROL_FORBIDDEN",
            "Service lifecycle control requires owner/root operational authority.",
        )
        .with_correlation(resolved.service.correlation_id.clone()));
    }

    let correlation_id = resolved.service.correlation_id.clone();
    let value = state
        .service_harness()
        .control(&domain, body.command)
        .await
        .map_err(|error| service_dispatch_error(error).with_correlation(correlation_id))?;
    Ok(success(value, &resolved))
}

async fn command_dispatch(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CommandRequest>,
) -> Result<Json<ApiSuccess<CommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation_id = resolved.service.correlation_id.clone();
    let value = state
        .service_harness()
        .execute_command(&request, &resolved.service)
        .await
        .map_err(|error| ApiError::from(error).with_correlation(correlation_id.clone()))?;
    Ok(success(value, &resolved))
}

async fn service_dispatch(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(envelope): Json<ServiceEnvelope>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation_id = resolved.service.correlation_id.clone();
    let value = state
        .service_gateway()
        .dispatch(&envelope, &resolved.service)
        .await
        .map_err(|error| service_dispatch_error(error).with_correlation(correlation_id))?;
    Ok(success(value, &resolved))
}

fn service_dispatch_error(error: ServiceDispatchError) -> ApiError {
    match error {
        ServiceDispatchError::ServiceNotFound(domain) => ApiError::not_found(
            "SERVICE_NOT_FOUND",
            format!("Service not registered for domain: {domain}"),
        ),
        ServiceDispatchError::UnknownOperation { domain, operation } => ApiError::not_found(
            "UNKNOWN_OPERATION",
            format!("Unknown service operation: {domain}.{operation}"),
        ),
        ServiceDispatchError::InvalidPayload {
            domain,
            operation,
            message,
        } => ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_SERVICE_PAYLOAD",
            format!("{domain}.{operation}: {message}"),
            false,
        ),
        ServiceDispatchError::Operation {
            code,
            message,
            retryable,
            ..
        } => {
            let status = match code.as_str() {
                "FORBIDDEN" => StatusCode::FORBIDDEN,
                "AUTHORIZATION_UNAVAILABLE"
                | "AUDIT_UNAVAILABLE"
                | "DOMAIN_EVENT_UNAVAILABLE"
                | "SERVICE_ROUTER_UNAVAILABLE"
                | "DATABASE" => StatusCode::SERVICE_UNAVAILABLE,
                _ if code.ends_with("_NOT_FOUND") => StatusCode::NOT_FOUND,
                _ if code.contains("CONFLICT") => StatusCode::CONFLICT,
                _ => StatusCode::BAD_REQUEST,
            };
            ApiError::new(status, code, message, retryable)
        }
        ServiceDispatchError::ServiceDraining(domain) => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_DRAINING",
            format!("Service is draining: {domain}"),
            true,
        ),
        ServiceDispatchError::ServiceStopped(domain) => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_STOPPED",
            format!("Service is stopped: {domain}"),
            true,
        ),
        ServiceDispatchError::OperationPanicked {
            domain,
            operation,
            message,
        } => ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SERVICE_OPERATION_PANICKED",
            format!("{domain}.{operation}: {message}"),
            true,
        ),
    }
}

async fn whoami(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<WhoAmI>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let level = resolved
        .service
        .principal
        .as_ref()
        .map(|principal| principal.level.clone())
        .unwrap_or_else(|| "GUEST".into());
    let actor = resolved.acting_user.clone();
    Ok(success(
        WhoAmI {
            app_user_id: actor.app_user_id,
            display_name: actor.display_name,
            email: actor.email,
            account_type: actor.account_type,
            role_codes: actor.role_codes,
            authority_codes: actor.authority_codes,
            entitlement_codes: actor.entitlement_codes,
            person_id: actor.person_id,
            security_level: level,
        },
        &resolved,
    ))
}

async fn cockpit(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::CockpitSnapshot>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().cockpit();
    let value = service
        .snapshot(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn workflows(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::WorkflowPortalList>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().workflow_portal();
    let value = service
        .list(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn workflow_detail(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::WorkflowPortalDetail>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().workflow_portal();
    let value = service
        .detail(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "WORKFLOW_NOT_FOUND",
                    format!("Workflow instance not found: {id}"),
                ),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn flight_recorder(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::FlightRecorderTransaction>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err(correlate(
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "FLIGHT_RECORDER_INSTANCE_INVALID",
                "Flight Recorder requires a process-instance UUID.",
                false,
            ),
            &resolved,
        ));
    }
    let mut service = state.services().flight_recorder();
    let value = service
        .transaction(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "FLIGHT_RECORDER_NOT_FOUND",
                    format!("Workflow instance not found: {id}"),
                ),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn projects(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::Project>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().project();
    let value = service
        .list(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn create_project(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateProjectBody>,
) -> Result<Json<ApiSuccess<domain::Project>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut areas = Vec::with_capacity(body.areas.as_ref().map(Vec::len).unwrap_or(0));
    for value in body.areas.unwrap_or_default() {
        areas.push(
            domain::WbsCategory::try_from(value.as_str()).map_err(|error| {
                correlate(
                    ApiError::from(CoreServiceError::business(
                        "PROJECT_AREA_INVALID",
                        error.to_string(),
                    )),
                    &resolved,
                )
            })?,
        );
    }

    let parse_time = |raw: Option<String>, field: &'static str| {
        raw.map(|value| {
            chrono::DateTime::parse_from_rfc3339(&value)
                .map(|parsed| parsed.with_timezone(&chrono::Utc))
                .map_err(|_| {
                    correlate(
                        ApiError::from(CoreServiceError::business(
                            "PROJECT_TIME_INVALID",
                            format!("{field} must be an RFC3339 timestamp."),
                        )),
                        &resolved,
                    )
                })
        })
        .transpose()
    };

    let value = state
        .services()
        .project()
        .create(
            &domain::CreateProjectRequest {
                id: body.id,
                name: body.name,
                owner: body.owner,
                description: body.description.unwrap_or_default(),
                areas,
                starts_at: parse_time(body.starts_at, "startsAt")?,
                ends_at: parse_time(body.ends_at, "endsAt")?,
                project_type: body.project_type,
                playbook_id: body.playbook_id,
                playbook_version: body.playbook_version,
                person_id: body.person_id,
                property_id: body.property_id,
                contract_id: body.contract_id,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn project(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::Project>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().project();
    let value = service
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("PROJECT_NOT_FOUND", format!("Project not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn update_project(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateProjectBody>,
) -> Result<Json<ApiSuccess<domain::Project>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let status = match body.status.as_deref() {
        Some(value) => Some(domain::ProjectStatus::try_from(value).map_err(|error| {
            correlate(
                ApiError::from(CoreServiceError::business(
                    "PROJECT_STATUS_INVALID",
                    error.to_string(),
                )),
                &resolved,
            )
        })?),
        None => None,
    };
    let mut service = state.services().project();
    let value = service
        .update(
            &domain::UpdateProjectRequest {
                id,
                name: body.name,
                owner: body.owner,
                status,
                description: body.description,
                areas: match body.areas {
                    Some(values) => {
                        let mut parsed = Vec::with_capacity(values.len());
                        for value in values {
                            parsed.push(domain::WbsCategory::try_from(value.as_str()).map_err(
                                |error| {
                                    correlate(
                                        ApiError::from(CoreServiceError::business(
                                            "PROJECT_AREA_INVALID",
                                            error.to_string(),
                                        )),
                                        &resolved,
                                    )
                                },
                            )?);
                        }
                        Some(parsed)
                    }
                    None => None,
                },
                starts_at: None,
                ends_at: None,
                project_type: body.project_type,
                playbook_id: body.playbook_id,
                playbook_version: body.playbook_version,
                person_id: body.person_id,
                property_id: body.property_id,
                contract_id: body.contract_id,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn complete_task(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::TaskCompletion>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().task();
    let value = service
        .complete(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn create_wbs_item(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateWbsBody>,
) -> Result<Json<ApiSuccess<domain::WbsItem>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let category = domain::WbsCategory::try_from(body.category.as_str()).map_err(|error| {
        correlate(
            ApiError::from(CoreServiceError::business(
                "WBS_CATEGORY_INVALID",
                error.to_string(),
            )),
            &resolved,
        )
    })?;
    let value = state
        .services()
        .wbs()
        .create(
            &domain::CreateWbsItemRequest {
                id: body.id,
                title: body.title,
                notes: body.notes,
                category,
                project_id: body.project_id,
                parent_id: body.parent_id,
                due_at: body.due_at,
                owner: body.owner,
                order: body.order,
                entity: None,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn wbs_project_items(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::WbsItem>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().wbs();
    let value = service
        .list_project_items(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn wbs_item(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::WbsItem>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().wbs();
    let value = service
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("WBS_NOT_FOUND", format!("WBS item not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn update_wbs_item(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateWbsBody>,
) -> Result<Json<ApiSuccess<domain::WbsItem>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().wbs();
    let current = service
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("WBS_NOT_FOUND", format!("WBS item not found: {id}")),
                &resolved,
            )
        })?;
    let status = match body.status.as_deref() {
        Some(value) => Some(domain::WbsStatus::try_from(value).map_err(|message| {
            correlate(
                ApiError::from(CoreServiceError::business("WBS_STATUS_INVALID", message)),
                &resolved,
            )
        })?),
        None => None,
    };
    let value = service
        .save(
            &domain::SaveWbsItemRequest {
                create: domain::CreateWbsItemRequest {
                    id: current.id.clone(),
                    title: body.title.unwrap_or(current.title),
                    notes: Some(body.notes.unwrap_or(current.notes)),
                    category: current.category,
                    project_id: current.project_id,
                    parent_id: current.parent_id,
                    due_at: match body.due_at {
                        Some(value) => value,
                        None => current.due_at,
                    },
                    owner: match body.owner {
                        Some(value) => value,
                        None => current.owner,
                    },
                    order: current.order,
                    entity: current.entity,
                },
                status,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn clients(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ClientsQuery>,
) -> Result<Json<ApiSuccess<ClientPageResponse>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().clients();
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 50);
    let search = query.search.unwrap_or_default();

    let value = if query.view.as_deref() == Some("admin") {
        ClientPageResponse::Admin(
            service
                .admin(
                    &ClientAdminPageRequest {
                        search,
                        page,
                        page_size,
                    },
                    &resolved.service,
                )
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?,
        )
    } else {
        let status = query
            .status
            .filter(|value| matches!(value.as_str(), "new" | "warm" | "active" | "referral"));
        let role = query
            .role
            .filter(|value| matches!(value.as_str(), "buyer" | "seller" | "both"));
        let sort = query
            .sort
            .filter(|value| matches!(value.as_str(), "name" | "created" | "recent"))
            .unwrap_or_else(|| "name".into());

        ClientPageResponse::Directory(
            service
                .directory(
                    &ClientDirectoryPageRequest {
                        search,
                        status,
                        role,
                        sort,
                        page,
                        page_size,
                    },
                    &resolved.service,
                )
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?,
        )
    };

    Ok(success(value, &resolved))
}

async fn client_detail(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
) -> Result<Json<ApiSuccess<Option<domain::ClientDetail>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().clients();
    let value = service
        .detail(&person_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn client_agents(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::AssignableAgent>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().clients();
    let value = service
        .agents(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn client_history(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    Query(query): Query<ClientHistoryQuery>,
) -> Result<Json<ApiSuccess<domain::ClientContactHistoryResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().clients();
    let value = service
        .history(
            &ClientHistoryRequest {
                person_id,
                page: query.page.unwrap_or(1).max(1),
                page_size: query.page_size.unwrap_or(20).clamp(1, 50),
                recent: query.recent.unwrap_or(false),
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn search_people(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<PeopleSearchQuery>,
) -> Result<Json<ApiSuccess<Vec<domain::PersonSearchResult>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().person();
    let value = service
        .search(
            &SearchPeopleRequest {
                query: query.query,
                limit: query.limit,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn person(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::Person>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().person();
    let value = service
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("PERSON_NOT_FOUND", format!("Person not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn update_person_admin(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdatePersonAdminBody>,
) -> Result<Json<ApiSuccess<domain::Person>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().person();
    let value = service
        .update_admin(
            &domain::UpdatePersonAdminRequest {
                person_id: id,
                display_name: body.display_name,
                status: body.status,
                company: body.company,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn properties_for_person(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::PersonPropertyContext>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().property();
    let value = service
        .for_person(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn property_admin_page(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<PropertyAdminQuery>,
) -> Result<Json<ApiSuccess<domain::PropertyAdminPage>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().property();
    let value = service
        .admin_page(
            &domain::PropertyAdminPageRequest {
                search: query.search,
                page: query.page.unwrap_or(1).max(1),
                page_size: query.page_size.unwrap_or(50).clamp(1, 100),
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn create_property_admin(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreatePropertyAdminBody>,
) -> Result<Json<ApiSuccess<domain::PropertyAdminRecord>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().property();
    let value = service
        .admin_create(
            &domain::CreatePropertyAdminRequest {
                name: body.name,
                property_type: body.property_type,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn property_admin_detail(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::PropertyAdminRecord>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().property();
    let value = service
        .admin_get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("PROPERTY_NOT_FOUND", format!("Property not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn save_property_admin(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SavePropertyAdminBody>,
) -> Result<Json<ApiSuccess<domain::PropertyAdminRecord>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().property();
    let value = service
        .admin_save(
            &domain::SavePropertyAdminRequest {
                property_id: id,
                name: body.name,
                slug: body.slug,
                status: body.status,
                featured: body.featured,
                is_active_listing: body.is_active_listing,
                is_published: body.is_published,
                property_type: body.property_type,
                has_ocean_view: body.has_ocean_view,
                has_bay_view: body.has_bay_view,
                has_beach_view: body.has_beach_view,
                has_harbor_view: body.has_harbor_view,
                has_island_view: body.has_island_view,
                has_mountain_view: body.has_mountain_view,
                has_sunrise_view: body.has_sunrise_view,
                has_sunset_view: body.has_sunset_view,
                has_water_access: body.has_water_access,
                has_beach_access: body.has_beach_access,
                has_pool: body.has_pool,
                has_generator: body.has_generator,
                has_solar: body.has_solar,
                is_furnished: body.is_furnished,
                is_gated: body.is_gated,
                list_price: body.list_price,
                original_list_price: body.original_list_price,
                location: body.location,
                address_line1: body.address_line1,
                street_number: body.street_number,
                street_name: body.street_name,
                unit_number: body.unit_number,
                city: body.city,
                state_or_province: body.state_or_province,
                neighborhood: body.neighborhood,
                postal_code: body.postal_code,
                country: body.country,
                iso_country_code: body.iso_country_code,
                latitude: body.latitude,
                longitude: body.longitude,
                bedrooms: body.bedrooms,
                bathrooms: body.bathrooms,
                bathrooms_full: body.bathrooms_full,
                bathrooms_half: body.bathrooms_half,
                square_feet: body.square_feet,
                lot_size: body.lot_size,
                lot_size_units: body.lot_size_units,
                lot_size_acres: body.lot_size_acres,
                lot_size_sqft: body.lot_size_sqft,
                road_frontage_feet: body.road_frontage_feet,
                road_surface_type: body.road_surface_type,
                lot_description: body.lot_description,
                utilities_notes: body.utilities_notes,
                catastro_number: body.catastro_number,
                buildability: body.buildability,
                slope_description: body.slope_description,
                pool_potential: body.pool_potential,
                road_adjacency: body.road_adjacency,
                utilities_availability: body.utilities_availability,
                hoa_status: body.hoa_status,
                view_description: body.view_description,
                year_built: body.year_built,
                stories: body.stories,
                parking_spaces: body.parking_spaces,
                short_description: body.short_description,
                editorial_description: body.editorial_description,
                public_remarks: body.public_remarks,
                seo_title: body.seo_title,
                seo_description: body.seo_description,
                hero_title: body.hero_title,
                tagline: body.tagline,
                architecture_notes: body.architecture_notes,
                amenities_notes: body.amenities_notes,
                lifestyle_notes: body.lifestyle_notes,
                listing_agent_name: body.listing_agent_name,
                listing_agent_email: body.listing_agent_email,
                listing_agent_phone: body.listing_agent_phone,
                listing_office: body.listing_office,
                legal_owner_name: body.legal_owner_name,
                listing_identifier: body.listing_identifier,
                registry_entry: body.registry_entry,
                finca_number: body.finca_number,
                registry_section: body.registry_section,
                seller_person_id: body.seller_person_id,
                archived: body.archived,
                stellar: body.stellar,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn property(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::Property>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().property();
    let value = service
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("PROPERTY_NOT_FOUND", format!("Property not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn private_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let found = state
        .services()
        .media()
        .media_bytes(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    let Some((mime_type, bytes)) = found else {
        return Err(correlate(
            ApiError::not_found("MEDIA_NOT_FOUND", "Not found."),
            &resolved,
        ));
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(Body::from(bytes))
        .map_err(|error| {
            correlate(
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "MEDIA_RESPONSE_FAILED",
                    error.to_string(),
                    false,
                ),
                &resolved,
            )
        })
}

async fn upload_standalone_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut file: Option<(String, String, Vec<u8>)> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        correlate(
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "MEDIA_MULTIPART_INVALID",
                format!("Invalid media upload: {error}"),
                false,
            ),
            &resolved,
        )
    })? {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().unwrap_or("upload").to_owned();
        let mime_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_owned();
        let bytes = field.bytes().await.map_err(|error| {
            correlate(
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "MEDIA_FILE_INVALID",
                    format!("Invalid media file: {error}"),
                    false,
                ),
                &resolved,
            )
        })?;
        file = Some((filename, mime_type, bytes.to_vec()));
    }

    let (filename, mime_type, bytes) = file.ok_or_else(|| {
        correlate(
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "MEDIA_FILE_REQUIRED",
                "Image file is required.",
                false,
            ),
            &resolved,
        )
    })?;

    let value = state
        .services()
        .media()
        .upload_standalone(&filename, &mime_type, bytes, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    Ok(success(
        json!({
            "id": value.0,
            "filename": value.1,
            "mime_type": value.2,
            "file_size": value.3,
        }),
        &resolved,
    ))
}

async fn property_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<domain::MediaAsset>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().media();
    let value = service
        .for_property(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn create_property_video_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(_property_id): Path<String>,
    Json(body): Json<CreatePropertyVideoUploadBody>,
) -> Result<Json<ApiSuccess<crate::media::PropertyVideoUploadSession>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mux = mux_video()?;
    let mut service = state.services().media();
    let value = service
        .create_property_video_upload(&mux, &body.cors_origin, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn finalize_property_video_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((property_id, upload_id)): Path<(String, String)>,
    Json(body): Json<FinalizePropertyVideoUploadBody>,
) -> Result<Json<ApiSuccess<crate::media::PropertyVideoFinalizeResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mux = mux_video()?;
    let mut service = state.services().media();
    let value = service
        .finalize_property_video_upload(
            &mux,
            &property_id,
            &upload_id,
            &body.role,
            body.caption,
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn attach_property_video(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(property_id): Path<String>,
    Json(body): Json<AttachPropertyVideoBody>,
) -> Result<Json<ApiSuccess<domain::AttachPropertyVideoResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().media();
    let value = service
        .attach_property_video(
            AttachPropertyVideoRequest {
                property_id,
                role: body.role,
                mux_asset_id: body.mux_asset_id,
                mux_playback_id: body.mux_playback_id,
                duration_seconds: body.duration_seconds,
                aspect_ratio: body.aspect_ratio,
                caption: body.caption,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// Finishes a chunked upload. Everything expensive happens here: the chunks are checked, assembled, and turned into
/// the copies that make the photograph servable.
async fn complete_media_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((_property_id, upload_id)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<domain::UploadPropertyMediaResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().media();
    let value = service
        .complete_media_upload(&upload_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    Ok(success(value, &resolved))
}

/// One shape of failure for every step of a chunked upload, so the browser gets a message it can show.
fn media_upload_error(
    correlation: &str,
    status: StatusCode,
    code: &str,
    message: String,
) -> ApiError {
    ApiError::new(status, code, message, false).with_correlation(correlation.to_owned())
}

/// Opens a chunked upload: declares the file, the destination Property and the role before any bytes are sent.
async fn begin_media_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(property_id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<crate::media::BeginMediaUploadResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();

    let mut filename: Option<String> = None;
    let mut mime_type = String::new();
    let mut alt_text: Option<String> = None;
    let mut sha256: Option<String> = None;
    let mut role = String::from("gallery");
    let mut byte_size: i64 = 0;
    let mut chunk_count: i32 = 0;
    let mut chunk_size: i32 = 0;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        media_upload_error(
            &correlation,
            StatusCode::BAD_REQUEST,
            "MEDIA_MULTIPART_INVALID",
            format!("Invalid media upload: {error}"),
        )
    })? {
        let name = field.name().unwrap_or_default().to_owned();
        let value = field.text().await.map_err(|error| {
            media_upload_error(
                &correlation,
                StatusCode::BAD_REQUEST,
                "MEDIA_MULTIPART_INVALID",
                format!("Invalid media upload: {error}"),
            )
        })?;
        match name.as_str() {
            "filename" => filename = Some(value),
            "mimeType" => mime_type = value,
            "altText" => alt_text = Some(value),
            "sha256" => sha256 = Some(value),
            "role" => role = value,
            "byteSize" => byte_size = value.parse().unwrap_or(0),
            "chunkCount" => chunk_count = value.parse().unwrap_or(0),
            "chunkSize" => chunk_size = value.parse().unwrap_or(0),
            _ => {}
        }
    }

    let filename = filename
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            media_upload_error(
                &correlation,
                StatusCode::BAD_REQUEST,
                "MEDIA_FILENAME_REQUIRED",
                "An image filename is required.".to_owned(),
            )
        })?;

    // The declared shape is checked here as well as in the database, so an obviously wrong declaration is refused
    // with a sentence rather than a constraint violation.
    if byte_size <= 0 || chunk_count <= 0 || chunk_size <= 0 {
        return Err(media_upload_error(
            &correlation,
            StatusCode::BAD_REQUEST,
            "MEDIA_UPLOAD_SHAPE_INVALID",
            "The upload must declare a positive size, chunk count and chunk size.".to_owned(),
        ));
    }
    if byte_size > MAX_MEDIA_UPLOAD_BYTES as i64 {
        return Err(media_upload_error(
            &correlation,
            StatusCode::BAD_REQUEST,
            "MEDIA_UPLOAD_TOO_LARGE",
            "Image is too large (max 50 MB).".to_owned(),
        ));
    }

    let mut service = state.services().media();
    let value = service
        .begin_media_upload(
            db::BeginMediaUpload {
                upload_id: uuid::Uuid::new_v4().to_string(),
                property_id,
                filename,
                mime_type,
                byte_size,
                chunk_count,
                chunk_size,
                sha256,
                role,
                alt_text,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    Ok(success(value, &resolved))
}

/// Accepts one piece of the file. The bytes are opaque here: nothing inspects a partial upload.
async fn stage_media_chunk(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((_property_id, upload_id, chunk_index)): Path<(String, String, i32)>,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<crate::media::MediaUploadProgress>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();

    let mut bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.map_err(|error| {
        media_upload_error(
            &correlation,
            StatusCode::BAD_REQUEST,
            "MEDIA_MULTIPART_INVALID",
            format!("Invalid media upload: {error}"),
        )
    })? {
        if field.name().unwrap_or_default() == "chunk" {
            bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|error| {
                        media_upload_error(
                            &correlation,
                            StatusCode::BAD_REQUEST,
                            "MEDIA_CHUNK_INVALID",
                            format!("Invalid media chunk: {error}"),
                        )
                    })?
                    .to_vec(),
            );
        }
    }

    let bytes = bytes.ok_or_else(|| {
        media_upload_error(
            &correlation,
            StatusCode::BAD_REQUEST,
            "MEDIA_CHUNK_REQUIRED",
            "An image chunk is required.".to_owned(),
        )
    })?;

    let mut service = state.services().media();
    let value = service
        .stage_media_chunk(&upload_id, chunk_index, bytes, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    Ok(success(value, &resolved))
}

async fn upload_property_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(property_id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<domain::UploadPropertyMediaResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut role: Option<String> = None;
    let mut alt_text: Option<String> = None;
    let mut file: Option<(String, String, Vec<u8>)> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "MEDIA_MULTIPART_INVALID",
            format!("Invalid media upload: {error}"),
            false,
        )
        .with_correlation(resolved.service.correlation_id.clone())
    })? {
        let name = field.name().unwrap_or_default().to_owned();
        match name.as_str() {
            "role" => {
                role = Some(field.text().await.map_err(|error| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "MEDIA_ROLE_INVALID",
                        format!("Invalid media role: {error}"),
                        false,
                    )
                    .with_correlation(resolved.service.correlation_id.clone())
                })?);
            }
            "altText" => {
                alt_text = Some(field.text().await.map_err(|error| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "MEDIA_ALT_TEXT_INVALID",
                        format!("Invalid media alt text: {error}"),
                        false,
                    )
                    .with_correlation(resolved.service.correlation_id.clone())
                })?);
            }
            "file" => {
                let filename = field.file_name().unwrap_or("upload").to_owned();
                let mime_type = field.content_type().unwrap_or_default().to_owned();
                let bytes = field.bytes().await.map_err(|error| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "MEDIA_FILE_INVALID",
                        format!("Invalid media file: {error}"),
                        false,
                    )
                    .with_correlation(resolved.service.correlation_id.clone())
                })?;
                file = Some((filename, mime_type, bytes.to_vec()));
            }
            _ => {}
        }
    }

    let (filename, mime_type, bytes) = file.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "MEDIA_FILE_REQUIRED",
            "Image file is required.",
            false,
        )
        .with_correlation(resolved.service.correlation_id.clone())
    })?;

    let mut service = state.services().media();
    let value = service
        .upload_property_media(
            UploadPropertyMediaRequest {
                property_id,
                role: role.unwrap_or_default(),
                filename,
                mime_type,
                alt_text,
                bytes,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    Ok(success(value, &resolved))
}

async fn deals(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::DealPortfolioSnapshot>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().deal_portal();
    let value = service
        .portfolio(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn create_deal(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<domain::CreateDealRequest>,
) -> Result<Json<ApiSuccess<domain::CreateDealResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().deal_portal();
    let value = service
        .create(&body, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn deal_workspace(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::DealWorkspaceSnapshot>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().deal_portal();
    let value = service
        .workspace(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn deal_workspace_command(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<domain::DealWorkspaceCommand>,
) -> Result<Json<ApiSuccess<domain::DealWorkspaceCommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().deal_portal();
    let value = service
        .command(&id, &body, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn contracts(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::ContractSummary>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().contract();
    let value = service
        .list(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn contract(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::Contract>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().contract();
    let value = service
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("CONTRACT_NOT_FOUND", format!("Contract not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn contracts_for_process_instance(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<domain::ContractSummary>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().contract();
    let value = service
        .list_for_process_instance(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn forms(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::FormInstanceListItem>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().forms();
    let value = service
        .list_instances(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn form(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::FormInstance>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().forms();
    let value = service
        .get_instance(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("FORM_NOT_FOUND", format!("Form instance not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn create_form(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateFormBody>,
) -> Result<Json<ApiSuccess<domain::FormInstance>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().forms();
    let deal_id = body.deal_id.filter(|value| !value.trim().is_empty());
    let request = domain::CreateFormInstanceRequest {
        template_id: body.template_id,
        template_version: body.template_version,
        deal_id: deal_id.clone(),
        person_id: body.person_id.filter(|value| !value.trim().is_empty()),
        property_id: body.property_id.filter(|value| !value.trim().is_empty()),
        field_values: body.field_values,
        sections: body.sections,
        created_by_user_id: Some(resolved.acting_user.app_user_id.clone()),
    };
    let value = service
        .create_instance(&request, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    if let Some(deal_id) = deal_id.as_deref() {
        service
            .seed_participants_from_deal(&value.id, deal_id, &resolved.service)
            .await
            .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    }
    Ok(success(value, &resolved))
}

async fn update_form(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateFormBody>,
) -> Result<Json<ApiSuccess<domain::FormInstance>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let status = match body.status.as_deref() {
        Some(value) => Some(
            domain::FormInstanceStatus::try_from(value).map_err(|message| {
                correlate(
                    ApiError::from(CoreServiceError::business("FORM_STATUS_INVALID", message)),
                    &resolved,
                )
            })?,
        ),
        None => None,
    };
    let mut service = state.services().forms();
    let value = service
        .update_instance(
            &domain::UpdateFormInstanceRequest {
                form_instance_id: id.clone(),
                input: domain::UpdateFormInstanceInput {
                    field_values: body.field_values,
                    sections: body.sections,
                    status,
                    contract_id: body.contract_id,
                },
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("FORM_NOT_FOUND", format!("Form instance not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn form_deal_facts(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(deal_id): Path<String>,
) -> Result<Json<ApiSuccess<Option<domain::DealFormFacts>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().forms();
    let value = service
        .deal_facts(&deal_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn form_signers(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<domain::FormSignerPerson>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().forms();
    let value = service
        .list_signer_people(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn form_issued_document(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Option<domain::IssuedDocumentForFormInstance>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state
        .services()
        .vault(Arc::new(UnavailableVaultArtifactPort));
    let value = service
        .issued_for_form_instance(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn comms_panel(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    Query(query): Query<CommsPanelQuery>,
) -> Result<Json<ApiSuccess<domain::CommsPanel>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().comms();
    let value = service
        .panel(
            &GetCommsPanelRequest {
                person_id,
                moment_limit: query.moment_limit,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn comms_timeline(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    Query(query): Query<CommsTimelineQuery>,
) -> Result<Json<ApiSuccess<domain::CommsTimeline>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().comms();
    let value = service
        .timeline(
            &GetCommsTimelineRequest {
                person_id,
                page: query.page,
                page_size: query.page_size,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn activity(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<ApiSuccess<Vec<domain::ActivityFeedEntry>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().comms();
    let value = service
        .activity(query.limit.unwrap_or(200), &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// The Accounting dashboard: every figure a projection over the two canonical tables.
async fn issues(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<IssuesQuery>,
) -> Result<Json<ApiSuccess<domain::IssuesPage>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state
        .services()
        .issues()
        .page(
            query.scope.as_deref().unwrap_or("OPERATIONS_EXCEPTION"),
            query.state.as_deref().unwrap_or("OPEN"),
            query.page.unwrap_or(1),
            query.page_size.unwrap_or(50),
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn relationship_evidence_review(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<RelationshipReviewQuery>,
) -> Result<Json<ApiSuccess<domain::RelationshipEvidenceReview>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state
        .services()
        .relationship_evidence()
        .review(
            query.review_state.as_deref().unwrap_or("all"),
            query.search.as_deref().unwrap_or(""),
            query.limit.unwrap_or(50),
            query.offset.unwrap_or(0),
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn relationship_evidence_action(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<RelationshipActionBody>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().relationship_evidence();

    let value = match body.action.as_str() {
        "inspect" => {
            let id = body.id.as_deref().ok_or_else(|| {
                correlate(
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "RELATIONSHIP_ID_REQUIRED",
                        "id is required.",
                        false,
                    ),
                    &resolved,
                )
            })?;
            let row = service
                .inspect(id, &resolved.service)
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?;
            json!({ "ok": true, "row": row })
        }
        "classify_automated" | "classify_service" => {
            let id = body.id.as_deref().ok_or_else(|| {
                correlate(
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "RELATIONSHIP_ID_REQUIRED",
                        "id is required.",
                        false,
                    ),
                    &resolved,
                )
            })?;
            let result = service
                .classify_and_rerun(
                    id,
                    body.action == "classify_automated",
                    body.action == "classify_service",
                    &resolved.service,
                )
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?;
            json!({
                "ok": true,
                "tally": result.tally,
                "canonicalLinked": result.canonical_linked,
                "row": result.rows.first(),
            })
        }
        "link" => {
            let id = body.id.as_deref().ok_or_else(|| {
                correlate(
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "RELATIONSHIP_ID_REQUIRED",
                        "id is required.",
                        false,
                    ),
                    &resolved,
                )
            })?;
            let person_id = body.person_id.as_deref().ok_or_else(|| {
                correlate(
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "PERSON_ID_REQUIRED",
                        "personId is required.",
                        false,
                    ),
                    &resolved,
                )
            })?;
            let row = service
                .link(
                    id,
                    person_id,
                    body.confirm.unwrap_or(false),
                    &resolved.service,
                )
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?;
            json!({ "ok": true, "row": row })
        }
        "reject" => {
            let id = body.id.as_deref().ok_or_else(|| {
                correlate(
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "RELATIONSHIP_ID_REQUIRED",
                        "id is required.",
                        false,
                    ),
                    &resolved,
                )
            })?;
            let row = service
                .reject(id, body.confirm.unwrap_or(false), &resolved.service)
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?;
            json!({ "ok": true, "row": row })
        }
        "rerun" => {
            let result = service
                .rerun(
                    body.source.as_deref(),
                    body.review_state.as_deref(),
                    body.limit.unwrap_or(200),
                    &resolved.service,
                )
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?;
            json!({
                "ok": true,
                "rows": result.rows,
                "tally": result.tally,
                "canonicalLinked": result.canonical_linked,
            })
        }
        _ => {
            return Err(correlate(
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "RELATIONSHIP_ACTION_INVALID",
                    format!("Unknown action: {}", body.action),
                    false,
                ),
                &resolved,
            ));
        }
    };

    Ok(success(value, &resolved))
}

async fn accounting_dashboard(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::AccountingDashboard>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().accounting();
    let value = service
        .dashboard(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn accounting_receivables(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::Receivable>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().accounting();
    let value = service
        .receivables(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn accounting_expenses(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::Expense>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().accounting();
    let value = service
        .expenses(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// Every posted expense by category, for the Expenses screen's breakdown.
async fn accounting_expense_categories(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::CategoryShare>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().accounting();
    let value = service
        .expense_categories(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// The P&L for the period the caller asked for.
///
/// BOTH DATES ARE REQUIRED. Defaulting them here would mean a screen that lost its range silently reported a different
/// period's numbers, which is worse than an error: the figures would look right and be about another quarter.
/// BOTH DATES ARE REQUIRED. Defaulting them here would mean a screen that lost its range silently reported a different
/// period's numbers, which is worse than an error: the figures would look right and be about another quarter.
async fn accounting_pnl(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<AccountingPnlQuery>,
) -> Result<Json<ApiSuccess<domain::PnlStatement>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().accounting();
    let request = domain::PnlRequest {
        from: query.from,
        to: query.to,
    };
    let value = service
        .pnl(&request, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// Record an expense.
async fn create_expense(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateExpenseBody>,
) -> Result<Json<ApiSuccess<AccountingId>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().accounting();
    let command = domain::CreateExpenseCommand {
        vendor: body.vendor,
        category: body.category,
        amount: body.amount,
        expense_on: body.expense_on,
        memo: body.memo,
        deal_id: body.deal_id,
        property_id: body.property_id,
        person_id: body.person_id,
    };
    let id = service
        .create_expense(&command, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(AccountingId { id }, &resolved))
}

/// Record a receivable.
async fn create_receivable(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateReceivableBody>,
) -> Result<Json<ApiSuccess<AccountingId>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().accounting();
    let command = domain::CreateReceivableCommand {
        reference: body.reference,
        description: body.description,
        category: body.category,
        amount: body.amount,
        issued_on: body.issued_on,
        due_on: body.due_on,
        deal_id: body.deal_id,
        property_id: body.property_id,
        person_id: body.person_id,
    };
    let id = service
        .create_receivable(&command, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(AccountingId { id }, &resolved))
}

/// Mark a receivable paid.
///
/// THE ID IS IN THE PATH AND THE DATE IS IN THE BODY, so the transition names the thing it transitions and the date it
/// transitions it on. Both are required; neither has a default, because "today" is a decision the operator makes.
async fn mark_receivable_paid(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<MarkReceivablePaidBody>,
) -> Result<Json<ApiSuccess<domain::MarkReceivablePaidOutcome>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().accounting();
    let command = domain::MarkReceivablePaidCommand {
        receivable_id: id,
        paid_on: body.paid_on,
    };
    let value = service
        .mark_receivable_paid(&command, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// The id a create command produced, so a caller can point at what it made.
#[derive(Debug, serde::Serialize)]
struct AccountingId {
    id: String,
}

async fn route_project_work(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<RouteProjectWorkBody>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut wbs = state.services().wbs();
    let current = wbs
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("WBS_NOT_FOUND", format!("WBS item not found: {id}")),
                &resolved,
            )
        })?;
    let status = domain::WbsStatus::try_from(body.status.as_str()).map_err(|error| {
        correlate(
            ApiError::from(CoreServiceError::business("WBS_STATUS_INVALID", error)),
            &resolved,
        )
    })?;
    let saved = wbs
        .save(
            &domain::SaveWbsItemRequest {
                create: domain::CreateWbsItemRequest {
                    id: current.id.clone(),
                    title: body.title,
                    notes: Some(body.notes),
                    category: current.category.clone(),
                    project_id: current.project_id.clone(),
                    parent_id: current.parent_id.clone(),
                    due_at: body.due_at,
                    owner: body.owner,
                    order: current.order,
                    entity: current.entity.clone(),
                },
                status: Some(status),
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    let alert = body.alert.unwrap_or(true);
    let route = match body.destination.as_str() {
        "task" => serde_json::to_value(
            wbs.queue_apple_reminder(&saved.id, alert, &resolved.service)
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?,
        )
        .unwrap_or_else(|_| json!({"state":"queued"})),
        "calendar" => {
            let start_at = body.start_at.ok_or_else(|| {
                correlate(
                    ApiError::from(CoreServiceError::business(
                        "CALENDAR_TIME_REQUIRED",
                        "Calendar start time is required.",
                    )),
                    &resolved,
                )
            })?;
            let end_at = body.end_at.ok_or_else(|| {
                correlate(
                    ApiError::from(CoreServiceError::business(
                        "CALENDAR_TIME_REQUIRED",
                        "Calendar end time is required.",
                    )),
                    &resolved,
                )
            })?;
            let mut calendar = state.services().calendar();
            serde_json::to_value(
                calendar
                    .create_apple_event(
                        &domain::CreateAppleCalendarEventRequest {
                            title: saved.title.clone(),
                            start_at,
                            end_at,
                            all_day: Some(false),
                            location: body.location,
                            notes: (!saved.notes.trim().is_empty()).then_some(saved.notes.clone()),
                            alert: Some(alert),
                        },
                        &resolved.service,
                    )
                    .await
                    .map_err(|error| correlate(ApiError::from(error), &resolved))?,
            )
            .unwrap_or_else(|_| json!({"state":"queued"}))
        }
        _ => {
            return Err(correlate(
                ApiError::from(CoreServiceError::business(
                    "PROJECT_WORK_DESTINATION_INVALID",
                    "Project work destination must be task or calendar.",
                )),
                &resolved,
            ));
        }
    };

    Ok(success(json!({ "item": saved, "route": route }), &resolved))
}

async fn queue_apple_reminder(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<AppleReminderBody>,
) -> Result<Json<ApiSuccess<domain::AppleReminderCommandReceipt>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().wbs();
    let value = service
        .queue_apple_reminder(&id, body.alert.unwrap_or(false), &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn create_apple_calendar_event(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<domain::CreateAppleCalendarEventRequest>,
) -> Result<Json<ApiSuccess<domain::CalendarCommandReceipt>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().calendar();
    let value = service
        .create_apple_event(&request, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn calendar(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::CalendarEvent>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state.services().calendar();
    let value = service
        .list(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn vault_documents(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::IssuedDocumentListItem>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let scope = VaultActorScope {
        account_type: resolved.acting_user.account_type.clone(),
        person_id: resolved.acting_user.person_id.clone(),
    };
    let mut service = state
        .services()
        .vault(Arc::new(UnavailableVaultArtifactPort));
    let value = service
        .list_issued_documents(Some(&scope), &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn vault_document(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::TransactionDocument>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state
        .services()
        .vault(Arc::new(UnavailableVaultArtifactPort));
    let value = service
        .get_document(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "VAULT_DOCUMENT_NOT_FOUND",
                    format!("Vault document not found: {id}"),
                ),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

async fn vault_documents_by_deal(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<domain::TransactionDocument>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state
        .services()
        .vault(Arc::new(UnavailableVaultArtifactPort));
    let value = service
        .list_by_deal(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn vault_form_contract(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Option<String>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state
        .services()
        .vault(Arc::new(UnavailableVaultArtifactPort));
    let value = service
        .form_contract_id(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

async fn vault_bind_form_contract(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<BindVaultFormContractBody>,
) -> Result<Json<ApiSuccess<()>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state
        .services()
        .vault(Arc::new(UnavailableVaultArtifactPort));
    service
        .bind_form_to_contract(&id, &body.contract_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success((), &resolved))
}

async fn vault_prior_contract_document(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((contract_id, template_id)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<Option<domain::ContractIssuedLineage>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut service = state
        .services()
        .vault(Arc::new(UnavailableVaultArtifactPort));
    let value = service
        .prior_contract_document(&contract_id, &template_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

#[derive(Debug, Deserialize)]
struct VaultDownloadQuery {
    download: Option<String>,
}

fn vault_document_response(
    document: domain::VaultMediaBytes,
    download: bool,
) -> Result<Response, ApiError> {
    let content_type = HeaderValue::from_str(&document.mime_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let mut response = Response::new(Body::from(document.bytes));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, content_type);
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    let safe_filename = document
        .filename
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let disposition = format!(
        "{}; filename=\"{}\"",
        if download { "attachment" } else { "inline" },
        safe_filename
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "VAULT_INVALID_FILENAME",
                "Invalid document filename.",
                false,
            )
        })?,
    );
    Ok(response)
}

async fn submit_website_intake(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<domain::WebsiteIntakeRequest>,
) -> Result<Json<ApiSuccess<domain::WebsiteIntakeResult>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let value = state
        .services()
        .intake()
        .submit_website(&body, &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(value, &context.correlation_id))
}

async fn submit_catchup_lead(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<domain::CatchupLeadRequest>,
) -> Result<Json<ApiSuccess<domain::CatchupLeadResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state
        .services()
        .intake()
        .submit_catchup(&body, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// Email the team and the visitor about one website lead the public site has just saved. The body is empty: the id is
/// all the caller supplies, and the emails are written from the stored lead. Sent once per lead.
async fn notify_website_lead(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::WebsiteLeadNotice>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let notice = state
        .services()
        .website_leads()
        .notify(&id, &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(notice, &context.correlation_id))
}

/// The Island Guide, through its own Rust service and the anonymous public security door.
async fn tech_cockpit(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<TechCockpitQuery>,
) -> Result<Json<ApiSuccess<domain::TechCockpitSnapshot>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let snapshot = state
        .services()
        .tech()
        .snapshot(query.selected.as_deref(), &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(snapshot, &resolved))
}

async fn tech_command(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<domain::TechCommandRequest>,
) -> Result<Json<ApiSuccess<domain::TechCommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let result = state
        .services()
        .tech()
        .command(body, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(result, &resolved))
}

async fn public_marketing_content(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::MarketingContentBlock>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let value = state
        .services()
        .marketing()
        .public_content(&context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(value, &context.correlation_id))
}

async fn public_guide(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::GuideItem>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let items = state
        .services()
        .guide()
        .items(&context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(items, &context.correlation_id))
}

#[derive(serde::Deserialize)]
struct PublicPropertyQuery {
    /// Whatever names the Property: its slug, its name in any case, or its id.
    key: String,
}

#[derive(serde::Deserialize)]
struct PublicSimilarQuery {
    key: String,
    /// The page shows a strip; the service clamps whatever arrives.
    limit: Option<i64>,
}

/// Listings like this one, for the property page.
async fn public_similar(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<PublicSimilarQuery>,
) -> Result<Json<ApiSuccess<Vec<domain::PublicListing>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let listings = state
        .services()
        .public_listings()
        .similar(query.key.trim(), query.limit.unwrap_or(3), &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(listings, &context.correlation_id))
}

/// Every slug the site can serve, for the sitemap.
async fn public_slugs(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<String>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let slugs = state
        .services()
        .public_listings()
        .slugs(&context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(slugs, &context.correlation_id))
}

/// One photograph's bytes for the public site: the web copy if there is one, the original otherwise.
///
/// The bytes are returned as they are stored. An id that is unknown, or that belongs to media which is not published,
/// answers 404 either way — an anonymous visitor must not be able to tell those apart.
async fn public_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let found = state
        .services()
        .public_listings()
        .media_bytes(&id, &context)
        .await
        .map_err(ApiError::from)?;

    let Some((mime_type, bytes)) = found else {
        return Err(ApiError::not_found("MEDIA_NOT_FOUND", "Not found."));
    };

    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, mime_type)
        // Photograph bytes do not change under an id: a replacement upload is a new row.
        .header(axum::http::header::CACHE_CONTROL, "public, max-age=3600")
        .body(axum::body::Body::from(bytes))
        .map_err(|error| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "MEDIA_RESPONSE_FAILED",
                error.to_string(),
                false,
            )
        })
}

/// One Property, for the public page. Anonymous, the same guest door as the inventory, the same published action.
///
/// A key that resolves to nothing answers `null`, not an error: "no such listing" is a 404 for the page, not a fault
/// in the service.
async fn public_property(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<PublicPropertyQuery>,
) -> Result<Json<ApiSuccess<Option<domain::PublicProperty>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let property = state
        .services()
        .public_listings()
        .property(query.key.trim(), &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(property, &context.correlation_id))
}

/// The inventory of the public site, for the anonymous buyer: the same guest door and the same published action as
/// the listing copy above. This is the read the buyers grid renders.
async fn public_listings(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::PublicListing>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let listings = state
        .services()
        .public_listings()
        .listings(&context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(listings, &context.correlation_id))
}

/// The taglines of published listings, for the anonymous public site: the same guest door the public document route
/// uses (no identity headers), authorized as the published `property.public.read`.
async fn public_listing_copy(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::PublicListingCopy>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let copy = state
        .services()
        .public_listings()
        .listing_copy(&context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(copy, &context.correlation_id))
}

async fn vault_public_listing_document_bytes(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<VaultDownloadQuery>,
) -> Result<Response, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let id = uuid::Uuid::parse_str(&id)
        .map_err(|_| ApiError::not_found("VAULT_DOCUMENT_NOT_FOUND", "Document not found."))?;
    let mut vault = state
        .services()
        .vault(Arc::new(UnavailableVaultArtifactPort));
    let document = vault
        .public_listing_document_bytes(&id.to_string(), &context)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("VAULT_DOCUMENT_NOT_FOUND", "Document not found."))?;
    vault_document_response(document, query.download.as_deref() == Some("1"))
}

async fn vault_private_document_bytes(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<VaultDownloadQuery>,
) -> Result<Response, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let id = uuid::Uuid::parse_str(&id)
        .map_err(|_| ApiError::not_found("VAULT_DOCUMENT_NOT_FOUND", "Document not found."))?;
    let mut vault = state
        .services()
        .vault(Arc::new(UnavailableVaultArtifactPort));
    let document = vault
        .media_bytes(&id.to_string(), &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| ApiError::not_found("VAULT_DOCUMENT_NOT_FOUND", "Document not found."))?;
    vault_document_response(document, query.download.as_deref() == Some("1"))
}

pub(crate) fn success<T>(value: T, resolved: &ResolvedRequestContext) -> Json<ApiSuccess<T>> {
    Json(ApiSuccess {
        ok: true,
        value,
        correlation_id: resolved.service.correlation_id.clone(),
    })
}

/// The same envelope for callers that have a correlation id but not a full resolved request context - an engine command
/// from a background job has no acting user, and inventing one would be worse than admitting there is not one.
pub(crate) fn success_with_correlation<T>(value: T, correlation_id: &str) -> Json<ApiSuccess<T>> {
    Json(ApiSuccess {
        ok: true,
        value,
        correlation_id: correlation_id.to_owned(),
    })
}

fn correlate(error: ApiError, resolved: &ResolvedRequestContext) -> ApiError {
    error.with_correlation(resolved.service.correlation_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_failure_for_vault_renderer_is_explicit() {
        let failure = VaultArtifactFailure {
            outcome: VaultCommandOutcome::PreconditionFailure,
            message: "not configured".into(),
        };
        assert_eq!(failure.outcome, VaultCommandOutcome::PreconditionFailure);
    }

    #[test]
    fn api_success_shape_is_camel_case() {
        let body = ApiSuccess {
            ok: true,
            value: json!({"hello": "world"}),
            correlation_id: "corr-1".into(),
        };
        let value = serde_json::to_value(body).unwrap();
        assert_eq!(value["correlationId"], "corr-1");
        assert_eq!(value["ok"], true);
    }
}
