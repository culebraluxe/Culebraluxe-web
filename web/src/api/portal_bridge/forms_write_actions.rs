//! The Forms write actions, one function each.
//!
//! SPLIT OUT OF `forms_write.rs` (2026-09-28) because that one function was 683 lines: the review called it over-long,
//! and it was — not because the logic is tangled, but because four whole handlers were stacked in one `match`. The
//! move is text-exact: each arm's body was sliced out and re-emitted here unchanged, and the arm became a call, so
//! the compiler checked every captured local. A rewrite of this path is the thing that is NOT safe: forms are the
//! contract record, `forms_write` is every write the editor makes, and the rules about form context (a new form is
//! for the seller on the contract and its catastro, never for the open form's deal) live inside these bodies.
//!
//! These are internal to the portal bridge: the router mounts `forms_write`, and the dispatcher calls these.

#[allow(unused_imports)]
use super::*;

pub(super) async fn create_form(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    body: &Value,
) -> Result<Json<Value>, ApiError> {
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
            &model::CreateFormInstanceRequest {
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

pub(super) async fn fill_client(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    body: &Value,
    form_id: Option<&str>,
) -> Result<Json<Value>, ApiError> {
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
    if current.status == model::FormInstanceStatus::Issued {
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
            &model::ClientDirectoryPageRequest {
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
        .find(|row| row.relation == model::PersonPropertyRelation::LegalAddress);
    let related_physical = property_context
        .properties
        .iter()
        .find(|row| row.relation == model::PersonPropertyRelation::PhysicalProperty);

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
            &model::BindListingFormContextRequest {
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
            &model::UpdateFormInstanceRequest {
                form_instance_id: form_id.to_owned(),
                input: model::UpdateFormInstanceInput {
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

pub(super) async fn send_signature(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    body: &Value,
    form_id: Option<&str>,
) -> Result<Json<Value>, ApiError> {
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

    let saved = save_form_values(&state, &resolved, form_id, field_values, sections).await?;

    let services = state.services();
    let signature = services.signature();

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
            let page = forms_page(&state, &resolved, Some(form_id), None, None, None).await?;
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
            &model::IssueDocumentRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                form_instance_id: form_id.to_owned(),
                actor_app_user_id: Some(resolved.acting_user.app_user_id.clone()),
                // The issuance instant, taken here at the command boundary: it is the document's deterministic
                // issuance date and the date Lisa's standing signature carries. Passing `None` is what made issuance
                // of a form whose broker line names her fail outright.
                issued_at: issued_at_now(),
            },
            &resolved.service,
        )
        .await
        .map_err(failed(&resolved))?;
    if issue.outcome != model::VaultCommandOutcome::Success {
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

    // The form's own signers, by slot and role. Each person signs only their own block (the form's fixed positions);
    // the broker signs nothing here and is copied on the completed document instead.
    let mut recipients: Vec<Value> = Vec::new();
    let mut copy_to: Vec<String> = Vec::new();
    // A template whose signature blocks name email fields sends to the parties it names, typed on the form: each one
    // signs its own block, and the broker's pre-signed block is not an envelope recipient at all.
    let library = load_form_templates(resolved)?;
    let template = library
        .version(&saved.template_id, saved.template_version)
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "FORM_TEMPLATE_NOT_FOUND",
                    format!(
                        "Template {} v{} is not available.",
                        saved.template_id, saved.template_version
                    ),
                ),
                &resolved,
            )
        })?;
    let mut absent_roles: Vec<String> = Vec::new();
    let party_blocks: Vec<_> = template
        .signature_groups
        .iter()
        .filter(|group| group.email.is_some())
        .collect();
    if !party_blocks.is_empty() {
        let lisa_role =
            model::forms_broker_signature::policy_for_form(&saved.template_id, &saved.field_values)
                .map(|policy| policy.role);
        for group in party_blocks {
            if Some(group.role.as_str()) == lisa_role {
                // Lisa's own side is signed at issuance; it is drawn on the document and nobody else signs it.
                absent_roles.push(group.role.clone());
                continue;
            }
            let value = |name: &str| {
                saved
                    .field_values
                    .get(name)
                    .map(|value| value.trim().to_owned())
                    .unwrap_or_default()
            };
            let name = group.field.as_deref().map(value).unwrap_or_default();
            let typed = group.email.as_deref().map(value).unwrap_or_default();
            if name.is_empty() && typed.is_empty() {
                // This party is not on this form (a spouse, say): its block is drawn but nobody signs it.
                absent_roles.push(group.role.clone());
                continue;
            }
            if name.is_empty() {
                return Err(correlate(
                    ApiError::bad_request(
                        "FORM_SIGNER_NAME_MISSING",
                        format!("Add the name for {} before sending.", group.label),
                    ),
                    &resolved,
                ));
            }
            if typed.is_empty() {
                return Err(correlate(
                    ApiError::bad_request(
                        "FORM_SIGNER_EMAIL_MISSING",
                        format!("Add the email for {} before sending.", group.label),
                    ),
                    &resolved,
                ));
            }
            let email = crate::security::guest::normalize_email(&typed).map_err(|error| {
                ApiError::from(error).with_correlation(resolved.service.correlation_id.clone())
            })?;
            let order = recipients.len() as i32 + 1;
            recipients.push(json!({
                "role": "signer",
                "name": name,
                "email": email,
                "signerOrder": order,
                "signingStep": 1,
                // The block's slot, as the issued document names it (`ROLE:1`): role and slot go together.
                "executionRole": group.role,
                "executionSlotId": format!("{}:1", group.role),
            }));
        }
        for signer in signers
            .iter()
            .filter(|signer| signer.role == "SELLER_BROKER")
        {
            if let Some(email) = signer
                .email
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                copy_to.push(email.to_owned());
            }
        }
    }
    let legacy_signers: &[model::FormSignerPerson] = if party_blocks_is_empty(&template) {
        &signers
    } else {
        &[]
    };
    for signer in legacy_signers {
        let Some(typed) = signer
            .email
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        // Checked before the envelope exists, so a bad address is a message on the screen, not a failed invitation.
        let email = crate::security::guest::normalize_email(typed).map_err(|error| {
            ApiError::from(error).with_correlation(resolved.service.correlation_id.clone())
        })?;
        let email = email.as_str();
        if signer.role == "SELLER_BROKER" {
            copy_to.push(email.to_owned());
            continue;
        }
        let order = recipients.len() as i32 + 1;
        recipients.push(json!({
            "role": "signer",
            "name": signer.name,
            "email": email,
            "signerOrder": order,
            "signingStep": 1,
            "executionRole": signer.slot_id.as_ref().map(|_| signer.role.clone()),
            "executionSlotId": signer.slot_id,
        }));
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

    // The envelope's subject reads "<client> <document>", e.g. "Ada Alvarez Listing Agreement". The Vault title carries
    // the issue number ("Listing Agreement v9"), which the people signing have no use for.
    let client = signers
        .iter()
        .find(|signer| signer.role != "SELLER_BROKER")
        .map(|signer| signer.name.trim().to_owned())
        .filter(|name| !name.is_empty());
    let subject = services
        .vault()
        .get_document(&issued.document_id, &resolved.service)
        .await
        .map_err(failed(&resolved))?
        .and_then(|document| document.title)
        .map(|title| without_issue_number(title.trim()))
        .filter(|title| !title.is_empty())
        .map(|title| match client {
            Some(client) => format!("{client} {title}"),
            None => title,
        });
    let party_count = recipients.len();
    let input = json!({
        "transactionDocumentId": issued.document_id,
        "recipients": recipients,
        "subject": subject,
        "message": null,
        "signingMode": "parallel",
        "expiresAt": null,
        "placement": { "kind": "template" },
        "copyTo": copy_to,
        "absentRoles": absent_roles,
    });
    let Value::Object(input) = input else {
        unreachable!("json! of an object literal is an object")
    };
    let result = state
        .service_harness()
        .execute_command(
            &services::CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                command_type: "luxesign.send".into(),
                aggregate_type: "transaction_document".into(),
                aggregate_id: Some(issued.document_id.clone()),
                requested_at: chrono::Utc::now().to_rfc3339(),
                input,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    if result.outcome != services::CommandOutcome::Success {
        let message = result
            .error
            .map(|error| error.message)
            .or(result.message)
            .unwrap_or_else(|| "Could not send the document for signature.".into());
        return Err(correlate(
            ApiError::new(
                StatusCode::CONFLICT,
                "FORM_SIGNATURE_SEND_FAILED",
                message,
                false,
            ),
            &resolved,
        ));
    }

    let page = forms_page(&state, &resolved, Some(form_id), None, None, None).await?;
    Ok(Json(json!({
        "formId": form_id,
        "forms": page,
        "message": format!(
            "Sent for signature · {party_count} {}",
            if party_count == 1 { "signer" } else { "signers" },
        ),
    })))
}

pub(super) async fn save_or_issue(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    body: &Value,
    form_id: Option<&str>,
    // Which of the two this call is: the arm handles `save` and `issue`, and the older single function read this from
    // its own local. Passing it in keeps ONE derivation of the action (`forms_write` reads it from the body).
    action: &str,
) -> Result<Json<Value>, ApiError> {
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
                &model::IssueDocumentRequest {
                    command_id: uuid::Uuid::new_v4().to_string(),
                    form_instance_id: form_id.to_owned(),
                    actor_app_user_id: Some(resolved.acting_user.app_user_id.clone()),
                    // The operator's issuance instant, taken at this command boundary — never `None`, which the
                    // broker-signature policy refuses rather than dating a signature from an invented moment.
                    issued_at: issued_at_now(),
                },
                &resolved.service,
            )
            .await
            .map_err(failed(&resolved))?;

        if command.outcome != model::VaultCommandOutcome::Success {
            let status = match command.outcome {
                model::VaultCommandOutcome::NotFound => StatusCode::NOT_FOUND,
                model::VaultCommandOutcome::Conflict => StatusCode::CONFLICT,
                model::VaultCommandOutcome::Unauthorized => StatusCode::FORBIDDEN,
                model::VaultCommandOutcome::ValidationFailure
                | model::VaultCommandOutcome::PreconditionFailure => StatusCode::BAD_REQUEST,
                model::VaultCommandOutcome::Success => StatusCode::OK,
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

/// `"Listing Agreement v9"` becomes `"Listing Agreement"`: a trailing ` v<digits>` only, so a title that merely has a
/// v in it is left alone. The same rule the signing page applies to the title it shows.
fn without_issue_number(title: &str) -> String {
    match title.rsplit_once(" v") {
        Some((name, number))
            if !name.trim().is_empty()
                && !number.is_empty()
                && number.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            name.to_owned()
        }
        _ => title.to_owned(),
    }
}

#[cfg(test)]
mod issue_number_tests {
    use super::without_issue_number;

    #[test]
    fn the_issue_number_is_not_part_of_the_subject() {
        assert_eq!(
            without_issue_number("Listing Agreement v9"),
            "Listing Agreement"
        );
        assert_eq!(
            without_issue_number("Listing Agreement v12"),
            "Listing Agreement"
        );
        assert_eq!(
            without_issue_number("Listing Agreement"),
            "Listing Agreement"
        );
        assert_eq!(without_issue_number("Offer v2 draft"), "Offer v2 draft");
    }
}

/// True when the template has no signature block that names an email, so its signers come from the linked people.
fn party_blocks_is_empty(template: &model::forms_template::TemplateDefinition) -> bool {
    template
        .signature_groups
        .iter()
        .all(|group| group.email.is_none())
}
