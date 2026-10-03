//! Who am I, the cockpit, workflows, the flight recorder, projects, tasks and the WBS.

#[allow(unused_imports)]
use super::*;

pub(super) async fn whoami(
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

pub(super) async fn cockpit(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<model::CockpitSnapshot>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().cockpit();
    let context = resolved.service.clone();
    let value = execute_registered(
        &state,
        "cockpit",
        "cockpit.snapshot",
        json!({}),
        async move { service.snapshot(&context).await },
    )
    .await
    .map_err(|error| correlate(error, &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn workflows(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<model::WorkflowPortalList>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().workflow_portal();
    let context = resolved.service.clone();
    let value = execute_registered(
        &state,
        "workflow-portal",
        "workflow.list",
        json!({}),
        async move { service.list(&context).await },
    )
    .await
    .map_err(|error| correlate(error, &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn workflow_detail(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<model::WorkflowPortalDetail>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().workflow_portal();
    let context = resolved.service.clone();
    let work_id = id.clone();
    let value = execute_registered(
        &state,
        "workflow-portal",
        "workflow.detail",
        json!({ "id": id }),
        async move { service.detail(&work_id, &context).await },
    )
    .await
    .map_err(|error| correlate(error, &resolved))?
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

pub(super) async fn flight_recorder(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<model::FlightRecorderTransaction>>, ApiError> {
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
    let service = state.services().flight_recorder();
    let context = resolved.service.clone();
    let work_id = id.clone();
    let value = execute_registered(
        &state,
        "flight-recorder",
        "flight-recorder.transaction",
        json!({ "id": id }),
        async move { service.transaction(&work_id, &context).await },
    )
    .await
    .map_err(|error| correlate(error, &resolved))?
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

pub(super) async fn projects(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::Project>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().project();
    let mut service = service.lock().await;
    let value = service
        .list(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn create_project(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateProjectBody>,
) -> Result<Json<ApiSuccess<model::Project>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut areas = Vec::with_capacity(body.areas.as_ref().map(Vec::len).unwrap_or(0));
    for value in body.areas.unwrap_or_default() {
        areas.push(
            model::WbsCategory::try_from(value.as_str()).map_err(|error| {
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
        .lock()
        .await
        .create(
            &model::CreateProjectRequest {
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

pub(super) async fn project(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<model::Project>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().project();
    let mut service = service.lock().await;
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

pub(super) async fn update_project(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateProjectBody>,
) -> Result<Json<ApiSuccess<model::Project>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = apply_project_update(&state, &resolved, id, body).await?;
    Ok(success(value, &resolved))
}

/// Shared by `/v1` and the portal page (`portal_bridge`), so the merge rules live once.
pub(in super::super) async fn apply_project_update(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    id: String,
    body: UpdateProjectBody,
) -> Result<model::Project, ApiError> {
    let status = match body.status.as_deref() {
        Some(value) => Some(model::ProjectStatus::try_from(value).map_err(|error| {
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
    let service = state.services().project();
    let mut service = service.lock().await;
    let value = service
        .update(
            &model::UpdateProjectRequest {
                id,
                name: body.name,
                owner: body.owner,
                status,
                description: body.description,
                areas: match body.areas {
                    Some(values) => {
                        let mut parsed = Vec::with_capacity(values.len());
                        for value in values {
                            parsed.push(model::WbsCategory::try_from(value.as_str()).map_err(
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
        .map_err(|error| correlate(ApiError::from(error), resolved))?;
    Ok(value)
}

pub(super) async fn complete_task(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<model::TaskCompletion>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().task();
    let context = resolved.service.clone();
    let value = execute_registered(
        &state,
        "task",
        "task.complete",
        json!({ "id": id }),
        async move { service.complete(&id, &context).await },
    )
    .await
    .map_err(|error| correlate(error, &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn create_wbs_item(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateWbsBody>,
) -> Result<Json<ApiSuccess<model::WbsItem>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let category = model::WbsCategory::try_from(body.category.as_str()).map_err(|error| {
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
            &model::CreateWbsItemRequest {
                id: body.id,
                title: body.title,
                notes: body.notes,
                category,
                project_id: body.project_id,
                parent_id: body.parent_id,
                due_at: body.due_at,
                planned_start: body.planned_start,
                planned_finish: body.planned_finish,
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

pub(super) async fn wbs_project_items(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::WbsItem>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().wbs();
    let value = service
        .list_project_items(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn wbs_dependencies(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<model::WbsDependency>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state
        .services()
        .wbs()
        .list_dependencies(&project_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn add_wbs_dependency(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(body): Json<AddWbsDependencyBody>,
) -> Result<Json<ApiSuccess<model::WbsDependency>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state
        .services()
        .wbs()
        .add_dependency(
            &model::WbsDependency {
                project_id,
                source_id: body.source_id,
                target_id: body.target_id,
                kind: body.kind,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn remove_wbs_dependency(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((project_id, source_id, target_id)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    state
        .services()
        .wbs()
        .remove_dependency(&project_id, &source_id, &target_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(json!({"removed": true}), &resolved))
}

pub(super) async fn wbs_item(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<model::WbsItem>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().wbs();
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

pub(super) async fn update_wbs_item(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateWbsBody>,
) -> Result<Json<ApiSuccess<model::WbsItem>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = apply_wbs_update(&state, &resolved, id, body).await?;
    Ok(success(value, &resolved))
}

/// Shared by `/v1` and the portal page (`portal_bridge`), so the merge rules live once.
pub(in super::super) async fn apply_wbs_update(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    id: String,
    body: UpdateWbsBody,
) -> Result<model::WbsItem, ApiError> {
    let service = state.services().wbs();
    let current = service
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("WBS_NOT_FOUND", format!("WBS item not found: {id}")),
                &resolved,
            )
        })?;
    let status = match body.status.as_deref() {
        Some(value) => Some(model::WbsStatus::try_from(value).map_err(|message| {
            correlate(
                ApiError::from(CoreServiceError::business("WBS_STATUS_INVALID", message)),
                &resolved,
            )
        })?),
        None => None,
    };
    let value = service
        .save(
            &model::SaveWbsItemRequest {
                create: model::CreateWbsItemRequest {
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
                    planned_start: body.planned_start.unwrap_or(current.planned_start),
                    planned_finish: body.planned_finish.unwrap_or(current.planned_finish),
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
        .map_err(|error| correlate(ApiError::from(error), resolved))?;
    Ok(value)
}
