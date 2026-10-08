//! Which form is open, the composed document text, value formatting, visibility rules and session labels.

#[allow(unused_imports)]
use super::*;

pub(super) fn preferred_form_id(page: &FormsPage) -> Option<String> {
    page.items
        .iter()
        .find(|item| {
            item.template_id == "LISTING-01"
                && item.template_version == item.active_version
                && item.status != "issued"
        })
        .or_else(|| {
            page.items.iter().find(|item| {
                item.template_id == "LISTING-01" && item.template_version == item.active_version
            })
        })
        .or_else(|| {
            page.items
                .iter()
                .find(|item| item.template_id == "LISTING-01")
        })
        .or_else(|| page.items.first())
        .map(|item| item.id.clone())
}

pub(super) fn current_form_id(model: &Model) -> Option<String> {
    model
        .page
        .as_ref()
        .and_then(|page| page.selected.as_ref())
        .map(|form| form.id.clone())
}

pub(super) fn current_template_id(model: &Model) -> String {
    model
        .page
        .as_ref()
        .and_then(|page| page.selected.as_ref())
        .map(|form| form.template_id.clone())
        .filter(|value| !value.is_empty())
        .or_else(|| (!model.selected_template.is_empty()).then(|| model.selected_template.clone()))
        .unwrap_or_else(|| "LISTING-01".into())
}

pub(super) fn composed_sections(model: &Model) -> BTreeMap<String, String> {
    let mut sections = model.sections.clone();
    sections.insert("body".into(), model.details_text.clone());
    sections.insert(
        "bodyEdited".into(),
        if model.body_edited { "true" } else { "false" }.into(),
    );
    sections
}

pub(super) fn resolve_document_body(
    template: &FormTemplate,
    values: &BTreeMap<String, String>,
    sections: &BTreeMap<String, String>,
) -> String {
    let edited = sections
        .get("bodyEdited")
        .is_some_and(|value| value == "true");
    let body = sections
        .get("body")
        .map(String::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if edited && !body.is_empty() {
        body.to_owned()
    } else {
        document_body_text(template, values, sections)
    }
}

pub(super) fn document_body_text(
    template: &FormTemplate,
    values: &BTreeMap<String, String>,
    sections: &BTreeMap<String, String>,
) -> String {
    template
        .sections
        .iter()
        .filter(|section| when_visible(section.when.as_ref(), values))
        .map(|section| {
            let generated = section
                .segments
                .iter()
                .map(|segment| match segment.kind.as_str() {
                    "value" => segment
                        .field
                        .as_deref()
                        .and_then(|name| {
                            let value = values.get(name).cloned().unwrap_or_default();
                            template
                                .fields
                                .iter()
                                .find(|field| field.name == name)
                                .map(|field| format_field_value(field, &value))
                                .or(Some(value))
                        })
                        .unwrap_or_default(),
                    _ => segment.text.clone().unwrap_or_default(),
                })
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let edited = if section.editable {
                sections
                    .get(&section.name)
                    .map(String::as_str)
                    .map(str::trim)
                    .unwrap_or_default()
            } else {
                ""
            };
            let text = if edited.is_empty() {
                generated
            } else {
                edited.into()
            };
            if text.trim().is_empty() {
                section.label.clone()
            } else {
                format!("{}\n{}", section.label, text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(super) fn format_field_value(field: &FormTemplateField, value: &str) -> String {
    if field.field_type == "money" {
        model::forms_format::format_money(value)
    } else {
        value.to_owned()
    }
}

pub(super) fn when_visible(when: Option<&FormWhen>, values: &BTreeMap<String, String>) -> bool {
    let Some(when) = when else {
        return true;
    };
    let actual = values
        .get(&when.field)
        .map(String::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if actual.is_empty() {
        return false;
    }
    actual.eq_ignore_ascii_case("Show All")
        || when
            .values
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(actual))
}

/// Who a form is about, from the names written ON the form: buyer (or visitor) and seller. The linked client is only a
/// last resort — it is the DEAL's client, so on a listing agreement (no buyer) it used to show up as a phantom buyer
/// ("James Lee / Juan A. Santa Cruz") on every agreement attached to that deal.
pub(super) fn party_name(item: &FormItem) -> String {
    let buyer = item
        .field_values
        .get("buyerName")
        .or_else(|| item.field_values.get("visitorName"))
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty());
    let seller = item
        .field_values
        .get("sellerName")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty());
    match (buyer, seller) {
        (Some(left), Some(right)) if left != right => format!("{left} / {right}"),
        (Some(left), _) => left.to_owned(),
        (_, Some(right)) => right.to_owned(),
        _ => item
            .client_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Untitled")
            .to_owned(),
    }
}

pub(super) fn session_label(item: &FormItem) -> String {
    [
        Some(party_name(item)),
        item.property_label.clone(),
        Some(
            item.updated_at
                .get(0..10)
                .unwrap_or(&item.updated_at)
                .to_owned(),
        ),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

pub(super) fn session_secondary(item: &FormItem, template: &FormTemplate) -> String {
    let updated = item.updated_at.get(0..10).unwrap_or(&item.updated_at);
    [item.property_label.as_deref(), Some(updated)]
        .into_iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
        .trim()
        .to_owned()
        .pipe(|value| {
            if value.is_empty() {
                template.display_name.clone()
            } else {
                value
            }
        })
}

pub(super) fn status_dot_class(status: &str) -> &'static str {
    match status {
        "issued" => "bg-[var(--portal-success)]",
        "ready" => "bg-[var(--portal-navy-soft)]",
        _ => "bg-black/25",
    }
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

pub(super) fn error_band(error: Option<&str>) -> Html {
    error
        .map(|message| {
            html! {
                <div class="mb-3 rounded-[var(--portal-tab-radius)] border border-[var(--portal-archive)]/25 bg-white/80 px-3 py-2 text-sm text-[var(--portal-archive)]">
                    { message }
                </div>
            }
        })
        .unwrap_or_default()
}
