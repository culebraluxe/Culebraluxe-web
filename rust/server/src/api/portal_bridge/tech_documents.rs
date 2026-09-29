//! Tech, property films, project documents and signing, finding sellers and properties, and property media commands.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct TechQuery {
    pub(super) selected: Option<String>,
}

/// The TECH Cockpit: KPIs, the workbench, the selected story and its runs, the Kanban, flights and the engine.
pub(super) async fn tech(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<TechQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let selected = query
        .selected
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let snapshot = state
        .services()
        .tech()
        .snapshot(selected, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    let now = chrono::Utc::now().to_rfc3339();
    Ok(Json(super::super::tech_page::cockpit(
        &to_json(snapshot),
        selected,
        &now,
    )))
}

/// A Cockpit command (a Kanban move, the workbench, a flight): the tech service's own command, answered as it answers.
pub(super) async fn tech_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<domain::TechCommandRequest>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if body.action.trim().is_empty() {
        return Err(ApiError::bad_request(
            "TECH_COMMAND_REQUIRED",
            "Missing TECH Cockpit command.",
        ));
    }
    let result = state
        .services()
        .tech()
        .command(body, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(to_json(result)))
}

/// Make a photograph the property's hero after upload.
/// A Mux direct upload for a property film: the browser sends the video straight to Mux, in pieces, never through
/// this server. The upload is allowed from the page's own origin only.
pub(super) async fn property_video_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let origin = headers
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let mux = super::super::routes::mux_video().map_err(|error| correlate(error, &resolved))?;
    let session = state
        .services()
        .media()
        .create_property_video_upload(&mux, &origin, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(
        json!({ "ok": true, "uploadId": session.upload_id, "uploadUrl": session.upload_url }),
    ))
}

/// Where a Mux upload stands (`waiting`, `preparing`, …); once Mux has it ready, the film is attached to the property.
pub(super) async fn property_video_finalize(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let (Some(property_id), Some(upload_id)) =
        (str_at(&body, "propertyId"), str_at(&body, "uploadId"))
    else {
        return Err(correlate(
            ApiError::bad_request(
                "VIDEO_UPLOAD_REQUIRED",
                "propertyId and uploadId are required.",
            ),
            &resolved,
        ));
    };
    let role = str_at(&body, "role").unwrap_or("video").to_owned();
    let caption = str_at(&body, "caption")
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_owned);
    let mux = super::super::routes::mux_video().map_err(|error| correlate(error, &resolved))?;
    let result = state
        .services()
        .media()
        .finalize_property_video_upload(
            &mux,
            property_id,
            upload_id,
            &role,
            caption,
            &resolved.service,
        )
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(json!({
        "ok": true,
        "status": result.status,
        "attached": result.attached,
        "mediaId": result.media_id,
        "muxPlaybackId": result.mux_playback_id,
    })))
}

#[derive(Debug, Deserialize)]
pub(super) struct DocumentFileQuery {
    /// `signed` (the executed copy), `audit` (the signing audit trail); otherwise the issued PDF.
    pub(super) artifact: Option<String>,
}

/// A Vault document's PDF, opened from the portal: the issued copy, the signed copy, or the audit trail.
pub(super) async fn portal_document_file(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Query(query): Query<DocumentFileQuery>,
) -> Result<axum::response::Response, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let missing = || {
        correlate(
            ApiError::not_found(
                "VAULT_DOCUMENT_NOT_FOUND",
                "That document has no such file.",
            ),
            &resolved,
        )
    };
    let vault = state.services().vault();
    let document = vault
        .get_document(&id, &resolved.service)
        .await
        .map_err(failed(&resolved))?
        .ok_or_else(missing)?;
    let media_id = match query.artifact.as_deref() {
        Some("signed") => document.signed_artifact.map(|artifact| artifact.media_id),
        Some("audit") => document.signed_audit_media_id,
        _ => document.media_id,
    }
    .ok_or_else(missing)?;
    let bytes = vault
        .media_bytes(&media_id, &resolved.service)
        .await
        .map_err(failed(&resolved))?
        .ok_or_else(missing)?;
    super::super::routes::vault_document_response(bytes, false)
}

/// RECORD A SIGNED COPY — the signing done outside the system (a PDF signed by email, until BoldSign is on). The
/// dual-signed PDF is stored as the document's executed copy with its signing date, through the Vault's own steps
/// (issued -> sent -> signed, the history it really had), and the project's "Listing Contract Signed" step is done.
pub(super) async fn project_document_signed(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let bad =
        |code: &str, message: &str| correlate(ApiError::bad_request(code, message), &resolved);
    let (mut file, mut filename, mut signed_on, mut project_id) =
        (None::<Vec<u8>>, String::new(), String::new(), String::new());
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| bad("SIGNED_COPY_INVALID", "The upload could not be read."))?
    {
        match field.name().unwrap_or_default() {
            "file" => {
                filename = field.file_name().unwrap_or("signed.pdf").to_owned();
                file = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| bad("SIGNED_COPY_INVALID", "The PDF could not be read."))?
                        .to_vec(),
                );
            }
            "signedAt" => signed_on = field.text().await.unwrap_or_default().trim().to_owned(),
            "projectId" => project_id = field.text().await.unwrap_or_default().trim().to_owned(),
            _ => {}
        }
    }
    let Some(bytes) = file.filter(|bytes| bytes.starts_with(b"%PDF-")) else {
        return Err(bad(
            "SIGNED_COPY_NOT_PDF",
            "Choose the signed contract as a PDF.",
        ));
    };
    let Ok(signed_day) = chrono::NaiveDate::parse_from_str(&signed_on, "%Y-%m-%d") else {
        return Err(bad("SIGNED_DATE_REQUIRED", "Give the date it was signed."));
    };
    let signed_at = format!("{signed_day}T12:00:00Z");

    let services = state.services();
    let vault = services.vault();
    let document = vault
        .get_document(&id, &resolved.service)
        .await
        .map_err(failed(&resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("VAULT_DOCUMENT_NOT_FOUND", "That document was not found."),
                &resolved,
            )
        })?;
    if !matches!(
        document.state,
        domain::TransactionDocumentState::Ready | domain::TransactionDocumentState::Sent
    ) {
        return Err(bad(
            "SIGNED_COPY_STATE",
            &format!(
                "This document is {} — only an issued one can be recorded as signed.",
                document.state.as_str()
            ),
        ));
    }

    let (media_id, ..) = services
        .media()
        .upload_standalone(&filename, "application/pdf", bytes, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    let transition = |to: domain::TransactionDocumentState,
                      signed_artifact: Option<domain::SignedArtifactRef>| {
        domain::TransitionTransactionDocumentRequest {
            command_id: uuid::Uuid::new_v4().to_string(),
            document_id: id.clone(),
            to,
            signed_artifact,
            actor_app_user_id: resolved.service.actor.id.clone(),
        }
    };
    if document.state == domain::TransactionDocumentState::Ready {
        vault
            .transition_state(
                &transition(domain::TransactionDocumentState::Sent, None),
                &resolved.service,
            )
            .await
            .map_err(failed(&resolved))?;
    }
    vault
        .transition_state(
            &transition(
                domain::TransactionDocumentState::Signed,
                Some(domain::SignedArtifactRef {
                    media_id,
                    signed_at,
                }),
            ),
            &resolved.service,
        )
        .await
        .map_err(failed(&resolved))?;

    let step_done = mark_signing_step(&state, &resolved, &project_id, signed_day).await?;
    Ok(Json(
        json!({ "ok": true, "signed": true, "signedAt": signed_day.to_string(), "stepDone": step_done }),
    ))
}

/// The words of a name, lowercased and sorted, titles dropped: "LAMKEN WAYNE" and "Wayne Lamken" are one key.
pub(super) fn name_key(name: &str) -> String {
    let mut words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .filter(|word| !matches!(word.as_str(), "dr" | "mr" | "mrs" | "ms"))
        .collect();
    words.sort();
    words.join(" ")
}

/// The person a contract names as seller: the one person with that name (any word order), or a new person made
/// from it. Two people with that name is a question for a person, not a guess.
pub(super) async fn seller_person(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    seller: &str,
) -> Result<String, ApiError> {
    let people = state.services().person();
    let key = name_key(seller);
    let request = domain::SearchPeopleRequest {
        query: seller.to_owned(),
        limit: Some(50),
    };
    let found: Vec<_> = people
        .search(&request, &resolved.service)
        .await
        .map_err(failed(resolved))?
        .into_iter()
        .filter(|person| name_key(&person.display_name) == key)
        .collect();
    match found.as_slice() {
        [one] => Ok(one.id.clone()),
        [] => Ok(people
            .create_seller(seller, &resolved.service)
            .await
            .map_err(failed(resolved))?
            .id),
        _ => Err(correlate(
            ApiError::bad_request(
                "FORM_SELLER_AMBIGUOUS",
                format!(
                    "{} people are named {seller} — merge them on Records → Person first.",
                    found.len()
                ),
            ),
            resolved,
        )),
    }
}

/// The one live property carrying this catastro number, if there is exactly one.
pub(super) async fn property_by_catastro(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    catastro: &str,
) -> Result<Option<String>, ApiError> {
    let digits: String = catastro.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return Ok(None);
    }
    let page = state
        .services()
        .property()
        .admin_page(
            &domain::PropertyAdminPageRequest {
                search: catastro.to_owned(),
                page: 1,
                page_size: 20,
            },
            &resolved.service,
        )
        .await
        .map_err(failed(resolved))?;
    // The search matches catastro numbers; each hit is confirmed on its full record.
    let properties = state.services().property();
    let mut matches = Vec::new();
    for row in page.rows.iter().filter(|row| !row.archived) {
        if let Some(record) = properties
            .admin_get(&row.id, &resolved.service)
            .await
            .map_err(failed(resolved))?
        {
            let stored: String = to_json(&record)
                .get("catastroNumber")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .filter(char::is_ascii_digit)
                .collect();
            if stored == digits {
                matches.push(row.id.clone());
            }
        }
    }
    Ok((matches.len() == 1).then(|| matches[0].clone()))
}

/// The project's "Listing Contract Signed" step, done on the day the contract was signed — the same for a signed copy
/// recorded and for a signing whose copy is still to come. `false` when the project has no such step.
pub(super) async fn mark_signing_step(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    project_id: &str,
    signed_day: chrono::NaiveDate,
) -> Result<bool, ApiError> {
    if project_id.is_empty() {
        return Ok(false);
    }
    let wbs = state.services().wbs();
    let items = wbs
        .list_project_items(&resolved.service)
        .await
        .map_err(failed(resolved))?;
    let Some(step) = items.into_iter().find(|item| {
        item.project_id.as_deref() == Some(project_id) && item.title == "Listing Contract Signed"
    }) else {
        return Ok(false);
    };
    let day = signed_day.to_string();
    let request = domain::SaveWbsItemRequest {
        create: domain::CreateWbsItemRequest {
            id: step.id,
            title: step.title,
            notes: Some(step.notes),
            category: step.category,
            project_id: step.project_id,
            parent_id: step.parent_id,
            due_at: step.due_at,
            planned_start: Some(day.clone()),
            planned_finish: Some(day),
            owner: step.owner,
            order: step.order,
            entity: step.entity,
        },
        status: Some(domain::WbsStatus::Done),
    };
    wbs.save(&request, &resolved.service)
        .await
        .map_err(failed(resolved))?;
    Ok(true)
}

/// SIGNED, COPY TO COME — the contract is known to be signed but its PDF has not arrived. The Vault only calls a
/// document signed with the signed copy in hand, so the document is recorded as sent (it went out), and the project's
/// signing step is done on the signing date. Recording the copy later makes the document signed.
pub(super) async fn project_document_signed_copy_to_come(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let bad =
        |code: &str, message: &str| correlate(ApiError::bad_request(code, message), &resolved);
    let Ok(signed_day) = chrono::NaiveDate::parse_from_str(
        str_at(&body, "signedAt").unwrap_or_default().trim(),
        "%Y-%m-%d",
    ) else {
        return Err(bad("SIGNED_DATE_REQUIRED", "Give the date it was signed."));
    };
    let project_id = str_at(&body, "projectId")
        .unwrap_or_default()
        .trim()
        .to_owned();
    let vault = state.services().vault();
    let document = vault
        .get_document(&id, &resolved.service)
        .await
        .map_err(failed(&resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("VAULT_DOCUMENT_NOT_FOUND", "That document was not found."),
                &resolved,
            )
        })?;
    match document.state {
        domain::TransactionDocumentState::Ready => {
            vault
                .transition_state(
                    &domain::TransitionTransactionDocumentRequest {
                        command_id: uuid::Uuid::new_v4().to_string(),
                        document_id: id.clone(),
                        to: domain::TransactionDocumentState::Sent,
                        signed_artifact: None,
                        actor_app_user_id: resolved.service.actor.id.clone(),
                    },
                    &resolved.service,
                )
                .await
                .map_err(failed(&resolved))?;
        }
        domain::TransactionDocumentState::Sent => {}
        other => {
            return Err(bad(
                "SIGNED_COPY_STATE",
                &format!(
                    "This document is {} — only an issued one can be marked signed.",
                    other.as_str()
                ),
            ));
        }
    }
    let step_done = mark_signing_step(&state, &resolved, &project_id, signed_day).await?;
    Ok(Json(
        json!({ "ok": true, "signed": false, "copyToCome": true, "signedAt": signed_day.to_string(), "stepDone": step_done }),
    ))
}

/// FIND by catastro on the Records screen: the other record for that parcel is merged into the open one.
pub(super) async fn property_merge_parcel(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let (Some(property_id), Some(catastro)) =
        (str_at(&body, "propertyId"), str_at(&body, "catastro"))
    else {
        return Err(correlate(
            ApiError::bad_request(
                "PROPERTY_MERGE_INVALID",
                "propertyId and catastro are required.",
            ),
            &resolved,
        ));
    };
    let service = state.services().property();
    let merged = service
        .merge_parcel_record(property_id, catastro, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    if merged.is_some() {
        service.warm_read_cache().await.map_err(failed(&resolved))?;
    }
    Ok(Json(
        json!({ "ok": true, "merged": merged.is_some(), "mergedName": merged }),
    ))
}

/// Takes a photograph off a property (and deletes it, with its copies, unless another property shows it).
pub(super) async fn property_media_remove(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let (Some(property_id), Some(media_id)) =
        (str_at(&body, "propertyId"), str_at(&body, "mediaId"))
    else {
        return Err(ApiError::bad_request(
            "MEDIA_REMOVE_INVALID",
            "propertyId and mediaId are required.",
        ));
    };
    state
        .services()
        .media()
        .remove_property_media(property_id, media_id, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(json!({ "ok": true })))
}

pub(super) async fn property_media_hero(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let (Some(property_id), Some(media_id)) =
        (str_at(&body, "propertyId"), str_at(&body, "mediaId"))
    else {
        return Err(ApiError::bad_request(
            "MEDIA_HERO_INVALID",
            "propertyId and mediaId are required.",
        ));
    };
    state
        .services()
        .media()
        .set_property_hero(property_id, media_id, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(json!({ "ok": true })))
}
