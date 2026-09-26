//! Rust-owned compatibility endpoints for the WASM portal.
//!
//! These replace server-side TypeScript relays. A request resolves its caller
//! once, calls the long-lived Rust services directly, and returns the payload
//! shape consumed by the Rust UI.

use super::context::ResolvedRequestContext;
use super::ui_auth::resolve_portal_context;
use super::{ApiError, ApiState};
use crate::service_support::CoreServiceError;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use domain::{
    ClientDetail, ClientDirectoryPageRequest, ClientsPageResult, CommsPanel, GetCommsPanelRequest,
    PersonPropertyContext,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::routes::execute_registered;

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/portal/rust-ui/clients", get(clients))
        .route("/api/portal/rust-ui/page", get(page))
        .route("/api/portal/rust-ui/cabinet", get(cabinet))
}

#[derive(Debug, Deserialize)]
struct ClientsBridgeQuery {
    #[serde(default = "default_client_screen")]
    screen: String,
    scope: Option<String>,
    selected: Option<String>,
    #[serde(default)]
    search: String,
    page: Option<i64>,
}

fn default_client_screen() -> String {
    "clients".into()
}

#[derive(Debug, Serialize)]
struct ClientsBridgeResponse {
    clients: ClientsBridgePage,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientsBridgePage {
    rows: Vec<ClientBridgeSummary>,
    total: i64,
    page: i64,
    page_size: i64,
    selected_id: Option<String>,
    selected: Option<ClientDetail>,
    comms: Option<CommsPanel>,
    properties: Vec<ClientBridgeProperty>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientBridgeSummary {
    id: String,
    display_name: String,
    name_resolved: bool,
    role: String,
    status: String,
    primary_email: Option<String>,
    primary_phone: Option<String>,
    observed_count: i64,
    two_way: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientBridgeProperty {
    id: String,
    display_name: String,
    relation: String,
    relation_status: Option<String>,
    address: String,
}

struct HydratedClient {
    selected: Option<ClientDetail>,
    comms: CommsPanel,
    properties: Vec<ClientBridgeProperty>,
}

async fn clients(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ClientsBridgeQuery>,
) -> Result<Json<ClientsBridgeResponse>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;

    if query.screen == "client-record" {
        let person_id = query
            .scope
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "CLIENT_SCOPE_REQUIRED",
                    "client-record requires scope.",
                    false,
                )
            })?;
        let hydrated = hydrate_client(&state, &resolved, &person_id)
            .await
            .map_err(|error| correlate(ApiError::from(error), &resolved))?;
        return Ok(Json(ClientsBridgeResponse {
            clients: ClientsBridgePage {
                rows: Vec::new(),
                total: 0,
                page: 1,
                page_size: 50,
                selected_id: Some(person_id),
                selected: hydrated.selected,
                comms: Some(hydrated.comms),
                properties: hydrated.properties,
            },
        }));
    }

    if query.screen != "clients" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "CLIENT_SCREEN_UNSUPPORTED",
            format!("unsupported client screen '{}'", query.screen),
            false,
        ));
    }

    let page_index = query.page.unwrap_or(0).max(0);
    let selected_id = query.selected.filter(|value| !value.trim().is_empty());
    let clients_service = state.services().clients();
    let context = resolved.service.clone();
    let directory_request = ClientDirectoryPageRequest {
        search: query.search.trim().to_owned(),
        status: None,
        role: None,
        sort: "name".into(),
        page: page_index + 1,
        page_size: 50,
    };

    if let Some(person_id) = selected_id.as_deref() {
        let (directory, hydrated) = tokio::try_join!(
            clients_service.directory(&directory_request, &context),
            hydrate_client(&state, &resolved, person_id),
        )
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
        return Ok(Json(build_clients_response(
            directory,
            Some(person_id.to_owned()),
            Some(hydrated),
        )));
    }

    let directory = clients_service
        .directory(&directory_request, &context)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    let selected_id = directory.rows.first().map(|row| row.id.clone());
    let hydrated = match selected_id.as_deref() {
        Some(person_id) => Some(
            hydrate_client(&state, &resolved, person_id)
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?,
        ),
        None => None,
    };
    Ok(Json(build_clients_response(
        directory,
        selected_id,
        hydrated,
    )))
}

async fn hydrate_client(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    person_id: &str,
) -> Result<HydratedClient, CoreServiceError> {
    let services = state.services();
    let clients = services.clients();
    let comms = services.comms();
    let property = services.property();
    let context = resolved.service.clone();
    let person_id = person_id.to_owned();
    let comms_request = GetCommsPanelRequest {
        person_id: person_id.clone(),
        moment_limit: Some(20),
    };

    let (selected, comms, properties) = tokio::try_join!(
        clients.detail(&person_id, &context),
        comms.panel(&comms_request, &context),
        property.for_person(&person_id, &context),
    )?;

    Ok(HydratedClient {
        selected,
        comms,
        properties: map_properties(properties),
    })
}

fn build_clients_response(
    directory: ClientsPageResult,
    selected_id: Option<String>,
    hydrated: Option<HydratedClient>,
) -> ClientsBridgeResponse {
    let (selected, comms, properties) = match hydrated {
        Some(value) => (value.selected, Some(value.comms), value.properties),
        None => (None, None, Vec::new()),
    };
    ClientsBridgeResponse {
        clients: ClientsBridgePage {
            rows: directory
                .rows
                .into_iter()
                .map(|row| ClientBridgeSummary {
                    id: row.id,
                    display_name: row.display_name,
                    name_resolved: row.name_resolved,
                    role: row.role,
                    status: row.status,
                    primary_email: row.primary_email,
                    primary_phone: row.primary_phone,
                    observed_count: row.relationship_activity.observed_communication_count,
                    two_way: row.relationship_activity.two_way,
                })
                .collect(),
            total: directory.total,
            page: directory.page,
            page_size: directory.page_size,
            selected_id,
            selected,
            comms,
            properties,
        },
    }
}

fn map_properties(context: PersonPropertyContext) -> Vec<ClientBridgeProperty> {
    context
        .properties
        .into_iter()
        .map(|entry| {
            let property = entry.property;
            let region = [
                property.address.state_or_province.as_deref(),
                property.address.postal_code.as_deref(),
            ]
            .into_iter()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ");
            let address = [
                property.address.address_line1.as_deref(),
                property.address.neighborhood.as_deref(),
                property.address.city.as_deref(),
                (!region.is_empty()).then_some(region.as_str()),
                property.address.country.as_deref(),
            ]
            .into_iter()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join(", ");
            ClientBridgeProperty {
                id: property.id,
                display_name: property.display_name,
                relation: entry.relation.as_str().into(),
                relation_status: entry.relation_status,
                address,
            }
        })
        .collect()
}

fn correlate(error: ApiError, resolved: &ResolvedRequestContext) -> ApiError {
    error.with_correlation(resolved.service.correlation_id.clone())
}

/// A service answer as JSON, or the service's failure tagged with this request's correlation id.
fn to_json<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn failed(resolved: &ResolvedRequestContext) -> impl Fn(CoreServiceError) -> ApiError + '_ {
    move |error| correlate(ApiError::from(error), resolved)
}

#[derive(Debug, Deserialize)]
struct PageQuery {
    #[serde(default)]
    screen: String,
    scope: Option<String>,
}

/// A portal screen's page payload, in the screen's own shape (`{ activity }`, `{ workflows }`, `{ workflow }`).
async fn page(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let scope = query.scope.as_deref().map(str::trim).filter(|value| !value.is_empty());
    match query.screen.as_str() {
        "activity" => {
            let entries = state
                .services()
                .comms()
                .activity(50, &resolved.service)
                .await
                .map_err(failed(&resolved))?;
            Ok(Json(json!({ "activity": entries })))
        }
        "workflows" | "tech-flight-recorder" => {
            let service = state.services().workflow_portal();
            let context = resolved.service.clone();
            let list = execute_registered(&state, "workflow-portal", "workflow.list", json!({}), async move {
                service.list(&context).await
            })
            .await
            .map_err(|error| correlate(error, &resolved))?;
            Ok(Json(json!({ "workflows": list })))
        }
        "workflow-record" => {
            let Some(id) = scope.map(str::to_owned) else {
                return Err(ApiError::bad_request("WORKFLOW_SCOPE_REQUIRED", "workflow-record requires scope."));
            };
            let service = state.services().workflow_portal();
            let context = resolved.service.clone();
            let work_id = id.clone();
            let detail = execute_registered(
                &state,
                "workflow-portal",
                "workflow.detail",
                json!({ "id": id }),
                async move { service.detail(&work_id, &context).await },
            )
            .await
            .map_err(|error| correlate(error, &resolved))?
            .ok_or_else(|| {
                correlate(ApiError::not_found("WORKFLOW_NOT_FOUND", format!("Workflow instance not found: {id}")), &resolved)
            })?;
            Ok(Json(json!({ "workflow": detail })))
        }
        other => Err(correlate(
            ApiError::new(
                StatusCode::NOT_IMPLEMENTED,
                "PAGE_NOT_IN_RUST_YET",
                format!("The '{other}' screen has no Rust service yet."),
                false,
            ),
            &resolved,
        )),
    }
}

/// The Cabinet: every issued document the caller may see, as `{ cabinet: { documents } }`.
async fn cabinet(State(state): State<ApiState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let scope = domain::VaultActorScope {
        account_type: resolved.acting_user.account_type.clone(),
        person_id: resolved.acting_user.person_id.clone(),
    };
    let documents = state
        .services()
        .vault()
        .list_issued_documents(Some(&scope), &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(json!({ "cabinet": { "documents": to_json(documents) } })))
}
