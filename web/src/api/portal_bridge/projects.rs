//! Projects: the page, the calendar viewport and its commands, and project actions.

#[allow(unused_imports)]
use super::*;

/// The whole Projects workspace: projects, work items, documents, the properties' media, activity, calendar and the
/// display names of everything the projects point at.
pub(super) async fn projects_page(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
) -> Result<Json<Value>, ApiError> {
    let services = state.services();
    let context = &resolved.service;
    let scope = model::VaultActorScope {
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
    let (items, documents) = (
        items.map_err(failed(resolved))?,
        documents.map_err(failed(resolved))?,
    );
    let (projects, items, documents) = (to_json(projects), to_json(items), to_json(documents));
    let empty = Vec::new();
    let (projects, items) = (
        projects.as_array().unwrap_or(&empty),
        items.as_array().unwrap_or(&empty),
    );
    // Every project's links in one query, not one round trip per project.
    let project_ids: Vec<String> = projects
        .iter()
        .filter_map(|project| str_at(project, "id"))
        .map(str::to_owned)
        .collect();
    let dependencies = wbs
        .list_dependencies_for(&project_ids, context)
        .await
        .map_err(failed(resolved))?;

    let mut people = std::collections::BTreeSet::new();
    let mut properties = std::collections::BTreeSet::new();
    let mut contracts = std::collections::BTreeSet::new();
    for project in projects {
        people.extend(str_at(project, "person_id").map(str::to_owned));
        properties.extend(str_at(project, "property_id").map(str::to_owned));
        contracts.extend(str_at(project, "contract_id").map(str::to_owned));
    }
    for item in items {
        let Some(entity) = item.get("entity") else {
            continue;
        };
        let Some(id) = str_at(entity, "id").map(str::to_owned) else {
            continue;
        };
        match str_at(entity, "entity_type") {
            Some("person") => {
                people.insert(id);
            }
            Some("property") => {
                properties.insert(id);
            }
            Some("contract") => {
                contracts.insert(id);
            }
            _ => {}
        }
    }

    // Names and media are supplemental: a record that cannot be read keeps its id on screen, as the relay did.
    let mut names = serde_json::Map::new();
    for id in &people {
        if let Some(person) = services
            .person()
            .get(id, context)
            .await
            .ok()
            .flatten()
            .map(to_json)
        {
            if let Some(name) = str_at(&person, "display_name") {
                names.insert(format!("person:{id}"), json!(name));
            }
        }
    }
    let mut media = Vec::new();
    for id in &properties {
        if let Some(property) = services
            .property()
            .get(id, context)
            .await
            .ok()
            .flatten()
            .map(to_json)
        {
            if let Some(name) = str_at(&property, "display_name") {
                names.insert(format!("property:{id}"), json!(name));
            }
        }
        if let Ok(assets) = services.media().for_property(id, context).await {
            media.extend(to_json(assets).as_array().cloned().unwrap_or_default());
        }
    }
    for id in &contracts {
        if let Some(contract) = services
            .contract()
            .get(id, context)
            .await
            .ok()
            .flatten()
            .map(to_json)
        {
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
            let title = str_at(document, "title")
                .or_else(|| str_at(document, "documentTypeLabel"))
                .unwrap_or("Document");
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
                "partyPersonId": document.get("partyPersonId"),
                "signedAt": document.get("signedAt"),
                "formInstanceId": document.get("formInstanceId"),
            })
        })
        .collect();
    Ok(Json(json!({ "projects": {
        "projects": camel_keys(Value::Array(projects.clone())),
        "items": camel_keys(Value::Array(items.clone())),
        "dependencies": to_json(dependencies),
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

pub(super) async fn projects(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    projects_page(&state, &resolved).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProjectsCalendarQuery {
    pub(super) start_at: String,
    pub(super) end_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProjectsCalendarCommandQuery {
    pub(super) command_id: String,
}

pub(super) fn calendar_display_hour(key: &str, fallback: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|hour| *hour <= 23)
        .unwrap_or(fallback)
}

pub(super) fn calendar_slot_minutes() -> u32 {
    std::env::var("CALENDAR_SLOT_MINUTES")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|minutes| matches!(*minutes, 15 | 30 | 60))
        .unwrap_or(30)
}

pub(super) async fn projects_calendar(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ProjectsCalendarQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let request = model::CalendarViewportQuery {
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

pub(super) async fn projects_calendar_update(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let request: model::UpdateAppleCalendarEventRequest = serde_json::from_value(body)
        .map_err(|error| ApiError::bad_request("CALENDAR_UPDATE_INVALID", error.to_string()))?;
    let receipt = state
        .services()
        .calendar()
        .update_apple_event(&request, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(to_json(receipt)))
}

pub(super) async fn projects_calendar_command(
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
pub(super) async fn projects_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let id_of = |key: &str| {
        str_at(&body, key)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
    };
    match str_at(&body, "action") {
        Some("projectStatus") => {
            let Some(id) = id_of("projectId") else {
                return Err(ApiError::bad_request(
                    "PROJECT_ID_REQUIRED",
                    "projectId is required.",
                ));
            };
            let update: UpdateProjectBody = serde_json::from_value(
                json!({ "status": body.get("status") }),
            )
            .map_err(|error| ApiError::bad_request("PROJECT_UPDATE_INVALID", error.to_string()))?;
            apply_project_update(&state, &resolved, id, update).await?;
        }
        Some("wbsSave") => {
            let Some(id) = id_of("itemId") else {
                return Err(ApiError::bad_request(
                    "WBS_ID_REQUIRED",
                    "itemId is required.",
                ));
            };
            let mut update_body = json!({
                "title": body.get("title"),
                "notes": body.get("notes"),
                "status": body.get("status"),
                "dueAt": body.get("dueAt"),
                "owner": body.get("owner"),
            });
            for key in ["plannedStart", "plannedFinish"] {
                if let Some(value) = body.get(key) {
                    update_body[key] = value.clone();
                }
            }
            let update: UpdateWbsBody = serde_json::from_value(update_body)
                .map_err(|error| ApiError::bad_request("WBS_UPDATE_INVALID", error.to_string()))?;
            apply_wbs_update(&state, &resolved, id, update).await?;
        }
        Some("wbsDependencyAdd") | Some("wbsDependencyRemove") => {
            let (Some(project_id), Some(source_id), Some(target_id)) =
                (id_of("projectId"), id_of("sourceId"), id_of("targetId"))
            else {
                return Err(ApiError::bad_request(
                    "WBS_DEPENDENCY_INVALID",
                    "projectId, sourceId and targetId are required.",
                ));
            };
            if str_at(&body, "action") == Some("wbsDependencyAdd") {
                state
                    .services()
                    .wbs()
                    .add_dependency(
                        &model::WbsDependency {
                            project_id,
                            source_id,
                            target_id,
                            kind: "finish_to_start".into(),
                        },
                        &resolved.service,
                    )
                    .await
                    .map_err(failed(&resolved))?;
            } else {
                state
                    .services()
                    .wbs()
                    .remove_dependency(&project_id, &source_id, &target_id, &resolved.service)
                    .await
                    .map_err(failed(&resolved))?;
            }
        }
        _ => {
            return Err(ApiError::bad_request(
                "PROJECT_ACTION_UNSUPPORTED",
                "Unsupported Projects action.",
            ))
        }
    }
    projects_page(&state, &resolved).await
}
