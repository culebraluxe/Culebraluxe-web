//! Carrying a saved form forward to a new template version.
//!
//! A form instance is stamped with the template version it was created against, and the stamp never moves: the create
//! path writes it once (`db/src/forms/database.rs::create_instance`) and no later edit re-stamps it. So a revised
//! template — LISTING-01 v4 to v5 — leaves every form already made from the old version on the old version for good,
//! and the only way an existing agreement appears in the new format is a NEW instance on the new version. The v5
//! template says the same thing in its own header ("Forms already issued on v4 keep v4; new forms are v5").
//!
//! This module is that copy's rule, and only its rule: it is pure, it reads no database, and the operator tool
//! (`cli` `db-tool carry-forward`) supplies the rows. Three decisions live here, each one a sentence:
//!
//!   1. The older form's value travels, and it WINS over whatever the newer form holds under that field name — the
//!      signed agreement is the source of truth for what it says, and a newer draft is usually the app's own prefill
//!      (a default date, an empty price). Anything it replaces is NAMED, so nothing vanishes quietly.
//!   2. A field the older VERSION did not have takes the derived email, and nothing else is invented.
//!   3. A field the TEMPLATE fixes (`fixed="Lisa Penfield"`) holds the template's value whatever travelled — the same
//!      rule `save_form_values` applies, for the same reason.
//!   4. The operator's whole-document prose (`body`, with the `bodyEdited` marker) travels ONLY between versions that
//!      print the same sections. Prose edited against one version's legal text is not the other version's text.

use crate::forms_template::{TemplateDefinition, TemplateFieldDefinition, TemplateFieldType};
use std::collections::BTreeMap;

/// The fields a carried form holds, in the target template's order, plus what did not come with it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CarriedValues {
    pub values: BTreeMap<String, String>,
    /// Source keys the target template does not declare and that held something: named, so nothing vanishes quietly.
    pub dropped: Vec<String>,
    /// Values the newer form held that this conversion REPLACED, with what they were: a prefill that turned out to be
    /// wrong is a thing the operator must see, not a thing this decides in silence.
    pub replaced: Vec<(String, String)>,
    /// The field that took the derived email, when the older version had no such field.
    pub derived_email: Option<String>,
}

/// Whether two versions of a template print the same prose.
///
/// THE TEST IS THE SECTIONS, NOT THE FILE AND NOT THE VERSION NUMBER: each section's name, label, editability,
/// visibility gate and its literal/`<value>` runs, in order. Two versions can differ only in their field list
/// (v4 → v5 does exactly that) and still print the same document.
pub fn sections_match(source: &TemplateDefinition, target: &TemplateDefinition) -> bool {
    prose(source) == prose(target)
}

fn prose(template: &TemplateDefinition) -> Vec<String> {
    template
        .sections
        .iter()
        .map(|section| {
            format!(
                "{}|{}|{}|{:?}|{:?}",
                section.name, section.label, section.editable, section.when, section.segments
            )
        })
        .collect()
}

/// The values a form on `target` holds when it is carried forward from `source`.
///
/// The order of the answers is the rule, and the first non-empty one wins:
///
///   1. what the older form holds under the same field name — the signed agreement is the source of truth for what it
///      says, and that includes values the property or person record has since drifted away from. It wins over what
///      the newer form holds, and what it wins over is reported in `replaced`;
///   2. what the newer form holds — kept where the older form has nothing, so a conversion fills gaps rather than
///      blanking them;
///   3. the derived email, for an `email` field the older version did not have: a field the target ADDED cannot be
///      known from the older form, and it is where the signing link goes.
///
/// Then a `fixed` field overrides everything with the template's own value.
pub fn carried_values(
    source: &TemplateDefinition,
    target: &TemplateDefinition,
    source_values: &BTreeMap<String, String>,
    existing_values: &BTreeMap<String, String>,
    derived_email: Option<&str>,
) -> CarriedValues {
    let mut carried = CarriedValues::default();

    for field in &target.fields {
        let older = trimmed(source_values.get(&field.name));
        let existing = trimmed(existing_values.get(&field.name));
        if let (Some(older), Some(existing)) = (older.as_ref(), existing.as_ref()) {
            if older != existing {
                carried
                    .replaced
                    .push((field.name.clone(), existing.clone()));
            }
        }
        let value = match older.or(existing) {
            Some(value) => value,
            None => match derived_email_for(source, field, derived_email) {
                Some(value) => {
                    carried.derived_email = Some(field.name.clone());
                    value
                }
                None => String::new(),
            },
        };
        // A fixed field holds the template's value whatever travelled.
        let value = field.fixed.clone().unwrap_or(value);
        carried.values.insert(field.name.clone(), value);
    }

    carried.dropped = source_values
        .iter()
        .filter(|(name, value)| target.field(name).is_none() && !value.trim().is_empty())
        .map(|(name, _)| name.clone())
        .collect();

    carried
}

/// The derived email, only for a field the older VERSION did not declare and the target types as an email.
fn derived_email_for(
    source: &TemplateDefinition,
    field: &TemplateFieldDefinition,
    derived_email: Option<&str>,
) -> Option<String> {
    let added_by_target = source.field(&field.name).is_none();
    if !added_by_target || field.field_type != TemplateFieldType::Email {
        return None;
    }
    derived_email
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// How the whole-document prose was decided, so the operator tool can say it out loud instead of re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarriedBody {
    /// The newer form already held an edit: it is the one somebody is working on, and it stays.
    KeptTheNewerEdit,
    /// The older form's edited prose travelled, because both versions print the same sections.
    CarriedTheOlderProse,
    /// Nothing travelled — there was no edit to carry, or the versions do not print the same sections — so the body
    /// is composed from the target template's own sections when it is rendered.
    Regenerated,
}

/// The section text a carried form holds, and how its whole-document prose was decided.
///
/// The per-section text is not version-scoped — it is what somebody typed into this form — so it travels as it is.
/// The whole-document `body` IS version-scoped: it is the rendered agreement with the operator's edits inside it,
/// written against the sections of its own version. So it travels only when the two versions print the same sections
/// AND it was marked edited — `bodyEdited=true` is the marker `resolve_document_body` requires, and a body kept
/// without it is not prose at all, it is stale interpolated fields waiting for somebody to flip the flag.
pub fn carried_sections(
    source: &TemplateDefinition,
    target: &TemplateDefinition,
    source_sections: &BTreeMap<String, String>,
    existing_sections: &BTreeMap<String, String>,
) -> (BTreeMap<String, String>, CarriedBody) {
    let mut sections = source_sections.clone();
    let source_body = sections.remove("body").unwrap_or_default();
    sections.remove("bodyEdited");

    // The newer form's own text wins where it has any: it is the one somebody is working on now.
    for (key, value) in existing_sections {
        if key == "body" || key == "bodyEdited" {
            continue;
        }
        if !value.trim().is_empty() {
            sections.insert(key.clone(), value.clone());
        }
    }

    let existing_body = existing_sections
        .get("body")
        .map(|body| body.trim().to_string())
        .unwrap_or_default();
    let existing_edited = is_edited(existing_sections) && !existing_body.is_empty();

    let (body, decision) = if existing_edited {
        (existing_body, CarriedBody::KeptTheNewerEdit)
    } else if is_edited(source_sections)
        && !source_body.trim().is_empty()
        && sections_match(source, target)
    {
        (source_body, CarriedBody::CarriedTheOlderProse)
    } else {
        (String::new(), CarriedBody::Regenerated)
    };

    if decision == CarriedBody::Regenerated {
        // No prose travels, and the marker says so, so the composer renders the target template's own sections.
        sections.insert("bodyEdited".into(), "false".into());
        return (sections, decision);
    }

    sections.insert("body".into(), body);
    sections.insert("bodyEdited".into(), "true".into());
    (sections, decision)
}

fn is_edited(sections: &BTreeMap<String, String>) -> bool {
    sections.get("bodyEdited").map(String::as_str) == Some("true")
}

fn trimmed(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms_template::{
        TemplateParticipantRole, TemplatePresentation, TemplateRendering,
        TemplateSectionDefinition, TemplateSectionSegment, TemplateSignatureGroup,
    };

    fn field(name: &str) -> TemplateFieldDefinition {
        TemplateFieldDefinition {
            name: name.into(),
            label: name.into(),
            field_type: TemplateFieldType::Text,
            required: false,
            binding: None,
            options: Vec::new(),
            when: None,
            fixed: None,
        }
    }

    fn template(
        version: i32,
        fields: Vec<TemplateFieldDefinition>,
        prose: &str,
    ) -> TemplateDefinition {
        TemplateDefinition {
            id: "LISTING-01".into(),
            version,
            display_name: "Listing Agreement".into(),
            document_type_label: "Listing Agreement".into(),
            fields,
            sections: vec![TemplateSectionDefinition {
                name: "partnership".into(),
                label: "Parties, Appointment and Partnership".into(),
                editable: false,
                segments: vec![TemplateSectionSegment::Text(prose.into())],
                values: Vec::new(),
                when: None,
            }],
            rendering: TemplateRendering {
                title: "REAL ESTATE LISTING AGREEMENT".into(),
                issuer: "Culebraluxe LLC".into(),
                presentation: TemplatePresentation::Agreement,
            },
            participants: Vec::<TemplateParticipantRole>::new(),
            signature_groups: Vec::<TemplateSignatureGroup>::new(),
        }
    }

    fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    /// The plain case: the same field names carry, and the one field the new version added is filled from the seller's
    /// own record — the same email the signing flow resolves.
    #[test]
    fn values_travel_and_a_field_the_older_version_lacked_takes_the_derived_email() {
        let older = template(4, vec![field("sellerName"), field("listPrice")], "same");
        let mut seller_email = field("sellerEmail");
        seller_email.field_type = TemplateFieldType::Email;
        let target = template(
            5,
            vec![field("sellerName"), seller_email, field("listPrice")],
            "same",
        );

        let carried = carried_values(
            &older,
            &target,
            &values(&[
                ("sellerName", "Juan A. Santa Cruz"),
                ("listPrice", "522000"),
            ]),
            &BTreeMap::new(),
            Some("tutisantacruz@outlook.com"),
        );

        assert_eq!(
            carried.values.get("sellerName").unwrap(),
            "Juan A. Santa Cruz"
        );
        assert_eq!(carried.values.get("listPrice").unwrap(), "522000");
        assert_eq!(
            carried.values.get("sellerEmail").unwrap(),
            "tutisantacruz@outlook.com"
        );
        assert_eq!(carried.derived_email.as_deref(), Some("sellerEmail"));
    }

    /// A field the older version DID declare is never invented from the record: the agreement is the source of truth,
    /// and the record may have drifted (the live agreements disagree with their own property rows).
    #[test]
    fn an_email_field_the_older_version_had_keeps_the_older_value() {
        let mut older_email = field("sellerEmail");
        older_email.field_type = TemplateFieldType::Email;
        let older = template(4, vec![older_email.clone()], "same");
        let target = template(5, vec![older_email], "same");

        let carried = carried_values(
            &older,
            &target,
            &values(&[("sellerEmail", "typed-by-hand@example.com")]),
            &BTreeMap::new(),
            Some("from-the-record@example.com"),
        );

        assert_eq!(
            carried.values.get("sellerEmail").unwrap(),
            "typed-by-hand@example.com"
        );
        assert_eq!(carried.derived_email, None);
    }

    /// The signed agreement wins over the newer draft, and what it replaced is NAMED — a newer draft is usually the
    /// app's own prefill, and a prefill that turns out to be wrong must be visible rather than silently kept.
    #[test]
    fn the_older_value_wins_and_what_it_replaced_is_named() {
        let older = template(3, vec![field("catastroNumber"), field("startDate")], "same");
        let target = template(5, vec![field("catastroNumber"), field("startDate")], "same");

        let carried = carried_values(
            &older,
            &target,
            &values(&[
                ("catastroNumber", "476-000-005-08-000"),
                ("startDate", "2026-09-22"),
            ]),
            &values(&[
                ("catastroNumber", "476-003-022-15-000"),
                ("startDate", "2026-10-08"),
            ]),
            None,
        );

        assert_eq!(
            carried.values.get("catastroNumber").unwrap(),
            "476-000-005-08-000"
        );
        assert_eq!(carried.values.get("startDate").unwrap(), "2026-09-22");
        assert_eq!(
            carried.replaced,
            vec![
                (
                    "catastroNumber".to_string(),
                    "476-003-022-15-000".to_string()
                ),
                ("startDate".to_string(), "2026-10-08".to_string()),
            ]
        );
    }

    /// A value the older form does not have is kept rather than blanked: a conversion fills gaps, it does not erase.
    #[test]
    fn a_value_only_the_newer_form_holds_is_kept() {
        let older = template(3, vec![field("sellerName")], "same");
        let target = template(5, vec![field("sellerName"), field("commission")], "same");

        let carried = carried_values(
            &older,
            &target,
            &values(&[("sellerName", "Juan A. Santa Cruz")]),
            &values(&[("commission", "4%")]),
            None,
        );

        assert_eq!(
            carried.values.get("sellerName").unwrap(),
            "Juan A. Santa Cruz"
        );
        assert_eq!(carried.values.get("commission").unwrap(), "4%");
        assert_eq!(carried.replaced, Vec::<(String, String)>::new());
    }

    /// A fixed field is the template's fact whatever travelled, exactly as `save_form_values` enforces.
    #[test]
    fn a_fixed_field_holds_the_template_value_whatever_the_form_held() {
        let older = template(4, vec![field("brokerName")], "same");
        let mut fixed = field("brokerName");
        fixed.fixed = Some("Lisa Penfield".into());
        let target = template(5, vec![fixed], "same");

        let carried = carried_values(
            &older,
            &target,
            &values(&[("brokerName", "Somebody Else")]),
            &BTreeMap::new(),
            None,
        );

        assert_eq!(carried.values.get("brokerName").unwrap(), "Lisa Penfield");
    }

    /// A key the target template does not declare is named rather than dropped in silence.
    #[test]
    fn a_source_key_the_target_does_not_declare_is_named_not_silently_dropped() {
        let older = template(
            4,
            vec![field("sellerName"), field("legalOwnerName")],
            "same",
        );
        let target = template(5, vec![field("sellerName")], "same");

        let empty = carried_values(
            &older,
            &target,
            &values(&[
                ("sellerName", "Valarie Matias Monell"),
                ("legalOwnerName", ""),
            ]),
            &BTreeMap::new(),
            None,
        );
        assert_eq!(
            empty.dropped,
            Vec::<String>::new(),
            "an empty key carries nothing to lose"
        );

        let filled = carried_values(
            &older,
            &target,
            &values(&[("legalOwnerName", "Somebody")]),
            &BTreeMap::new(),
            None,
        );
        assert_eq!(filled.dropped, vec!["legalOwnerName".to_string()]);
    }

    /// v4 → v5 is exactly this shape: the field list changed, the prose did not, so the operator's edited body travels.
    #[test]
    fn the_operators_prose_travels_between_versions_that_print_the_same_sections() {
        let older = template(4, vec![field("sellerName")], "the v4 and v5 prose");
        let target = template(
            5,
            vec![field("sellerName"), field("sellerEmail")],
            "the v4 and v5 prose",
        );

        let (sections, carried) = carried_sections(
            &older,
            &target,
            &values(&[
                ("body", "edited prose"),
                ("bodyEdited", "true"),
                ("additional", "wire the deed"),
            ]),
            &BTreeMap::new(),
        );

        assert_eq!(carried, CarriedBody::CarriedTheOlderProse);
        assert_eq!(sections.get("body").unwrap(), "edited prose");
        assert_eq!(sections.get("bodyEdited").unwrap(), "true");
        assert_eq!(sections.get("additional").unwrap(), "wire the deed");
    }

    /// A version whose legal text changed (v3 → v4 did: forfeited escrow, full-price offer) must not carry prose
    /// written against the old text, and it must say so with the marker rather than leave a stale body behind.
    #[test]
    fn prose_does_not_travel_from_a_version_that_printed_different_sections() {
        let older = template(3, vec![field("sellerName")], "the old legal text");
        let target = template(5, vec![field("sellerName")], "the new legal text");

        let (sections, carried) = carried_sections(
            &older,
            &target,
            &values(&[
                ("body", "edited against the old text"),
                ("bodyEdited", "true"),
            ]),
            &BTreeMap::new(),
        );

        assert_eq!(carried, CarriedBody::Regenerated);
        assert_eq!(sections.get("bodyEdited").unwrap(), "false");
        assert!(
            !sections.contains_key("body"),
            "a stale body is not left behind"
        );
    }

    /// A body with no marker was never rendered as prose (`resolve_document_body` requires the marker), so it is not
    /// prose here either — even between versions that print the same sections.
    #[test]
    fn an_unmarked_body_is_not_carried() {
        let older = template(4, vec![field("sellerName")], "same");
        let target = template(5, vec![field("sellerName")], "same");

        let (sections, carried) = carried_sections(
            &older,
            &target,
            &values(&[("body", "a leftover rendered blob")]),
            &BTreeMap::new(),
        );

        assert_eq!(carried, CarriedBody::Regenerated);
        assert_eq!(sections.get("bodyEdited").unwrap(), "false");
    }

    /// If somebody is already editing the newer form, their prose is the one that stays.
    #[test]
    fn the_newer_forms_own_edit_wins_over_the_older_body() {
        let older = template(4, vec![field("sellerName")], "the v4 and v5 prose");
        let target = template(5, vec![field("sellerName")], "the v4 and v5 prose");

        let (sections, carried) = carried_sections(
            &older,
            &target,
            &values(&[("body", "the v4 body"), ("bodyEdited", "true")]),
            &values(&[("body", "typed into v5"), ("bodyEdited", "true")]),
        );

        assert_eq!(carried, CarriedBody::KeptTheNewerEdit);
        assert_eq!(sections.get("body").unwrap(), "typed into v5");
    }
}
