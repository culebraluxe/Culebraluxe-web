//! The generic portal page read, Cabinet, and Deals (read and write).

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct PageQuery {
    #[serde(default)]
    pub(super) screen: String,
    pub(super) scope: Option<String>,
    pub(super) from: Option<String>,
    pub(super) to: Option<String>,
}

/// A portal screen's page payload, in the screen's own shape (`{ activity }`, `{ workflows }`, `{ workflow }`).
pub(super) async fn page(
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
            Ok(Json(super::super::tech_page::storyboard(&to_json(snapshot))))
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
pub(super) async fn cabinet(State(state): State<ApiState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
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
pub(super) struct DealsQuery {
    pub(super) people_search: Option<String>,
    pub(super) scope: Option<String>,
    pub(super) id: Option<String>,
}

impl DealsQuery {
    pub(super) fn deal_id(&self) -> Option<String> {
        self.scope.as_deref().or(self.id.as_deref()).map(str::trim).filter(|id| !id.is_empty()).map(str::to_owned)
    }
}

/// Contracts: the deal portfolio, one deal's workspace (`scope`), or a people search for a new deal (`peopleSearch`).
pub(super) async fn deals(
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
pub(super) async fn deals_write(
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
