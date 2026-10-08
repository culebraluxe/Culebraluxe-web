//! Forms values: prefill from the deal and property, empty sections, saving values, the forms read, Grok fill and preview.

#[allow(unused_imports)]
use super::*;

pub(super) fn prefill_form_values(
    template: &model::forms_template::TemplateDefinition,
    facts: Option<&model::DealFormFacts>,
    person: Option<&model::Person>,
    property: Option<&model::Property>,
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
                model::forms_template::TemplateFieldType::Date
            )
        {
            value = date_default(&field.name);
        }
        if let Some(fixed) = field.fixed.as_deref() {
            value = fixed.to_owned();
        }
        values.insert(field.name.clone(), value);
    }
    values
}

pub(super) fn empty_form_sections(
    template: &model::forms_template::TemplateDefinition,
) -> std::collections::BTreeMap<String, String> {
    template
        .sections
        .iter()
        .map(|section| (section.name.clone(), String::new()))
        .collect()
}

pub(super) async fn save_form_values(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    form_id: &str,
    mut field_values: std::collections::BTreeMap<String, String>,
    sections: std::collections::BTreeMap<String, String>,
) -> Result<model::FormInstance, ApiError> {
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

    // A fixed field holds the template's value whatever the client sent.
    for field in &template.fields {
        if let Some(fixed) = field.fixed.as_deref() {
            field_values.insert(field.name.clone(), fixed.to_owned());
        }
    }

    let updated = forms
        .update_instance(
            &model::UpdateFormInstanceRequest {
                form_instance_id: form_id.to_owned(),
                input: model::UpdateFormInstanceInput {
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
            let desired = raw_civil_status.trim().to_owned();
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
                            &model::UpdatePersonAdminRequest {
                                person_id: person.id,
                                display_name: person.display_name,
                                civil_status: desired,
                                status: person.status,
                                company: person.company,
                                location: None,
                                email: None,
                                phone: None,
                                // A civil-status edit is not the hold: leave it as it is.
                                manual_override: None,
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

    if template.field("sellerEmail").is_some() {
        let desired = field_values
            .get("sellerEmail")
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if let (Some(person_id), Some(desired)) = (current.person_id.as_deref(), desired) {
            let known = forms
                .list_signer_people(form_id, &resolved.service)
                .await
                .map_err(failed(resolved))?
                .into_iter()
                .find(|signer| signer.person_id.as_deref() == Some(person_id))
                .and_then(|signer| signer.email);
            if known.as_deref().map(str::trim) != Some(desired.as_str()) {
                if let Some(person) = services
                    .person()
                    .get(person_id, &resolved.service)
                    .await
                    .map_err(failed(resolved))?
                {
                    let updated_person = services
                        .person()
                        .update_admin(
                            &model::UpdatePersonAdminRequest {
                                person_id: person.id,
                                display_name: person.display_name,
                                civil_status: person.civil_status,
                                status: person.status,
                                company: person.company,
                                location: None,
                                email: Some(desired),
                                phone: None,
                                manual_override: None,
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
                    &model::SetPropertyListingTypeRequest {
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

pub(super) async fn forms(
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
pub(super) struct FormsPreviewBody {
    pub(super) form_id: String,
    #[serde(default)]
    pub(super) field_values: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub(super) sections: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FormsGrokBody {
    pub(super) form_id: String,
    pub(super) form_name: String,
    pub(super) prompt: String,
    #[serde(default)]
    pub(super) details_text: String,
    #[serde(default)]
    pub(super) field_values: serde_json::Map<String, Value>,
    #[serde(default)]
    pub(super) fields: Vec<super::super::forms_grok::GrokField>,
}

/// Grok's suggestion for the open form (see `forms_grok`). Opening the form is the access check; nothing is saved.
pub(super) async fn forms_grok(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<FormsGrokBody>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if body.prompt.trim().is_empty() {
        return Err(correlate(
            ApiError::bad_request("GROK_PROMPT_REQUIRED", "Tell Grok what happened first."),
            &resolved,
        ));
    }
    state
        .services()
        .forms()
        .get_instance(body.form_id.trim(), &resolved.service)
        .await
        .map_err(failed(&resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("FORM_NOT_FOUND", "That form was not found."),
                &resolved,
            )
        })?;
    let fill = super::super::forms_grok::fill(
        &body.form_name,
        &body.fields,
        &body.field_values,
        &body.details_text,
        body.prompt.trim(),
    )
    .await
    .map_err(|failure| {
        correlate(
            ApiError::new(
                axum::http::StatusCode::BAD_GATEWAY,
                "GROK_UNAVAILABLE",
                failure.0,
                true,
            ),
            &resolved,
        )
    })?;
    Ok(Json(
        json!({ "ok": true, "fieldValues": fill.field_values, "body": fill.body, "note": fill.note }),
    ))
}

pub(super) async fn forms_preview(
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
            model::VaultRenderRequest {
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
