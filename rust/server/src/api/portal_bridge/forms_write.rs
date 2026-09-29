//! The Forms write: every form action the editor sends.

#[allow(unused_imports)]
use super::*;

pub(super) async fn forms_write(
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
            let mut person_id = person_id;
            let mut property_id = str_at(&body, "propertyId")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            // A NEW FORM IS FOR SOMEONE: the seller as named on the contract, and its property by catastro. Never an
            // inherited deal — new forms used to take the open form's deal, and every contract ended up filed under
            // one demo deal ("Sunset Point").
            if person_id.is_none() {
                if let Some(seller) = str_at(&body, "sellerName")
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                {
                    person_id = Some(seller_person(&state, &resolved, seller).await?);
                }
            }
            if property_id.is_none() {
                if let Some(catastro) = str_at(&body, "catastro")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    property_id = property_by_catastro(&state, &resolved, catastro).await?;
                }
            }
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
                let execution_role = execution_slot_id.as_ref().map(|_| signer.role.clone());
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
pub(super) fn camel_keys(value: Value) -> Value {
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

pub(super) fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}
