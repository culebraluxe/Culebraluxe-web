//! Deals, contracts, forms, communications, activity, issues and relationship evidence.

#[allow(unused_imports)]
use super::*;

pub(super) async fn deals(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<model::DealPortfolioSnapshot>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().deal_portal();
    let value = service
        .portfolio(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn create_deal(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<model::CreateDealRequest>,
) -> Result<Json<ApiSuccess<model::CreateDealResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().deal_portal();
    let value = service
        .create(&body, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn deal_workspace(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<model::DealWorkspaceSnapshot>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().deal_portal();
    let value = service
        .workspace(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn deal_workspace_command(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<model::DealWorkspaceCommand>,
) -> Result<Json<ApiSuccess<model::DealWorkspaceCommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().deal_portal();
    let value = service
        .command(&id, &body, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn contracts(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::ContractSummary>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().contract();
    let value = service
        .list(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn contract(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<model::Contract>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().contract();
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

pub(super) async fn contracts_for_process_instance(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<model::ContractSummary>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().contract();
    let value = service
        .list_for_process_instance(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn forms(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::FormInstanceListItem>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().forms();
    let value = service
        .list_instances(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn form(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<model::FormInstance>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().forms();
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

pub(super) async fn create_form(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateFormBody>,
) -> Result<Json<ApiSuccess<model::FormInstance>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().forms();
    let deal_id = body.deal_id.filter(|value| !value.trim().is_empty());
    let request = model::CreateFormInstanceRequest {
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

pub(super) async fn update_form(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateFormBody>,
) -> Result<Json<ApiSuccess<model::FormInstance>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let status = match body.status.as_deref() {
        Some(value) => Some(
            model::FormInstanceStatus::try_from(value).map_err(|message| {
                correlate(
                    ApiError::from(CoreServiceError::business("FORM_STATUS_INVALID", message)),
                    &resolved,
                )
            })?,
        ),
        None => None,
    };
    let service = state.services().forms();
    let value = service
        .update_instance(
            &model::UpdateFormInstanceRequest {
                form_instance_id: id.clone(),
                input: model::UpdateFormInstanceInput {
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

pub(super) async fn form_deal_facts(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(deal_id): Path<String>,
) -> Result<Json<ApiSuccess<Option<model::DealFormFacts>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().forms();
    let value = service
        .deal_facts(&deal_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn form_signers(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<model::FormSignerPerson>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().forms();
    let value = service
        .list_signer_people(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn form_issued_document(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Option<model::IssuedDocumentForFormInstance>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().vault();
    let value = service
        .issued_for_form_instance(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn comms_panel(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    Query(query): Query<CommsPanelQuery>,
) -> Result<Json<ApiSuccess<model::CommsPanel>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().comms();
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

pub(super) async fn comms_timeline(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    Query(query): Query<CommsTimelineQuery>,
) -> Result<Json<ApiSuccess<model::CommsTimeline>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().comms();
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

pub(super) async fn activity(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<ApiSuccess<Vec<model::ActivityFeedEntry>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().comms();
    let value = service
        .activity(query.limit.unwrap_or(200), &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// The Accounting dashboard: every figure a projection over the two canonical tables.
pub(super) async fn issues(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<IssuesQuery>,
) -> Result<Json<ApiSuccess<model::IssuesPage>>, ApiError> {
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

pub(super) async fn relationship_evidence_review(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<RelationshipReviewQuery>,
) -> Result<Json<ApiSuccess<model::RelationshipEvidenceReview>>, ApiError> {
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

pub(super) async fn relationship_evidence_action(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<RelationshipActionBody>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().relationship_evidence();

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
