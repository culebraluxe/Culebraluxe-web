//! Forms reads: which templates are active, their payloads and choices, the form items, and the forms page.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FormsBridgeQuery {
    #[serde(default = "default_forms_screen")]
    pub(super) screen: String,
    pub(super) scope: Option<String>,
    pub(super) deal_id: Option<String>,
    pub(super) person_id: Option<String>,
    pub(super) property_id: Option<String>,
}

pub(super) fn default_forms_screen() -> String {
    "forms".into()
}

pub(super) fn load_form_templates(
    resolved: &ResolvedRequestContext,
) -> Result<model::forms_template::TemplateLibrary, ApiError> {
    model::forms_template::TemplateLibrary::load_default().map_err(|error| {
        correlate(
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "FORM_TEMPLATE_LOAD_FAILED",
                error.to_string(),
                false,
            ),
            resolved,
        )
    })
}

pub(super) const ACTIVE_FORM_TEMPLATE_VERSIONS: &[(&str, i32)] = &[
    ("OFFER-01", 3),
    ("PR-PNS", 4),
    ("PR-PNS-AMD", 1),
    ("LISTING-01", 5),
    ("SHOW-INFO", 1),
    ("SHOW-RPT", 2),
];

pub(super) const PORTAL_FORM_TEMPLATE_IDS: &[&str] =
    &["SHOW-RPT", "OFFER-01", "PR-PNS", "LISTING-01"];

pub(super) fn active_form_template<'a>(
    library: &'a model::forms_template::TemplateLibrary,
    id: &str,
) -> Option<&'a model::forms_template::TemplateDefinition> {
    let version = ACTIVE_FORM_TEMPLATE_VERSIONS
        .iter()
        .find_map(|(template_id, version)| (*template_id == id).then_some(*version))?;
    library.version(id, version)
}

pub(super) fn form_presentation(
    value: model::forms_template::TemplatePresentation,
) -> &'static str {
    match value {
        model::forms_template::TemplatePresentation::Agreement => "agreement",
        model::forms_template::TemplatePresentation::Letter => "letter",
        model::forms_template::TemplatePresentation::Information => "information",
        model::forms_template::TemplatePresentation::Report => "report",
    }
}

pub(super) fn form_field_type(value: model::forms_template::TemplateFieldType) -> &'static str {
    match value {
        model::forms_template::TemplateFieldType::Text => "text",
        model::forms_template::TemplateFieldType::Email => "email",
        model::forms_template::TemplateFieldType::Money => "money",
        model::forms_template::TemplateFieldType::Date => "date",
        model::forms_template::TemplateFieldType::Textarea => "textarea",
        model::forms_template::TemplateFieldType::Select => "select",
    }
}

pub(super) fn when_payload(when: Option<&model::forms_template::TemplateWhen>) -> Value {
    when.map_or(
        Value::Null,
        |when| json!({ "field": when.field, "values": when.values }),
    )
}

pub(super) fn form_template_payload(
    template: &model::forms_template::TemplateDefinition,
    library: &model::forms_template::TemplateLibrary,
) -> Value {
    let fields: Vec<Value> = template
        .fields
        .iter()
        .map(|field| {
            json!({
                "name": field.name,
                "label": field.label,
                "type": form_field_type(field.field_type),
                "required": field.required,
                "options": field.options,
                "when": when_payload(field.when.as_ref()),
                // A fixed field is the template's own fact; the screen does not show it.
                "fixed": field.fixed,
            })
        })
        .collect();
    let sections: Vec<Value> = template
        .sections
        .iter()
        .map(|section| {
            let segments: Vec<Value> = section
                .segments
                .iter()
                .map(|segment| match segment {
                    model::forms_template::TemplateSectionSegment::Text(text) => {
                        json!({ "kind": "text", "text": text })
                    }
                    model::forms_template::TemplateSectionSegment::Value(field) => {
                        json!({ "kind": "value", "field": field })
                    }
                })
                .collect();
            json!({
                "name": section.name,
                "label": section.label,
                "editable": section.editable,
                "segments": segments,
                "when": when_payload(section.when.as_ref()),
            })
        })
        .collect();
    let signature_groups: Vec<Value> = template
        .signature_groups
        .iter()
        .map(|group| {
            json!({
                "role": group.role,
                "label": group.label,
                "field": group.field,
                "initials": group.initials,
            })
        })
        .collect();
    json!({
        "id": template.id,
        "version": template.version,
        "activeVersion": active_form_template(library, &template.id).map(|item| item.version).unwrap_or(template.version),
        "displayName": template.display_name,
        "documentTypeLabel": template.document_type_label,
        "renderingTitle": template.rendering.title,
        "presentation": form_presentation(template.rendering.presentation),
        "fields": fields,
        "sections": sections,
        "signatureGroups": signature_groups,
    })
}

pub(super) fn form_template_choices(
    library: &model::forms_template::TemplateLibrary,
) -> Vec<Value> {
    PORTAL_FORM_TEMPLATE_IDS
        .iter()
        .filter_map(|id| active_form_template(library, id))
        .map(|template| {
            json!({
                "id": template.id,
                "displayName": template.display_name,
                "activeVersion": template.version,
            })
        })
        .collect()
}

pub(super) fn form_item_payload(
    item: &model::FormInstanceListItem,
    library: &model::forms_template::TemplateLibrary,
) -> Value {
    let mut value = to_json(item);
    if let Some(object) = value.as_object_mut() {
        let name = library
            .version(&item.instance.template_id, item.instance.template_version)
            .map(|template| template.display_name.clone())
            .unwrap_or_else(|| item.instance.template_id.clone());
        let active = active_form_template(library, &item.instance.template_id)
            .map(|template| template.version)
            .unwrap_or(item.instance.template_version);
        object.insert("templateName".into(), json!(name));
        object.insert("activeVersion".into(), json!(active));
    }
    value
}

pub(super) fn selected_form_payload(
    form: &model::FormInstance,
    field_values: std::collections::BTreeMap<String, String>,
    library: &model::forms_template::TemplateLibrary,
) -> Value {
    let mut value = to_json(form);
    if let Some(object) = value.as_object_mut() {
        let name = library
            .version(&form.template_id, form.template_version)
            .map(|template| template.display_name.clone())
            .unwrap_or_else(|| form.template_id.clone());
        let active = active_form_template(library, &form.template_id)
            .map(|template| template.version)
            .unwrap_or(form.template_version);
        object.insert("templateName".into(), json!(name));
        object.insert("activeVersion".into(), json!(active));
        object.insert("dealLabel".into(), Value::Null);
        object.insert("propertyLabel".into(), Value::Null);
        object.insert("clientName".into(), Value::Null);
        object.insert("fieldValues".into(), json!(field_values));
    }
    value
}

pub(super) async fn forms_page(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    record: Option<&str>,
    deal_id: Option<&str>,
    person_id: Option<&str>,
    property_id: Option<&str>,
) -> Result<Value, ApiError> {
    let services = state.services();
    let forms = services.forms();
    let library = load_form_templates(resolved)?;
    let mut items = forms
        .list_instances(&resolved.service)
        .await
        .map_err(failed(resolved))?;

    if record.is_none() {
        if let Some(deal_id) = deal_id.map(str::trim).filter(|value| !value.is_empty()) {
            items.retain(|item| item.instance.deal_id.as_deref() == Some(deal_id));
        }
        if let Some(person_id) = person_id.map(str::trim).filter(|value| !value.is_empty()) {
            items.retain(|item| item.instance.person_id.as_deref() == Some(person_id));
        }
        if let Some(property_id) = property_id.map(str::trim).filter(|value| !value.is_empty()) {
            items.retain(|item| item.instance.property_id.as_deref() == Some(property_id));
        }
    }

    let item_payloads = items
        .iter()
        .map(|item| form_item_payload(item, &library))
        .collect::<Vec<_>>();

    let Some(form_id) = record.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(json!({
            "items": item_payloads,
            "selected": Value::Null,
            "template": Value::Null,
            "issued": Value::Null,
            "signers": [],
            "signature": Value::Null,
            "templateChoices": form_template_choices(&library),
        }));
    };

    let form = forms
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
    let template = library
        .version(&form.template_id, form.template_version)
        .ok_or_else(|| {
            correlate(
                ApiError::not_found(
                    "FORM_TEMPLATE_NOT_FOUND",
                    format!(
                        "Template {} v{} is not available.",
                        form.template_id, form.template_version
                    ),
                ),
                resolved,
            )
        })?;

    let mut field_values = form.field_values.clone();
    if template.field("sellerCivilStatus").is_some() {
        if let Some(person_id) = form.person_id.as_deref() {
            if let Some(person) = services
                .person()
                .get(person_id, &resolved.service)
                .await
                .map_err(failed(resolved))?
            {
                if let Some(civil_status) =
                    person.civil_status.filter(|value| !value.trim().is_empty())
                {
                    // Person is canonical for civil status. Old form JSON must never shadow a repaired Person value.
                    field_values.insert("sellerCivilStatus".into(), civil_status);
                }
            }
        }
    }
    if form.template_id == "LISTING-01" {
        if let Some(property_id) = form.property_id.as_deref() {
            if let Some(property) = services
                .property()
                .admin_get(property_id, &resolved.service)
                .await
                .map_err(failed(resolved))?
            {
                if let Some(listing_type) = property
                    .stellar
                    .listing_type
                    .filter(|value| !value.trim().is_empty())
                {
                    // Property's Stellar listing record is canonical for listing type when it already has a value.
                    field_values.insert("listingType".into(), listing_type);
                }
            }
        }
    }

    let signers = forms
        .list_signer_people(form_id, &resolved.service)
        .await
        .map_err(failed(resolved))?;
    if form.template_id == "LISTING-01" && template.field("sellerEmail").is_some() {
        // The person's own email is canonical; the form shows it, and the signing link goes there.
        let seller_email = form.person_id.as_deref().and_then(|person_id| {
            signers
                .iter()
                .find(|signer| signer.person_id.as_deref() == Some(person_id))
                .and_then(|signer| signer.email.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        });
        if let Some(email) = seller_email {
            field_values.insert("sellerEmail".into(), email.to_owned());
        }
    }
    for field in &template.fields {
        if let Some(fixed) = field.fixed.as_deref() {
            field_values.insert(field.name.clone(), fixed.to_owned());
        }
    }
    let issued = services
        .vault()
        .issued_for_form_instance(form_id, &resolved.service)
        .await
        .map_err(failed(resolved))?;
    let signature = if let Some(document) = issued.as_ref() {
        services
            .signature()
            .active_for_document(&document.document_id, &resolved.service)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    Ok(json!({
        "items": item_payloads,
        "selected": selected_form_payload(&form, field_values, &library),
        "template": form_template_payload(template, &library),
        "issued": issued,
        "signers": signers,
        "signature": signature,
        "templateChoices": form_template_choices(&library),
    }))
}

pub(super) fn form_default(template_id: &str, field_name: &str) -> Option<&'static str> {
    match (template_id, field_name) {
        ("LISTING-01", "brokerName") => Some("Lisa Penfield"),
        ("LISTING-01", "sellerCivilStatus") => Some("Single"),
        ("LISTING-01", "commission") => Some("4%"),
        ("LISTING-01", "listingType") => Some("Exclusive Right to Sell"),
        ("PR-PNS", "sellerBrokerName") => Some("Lisa Penfield"),
        ("OFFER-01", "brokerName") => Some("Lisa Penfield"),
        ("SHOW-RPT", "agentName") => Some("Lisa Penfield"),
        _ => None,
    }
}

pub(super) fn date_default(field_name: &str) -> String {
    let today = chrono::Utc::now().date_naive();
    let days = if field_name.to_ascii_lowercase().contains("expir") {
        14
    } else if field_name.to_ascii_lowercase().contains("end") {
        90
    } else {
        0
    };
    (today + chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

pub(super) fn one_line_address_part(value: Option<&str>) -> Option<String> {
    let parts = value?
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|part| part.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join(", "))
}

pub(super) fn redundant_pr_country(country: Option<&str>, state: Option<&str>) -> bool {
    if state
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_uppercase()
        != "PR"
    {
        return false;
    }
    let country = country
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace('.', "");
    matches!(
        country.as_str(),
        "united states" | "united states of america" | "us" | "usa"
    )
}

pub(super) fn format_property_address(address: &model::PropertyAddress) -> String {
    let state_postal = [
        one_line_address_part(address.state_or_province.as_deref()),
        one_line_address_part(address.postal_code.as_deref()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    [
        one_line_address_part(address.address_line1.as_deref()),
        one_line_address_part(address.neighborhood.as_deref()),
        one_line_address_part(address.city.as_deref()),
        (!state_postal.is_empty()).then_some(state_postal),
        (!redundant_pr_country(
            address.country.as_deref(),
            address.state_or_province.as_deref(),
        ))
        .then(|| one_line_address_part(address.country.as_deref()))
        .flatten(),
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.trim().is_empty())
    .collect::<Vec<_>>()
    .join(", ")
}

pub(super) fn binding_value(
    binding: &str,
    facts: Option<&model::DealFormFacts>,
    person: Option<&model::Person>,
    property: Option<&model::Property>,
) -> Option<String> {
    match binding {
        "deal.client.name" => facts.and_then(|facts| facts.client_name.clone()),
        "deal.property.label" => facts.and_then(|facts| facts.property_label.clone()),
        "deal.offer.amount" => facts.and_then(|facts| facts.offer_amount.clone()),
        "deal.financing.type" => facts.and_then(|facts| facts.financing_type.clone()),
        "deal.closing.date" => facts.and_then(|facts| facts.closing_date.clone()),
        "person.displayName" => person
            .map(|person| person.display_name.clone())
            .or_else(|| facts.and_then(|facts| facts.person_display_name.clone()))
            .or_else(|| facts.and_then(|facts| facts.client_name.clone())),
        "property.name" => property
            .map(|property| property.display_name.clone())
            .or_else(|| facts.and_then(|facts| facts.property_name.clone()))
            .or_else(|| facts.and_then(|facts| facts.property_label.clone())),
        "property.location" => property
            .map(|property| format_property_address(&property.address))
            .filter(|value| !value.trim().is_empty())
            .or_else(|| facts.and_then(|facts| facts.property_location.clone())),
        _ => None,
    }
}
