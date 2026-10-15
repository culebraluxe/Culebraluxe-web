//! The new-form write action, moved out of `forms_write_actions.rs` (2026-10-14).
//!
//! The move is text-exact apart from `pub(crate)` — the smallest visibility that lets the parent re-export it as
//! `pub(super)` under the same name — so the arm that calls it is unchanged. The rules about form context still
//! live in the body that enforces them.

#[allow(unused_imports)]
use super::*;

pub(crate) async fn create_form(
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
