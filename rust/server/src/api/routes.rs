use super::context::{resolve_request_context, ResolvedRequestContext};
use super::{diagnostics, engine, ApiError, ApiState};
use crate::service_support::CoreServiceError;
use crate::vault::VaultArtifactPort;
use async_trait::async_trait;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use domain::{
    ClientAdminPageRequest, ClientDirectoryPageRequest, ClientHistoryRequest, GetCommsPanelRequest,
    GetCommsTimelineRequest, SearchPeopleRequest, UploadPropertyMediaRequest, VaultActorScope,
    VaultArtifactFailure, VaultCommandOutcome, VaultRenderRequest, VaultRenderedArtifact,
    MAX_MEDIA_UPLOAD_BYTES,
};
use integrations::boldsign::{BoldSignConfig, BoldSignSignatureProvider};
use serde::{Deserialize, Serialize};
use serde_json::json;
use service::SignatureProvider;
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
    person_id: Option<String>,
    security_level: String,
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
struct UpdateProjectBody {
    status: Option<String>,
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

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/v1/whoami", get(whoami))
        .route("/v1/cockpit", get(cockpit))
        .route("/v1/workflows", get(workflows))
        .route("/v1/workflows/{id}", get(workflow_detail))
        .route("/v1/projects", get(projects))
        .route("/v1/projects/{id}", get(project).patch(update_project))
        .route("/v1/wbs/project-items", get(wbs_project_items))
        .route("/v1/wbs/{id}", get(wbs_item).patch(update_wbs_item))
        .route("/v1/tasks/{id}/complete", post(complete_task))
        .route(
            "/v1/wbs/{id}/apple-reminder",
            post(queue_apple_reminder),
        )
        .route("/v1/wbs/{id}/route", post(route_project_work))
        .route("/v1/clients", get(clients))
        .route("/v1/clients/agents", get(client_agents))
        .route("/v1/clients/{person_id}/history", get(client_history))
        .route("/v1/clients/{person_id}", get(client_detail))
        .route("/v1/people/search", get(search_people))
        .route("/v1/people/{id}", get(person))
        .route("/v1/people/{id}/properties", get(properties_for_person))
        .route("/v1/properties/{id}", get(property))
        .route(
            "/v1/properties/{id}/media",
            get(property_media)
                .post(upload_property_media)
                .layer(DefaultBodyLimit::max(MAX_MEDIA_UPLOAD_BYTES + 1024 * 1024)),
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
        .route("/v1/calendar", get(calendar).post(create_apple_calendar_event))
        .route("/v1/vault/documents", get(vault_documents))
        .route("/v1/vault/documents/{id}", get(vault_document))
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
                name: None,
                owner: None,
                status,
                description: None,
                areas: None,
                starts_at: None,
                ends_at: None,
                project_type: None,
                playbook_id: None,
                playbook_version: None,
                person_id: None,
                property_id: None,
                contract_id: None,
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
            ))
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
