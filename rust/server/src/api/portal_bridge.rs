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

use super::routes::{
    apply_person_admin_update, apply_project_update, apply_property_admin_create, apply_property_admin_save,
    apply_wbs_update, execute_registered, CreatePropertyAdminBody, SavePropertyAdminBody, UpdatePersonAdminBody,
    UpdateProjectBody, UpdateWbsBody,
};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/portal/rust-ui/clients", get(clients))
        .route("/api/portal/rust-ui/page", get(page))
        .route("/api/portal/rust-ui/cabinet", get(cabinet))
        .route("/api/portal/rust-ui/deals", get(deals).post(deals_write))
        .route("/api/portal/rust-ui/projects", get(projects).post(projects_act))
        .route("/api/portal/rust-ui/accounting", axum::routing::post(accounting_act))
        .route("/api/portal/rust-ui/opps", get(opps).post(opps_act))
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
    from: Option<String>,
    to: Option<String>,
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
        screen if ACCOUNTING_SCREENS.contains(&screen) => {
            let payload = accounting_payload(&state, &resolved, screen, query.from.as_deref(), query.to.as_deref()).await?;
            Ok(Json(payload))
        }
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DealsQuery {
    people_search: Option<String>,
    scope: Option<String>,
    id: Option<String>,
}

impl DealsQuery {
    fn deal_id(&self) -> Option<String> {
        self.scope.as_deref().or(self.id.as_deref()).map(str::trim).filter(|id| !id.is_empty()).map(str::to_owned)
    }
}

/// Contracts: the deal portfolio, one deal's workspace (`scope`), or a people search for a new deal (`peopleSearch`).
async fn deals(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<DealsQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if let Some(search) = query.people_search.as_deref() {
        let search = search.trim();
        if search.chars().count() < 2 {
            return Ok(Json(json!({ "people": [] })));
        }
        let request = domain::SearchPeopleRequest { query: search.to_owned(), limit: Some(20) };
        let people = state.services().person().search(&request, &resolved.service).await.map_err(failed(&resolved))?;
        let people: Vec<Value> = people
            .into_iter()
            .map(|person| {
                json!({
                    "id": person.id,
                    "displayName": person.display_name,
                    "role": person.role,
                    "status": person.status,
                    "location": person.location,
                    "email": person.email,
                    "phone": person.phone,
                })
            })
            .collect();
        return Ok(Json(json!({ "people": people })));
    }
    let service = state.services().deal_portal();
    if let Some(deal_id) = query.deal_id() {
        let workspace = service.workspace(&deal_id, &resolved.service).await.map_err(failed(&resolved))?;
        return Ok(Json(json!({ "deals": {
            "deals": [], "contracts": [], "properties": [], "users": [], "workspace": to_json(workspace),
        } })));
    }
    let portfolio = service.portfolio(&resolved.service).await.map_err(failed(&resolved))?;
    let mut deals = to_json(portfolio);
    if let Some(object) = deals.as_object_mut() {
        object.insert("workspace".into(), Value::Null);
    }
    Ok(Json(json!({ "deals": deals })))
}

/// Contracts' writes: a command on one deal (`scope`), or a new deal. Both answer `{ id }`.
async fn deals_write(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<DealsQuery>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let service = state.services().deal_portal();
    if let Some(deal_id) = query.deal_id() {
        let command: domain::DealWorkspaceCommand = serde_json::from_value(body)
            .map_err(|error| ApiError::bad_request("DEAL_COMMAND_INVALID", format!("Invalid command body: {error}")))?;
        let result = service.command(&deal_id, &command, &resolved.service).await.map_err(failed(&resolved))?;
        return Ok(Json(json!({ "id": result.id })));
    }
    let text = |key: &str| {
        body.get(key).and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned)
    };
    let (Some(property_id), Some(client_person_id)) = (text("propertyId"), text("clientPersonId")) else {
        return Err(ApiError::bad_request("DEAL_CREATE_INVALID", "propertyId and clientPersonId are required."));
    };
    let request = domain::CreateDealRequest {
        property_id,
        client_person_id,
        owner_user_id: text("ownerUserId"),
        notes: text("notes"),
    };
    let created = service.create(&request, &resolved.service).await.map_err(failed(&resolved))?;
    Ok(Json(json!({ "id": created.id })))
}

/// snake_case keys to camelCase, all the way down: the project and work-item records are serialized snake_case by the
/// domain, and the Projects screen reads them camelCase (as the relay renamed them field by field).
fn camel_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let mut camel = String::with_capacity(key.len());
                    let mut upper = false;
                    for ch in key.chars() {
                        if ch == '_' {
                            upper = true;
                        } else if upper {
                            camel.extend(ch.to_uppercase());
                            upper = false;
                        } else {
                            camel.push(ch);
                        }
                    }
                    (camel, camel_keys(value))
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(camel_keys).collect()),
        other => other,
    }
}

fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str).filter(|text| !text.is_empty())
}

/// The whole Projects workspace: projects, work items, documents, the properties' media, activity, calendar and the
/// display names of everything the projects point at.
async fn projects_page(state: &ApiState, resolved: &ResolvedRequestContext) -> Result<Json<Value>, ApiError> {
    let services = state.services();
    let context = &resolved.service;
    let scope = domain::VaultActorScope {
        account_type: resolved.acting_user.account_type.clone(),
        person_id: resolved.acting_user.person_id.clone(),
    };
    let project_service = services.project();
    let (wbs, vault) = (services.wbs(), services.vault());
    let (projects, items, documents) = tokio::join!(
        async { project_service.lock().await.list(context).await },
        wbs.list_project_items(context),
        vault.list_issued_documents(Some(&scope), context),
    );
    let projects = projects.map_err(|error| correlate(ApiError::from(error), resolved))?;
    let (items, documents) = (items.map_err(failed(resolved))?, documents.map_err(failed(resolved))?);
    let (projects, items, documents) = (to_json(projects), to_json(items), to_json(documents));
    let empty = Vec::new();
    let (projects, items) = (projects.as_array().unwrap_or(&empty), items.as_array().unwrap_or(&empty));

    let mut people = std::collections::BTreeSet::new();
    let mut properties = std::collections::BTreeSet::new();
    let mut contracts = std::collections::BTreeSet::new();
    for project in projects {
        people.extend(str_at(project, "person_id").map(str::to_owned));
        properties.extend(str_at(project, "property_id").map(str::to_owned));
        contracts.extend(str_at(project, "contract_id").map(str::to_owned));
    }
    for item in items {
        let Some(entity) = item.get("entity") else { continue };
        let Some(id) = str_at(entity, "id").map(str::to_owned) else { continue };
        match str_at(entity, "entity_type") {
            Some("person") => { people.insert(id); }
            Some("property") => { properties.insert(id); }
            Some("contract") => { contracts.insert(id); }
            _ => {}
        }
    }

    // Names and media are supplemental: a record that cannot be read keeps its id on screen, as the relay did.
    let mut names = serde_json::Map::new();
    for id in &people {
        if let Some(person) = services.person().get(id, context).await.ok().flatten().map(to_json) {
            if let Some(name) = str_at(&person, "display_name") {
                names.insert(format!("person:{id}"), json!(name));
            }
        }
    }
    let mut media = Vec::new();
    for id in &properties {
        if let Some(property) = services.property().get(id, context).await.ok().flatten().map(to_json) {
            if let Some(name) = str_at(&property, "display_name") {
                names.insert(format!("property:{id}"), json!(name));
            }
        }
        if let Ok(assets) = services.media().for_property(id, context).await {
            media.extend(to_json(assets).as_array().cloned().unwrap_or_default());
        }
    }
    for id in &contracts {
        if let Some(contract) = services.contract().get(id, context).await.ok().flatten().map(to_json) {
            if let Some(kind) = str_at(&contract, "contract_type") {
                names.insert(format!("contract:{id}"), json!(kind.replace('_', " ")));
            }
        }
    }
    let (comms, calendar) = (services.comms(), services.calendar());
    let (activity, calendar) = tokio::join!(comms.activity(200, context), calendar.list(context));
    let activity = activity.map(to_json).unwrap_or_else(|_| json!([]));
    let calendar = calendar.map(to_json).unwrap_or_else(|_| json!([]));
    let documents: Vec<Value> = documents
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|document| {
            let title = str_at(document, "title").or_else(|| str_at(document, "documentTypeLabel")).unwrap_or("Document");
            json!({
                "id": document.get("id"),
                "propertyId": document.get("propertyId"),
                "title": title,
                "state": document.get("state"),
                "templateId": document.get("templateId"),
                "templateVersion": document.get("templateVersion"),
                "issuedVersion": document.get("issuedVersion"),
                "createdAt": document.get("createdAt"),
                "signedArtifactAvailable": document.get("signedArtifactAvailable"),
                "signedAuditAvailable": document.get("signedAuditAvailable"),
            })
        })
        .collect();
    Ok(Json(json!({ "projects": {
        "projects": camel_keys(Value::Array(projects.clone())),
        "items": camel_keys(Value::Array(items.clone())),
        "documents": documents,
        "media": media,
        "activity": activity,
        "calendar": calendar,
        "identityNames": names,
    } })))
}

async fn projects(State(state): State<ApiState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    projects_page(&state, &resolved).await
}

/// Projects' two writes — a project's status, a work item's save — each answering the refreshed workspace.
async fn projects_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let id_of = |key: &str| str_at(&body, key).map(str::trim).filter(|id| !id.is_empty()).map(str::to_owned);
    match str_at(&body, "action") {
        Some("projectStatus") => {
            let Some(id) = id_of("projectId") else {
                return Err(ApiError::bad_request("PROJECT_ID_REQUIRED", "projectId is required."));
            };
            let update: UpdateProjectBody = serde_json::from_value(json!({ "status": body.get("status") }))
                .map_err(|error| ApiError::bad_request("PROJECT_UPDATE_INVALID", error.to_string()))?;
            apply_project_update(&state, &resolved, id, update).await?;
        }
        Some("wbsSave") => {
            let Some(id) = id_of("itemId") else {
                return Err(ApiError::bad_request("WBS_ID_REQUIRED", "itemId is required."));
            };
            let update: UpdateWbsBody = serde_json::from_value(json!({
                "title": body.get("title"),
                "notes": body.get("notes"),
                "status": body.get("status"),
                "dueAt": body.get("dueAt"),
                "owner": body.get("owner"),
            }))
            .map_err(|error| ApiError::bad_request("WBS_UPDATE_INVALID", error.to_string()))?;
            apply_wbs_update(&state, &resolved, id, update).await?;
        }
        _ => return Err(ApiError::bad_request("PROJECT_ACTION_UNSUPPORTED", "Unsupported Projects action.")),
    }
    projects_page(&state, &resolved).await
}

const ACCOUNTING_SCREENS: &[&str] =
    &["accounting", "accounting-expenses", "accounting-receivables", "accounting-pnl", "accounting-receipt-scanner"];

/// The book's date (UTC, as the relay's `todayISO`), which the create forms default to.
fn book_today() -> chrono::NaiveDate {
    chrono::Utc::now().date_naive()
}

/// One Accounting screen's payload, `{ accounting: ... }`. The P&L period defaults to the current month.
async fn accounting_payload(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    screen: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Value, ApiError> {
    use chrono::Datelike;
    let accounting = state.services().accounting();
    let context = &resolved.service;
    let today = book_today();
    let body = match screen {
        "accounting" => json!({ "dashboard": to_json(accounting.dashboard(context).await.map_err(failed(resolved))?) }),
        "accounting-expenses" => {
            let (expenses, categories) = tokio::join!(accounting.expenses(context), accounting.expense_categories(context));
            json!({
                "expenses": to_json(expenses.map_err(failed(resolved))?),
                "expenseCategories": to_json(categories.map_err(failed(resolved))?),
                "today": today.to_string(),
            })
        }
        "accounting-receivables" => json!({
            "receivables": to_json(accounting.receivables(context).await.map_err(failed(resolved))?),
            "today": today.to_string(),
        }),
        "accounting-pnl" => {
            let first = today.with_day(1).unwrap_or(today);
            let last = first
                .checked_add_months(chrono::Months::new(1))
                .and_then(|next| next.pred_opt())
                .unwrap_or(today);
            let pick = |value: Option<&str>, fallback: chrono::NaiveDate| {
                value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned).unwrap_or_else(|| fallback.to_string())
            };
            let request = domain::PnlRequest { from: pick(from, first), to: pick(to, last) };
            json!({ "pnl": to_json(accounting.pnl(&request, context).await.map_err(failed(resolved))?) })
        }
        _ => json!({ "today": today.to_string() }),
    };
    Ok(json!({ "accounting": body }))
}

/// Accounting's three commands; each answers the screen that asked, as it now stands.
async fn accounting_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let raw = |key: &str| body.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
    let text = |key: &str| body.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
    let screen = raw("screen");
    if !ACCOUNTING_SCREENS.contains(&screen.as_str()) {
        return Err(ApiError::bad_request("ACCOUNTING_SCREEN_UNKNOWN", format!("no accounting screen '{screen}'")));
    }
    let accounting = state.services().accounting();
    let context = &resolved.service;
    match raw("action").as_str() {
        "createExpense" => {
            let command = domain::CreateExpenseCommand {
                vendor: raw("vendor"),
                category: raw("category"),
                // The digits the operator typed: Rust validates the decimal, nothing rounds it first.
                amount: raw("amount"),
                expense_on: raw("expenseOn"),
                memo: text("memo"),
                deal_id: text("dealId"),
                property_id: text("propertyId"),
                person_id: text("personId"),
            };
            accounting.create_expense(&command, context).await.map_err(failed(&resolved))?;
        }
        "createReceivable" => {
            let command = domain::CreateReceivableCommand {
                reference: text("reference"),
                description: raw("description"),
                category: raw("category"),
                amount: raw("amount"),
                issued_on: raw("issuedOn"),
                due_on: text("dueOn"),
                deal_id: text("dealId"),
                property_id: text("propertyId"),
                person_id: text("personId"),
            };
            accounting.create_receivable(&command, context).await.map_err(failed(&resolved))?;
        }
        "markReceivablePaid" => {
            let Some(receivable_id) = text("receivableId") else {
                return Err(ApiError::bad_request("RECEIVABLE_REQUIRED", "A receivable is required."));
            };
            let command = domain::MarkReceivablePaidCommand { receivable_id, paid_on: raw("paidOn") };
            accounting.mark_receivable_paid(&command, context).await.map_err(failed(&resolved))?;
        }
        other => {
            return Err(ApiError::bad_request("ACCOUNTING_COMMAND_UNKNOWN", format!("no accounting command '{other}'")))
        }
    }
    let from = text("from");
    let to = text("to");
    Ok(Json(accounting_payload(&state, &resolved, &screen, from.as_deref(), to.as_deref()).await?))
}

const OPPS_PAGE_SIZE: i64 = 50;

#[derive(Debug, Deserialize)]
struct OppsQuery {
    entity: Option<String>,
    #[serde(default)]
    search: String,
    page: Option<String>,
    selected: Option<String>,
}

fn opps_entity(value: Option<&str>) -> &'static str {
    match value {
        Some("person") => "person",
        Some("project") => "project",
        _ => "property",
    }
}

fn opps_row(id: &Value, title: &Value, subtitle: &Value, status: &Value, meta: &Value) -> Value {
    json!({ "id": id, "title": title, "subtitle": subtitle, "status": status, "meta": meta })
}

fn at<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(&Value::Null)
}

/// The Data Workbench: one entity's list (property, person or project), a page of it, and the selected record.
async fn opps_workbench(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    entity: &str,
    search: &str,
    page_index: i64,
    selected: Option<String>,
) -> Result<Value, ApiError> {
    let services = state.services();
    let context = &resolved.service;
    let empty = Vec::new();
    let mut payload = json!({
        "entity": entity, "rows": [], "total": 0, "page": page_index + 1, "pageSize": OPPS_PAGE_SIZE,
        "selectedId": null, "property": null, "person": null, "project": null, "media": [],
    });
    match entity {
        "person" => {
            let request = domain::ClientAdminPageRequest {
                search: search.to_owned(),
                page: page_index + 1,
                page_size: OPPS_PAGE_SIZE,
            };
            let page = to_json(services.clients().admin(&request, context).await.map_err(failed(resolved))?);
            let rows = page.get("rows").and_then(Value::as_array).unwrap_or(&empty);
            let selected_id = selected
                .filter(|id| rows.iter().any(|row| str_at(row, "id") == Some(id.as_str())))
                .or_else(|| rows.first().and_then(|row| str_at(row, "id")).map(str::to_owned));
            if let Some(id) = &selected_id {
                let (people, clients) = (services.person(), services.clients());
                let (canonical, client) = tokio::join!(people.get(id, context), clients.detail(id, context));
                let canonical = to_json(canonical.map_err(failed(resolved))?);
                let client = to_json(client.map_err(failed(resolved))?);
                payload["person"] = json!({
                    "id": at(&canonical, "id"),
                    "displayName": at(&canonical, "display_name"),
                    "role": client.get("role").filter(|v| !v.is_null()).cloned().unwrap_or(json!("unclassified")),
                    "status": at(&canonical, "status"),
                    "company": at(&canonical, "company"),
                    "civilStatus": at(&canonical, "civil_status"),
                    "location": at(&client, "location"),
                    "email": at(&client, "email"),
                    "phone": at(&client, "phone"),
                });
            }
            payload["rows"] = rows
                .iter()
                .map(|row| {
                    let meta = row.get("primaryEmail").filter(|v| !v.is_null()).unwrap_or(at(row, "primaryPhone"));
                    opps_row(at(row, "id"), at(row, "displayName"), at(row, "location"), at(row, "status"), meta)
                })
                .collect();
            payload["total"] = at(&page, "total").clone();
            payload["page"] = at(&page, "page").clone();
            payload["pageSize"] = at(&page, "pageSize").clone();
            payload["selectedId"] = json!(selected_id);
        }
        "project" => {
            let all = services.project().lock().await.list(context).await;
            let all = to_json(all.map_err(|error| correlate(ApiError::from(error), resolved))?);
            let all = all.as_array().unwrap_or(&empty);
            let needle = search.trim().to_lowercase();
            let filtered: Vec<&Value> = all
                .iter()
                .filter(|project| {
                    if needle.is_empty() {
                        return true;
                    }
                    let mut text: Vec<String> = ["name", "owner", "status", "description", "project_type"]
                        .iter()
                        .filter_map(|key| str_at(project, key).map(str::to_owned))
                        .collect();
                    text.extend(at(project, "areas").as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_owned)));
                    text.join(" ").to_lowercase().contains(&needle)
                })
                .collect();
            let start = (page_index * OPPS_PAGE_SIZE) as usize;
            let rows: Vec<&Value> = filtered.iter().skip(start).take(OPPS_PAGE_SIZE as usize).copied().collect();
            let selected_id = selected
                .filter(|id| rows.iter().any(|row| str_at(row, "id") == Some(id.as_str())))
                .or_else(|| rows.first().and_then(|row| str_at(row, "id")).map(str::to_owned));
            if let Some(project) = selected_id.as_deref().and_then(|id| all.iter().find(|p| str_at(p, "id") == Some(id))) {
                let mut project = camel_keys(project.clone());
                if let Some(object) = project.as_object_mut() {
                    object.remove("createdAt");
                    object.remove("updatedAt");
                }
                payload["project"] = project;
            }
            payload["rows"] = rows
                .iter()
                .map(|row| opps_row(at(row, "id"), at(row, "name"), at(row, "project_type"), at(row, "status"), at(row, "owner")))
                .collect();
            payload["total"] = json!(filtered.len());
            payload["selectedId"] = json!(selected_id);
        }
        _ => {
            let property = services.property();
            let request = domain::PropertyAdminPageRequest {
                search: search.to_owned(),
                page: page_index + 1,
                page_size: OPPS_PAGE_SIZE,
            };
            let page = to_json(property.admin_page(&request, context).await.map_err(failed(resolved))?);
            let rows = page.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
            let selected_id = selected.or_else(|| rows.first().and_then(|row| str_at(row, "id")).map(str::to_owned));
            let status_of = |row: &Value| {
                if at(row, "archived").as_bool() == Some(true) { json!("archived") } else { at(row, "status").clone() }
            };
            let mut out: Vec<Value> = rows
                .iter()
                .map(|row| opps_row(at(row, "id"), at(row, "name"), at(row, "location"), &status_of(row), at(row, "listPrice")))
                .collect();
            if let Some(id) = &selected_id {
                let media_service = services.media();
                let (detail, media) = tokio::join!(property.admin_get(id, context), media_service.for_property(id, context));
                let detail = detail.map_err(failed(resolved))?.ok_or_else(|| {
                    correlate(ApiError::not_found("PROPERTY_NOT_FOUND", format!("Property not found: {id}")), resolved)
                })?;
                let mut detail = to_json(detail);
                let seller = match str_at(&detail, "sellerPersonId").map(str::to_owned) {
                    Some(seller_id) => to_json(services.clients().detail(&seller_id, context).await.map_err(failed(resolved))?),
                    None => Value::Null,
                };
                if let Some(object) = detail.as_object_mut() {
                    if let Some(name) = seller.get("displayName").filter(|v| !v.is_null()) {
                        object.insert("sellerName".into(), name.clone());
                    }
                    object.insert("sellerEmail".into(), at(&seller, "email").clone());
                    object.insert("sellerPhone".into(), at(&seller, "phone").clone());
                    object.insert("sellerLocation".into(), at(&seller, "location").clone());
                }
                if !out.iter().any(|row| str_at(row, "id") == Some(id.as_str())) {
                    out.insert(0, opps_row(at(&detail, "id"), at(&detail, "name"), at(&detail, "location"), &status_of(&detail), at(&detail, "listPrice")));
                }
                payload["property"] = detail;
                payload["media"] = to_json(media.map_err(failed(resolved))?);
            }
            payload["rows"] = Value::Array(out);
            payload["total"] = at(&page, "total").clone();
            payload["page"] = at(&page, "page").clone();
            payload["pageSize"] = at(&page, "pageSize").clone();
            payload["selectedId"] = json!(selected_id);
        }
    }
    Ok(json!({ "ops": payload }))
}

async fn opps(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<OppsQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let page_index = query.page.as_deref().and_then(|page| page.trim().parse::<i64>().ok()).unwrap_or(0).max(0);
    let selected = query.selected.map(|id| id.trim().to_owned()).filter(|id| !id.is_empty());
    let entity = opps_entity(query.entity.as_deref());
    Ok(Json(opps_workbench(&state, &resolved, entity, query.search.trim(), page_index, selected).await?))
}

/// The property save body, from the workbench's string fields: blanks are absent, flags are "true".
fn property_save_body(fields: &serde_json::Map<String, Value>) -> Value {
    const FLAGS: &[&str] = &[
        "featured", "isActiveListing", "isPublished", "hasOceanView", "hasBayView", "hasBeachView", "hasHarborView",
        "hasIslandView", "hasMountainView", "hasSunriseView", "hasSunsetView", "hasWaterAccess", "hasBeachAccess",
        "hasPool", "hasGenerator", "hasSolar", "isFurnished", "isGated", "archived",
    ];
    const TEXT: &[&str] = &[
        "slug", "propertyType", "listPrice", "originalListPrice", "location", "addressLine1", "streetNumber", "streetName",
        "unitNumber", "city", "stateOrProvince", "neighborhood", "postalCode", "country", "isoCountryCode", "latitude",
        "longitude", "bedrooms", "bathrooms", "bathroomsFull", "bathroomsHalf", "squareFeet", "lotSize", "lotSizeUnits",
        "lotSizeAcres", "lotSizeSqft", "roadFrontageFeet", "roadSurfaceType", "lotDescription", "utilitiesNotes",
        "catastroNumber", "buildability", "slopeDescription", "poolPotential", "roadAdjacency", "utilitiesAvailability",
        "hoaStatus", "viewDescription", "yearBuilt", "stories", "parkingSpaces", "shortDescription",
        "editorialDescription", "publicRemarks", "seoTitle", "seoDescription", "heroTitle", "tagline",
        "architectureNotes", "amenitiesNotes", "lifestyleNotes", "listingAgentName", "listingAgentEmail",
        "listingAgentPhone", "listingOffice", "legalOwnerName", "listingIdentifier", "registryEntry", "fincaNumber",
        "registrySection", "sellerPersonId",
    ];
    const STELLAR: &[&str] = &[
        "listingContractDate", "expirationDate", "listingType", "agentMlsId", "taxId", "taxYear", "annualTax",
        "legalDescription", "zoning", "totalAreaSqft", "heatedAreaSource", "ownershipType", "hoaDetails",
        "showingInstructions", "occupantType",
    ];
    let raw = |key: &str| fields.get(key).and_then(Value::as_str).unwrap_or("");
    let clean = |key: &str| {
        let value = raw(key).trim();
        if value.is_empty() { Value::Null } else { json!(value) }
    };
    let mut body = serde_json::Map::new();
    body.insert("name".into(), json!(raw("name")));
    body.insert("status".into(), json!(if raw("status").is_empty() { "prospect" } else { raw("status") }));
    for key in FLAGS {
        body.insert((*key).into(), json!(raw(key) == "true"));
    }
    for key in TEXT {
        body.insert((*key).into(), clean(key));
    }
    let stellar: serde_json::Map<String, Value> = STELLAR.iter().map(|key| ((*key).to_owned(), clean(key))).collect();
    body.insert("stellar".into(), Value::Object(stellar));
    Value::Object(body)
}

/// The Workbench's writes: create a property, or save the open property, person or project; each answers the
/// workbench with the saved record selected.
async fn opps_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(command): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let invalid = |error: serde_json::Error| ApiError::bad_request("WORKBENCH_SAVE_INVALID", error.to_string());
    if str_at(&command, "action") == Some("createProperty") {
        let body: CreatePropertyAdminBody = serde_json::from_value(json!({
            "name": at(&command, "name"),
            "propertyType": at(&command, "propertyType"),
        }))
        .map_err(invalid)?;
        let property = to_json(apply_property_admin_create(&state, &resolved, body).await?);
        let mut record = property.clone();
        if let Some(object) = record.as_object_mut() {
            for key in ["sellerEmail", "sellerPhone", "sellerLocation"] {
                object.insert(key.into(), Value::Null);
            }
        }
        return Ok(Json(json!({ "ops": {
            "entity": "property",
            "rows": [opps_row(at(&property, "id"), at(&property, "name"), at(&property, "location"), at(&property, "status"), at(&property, "listPrice"))],
            "total": 1, "page": 1, "pageSize": OPPS_PAGE_SIZE, "selectedId": at(&property, "id"),
            "property": record, "person": null, "project": null, "media": [],
        } })));
    }
    let id = str_at(&command, "id").map(str::trim).filter(|id| !id.is_empty()).map(str::to_owned);
    let (Some("save"), Some(id)) = (str_at(&command, "action"), id) else {
        return Err(ApiError::bad_request("WORKBENCH_SAVE_INVALID", "A workbench save requires an entity and id."));
    };
    let empty = serde_json::Map::new();
    let fields = command.get("fields").and_then(Value::as_object).unwrap_or(&empty);
    let field = |key: &str| fields.get(key).and_then(Value::as_str).unwrap_or("");
    let clean = |key: &str| {
        let value = field(key).trim();
        if value.is_empty() { Value::Null } else { json!(value) }
    };
    let entity = opps_entity(str_at(&command, "entity"));
    match entity {
        "property" => {
            let body: SavePropertyAdminBody = serde_json::from_value(property_save_body(fields)).map_err(invalid)?;
            apply_property_admin_save(&state, &resolved, id.clone(), body).await?;
        }
        "person" => {
            let body: UpdatePersonAdminBody = serde_json::from_value(json!({
                "displayName": field("displayName"),
                "status": field("status"),
                "company": clean("company"),
                "civilStatus": clean("civilStatus"),
            }))
            .map_err(invalid)?;
            apply_person_admin_update(&state, &resolved, id.clone(), body).await?;
        }
        _ => {
            let areas: Vec<String> =
                field("areas").split(',').map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned).collect();
            let version = field("playbookVersion").trim().parse::<i64>().ok();
            let body: UpdateProjectBody = serde_json::from_value(json!({
                "name": clean("name"), "owner": clean("owner"), "status": clean("status"),
                "description": field("description"), "areas": areas, "projectType": clean("projectType"),
                "playbookId": clean("playbookId"), "playbookVersion": version, "personId": clean("personId"),
                "propertyId": clean("propertyId"), "contractId": clean("contractId"),
            }))
            .map_err(invalid)?;
            apply_project_update(&state, &resolved, id.clone(), body).await?;
        }
    }
    let search = str_at(&command, "search").unwrap_or("").trim().to_owned();
    let page_index = command.get("page").and_then(Value::as_i64).unwrap_or(0).max(0);
    Ok(Json(opps_workbench(&state, &resolved, entity, &search, page_index, Some(id)).await?))
}
