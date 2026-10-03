//! The Clients screen's read (directory, the selected client hydrated) and the bridge's small shared helpers.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct ClientsBridgeQuery {
    #[serde(default = "default_client_screen")]
    pub(super) screen: String,
    pub(super) scope: Option<String>,
    pub(super) selected: Option<String>,
    #[serde(default)]
    pub(super) search: String,
    pub(super) page: Option<i64>,
}

pub(super) fn default_client_screen() -> String {
    "clients".into()
}

#[derive(Debug, Serialize)]
pub(super) struct ClientsBridgeResponse {
    pub(super) clients: ClientsBridgePage,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClientsBridgePage {
    pub(super) rows: Vec<ClientBridgeSummary>,
    pub(super) total: i64,
    pub(super) page: i64,
    pub(super) page_size: i64,
    pub(super) selected_id: Option<String>,
    pub(super) selected: Option<ClientDetail>,
    pub(super) comms: Option<CommsPanel>,
    pub(super) properties: Vec<ClientBridgeProperty>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClientBridgeSummary {
    pub(super) id: String,
    pub(super) display_name: String,
    pub(super) name_resolved: bool,
    pub(super) role: String,
    pub(super) status: String,
    pub(super) primary_email: Option<String>,
    pub(super) primary_phone: Option<String>,
    pub(super) observed_count: i64,
    pub(super) two_way: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClientBridgeProperty {
    pub(super) id: String,
    pub(super) display_name: String,
    pub(super) relation: String,
    pub(super) relation_status: Option<String>,
    pub(super) address: String,
}

pub(super) struct HydratedClient {
    pub(super) selected: Option<ClientDetail>,
    pub(super) comms: CommsPanel,
    pub(super) properties: Vec<ClientBridgeProperty>,
}

pub(super) async fn clients(
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

pub(super) async fn hydrate_client(
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

pub(super) fn build_clients_response(
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

pub(super) fn map_properties(context: PersonPropertyContext) -> Vec<ClientBridgeProperty> {
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

pub(super) fn correlate(error: ApiError, resolved: &ResolvedRequestContext) -> ApiError {
    error.with_correlation(resolved.service.correlation_id.clone())
}

/// A service answer as JSON, or the service's failure tagged with this request's correlation id.
pub(super) fn to_json<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

pub(super) fn failed(
    resolved: &ResolvedRequestContext,
) -> impl Fn(CoreServiceError) -> ApiError + '_ {
    move |error| correlate(ApiError::from(error), resolved)
}
