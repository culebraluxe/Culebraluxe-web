//! THE PORTAL'S OWN ADDRESSES — every endpoint the Yew portal calls (`rust/ui/src/app/api.rs`), answered by the services.
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
use base64::Engine as _;
use chrono::Utc;
use domain::{
    ClientDetail, ClientDirectoryPageRequest, ClientsPageResult, CommsPanel, GetCommsPanelRequest,
    PersonPropertyContext,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::routes::{
    apply_person_admin_update, apply_project_update, apply_property_admin_create, apply_property_admin_save,
    apply_wbs_update, break_glass_readiness, execute_registered, CreatePropertyAdminBody, SavePropertyAdminBody, UpdatePersonAdminBody,
    UpdateProjectBody, UpdateWbsBody,
};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/portal/rust-ui/entitlements", get(entitlements))
        .route("/api/portal/rust-ui/cockpit", get(cockpit).post(cockpit_act))
        .route("/api/portal/rust-ui/tech", get(tech).post(tech_act))
        .route("/api/portal/rust-ui/clients", get(clients))
        .route("/api/portal/rust-ui/page", get(page))
        .route("/api/portal/rust-ui/cabinet", get(cabinet))
        .route("/api/portal/rust-ui/deals", get(deals).post(deals_write))
        .route("/api/portal/rust-ui/forms", get(forms).post(forms_write))
        .route("/api/portal/rust-ui/forms/preview", axum::routing::post(forms_preview))
        .route("/api/portal/rust-ui/projects", get(projects).post(projects_act))
        .route(
            "/api/portal/rust-ui/projects/calendar",
            get(projects_calendar).post(projects_calendar_update),
        )
        .route(
            "/api/portal/rust-ui/projects/calendar-command",
            get(projects_calendar_command),
        )
        .route("/api/portal/rust-ui/accounting", axum::routing::post(accounting_act))
        .route("/api/portal/rust-ui/opps", get(opps).post(opps_act))
        .route("/api/portal/rust-ui/listing-media", get(listing_media))
        .route("/api/portal/rust-ui/rows", get(rows))
        .route("/api/portal/rust-ui/security-users", axum::routing::put(security_users_put))
        .route("/api/portal/rust-ui/role-entitlements", axum::routing::put(role_entitlements_put))
        .route(
            "/api/property-media/chunked",
            axum::routing::post(property_media_chunked).layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
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
        screen if SUPPORT_SCREENS.contains(&screen) => {
            Ok(Json(json!({ "support": support_payload(&state, &resolved, screen, scope).await? })))
        }
        "storyboard" => {
            let snapshot = state.services().tech().snapshot(None, &resolved.service).await.map_err(failed(&resolved))?;
            Ok(Json(super::tech_page::storyboard(&to_json(snapshot))))
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


#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FormsBridgeQuery {
    #[serde(default = "default_forms_screen")]
    screen: String,
    scope: Option<String>,
    deal_id: Option<String>,
    person_id: Option<String>,
    property_id: Option<String>,
}

fn default_forms_screen() -> String {
    "forms".into()
}

fn load_form_templates(
    resolved: &ResolvedRequestContext,
) -> Result<domain::forms_template::TemplateLibrary, ApiError> {
    domain::forms_template::TemplateLibrary::load_default().map_err(|error| {
        correlate(
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "FORM_TEMPLATE_LOAD_FAILED",
                error.to_string(),
                false,
            ),
            resolved,
        )
    })
}

const ACTIVE_FORM_TEMPLATE_VERSIONS: &[(&str, i32)] = &[
    ("OFFER-01", 2),
    ("PR-PNS", 3),
    ("PR-PNS-AMD", 1),
    ("LISTING-01", 4),
    ("SHOW-INFO", 1),
    ("SHOW-RPT", 1),
];

const PORTAL_FORM_TEMPLATE_IDS: &[&str] = &["SHOW-RPT", "OFFER-01", "PR-PNS", "LISTING-01"];

fn active_form_template<'a>(
    library: &'a domain::forms_template::TemplateLibrary,
    id: &str,
) -> Option<&'a domain::forms_template::TemplateDefinition> {
    let version = ACTIVE_FORM_TEMPLATE_VERSIONS
        .iter()
        .find_map(|(template_id, version)| (*template_id == id).then_some(*version))?;
    library.version(id, version)
}

fn form_presentation(value: domain::forms_template::TemplatePresentation) -> &'static str {
    match value {
        domain::forms_template::TemplatePresentation::Agreement => "agreement",
        domain::forms_template::TemplatePresentation::Letter => "letter",
        domain::forms_template::TemplatePresentation::Information => "information",
        domain::forms_template::TemplatePresentation::Report => "report",
    }
}

fn form_field_type(value: domain::forms_template::TemplateFieldType) -> &'static str {
    match value {
        domain::forms_template::TemplateFieldType::Text => "text",
        domain::forms_template::TemplateFieldType::Money => "money",
        domain::forms_template::TemplateFieldType::Date => "date",
        domain::forms_template::TemplateFieldType::Textarea => "textarea",
        domain::forms_template::TemplateFieldType::Select => "select",
    }
}

fn when_payload(when: Option<&domain::forms_template::TemplateWhen>) -> Value {
    when.map_or(
        Value::Null,
        |when| json!({ "field": when.field, "values": when.values }),
    )
}

fn form_template_payload(
    template: &domain::forms_template::TemplateDefinition,
    library: &domain::forms_template::TemplateLibrary,
) -> Value {
    let fields: Vec<Value> = template
        .fields
        .iter()
        .map(|field| {
            json!({
                "name": field.name,
                "label": field.label,
                "type": form_field_type(field.field_type),
                "required": field.required,
                "options": field.options,
                "when": when_payload(field.when.as_ref()),
            })
        })
        .collect();
    let sections: Vec<Value> = template
        .sections
        .iter()
        .map(|section| {
            let segments: Vec<Value> = section
                .segments
                .iter()
                .map(|segment| match segment {
                    domain::forms_template::TemplateSectionSegment::Text(text) => {
                        json!({ "kind": "text", "text": text })
                    }
                    domain::forms_template::TemplateSectionSegment::Value(field) => {
                        json!({ "kind": "value", "field": field })
                    }
                })
                .collect();
            json!({
                "name": section.name,
                "label": section.label,
                "editable": section.editable,
                "segments": segments,
                "when": when_payload(section.when.as_ref()),
            })
        })
        .collect();
    let signature_groups: Vec<Value> = template
        .signature_groups
        .iter()
        .map(|group| {
            json!({
                "role": group.role,
                "label": group.label,
                "field": group.field,
                "initials": group.initials,
            })
        })
        .collect();
    json!({
        "id": template.id,
        "version": template.version,
        "activeVersion": active_form_template(library, &template.id).map(|item| item.version).unwrap_or(template.version),
        "displayName": template.display_name,
        "documentTypeLabel": template.document_type_label,
        "renderingTitle": template.rendering.title,
        "presentation": form_presentation(template.rendering.presentation),
        "fields": fields,
        "sections": sections,
        "signatureGroups": signature_groups,
    })
}

fn form_template_choices(library: &domain::forms_template::TemplateLibrary) -> Vec<Value> {
    PORTAL_FORM_TEMPLATE_IDS
        .iter()
        .filter_map(|id| active_form_template(library, id))
        .map(|template| {
            json!({
                "id": template.id,
                "displayName": template.display_name,
                "activeVersion": template.version,
            })
        })
        .collect()
}

fn form_item_payload(
    item: &domain::FormInstanceListItem,
    library: &domain::forms_template::TemplateLibrary,
) -> Value {
    let mut value = to_json(item);
    if let Some(object) = value.as_object_mut() {
        let name = library
            .version(&item.instance.template_id, item.instance.template_version)
            .map(|template| template.display_name.clone())
            .unwrap_or_else(|| item.instance.template_id.clone());
        let active = active_form_template(library, &item.instance.template_id)
            .map(|template| template.version)
            .unwrap_or(item.instance.template_version);
        object.insert("templateName".into(), json!(name));
        object.insert("activeVersion".into(), json!(active));
    }
    value
}

fn selected_form_payload(
    form: &domain::FormInstance,
    field_values: std::collections::BTreeMap<String, String>,
    library: &domain::forms_template::TemplateLibrary,
) -> Value {
    let mut value = to_json(form);
    if let Some(object) = value.as_object_mut() {
        let name = library
            .version(&form.template_id, form.template_version)
            .map(|template| template.display_name.clone())
            .unwrap_or_else(|| form.template_id.clone());
        let active = active_form_template(library, &form.template_id)
            .map(|template| template.version)
            .unwrap_or(form.template_version);
        object.insert("templateName".into(), json!(name));
        object.insert("activeVersion".into(), json!(active));
        object.insert("dealLabel".into(), Value::Null);
        object.insert("propertyLabel".into(), Value::Null);
        object.insert("clientName".into(), Value::Null);
        object.insert("fieldValues".into(), json!(field_values));
    }
    value
}

async fn forms_page(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    record: Option<&str>,
    deal_id: Option<&str>,
    person_id: Option<&str>,
    property_id: Option<&str>,
) -> Result<Value, ApiError> {
    let services = state.services();
    let forms = services.forms();
    let library = load_form_templates(resolved)?;
    let mut items = forms
        .list_instances(&resolved.service)
        .await
        .map_err(failed(resolved))?;

    if record.is_none() {
        if let Some(deal_id) = deal_id.map(str::trim).filter(|value| !value.is_empty()) {
            items.retain(|item| item.instance.deal_id.as_deref() == Some(deal_id));
        }
        if let Some(person_id) = person_id.map(str::trim).filter(|value| !value.is_empty()) {
            items.retain(|item| item.instance.person_id.as_deref() == Some(person_id));
        }
        if let Some(property_id) = property_id.map(str::trim).filter(|value| !value.is_empty()) {
            items.retain(|item| item.instance.property_id.as_deref() == Some(property_id));
        }
    }

    let item_payloads = items
        .iter()
        .map(|item| form_item_payload(item, &library))
        .collect::<Vec<_>>();

    let Some(form_id) = record.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(json!({
            "items": item_payloads,
            "selected": Value::Null,
            "template": Value::Null,
            "issued": Value::Null,
            "signers": [],
            "signature": Value::Null,
            "templateChoices": form_template_choices(&library),
        }));
    };

    let form = forms
        .get_instance(form_id, &resolved.service)
        .await
        .map_err(failed(resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "FORM_NOT_FOUND",
                    format!("Form instance not found: {form_id}"),
                ),
                resolved,
            )
        })?;
    let template = library
        .version(&form.template_id, form.template_version)
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "FORM_TEMPLATE_NOT_FOUND",
                    format!(
                        "Template {} v{} is not available.",
                        form.template_id, form.template_version
                    ),
                ),
                resolved,
            )
        })?;

    let mut field_values = form.field_values.clone();
    if template.field("sellerCivilStatus").is_some() {
        if let Some(person_id) = form.person_id.as_deref() {
            if let Some(person) = services
                .person()
                .get(person_id, &resolved.service)
                .await
                .map_err(failed(resolved))?
            {
                if let Some(civil_status) = person
                    .civil_status
                    .filter(|value| !value.trim().is_empty())
                {
                    // Person is canonical for civil status. Old form JSON must never shadow a repaired Person value.
                    field_values.insert("sellerCivilStatus".into(), civil_status);
                }
            }
        }
    }
    if form.template_id == "LISTING-01" {
        if let Some(property_id) = form.property_id.as_deref() {
            if let Some(property) = services
                .property()
                .admin_get(property_id, &resolved.service)
                .await
                .map_err(failed(resolved))?
            {
                if let Some(listing_type) = property
                    .stellar
                    .listing_type
                    .filter(|value| !value.trim().is_empty())
                {
                    // Property's Stellar listing record is canonical for listing type when it already has a value.
                    field_values.insert("listingType".into(), listing_type);
                }
            }
        }
    }

    let signers = forms
        .list_signer_people(form_id, &resolved.service)
        .await
        .map_err(failed(resolved))?;
    let issued = services
        .vault()
        .issued_for_form_instance(form_id, &resolved.service)
        .await
        .map_err(failed(resolved))?;
    let signature = if let Some(document) = issued.as_ref() {
        match services.signature() {
            Ok(signature) => signature
                .active_for_document(&document.document_id, &resolved.service)
                .await
                .ok()
                .flatten(),
            Err(_) => None,
        }
    } else {
        None
    };

    Ok(json!({
        "items": item_payloads,
        "selected": selected_form_payload(&form, field_values, &library),
        "template": form_template_payload(template, &library),
        "issued": issued,
        "signers": signers,
        "signature": signature,
        "templateChoices": form_template_choices(&library),
    }))
}

fn form_default(template_id: &str, field_name: &str) -> Option<&'static str> {
    match (template_id, field_name) {
        ("LISTING-01", "brokerName") => Some("Lisa Penfield"),
        ("LISTING-01", "sellerCivilStatus") => Some("Single"),
        ("LISTING-01", "commission") => Some("4%"),
        ("LISTING-01", "listingType") => Some("Exclusive Right to Sell"),
        ("PR-PNS", "sellerBrokerName") => Some("Lisa Penfield"),
        ("OFFER-01", "brokerName") => Some("Lisa Penfield"),
        ("SHOW-RPT", "agentName") => Some("Lisa Penfield"),
        _ => None,
    }
}

fn date_default(field_name: &str) -> String {
    let today = chrono::Utc::now().date_naive();
    let days = if field_name.to_ascii_lowercase().contains("expir") {
        14
    } else if field_name.to_ascii_lowercase().contains("end") {
        90
    } else {
        0
    };
    (today + chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

fn one_line_address_part(value: Option<&str>) -> Option<String> {
    let parts = value?
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|part| part.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn redundant_pr_country(country: Option<&str>, state: Option<&str>) -> bool {
    if state.map(str::trim).unwrap_or_default().to_ascii_uppercase() != "PR" {
        return false;
    }
    let country = country
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace('.', "");
    matches!(
        country.as_str(),
        "united states" | "united states of america" | "us" | "usa"
    )
}

fn format_property_address(address: &domain::PropertyAddress) -> String {
    let state_postal = [
        one_line_address_part(address.state_or_province.as_deref()),
        one_line_address_part(address.postal_code.as_deref()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    [
        one_line_address_part(address.address_line1.as_deref()),
        one_line_address_part(address.neighborhood.as_deref()),
        one_line_address_part(address.city.as_deref()),
        (!state_postal.is_empty()).then_some(state_postal),
        (!redundant_pr_country(
            address.country.as_deref(),
            address.state_or_province.as_deref(),
        ))
        .then(|| one_line_address_part(address.country.as_deref()))
        .flatten(),
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.trim().is_empty())
    .collect::<Vec<_>>()
    .join(", ")
}

fn binding_value(
    binding: &str,
    facts: Option<&domain::DealFormFacts>,
    person: Option<&domain::Person>,
    property: Option<&domain::Property>,
) -> Option<String> {
    match binding {
        "deal.client.name" => facts.and_then(|facts| facts.client_name.clone()),
        "deal.property.label" => facts.and_then(|facts| facts.property_label.clone()),
        "deal.offer.amount" => facts.and_then(|facts| facts.offer_amount.clone()),
        "deal.financing.type" => facts.and_then(|facts| facts.financing_type.clone()),
        "deal.closing.date" => facts.and_then(|facts| facts.closing_date.clone()),
        "person.displayName" => person
            .map(|person| person.display_name.clone())
            .or_else(|| facts.and_then(|facts| facts.person_display_name.clone()))
            .or_else(|| facts.and_then(|facts| facts.client_name.clone())),
        "property.name" => property
            .map(|property| property.display_name.clone())
            .or_else(|| facts.and_then(|facts| facts.property_name.clone()))
            .or_else(|| facts.and_then(|facts| facts.property_label.clone())),
        "property.location" => property
            .map(|property| format_property_address(&property.address))
            .filter(|value| !value.trim().is_empty())
            .or_else(|| facts.and_then(|facts| facts.property_location.clone())),
        _ => None,
    }
}

fn prefill_form_values(
    template: &domain::forms_template::TemplateDefinition,
    facts: Option<&domain::DealFormFacts>,
    person: Option<&domain::Person>,
    property: Option<&domain::Property>,
) -> std::collections::BTreeMap<String, String> {
    let mut values = std::collections::BTreeMap::new();
    for field in &template.fields {
        let mut value = field
            .binding
            .as_deref()
            .and_then(|binding| binding_value(binding, facts, person, property))
            .or_else(|| form_default(&template.id, &field.name).map(str::to_owned))
            .unwrap_or_default();

        if field.name == "sellerCivilStatus" {
            if let Some(civil_status) = person
                .and_then(|person| person.civil_status.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                value = civil_status.to_owned();
            }
        }

        if value.trim().is_empty()
            && matches!(
                field.field_type,
                domain::forms_template::TemplateFieldType::Date
            )
        {
            value = date_default(&field.name);
        }
        values.insert(field.name.clone(), value);
    }
    values
}

fn empty_form_sections(
    template: &domain::forms_template::TemplateDefinition,
) -> std::collections::BTreeMap<String, String> {
    template
        .sections
        .iter()
        .map(|section| (section.name.clone(), String::new()))
        .collect()
}

async fn save_form_values(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    form_id: &str,
    field_values: std::collections::BTreeMap<String, String>,
    sections: std::collections::BTreeMap<String, String>,
) -> Result<domain::FormInstance, ApiError> {
    let services = state.services();
    let forms = services.forms();
    let current = forms
        .get_instance(form_id, &resolved.service)
        .await
        .map_err(failed(resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "FORM_NOT_FOUND",
                    format!("Form instance not found: {form_id}"),
                ),
                resolved,
            )
        })?;

    let library = load_form_templates(resolved)?;
    let template = library
        .version(&current.template_id, current.template_version)
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "FORM_TEMPLATE_NOT_FOUND",
                    format!(
                        "Template {} v{} is not available.",
                        current.template_id, current.template_version
                    ),
                ),
                resolved,
            )
        })?;

    let updated = forms
        .update_instance(
            &domain::UpdateFormInstanceRequest {
                form_instance_id: form_id.to_owned(),
                input: domain::UpdateFormInstanceInput {
                    field_values: Some(field_values.clone()),
                    sections: Some(sections),
                    status: None,
                    contract_id: None,
                },
            },
            &resolved.service,
        )
        .await
        .map_err(failed(resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "FORM_NOT_FOUND",
                    format!("Form instance not found: {form_id}"),
                ),
                resolved,
            )
        })?;

    if template.field("sellerCivilStatus").is_some() {
        if let (Some(person_id), Some(raw_civil_status)) = (
            current.person_id.as_deref(),
            field_values.get("sellerCivilStatus"),
        ) {
            let desired = raw_civil_status
                .trim()
                .to_owned();
            let desired = (!desired.is_empty()).then_some(desired);
            if let Some(person) = services
                .person()
                .get(person_id, &resolved.service)
                .await
                .map_err(failed(resolved))?
            {
                let current_status = person
                    .civil_status
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                if current_status != desired.as_deref() {
                    let updated_person = services
                        .person()
                        .update_admin(
                            &domain::UpdatePersonAdminRequest {
                                person_id: person.id,
                                display_name: person.display_name,
                                civil_status: desired,
                                status: person.status,
                                company: person.company,
                            },
                            &resolved.service,
                        )
                        .await
                        .map_err(failed(resolved))?;
                    services.clients().update_cached_person(&updated_person);
                }
            }
        }
    }

    if current.template_id == "LISTING-01" {
        if let (Some(property_id), Some(raw_listing_type)) = (
            current.property_id.as_deref(),
            field_values.get("listingType"),
        ) {
            let desired = raw_listing_type.trim().to_owned();
            services
                .property()
                .set_listing_type(
                    &domain::SetPropertyListingTypeRequest {
                        property_id: property_id.to_owned(),
                        listing_type: (!desired.is_empty()).then_some(desired),
                    },
                    &resolved.service,
                )
                .await
                .map_err(failed(resolved))?;
        }
    }

    Ok(updated)
}

async fn forms(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<FormsBridgeQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let scope = query
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let record = match query.screen.as_str() {
        "forms" => None,
        "form-record" => Some(scope.ok_or_else(|| {
            correlate(
                ApiError::bad_request("FORM_SCOPE_REQUIRED", "form-record requires scope."),
                &resolved,
            )
        })?),
        _ => {
            return Err(correlate(
                ApiError::bad_request(
                    "FORM_SCREEN_UNSUPPORTED",
                    format!("Unsupported forms screen '{}'.", query.screen),
                ),
                &resolved,
            ))
        }
    };
    let page = forms_page(
        &state,
        &resolved,
        record,
        query.deal_id.as_deref(),
        query.person_id.as_deref(),
        query.property_id.as_deref(),
    )
    .await?;
    Ok(Json(json!({ "forms": page })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FormsPreviewBody {
    form_id: String,
    #[serde(default)]
    field_values: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    sections: std::collections::BTreeMap<String, String>,
}

async fn forms_preview(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<FormsPreviewBody>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let form_id = body.form_id.trim();
    if form_id.is_empty() {
        return Err(correlate(
            ApiError::bad_request("FORM_ID_REQUIRED", "formId is required."),
            &resolved,
        ));
    }

    let services = state.services();
    let forms = services.forms();
    let form = forms
        .get_instance(form_id, &resolved.service)
        .await
        .map_err(failed(&resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "FORM_NOT_FOUND",
                    format!("Form instance not found: {form_id}"),
                ),
                &resolved,
            )
        })?;
    let participants = forms
        .list_signer_people(form_id, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    let issued = services
        .vault()
        .issued_for_form_instance(form_id, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    let issued_version = issued
        .as_ref()
        .map(|document| document.issued_version.max(1))
        .unwrap_or(1);

    let artifact = services
        .vault()
        .render_form_preview(
            domain::VaultRenderRequest {
                form_instance_id: form.id.clone(),
                contract_id: form.contract_id.clone(),
                template_id: form.template_id.clone(),
                template_version: form.template_version,
                field_values: body.field_values,
                sections: body.sections,
                issued_version,
                participants,
                actor_app_user_id: Some(resolved.acting_user.app_user_id.clone()),
                issued_at: None,
                applied_signatures: Vec::new(),
            },
            &resolved.service,
        )
        .await
        .map_err(failed(&resolved))?;

    let encoded = base64::engine::general_purpose::STANDARD.encode(artifact.bytes);
    Ok(Json(json!({
        "dataUri": format!("data:application/pdf;base64,{encoded}"),
        "filename": artifact.filename,
    })))
}

async fn forms_write(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let action = str_at(&body, "action").unwrap_or_default();
    let form_id = str_at(&body, "formId")
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match action {
        "create" => {
            let template_id = str_at(&body, "templateId")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    correlate(
                        ApiError::bad_request("FORM_TEMPLATE_REQUIRED", "templateId is required."),
                        &resolved,
                    )
                })?;
            let deal_id = str_at(&body, "dealId")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            let person_id = str_at(&body, "personId")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            let property_id = str_at(&body, "propertyId")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            if deal_id.is_none() && person_id.is_none() && property_id.is_none() {
                return Err(correlate(
                    ApiError::bad_request(
                        "FORM_CONTEXT_REQUIRED",
                        "Select a deal, client, or property before creating a form.",
                    ),
                    &resolved,
                ));
            }

            let services = state.services();
            let forms = services.forms();
            let library = load_form_templates(&resolved)?;
            let template = active_form_template(&library, template_id).ok_or_else(|| {
                correlate(
                    ApiError::not_found("FORM_TEMPLATE_NOT_FOUND", "Template not found."),
                    &resolved,
                )
            })?;
            let facts = if let Some(deal_id) = deal_id.as_deref() {
                forms
                    .deal_facts(deal_id, &resolved.service)
                    .await
                    .map_err(failed(&resolved))?
            } else {
                None
            };
            let person = if let Some(person_id) = person_id.as_deref() {
                services
                    .person()
                    .get(person_id, &resolved.service)
                    .await
                    .map_err(failed(&resolved))?
            } else {
                None
            };
            let property = if let Some(property_id) = property_id.as_deref() {
                services
                    .property()
                    .get(property_id, &resolved.service)
                    .await
                    .map_err(failed(&resolved))?
            } else {
                None
            };
            let created = forms
                .create_instance(
                    &domain::CreateFormInstanceRequest {
                        template_id: template.id.clone(),
                        template_version: template.version,
                        deal_id: deal_id.clone(),
                        person_id: person_id.clone(),
                        property_id: property_id.clone(),
                        field_values: prefill_form_values(
                            template,
                            facts.as_ref(),
                            person.as_ref(),
                            property.as_ref(),
                        ),
                        sections: empty_form_sections(template),
                        created_by_user_id: Some(resolved.acting_user.app_user_id.clone()),
                    },
                    &resolved.service,
                )
                .await
                .map_err(failed(&resolved))?;
            if let Some(deal_id) = deal_id.as_deref() {
                forms
                    .seed_participants_from_deal(&created.id, deal_id, &resolved.service)
                    .await
                    .map_err(failed(&resolved))?;
            }
            let page = forms_page(&state, &resolved, Some(&created.id), None, None, None).await?;
            Ok(Json(json!({ "formId": created.id, "forms": page })))
        }
        "fillClient" => {
            let form_id = form_id.ok_or_else(|| {
                correlate(
                    ApiError::bad_request("FORM_ID_REQUIRED", "formId is required."),
                    &resolved,
                )
            })?;
            let seller_name = str_at(&body, "sellerName")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    correlate(
                        ApiError::bad_request(
                            "FORM_SELLER_REQUIRED",
                            "Enter the seller name before filling from Clients.",
                        ),
                        &resolved,
                    )
                })?;

            let services = state.services();
            let forms = services.forms();
            let current = forms
                .get_instance(form_id, &resolved.service)
                .await
                .map_err(failed(&resolved))?
                .ok_or_else(|| {
                    correlate(
                        ApiError::not_found(
                            "FORM_NOT_FOUND",
                            format!("Form instance not found: {form_id}"),
                        ),
                        &resolved,
                    )
                })?;
            if current.template_id != "LISTING-01" {
                return Err(correlate(
                    ApiError::bad_request(
                        "FORM_CLIENT_BIND_UNSUPPORTED",
                        "Fill Client is only available for the Listing Agreement.",
                    ),
                    &resolved,
                ));
            }
            if current.status == domain::FormInstanceStatus::Issued {
                return Err(correlate(
                    ApiError::new(
                        StatusCode::CONFLICT,
                        "FORM_CLIENT_BIND_LOCKED",
                        "Issued Listing Agreements cannot change client context.",
                        false,
                    ),
                    &resolved,
                ));
            }

            let directory = services
                .clients()
                .directory(
                    &domain::ClientDirectoryPageRequest {
                        search: seller_name.to_owned(),
                        status: None,
                        role: None,
                        sort: "name".into(),
                        page: 1,
                        page_size: 8,
                    },
                    &resolved.service,
                )
                .await
                .map_err(failed(&resolved))?;
            let normalized = |value: &str| {
                value
                    .trim()
                    .to_lowercase()
                    .replace('’', "")
                    .replace('\'', "")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let needle = normalized(seller_name);
            let exact = directory
                .rows
                .iter()
                .filter(|row| normalized(&row.display_name) == needle)
                .collect::<Vec<_>>();
            let chosen = if exact.len() == 1 {
                exact[0]
            } else if directory.rows.len() == 1 {
                &directory.rows[0]
            } else if directory.rows.is_empty() {
                return Err(correlate(
                    ApiError::not_found(
                        "FORM_CLIENT_NOT_FOUND",
                        format!("No Client found for “{seller_name}”."),
                    ),
                    &resolved,
                ));
            } else {
                let choices = directory
                    .rows
                    .iter()
                    .take(5)
                    .map(|row| row.display_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(correlate(
                    ApiError::new(
                        StatusCode::CONFLICT,
                        "FORM_CLIENT_AMBIGUOUS",
                        format!("More than one Client matches “{seller_name}”: {choices}."),
                        false,
                    ),
                    &resolved,
                ));
            };

            let chosen_id = chosen.id.clone();
            let chosen_display_name = chosen.display_name.clone();
            let person = services
                .person()
                .get(&chosen_id, &resolved.service)
                .await
                .map_err(failed(&resolved))?
                .ok_or_else(|| {
                    correlate(
                        ApiError::not_found(
                            "FORM_CLIENT_NOT_FOUND",
                            format!("Client Person not found: {chosen_id}"),
                        ),
                        &resolved,
                    )
                })?;
            let property_context = services
                .property()
                .for_person(&chosen_id, &resolved.service)
                .await
                .map_err(failed(&resolved))?;

            let legal_address = property_context
                .properties
                .iter()
                .find(|row| row.relation == domain::PersonPropertyRelation::LegalAddress);
            let related_physical = property_context
                .properties
                .iter()
                .find(|row| row.relation == domain::PersonPropertyRelation::PhysicalProperty);

            let fallback_property_id = if current.property_id.is_some() {
                current.property_id.clone()
            } else if let Some(deal_id) = current.deal_id.as_deref() {
                forms
                    .resolve_deal_launch_context(deal_id, &resolved.service)
                    .await
                    .map_err(failed(&resolved))?
                    .map(|context| context.property_id)
            } else {
                None
            };
            let physical = if let Some(physical) = related_physical {
                Some(physical.property.clone())
            } else if let Some(property_id) = fallback_property_id.as_deref() {
                services
                    .property()
                    .get(property_id, &resolved.service)
                    .await
                    .map_err(failed(&resolved))?
            } else {
                None
            };
            let physical_property_id = physical
                .as_ref()
                .map(|property| property.id.clone())
                .or(fallback_property_id);

            forms
                .bind_listing_context(
                    &domain::BindListingFormContextRequest {
                        form_instance_id: form_id.to_owned(),
                        person_id: chosen_id.clone(),
                        property_id: physical_property_id.clone(),
                    },
                    &resolved.service,
                )
                .await
                .map_err(failed(&resolved))?;

            // Port of the legacy Listing canonical binder: switching the Client deliberately replaces
            // only Person/Property-owned fields. Listing terms (price, commission, dates, etc.) stay untouched.
            let legal_address_text = legal_address
                .map(|row| format_property_address(&row.property.address))
                .unwrap_or_default();
            let physical_address = physical
                .as_ref()
                .map(|property| format_property_address(&property.address))
                .unwrap_or_default();
            let property_known_as = physical
                .as_ref()
                .and_then(|property| property.local_name.clone())
                .filter(|value| !value.trim().is_empty())
                .or_else(|| (!physical_address.is_empty()).then(|| physical_address.clone()))
                .unwrap_or_else(|| person.display_name.clone());

            let mut values = current.field_values.clone();
            values.insert("sellerName".into(), person.display_name.clone());
            values.insert("sellerResidenceAddress".into(), legal_address_text);
            values.insert("property".into(), property_known_as);
            values.insert("propertyLocation".into(), physical_address);
            if let Some(property) = physical.as_ref() {
                values.insert(
                    "legalOwnerName".into(),
                    property.legal_owner_name.clone().unwrap_or_default(),
                );
                values.insert(
                    "catastroNumber".into(),
                    property.catastro_number.clone().unwrap_or_default(),
                );
            } else {
                values.insert("legalOwnerName".into(), String::new());
                values.insert("catastroNumber".into(), String::new());
            }
            if let Some(civil_status) = person
                .civil_status
                .clone()
                .filter(|value| !value.trim().is_empty())
            {
                values.insert("sellerCivilStatus".into(), civil_status);
            }
            if let Some(property_id) = physical_property_id.as_deref() {
                if let Some(property) = services
                    .property()
                    .admin_get(property_id, &resolved.service)
                    .await
                    .map_err(failed(&resolved))?
                {
                    if let Some(listing_type) = property
                        .stellar
                        .listing_type
                        .filter(|value| !value.trim().is_empty())
                    {
                        values.insert("listingType".into(), listing_type);
                    }
                }
            }

            forms
                .update_instance(
                    &domain::UpdateFormInstanceRequest {
                        form_instance_id: form_id.to_owned(),
                        input: domain::UpdateFormInstanceInput {
                            field_values: Some(values),
                            sections: None,
                            status: None,
                            contract_id: None,
                        },
                    },
                    &resolved.service,
                )
                .await
                .map_err(failed(&resolved))?;

            let page = forms_page(&state, &resolved, Some(form_id), None, None, None).await?;
            Ok(Json(json!({
                "formId": form_id,
                "forms": page,
                "message": format!("Client linked · {}", chosen_display_name),
            })))
        }
        "sendSignature" => {
            let form_id = form_id.ok_or_else(|| {
                correlate(
                    ApiError::bad_request("FORM_ID_REQUIRED", "formId is required."),
                    &resolved,
                )
            })?;
            let field_values: std::collections::BTreeMap<String, String> = serde_json::from_value(
                body.get("fieldValues")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            )
            .map_err(|error| {
                correlate(
                    ApiError::bad_request(
                        "FORM_FIELDS_INVALID",
                        format!("Invalid form fields: {error}"),
                    ),
                    &resolved,
                )
            })?;
            let sections: std::collections::BTreeMap<String, String> =
                serde_json::from_value(body.get("sections").cloned().unwrap_or_else(|| json!({})))
                    .map_err(|error| {
                        correlate(
                            ApiError::bad_request(
                                "FORM_SECTIONS_INVALID",
                                format!("Invalid form sections: {error}"),
                            ),
                            &resolved,
                        )
                    })?;

            save_form_values(&state, &resolved, form_id, field_values, sections).await?;

            let services = state.services();
            let signature = services.signature().map_err(|reason| {
                correlate(
                    ApiError::new(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "SIGNATURE_UNAVAILABLE",
                        format!("Signature service is unavailable: {reason}"),
                        true,
                    ),
                    &resolved,
                )
            })?;

            if let Some(existing_document) = services
                .vault()
                .issued_for_form_instance(form_id, &resolved.service)
                .await
                .map_err(failed(&resolved))?
            {
                if let Some(active) = signature
                    .active_for_document(&existing_document.document_id, &resolved.service)
                    .await
                    .map_err(failed(&resolved))?
                {
                    let page =
                        forms_page(&state, &resolved, Some(form_id), None, None, None).await?;
                    return Ok(Json(json!({
                        "formId": form_id,
                        "forms": page,
                        "message": format!("Sent for signature · {}", active.status.as_str()),
                    })));
                }
            }

            let issue = services
                .vault()
                .issue_from_form_instance(
                    &domain::IssueDocumentRequest {
                        command_id: uuid::Uuid::new_v4().to_string(),
                        form_instance_id: form_id.to_owned(),
                        actor_app_user_id: Some(resolved.acting_user.app_user_id.clone()),
                        issued_at: None,
                    },
                    &resolved.service,
                )
                .await
                .map_err(failed(&resolved))?;
            if issue.outcome != domain::VaultCommandOutcome::Success {
                return Err(correlate(
                    ApiError::new(
                        StatusCode::CONFLICT,
                        "FORM_ISSUE_FAILED",
                        issue
                            .message
                            .unwrap_or_else(|| "Could not issue the form before signature.".into()),
                        false,
                    ),
                    &resolved,
                ));
            }

            let issued = services
                .vault()
                .issued_for_form_instance(form_id, &resolved.service)
                .await
                .map_err(failed(&resolved))?
                .ok_or_else(|| {
                    correlate(
                        ApiError::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "FORM_ISSUED_DOCUMENT_MISSING",
                            "The issued document could not be loaded.",
                            false,
                        ),
                        &resolved,
                    )
                })?;
            let signers = services
                .forms()
                .list_signer_people(form_id, &resolved.service)
                .await
                .map_err(failed(&resolved))?;

            let mut recipients = Vec::new();
            let mut completion_recipient_emails = Vec::new();
            for signer in &signers {
                let Some(email) = signer
                    .email
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                if signer.role == "SELLER_BROKER" {
                    completion_recipient_emails.push(email.to_owned());
                    continue;
                }
                let execution_slot_id = signer.slot_id.clone();
                let execution_role = execution_slot_id
                    .as_ref()
                    .map(|_| signer.role.clone());
                recipients.push(domain::SignatureRecipient {
                    role: domain::SignatureRecipientRole::Signer,
                    name: signer.name.clone(),
                    email: email.to_owned(),
                    order: recipients.len() as i32 + 1,
                    execution_role,
                    execution_slot_id,
                });
            }
            if recipients.is_empty() {
                return Err(correlate(
                    ApiError::bad_request(
                        "FORM_SIGNER_REQUIRED",
                        "No external signer with an email is available for this form.",
                    ),
                    &resolved,
                ));
            }

            let sent = signature
                .send(
                    &domain::SendSignatureRequest {
                        command_id: uuid::Uuid::new_v4().to_string(),
                        transaction_document_id: issued.document_id.clone(),
                        recipients: recipients.clone(),
                        message: None,
                        created_by_user_id: Some(resolved.acting_user.app_user_id.clone()),
                        execution_role: None,
                        execution_slot_id: None,
                        slot_recipient_email: None,
                        signature_role: None,
                        completion_recipient_emails,
                    },
                    &resolved.service,
                )
                .await
                .map_err(failed(&resolved))?;

            if sent.outcome != domain::SignatureCommandOutcome::Success {
                return Err(correlate(
                    ApiError::new(
                        StatusCode::BAD_GATEWAY,
                        "FORM_SIGNATURE_SEND_FAILED",
                        sent.message
                            .unwrap_or_else(|| "Could not send document for signature.".into()),
                        true,
                    ),
                    &resolved,
                ));
            }

            let page = forms_page(&state, &resolved, Some(form_id), None, None, None).await?;
            Ok(Json(json!({
                "formId": form_id,
                "forms": page,
                "message": format!(
                    "Sent for signature · {} external {}",
                    recipients.len(),
                    if recipients.len() == 1 { "party" } else { "parties" },
                ),
            })))
        }
        "save" | "issue" => {
            let form_id = form_id.ok_or_else(|| {
                correlate(
                    ApiError::bad_request("FORM_ID_REQUIRED", "formId is required."),
                    &resolved,
                )
            })?;
            let field_values: std::collections::BTreeMap<String, String> = serde_json::from_value(
                body.get("fieldValues")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            )
            .map_err(|error| {
                correlate(
                    ApiError::bad_request(
                        "FORM_FIELDS_INVALID",
                        format!("Invalid form fields: {error}"),
                    ),
                    &resolved,
                )
            })?;
            let sections: std::collections::BTreeMap<String, String> =
                serde_json::from_value(body.get("sections").cloned().unwrap_or_else(|| json!({})))
                    .map_err(|error| {
                        correlate(
                            ApiError::bad_request(
                                "FORM_SECTIONS_INVALID",
                                format!("Invalid form sections: {error}"),
                            ),
                            &resolved,
                        )
                    })?;

            save_form_values(&state, &resolved, form_id, field_values, sections).await?;

            if action == "issue" {
                let command = state
                    .services()
                    .vault()
                    .issue_from_form_instance(
                        &domain::IssueDocumentRequest {
                            command_id: uuid::Uuid::new_v4().to_string(),
                            form_instance_id: form_id.to_owned(),
                            actor_app_user_id: Some(resolved.acting_user.app_user_id.clone()),
                            issued_at: None,
                        },
                        &resolved.service,
                    )
                    .await
                    .map_err(failed(&resolved))?;

                if command.outcome != domain::VaultCommandOutcome::Success {
                    let status = match command.outcome {
                        domain::VaultCommandOutcome::NotFound => StatusCode::NOT_FOUND,
                        domain::VaultCommandOutcome::Conflict => StatusCode::CONFLICT,
                        domain::VaultCommandOutcome::Unauthorized => StatusCode::FORBIDDEN,
                        domain::VaultCommandOutcome::ValidationFailure
                        | domain::VaultCommandOutcome::PreconditionFailure => {
                            StatusCode::BAD_REQUEST
                        }
                        domain::VaultCommandOutcome::Success => StatusCode::OK,
                    };
                    return Err(correlate(
                        ApiError::new(
                            status,
                            "FORM_ISSUE_FAILED",
                            command
                                .message
                                .unwrap_or_else(|| "Could not issue the form PDF.".into()),
                            false,
                        ),
                        &resolved,
                    ));
                }
            }

            let page = forms_page(&state, &resolved, Some(form_id), None, None, None).await?;
            Ok(Json(json!({ "formId": form_id, "forms": page })))
        }
        _ => Err(correlate(
            ApiError::bad_request("FORM_ACTION_UNSUPPORTED", "Unsupported Forms action."),
            &resolved,
        )),
    }
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
    let activity = services
        .comms()
        .activity(200, context)
        .await
        .map(to_json)
        .unwrap_or_else(|_| json!([]));
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
        "calendar": [],
        "calendarToday": Utc::now().format("%Y-%m-%d").to_string(),
        "calendarDayStartHour": calendar_display_hour("CALENDAR_DISPLAY_START_HOUR", 8),
        "calendarDayEndHour": calendar_display_hour("CALENDAR_DISPLAY_END_HOUR", 20),
        "calendarSlotMinutes": calendar_slot_minutes(),
        "identityNames": names,
    } })))
}

async fn projects(State(state): State<ApiState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    projects_page(&state, &resolved).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectsCalendarQuery {
    start_at: String,
    end_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectsCalendarCommandQuery {
    command_id: String,
}

fn calendar_display_hour(key: &str, fallback: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|hour| *hour <= 23)
        .unwrap_or(fallback)
}

fn calendar_slot_minutes() -> u32 {
    std::env::var("CALENDAR_SLOT_MINUTES")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|minutes| matches!(*minutes, 15 | 30 | 60))
        .unwrap_or(30)
}

async fn projects_calendar(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ProjectsCalendarQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let request = domain::CalendarViewportQuery {
        start_at: query.start_at,
        end_at: query.end_at,
    };
    let events = state
        .services()
        .calendar()
        .viewport(&request, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(json!({ "calendar": to_json(events) })))
}

async fn projects_calendar_update(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let request: domain::UpdateAppleCalendarEventRequest = serde_json::from_value(body)
        .map_err(|error| ApiError::bad_request("CALENDAR_UPDATE_INVALID", error.to_string()))?;
    let receipt = state
        .services()
        .calendar()
        .update_apple_event(&request, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(to_json(receipt)))
}

async fn projects_calendar_command(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ProjectsCalendarCommandQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let state_value = state
        .services()
        .calendar()
        .command_state(&query.command_id, &resolved.service)
        .await
        .map_err(failed(&resolved))?
        .ok_or_else(|| {
            ApiError::not_found(
                "CALENDAR_COMMAND_NOT_FOUND",
                format!("Calendar command not found: {}", query.command_id),
            )
        })?;
    Ok(Json(to_json(state_value)))
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

/// Listing Media's property rail: a page of properties with their photo counts, and the selected one.
async fn listing_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<OppsQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let property = state.services().property();
    let page_index = query.page.as_deref().and_then(|page| page.trim().parse::<i64>().ok()).unwrap_or(0).max(0);
    let request = domain::PropertyAdminPageRequest {
        search: query.search.trim().to_owned(),
        page: page_index + 1,
        page_size: OPPS_PAGE_SIZE,
    };
    let page = to_json(property.admin_page(&request, &resolved.service).await.map_err(failed(&resolved))?);
    let rows = page.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
    let row = |row: &Value| {
        json!({ "id": at(row, "id"), "name": at(row, "name"), "status": at(row, "status"),
                "slug": at(row, "slug"), "imageCount": at(row, "imageCount") })
    };
    let selected_id = query
        .selected
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
        .or_else(|| rows.first().and_then(|r| str_at(r, "id")).map(str::to_owned));
    let mut selected = selected_id.as_deref().and_then(|id| rows.iter().find(|r| str_at(r, "id") == Some(id)).cloned());
    if let (Some(id), None) = (selected_id.as_deref(), &selected) {
        // A selection off this page is read on its own; one that cannot be read is simply not selected.
        selected = property.admin_get(id, &resolved.service).await.ok().flatten().map(to_json);
    }
    Ok(Json(json!({ "listingMedia": {
        "properties": rows.iter().map(row).collect::<Vec<_>>(),
        "total": at(&page, "total"),
        "page": at(&page, "page"),
        "pageSize": at(&page, "pageSize"),
        "selectedId": selected.as_ref().map(|r| at(r, "id").clone()).or_else(|| rows.first().map(|r| at(r, "id").clone())),
        "selected": selected.as_ref().map(row),
    } })))
}

/// The role table as generic rows `{ id, cells: [role, account type, entitlement count] }`, as the relay flattened it.
fn role_rows(roles: &Value) -> Vec<Value> {
    roles
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, role)| {
            let count = at(role, "entitlementCodes").as_array().map_or(0, Vec::len);
            let text = |key: &str| str_at(role, key).unwrap_or("").to_owned();
            json!({ "id": format!("item-{index}"), "cells": [text("roleCode"), text("accountType"), count.to_string()] })
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct RowsQuery {
    #[serde(default)]
    screen: String,
}

/// The portal's generic rows: the Security settings tables (roles and authorities).
async fn rows(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<RowsQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    match query.screen.as_str() {
        "settings-roles" | "settings-authorities" => {
            let roles = state.services().security().list_role_entitlements(&resolved.service).await;
            Ok(Json(Value::Array(role_rows(&to_json(roles.map_err(failed(&resolved))?)))))
        }
        other => Err(correlate(
            ApiError::new(StatusCode::NOT_IMPLEMENTED, "ROWS_NOT_IN_RUST_YET", format!("The '{other}' rows have no Rust service yet."), false),
            &resolved,
        )),
    }
}

const CANONICAL_INTERNAL_ROLES: &[&str] = &["internal_guest", "user", "business_power_user", "owner", "root"];

fn is_uuid(text: &str) -> bool {
    uuid::Uuid::parse_str(text).is_ok_and(|id| (1..=5).contains(&id.get_version_num()))
}

/// ROOT sets one internal user's canonical role; answers the users as they now stand.
async fn security_users_put(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let (Some(user), Some(role)) = (str_at(&body, "appUserId"), str_at(&body, "roleCode")) else {
        return Err(ApiError::bad_request("ROLE_ASSIGNMENT_INVALID", "Invalid user or canonical role."));
    };
    if !is_uuid(user) || !CANONICAL_INTERNAL_ROLES.contains(&role) {
        return Err(ApiError::bad_request("ROLE_ASSIGNMENT_INVALID", "Invalid user or canonical role."));
    }
    let security = state.services().security();
    security.set_user_primary_role(user, role, &resolved.service).await.map_err(failed(&resolved))?;
    let users = security.list_security_users(&resolved.service).await.map_err(failed(&resolved))?;
    Ok(Json(json!({ "users": to_json(users) })))
}

/// ROOT grants or revokes one entitlement for one internal role; answers the role table as it now stands.
async fn role_entitlements_put(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let role = str_at(&body, "roleCode").unwrap_or("");
    let action = str_at(&body, "action").unwrap_or("");
    let granted = body.get("granted").and_then(Value::as_bool);
    let role_ok = (1..=64).contains(&role.len()) && role.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    let action_ok = (2..=101).contains(&action.len())
        && action.starts_with(|c: char| c.is_ascii_lowercase())
        && action.chars().all(|c| c.is_ascii_lowercase() || c == '.');
    let (true, true, Some(granted)) = (role_ok, action_ok, granted) else {
        return Err(ApiError::bad_request("ROLE_GRANT_INVALID", "Invalid role or action."));
    };
    let security = state.services().security();
    security.set_role_entitlement(role, action, granted, &resolved.service).await.map_err(failed(&resolved))?;
    let roles = security.list_role_entitlements(&resolved.service).await.map_err(failed(&resolved))?;
    Ok(Json(json!({ "roles": to_json(roles) })))
}

/// A photograph uploaded in chunks, one step per request (`step`: init, chunk, complete), for photos larger than one
/// request may carry. The media service stages the chunks and assembles, stores and attaches the image.
async fn property_media_chunked(
    State(state): State<ApiState>,
    headers: HeaderMap,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let bad = |code: &str, message: &str| correlate(ApiError::bad_request(code, message), &resolved);
    let mut fields = std::collections::HashMap::<String, String>::new();
    let mut chunk: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| bad("MEDIA_MULTIPART_INVALID", &format!("Invalid media upload: {error}")))?
    {
        let name = field.name().unwrap_or_default().to_owned();
        if name == "chunk" {
            let bytes = field.bytes().await.map_err(|error| bad("MEDIA_CHUNK_INVALID", &format!("Invalid media chunk: {error}")))?;
            chunk = Some(bytes.to_vec());
        } else {
            let value = field.text().await.map_err(|error| bad("MEDIA_MULTIPART_INVALID", &format!("Invalid media upload: {error}")))?;
            fields.insert(name, value);
        }
    }
    let field = |key: &str| fields.get(key).map(String::as_str).unwrap_or("");
    let property_id = field("propertyId").to_owned();
    if property_id.is_empty() {
        return Err(bad("MEDIA_PROPERTY_REQUIRED", "Property is required."));
    }
    let media = state.services().media();
    let context = &resolved.service;
    let answer = if field("step") == "init" {
        let number = |key: &str| field(key).trim().parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0);
        let filename = field("filename").to_owned();
        if filename.is_empty() {
            return Err(bad("MEDIA_FILENAME_REQUIRED", "An image filename is required."));
        }
        let (Some(byte_size), Some(chunk_count), Some(chunk_size)) = (number("byteSize"), number("chunkCount"), number("chunkSize")) else {
            return Err(bad("MEDIA_UPLOAD_SHAPE_INVALID", "The upload must declare a positive size, chunk count and chunk size."));
        };
        if byte_size > domain::MAX_MEDIA_UPLOAD_BYTES as f64 {
            return Err(bad("MEDIA_UPLOAD_TOO_LARGE", "Image is too large (max 50 MB)."));
        }
        let role = if field("role").is_empty() { "gallery" } else { field("role") }.to_owned();
        if role != "hero" && role != "gallery" {
            return Err(bad("MEDIA_ROLE_INVALID", "Invalid media role."));
        }
        let mime_type = field("mimeType").to_owned();
        if !mime_type.starts_with("image/") {
            return Err(bad("MEDIA_TYPE_UNSUPPORTED", "Only image uploads are supported."));
        }
        let upload = db::BeginMediaUpload {
            upload_id: uuid::Uuid::new_v4().to_string(),
            property_id,
            filename,
            mime_type,
            byte_size: byte_size as i64,
            chunk_count: chunk_count as i32,
            chunk_size: chunk_size as i32,
            sha256: Some(field("sha256").to_owned()).filter(|v| !v.is_empty()),
            role,
            alt_text: Some(field("altText").trim().to_owned()).filter(|v| !v.is_empty()),
        };
        to_json(media.begin_media_upload(upload, context).await.map_err(failed(&resolved))?)
    } else {
        let upload_id = field("uploadId").to_owned();
        if upload_id.is_empty() {
            return Err(bad("MEDIA_UPLOAD_ID_REQUIRED", "An upload id is required."));
        }
        match field("step") {
            "chunk" => {
                let index = field("chunkIndex").trim().parse::<i32>().ok().filter(|i| *i >= 0);
                let (Some(bytes), Some(index)) = (chunk, index) else {
                    return Err(bad("MEDIA_CHUNK_REQUIRED", "An image chunk is required."));
                };
                to_json(media.stage_media_chunk(&upload_id, index, bytes, context).await.map_err(failed(&resolved))?)
            }
            "complete" => to_json(media.complete_media_upload(&upload_id, context).await.map_err(failed(&resolved))?),
            _ => return Err(bad("MEDIA_STEP_UNKNOWN", "Unknown upload step.")),
        }
    };
    Ok(Json(answer))
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

async fn cockpit_page(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
) -> Result<Json<Value>, ApiError> {
    let cockpit = state
        .services()
        .cockpit()
        .snapshot(&resolved.service)
        .await
        .map_err(failed(resolved))?;
    Ok(Json(json!({ "cockpit": cockpit })))
}

/// The Cockpit: KPIs, tasks, the featured deal, the pipeline and recent interactions, as `{ cockpit }`.
async fn cockpit(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    cockpit_page(&state, &resolved).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CockpitAction {
    action: Option<String>,
    task_id: Option<String>,
}

/// The Cockpit's one command, completing a task; answers the Cockpit as it now stands.
async fn cockpit_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CockpitAction>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if input.action.as_deref() != Some("completeTask") {
        return Err(ApiError::bad_request(
            "COCKPIT_ACTION_UNSUPPORTED",
            "Unsupported Cockpit action.",
        ));
    }
    let task = input
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let Some(task) = task else {
        return Err(ApiError::bad_request(
            "COCKPIT_TASK_REQUIRED",
            "taskId is required.",
        ));
    };
    state
        .services()
        .task()
        .complete(task, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    cockpit_page(&state, &resolved).await
}

const SUPPORT_SCREENS: &[&str] = &["system-health", "db-test", "whatsapp-meta", "security", "settings-users"];

/// One SUPPORT screen's payload. Diagnostics carry posture and counts only: never a token, secret, hash or URL.
async fn support_payload(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    screen: &str,
    scope: Option<&str>,
) -> Result<Value, ApiError> {
    let services = state.services();
    let context = &resolved.service;
    let support = services.support();
    Ok(match screen {
        "db-test" => {
            // Every client, a page at a time: the screen proves the database answers and shows what it holds.
            let clients = services.clients();
            let mut rows = Vec::new();
            let mut page = 1;
            let total = loop {
                let request = ClientDirectoryPageRequest {
                    search: String::new(), status: None, role: None, sort: "name".into(), page, page_size: 100,
                };
                let answer = to_json(clients.directory(&request, context).await.map_err(failed(resolved))?);
                let batch = answer.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
                let total = at(&answer, "total").as_i64().unwrap_or(0);
                let size = at(&answer, "pageSize").as_i64().unwrap_or(100).max(1);
                let done = batch.is_empty() || page * size >= total;
                rows.extend(batch.into_iter().map(|row| json!({
                    "id": at(&row, "id"), "displayName": at(&row, "displayName"), "role": at(&row, "role"),
                    "status": at(&row, "status"), "email": at(&row, "primaryEmail"), "phone": at(&row, "primaryPhone"),
                })));
                if done {
                    break total;
                }
                page += 1;
            };
            json!({ "dbTest": { "connected": true, "clientCount": total, "clients": rows } })
        }
        "settings-users" => {
            let users = services.security().list_security_users(context).await.map_err(failed(resolved))?;
            json!({ "securityUsers": to_json(users) })
        }
        "security" => {
            let security = services.security();
            let (status, grants) = tokio::join!(support.security_status(context), security.list_role_entitlements(context));
            let break_glass = break_glass_readiness(state, resolved).await?;
            json!({ "security": {
                "status": to_json(status.map_err(failed(resolved))?),
                "breakGlass": to_json(break_glass),
                "roleEntitlements": to_json(grants.map_err(failed(resolved))?),
            } })
        }
        "whatsapp-meta" => json!({ "whatsAppMeta": meta_phones().await }),
        _ => {
            let (health, diagnostics) = tokio::join!(support.system_health(context), support.workflow_diagnostics(context));
            let mut diagnostics = to_json(diagnostics.map_err(failed(resolved))?);
            let detail = match scope {
                Some(id) => to_json(support.workflow_detail(id, context).await.map_err(failed(resolved))?),
                None => Value::Null,
            };
            if let Some(object) = diagnostics.as_object_mut() {
                object.insert("detail".into(), detail);
            }
            json!({ "systemHealth": {
                "health": to_json(health.map_err(failed(resolved))?),
                "environment": environment_readiness(),
                "diagnostics": diagnostics,
            } })
        }
    })
}

/// Which settings this deployment has — booleans only, never a value.
fn environment_readiness() -> Value {
    let set = |key: &str| std::env::var(key).is_ok_and(|value| !value.trim().is_empty());
    let value = |key: &str| std::env::var(key).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    let production = ["APP_ENV", "VERCEL_ENV"].iter().any(|key| value(key).is_some_and(|v| v.eq_ignore_ascii_case("production")));
    let (prod_db, dev_db) = (value("DATABASE_URL_PROD"), value("DATABASE_URL_DEV"));
    let database = if production { prod_db.is_some() } else { dev_db.is_some() };
    let separated = matches!((&prod_db, &dev_db), (Some(prod), Some(dev)) if prod != dev);
    let auth_secret = set("AUTH_SECRET");
    let auth_provider = set("AUTH_GOOGLE_ID") && set("AUTH_GOOGLE_SECRET");
    let maps = if production { set("GOOGLE_MAPS_API_KEY") } else { set("GOOGLE_MAPS_DEMO_KEY") || set("GOOGLE_MAPS_API_KEY") };
    let maps_demo_absent = !production || !set("GOOGLE_MAPS_DEMO_KEY");
    let mux = if production {
        set("MUX_TOKEN_ID_PROD") && set("MUX_TOKEN_SECRET_PROD")
    } else {
        set("MUX_TOKEN_ID_DEV") && set("MUX_TOKEN_SECRET_DEV")
    };
    let signature_enabled = value("BROKER_SIGNATURE_ENABLED").is_some_and(|v| v.eq_ignore_ascii_case("true"));
    let signature = ["BROKER_SIGNATURE_APP_USER_ID", "BROKER_SIGNATURE_MEDIA_ID", "BROKER_SIGNATURE_SIGNER_NAME", "BROKER_SIGNATURE_LICENSE_NUMBER"]
        .iter()
        .all(|key| set(key));
    let all_required = if production {
        [database, auth_secret, auth_provider, maps, maps_demo_absent, mux, signature_enabled, signature].iter().all(|ok| *ok)
    } else {
        database
    };
    json!({
        "isProduction": production,
        "databaseConfigured": database,
        "databaseDevProdSeparated": separated,
        "authSecretConfigured": auth_secret,
        "authProviderConfigured": auth_provider,
        "breakGlassConfigured": set("AUTH_BREAK_GLASS_APP_USER_ID") && set("AUTH_BREAK_GLASS_SECRET_HASH"),
        "breakGlassEnabled": std::env::var("AUTH_BREAK_GLASS_ENABLED").is_ok_and(|v| v == "true"),
        "googleMapsKeyConfigured": maps,
        "googleMapsDemoKeyAbsentInProduction": maps_demo_absent,
        "muxConfigured": mux,
        "brokerSignatureConfigured": signature,
        "brokerSignatureEnabled": signature_enabled,
        "allProductionRequiredConfigured": all_required,
    })
}

/// The WhatsApp diagnostic: ONE read-only GET of this business account's phone numbers from Meta, made only when the
/// screen is opened, exactly as the TypeScript page did. It changes nothing at Meta. The token is used in a header and
/// never returned; what crosses is the account id, whether a token exists, Meta's error if any, and the phone fields.
async fn meta_phones() -> Value {
    const DEFAULT_WABA_ID: &str = "1605543247626812";
    const GRAPH_VERSION: &str = "v23.0";
    let waba_id = std::env::var("WHATSAPP_WABA_ID").ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_WABA_ID.to_owned());
    let answer = |phones: Vec<Value>, error: Option<String>, token: bool| {
        json!({ "wabaId": waba_id, "phones": phones, "error": error, "tokenConfigured": token })
    };
    let Some(token) = std::env::var("WHATSAPP_ACCESS_TOKEN").ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) else {
        return answer(Vec::new(), Some("WHATSAPP_ACCESS_TOKEN is not configured.".into()), false);
    };
    let url = format!("https://graph.facebook.com/{GRAPH_VERSION}/{}/phone_numbers", waba_id.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>());
    let response = match reqwest::Client::new().get(url).bearer_auth(token).send().await {
        Ok(response) => response,
        Err(error) => return answer(Vec::new(), Some(error.to_string()), true),
    };
    let status = response.status();
    let payload: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let message = payload.pointer("/error/message").and_then(Value::as_str).map(str::to_owned)
            .unwrap_or_else(|| format!("Meta returned HTTP {}.", status.as_u16()));
        return answer(Vec::new(), Some(message), true);
    }
    let phones = payload.get("data").and_then(Value::as_array).into_iter().flatten().map(|phone| json!({
        "id": at(phone, "id"),
        "displayPhoneNumber": at(phone, "display_phone_number"),
        "verifiedName": at(phone, "verified_name"),
        "qualityRating": at(phone, "quality_rating"),
        "codeVerificationStatus": at(phone, "code_verification_status"),
    })).collect();
    answer(phones, None, true)
}

#[derive(Debug, Deserialize)]
struct TechQuery {
    selected: Option<String>,
}

/// The TECH Cockpit: KPIs, the workbench, the selected story and its runs, the Kanban, flights and the engine.
async fn tech(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<TechQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let selected = query.selected.as_deref().map(str::trim).filter(|id| !id.is_empty());
    let snapshot = state.services().tech().snapshot(selected, &resolved.service).await.map_err(failed(&resolved))?;
    let now = chrono::Utc::now().to_rfc3339();
    Ok(Json(super::tech_page::cockpit(&to_json(snapshot), selected, &now)))
}

/// A Cockpit command (a Kanban move, the workbench, a flight): the tech service's own command, answered as it answers.
async fn tech_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<domain::TechCommandRequest>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if body.action.trim().is_empty() {
        return Err(ApiError::bad_request("TECH_COMMAND_REQUIRED", "Missing TECH Cockpit command."));
    }
    let result = state.services().tech().command(body, &resolved.service).await.map_err(failed(&resolved))?;
    Ok(Json(to_json(result)))
}
