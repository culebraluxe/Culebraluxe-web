//! Accounting, routing project work, Apple reminders and calendar, and the Vault documents.

#[allow(unused_imports)]
use super::*;

pub(super) async fn accounting_dashboard(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<domain::AccountingDashboard>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().accounting();
    let value = service
        .dashboard(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn accounting_receivables(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::Receivable>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().accounting();
    let value = service
        .receivables(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn accounting_expenses(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::Expense>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().accounting();
    let value = service
        .expenses(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// Every posted expense by category, for the Expenses screen's breakdown.
pub(super) async fn accounting_expense_categories(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::CategoryShare>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().accounting();
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
pub(super) async fn accounting_pnl(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<AccountingPnlQuery>,
) -> Result<Json<ApiSuccess<domain::PnlStatement>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().accounting();
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
pub(super) async fn create_expense(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateExpenseBody>,
) -> Result<Json<ApiSuccess<AccountingId>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().accounting();
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
pub(super) async fn create_receivable(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateReceivableBody>,
) -> Result<Json<ApiSuccess<AccountingId>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().accounting();
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
pub(super) async fn mark_receivable_paid(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<MarkReceivablePaidBody>,
) -> Result<Json<ApiSuccess<domain::MarkReceivablePaidOutcome>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().accounting();
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
pub(super) struct AccountingId {
    pub(super) id: String,
}

pub(super) async fn route_project_work(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<RouteProjectWorkBody>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let wbs = state.services().wbs();
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
                    planned_start: current.planned_start,
                    planned_finish: current.planned_finish,
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
            let calendar = state.services().calendar();
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

pub(super) async fn queue_apple_reminder(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<AppleReminderBody>,
) -> Result<Json<ApiSuccess<domain::AppleReminderCommandReceipt>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().wbs();
    let value = service
        .queue_apple_reminder(&id, body.alert.unwrap_or(false), &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn create_apple_calendar_event(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<domain::CreateAppleCalendarEventRequest>,
) -> Result<Json<ApiSuccess<domain::CalendarCommandReceipt>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().calendar();
    let value = service
        .create_apple_event(&request, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn calendar(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::CalendarEvent>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().calendar();
    let value = service
        .list(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn vault_documents(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::IssuedDocumentListItem>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let scope = VaultActorScope {
        account_type: resolved.acting_user.account_type.clone(),
        person_id: resolved.acting_user.person_id.clone(),
    };
    let service = state.services().vault();
    let value = service
        .list_issued_documents(Some(&scope), &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn vault_document(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::TransactionDocument>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().vault();
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

pub(super) async fn vault_documents_by_deal(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<domain::TransactionDocument>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().vault();
    let value = service
        .list_by_deal(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn vault_form_contract(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Option<String>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().vault();
    let value = service
        .form_contract_id(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn vault_bind_form_contract(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<BindVaultFormContractBody>,
) -> Result<Json<ApiSuccess<()>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().vault();
    service
        .bind_form_to_contract(&id, &body.contract_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success((), &resolved))
}

pub(super) async fn vault_prior_contract_document(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((contract_id, template_id)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<Option<domain::ContractIssuedLineage>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().vault();
    let value = service
        .prior_contract_document(&contract_id, &template_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

#[derive(Debug, Deserialize)]
pub(super) struct VaultDownloadQuery {
    pub(super) download: Option<String>,
}

pub(in super::super) fn vault_document_response(
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
