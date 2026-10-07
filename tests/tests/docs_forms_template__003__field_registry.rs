//! DOCS.FORMS.TEMPLATE — field registry (TST-DOCS-FORMS-TEMPLATE-003).
//!
//! Contract: a template's fields are its registry, and the production parser (`parse_field` in
//! `middle/model/src/forms_template/show_all_value.rs`, reached through `parse_template_xml`) keeps it closed
//! and exact: the five field types, the `required` flag, select options that must be non-empty, field ids
//! unique within the template, and the `source` prefill binding restricted to the eight canonical bindings in
//! `BINDINGS` — a template naming any other source is refused rather than silently prefilled from nowhere.
//! `TemplateDefinition::field` resolves a field by name and `required_fields` lists the required ones in
//! declaration order.
//!
//! Level: L0 Pure — the production parser on in-memory documents and the repository's real templates;
//! no database, no network, no providers.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_template__003__field_registry

use model::forms_template::{parse_template_xml, TemplateFieldType, TemplateLibrary};
use test_harness::source;

/// The eight canonical prefill bindings, as `BINDINGS` declares them in the production parser.
const CANONICAL_BINDINGS: [&str; 8] = [
    "deal.client.name",
    "deal.property.label",
    "deal.offer.amount",
    "deal.financing.type",
    "deal.closing.date",
    "person.displayName",
    "property.name",
    "property.location",
];

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-TEMPLATE-003); the file and the assay use it.
fn docs_forms_template_003__field_registry() {
    // 1. Every field type registers, with its label, the required flag, and a canonical source binding kept
    //    on the field.
    let template = parse_template_xml(
        r#"<form id="T" version="1" title="T">
             <field id="sellerName" label="Seller Name" type="text" required="true" source="person.displayName"/>
             <field id="listPrice" label="Asking Price" type="money" required="true"/>
             <field id="startDate" label="Start" type="date"/>
             <field id="notes" label="Notes" type="textarea"/>
             <field id="listingType" label="Listing Type" type="select" options="Exclusive Right to Sell,Exclusive Agency"/>
           </form>"#,
    )
    .expect("the registry parses");
    assert_eq!(template.fields.len(), 5);
    let seller = template
        .field("sellerName")
        .expect("field() resolves by name");
    assert_eq!(seller.field_type, TemplateFieldType::Text);
    assert!(seller.required);
    assert_eq!(seller.binding.as_deref(), Some("person.displayName"));
    assert_eq!(
        template.field("listPrice").map(|f| &f.field_type),
        Some(&TemplateFieldType::Money)
    );
    assert_eq!(
        template.field("startDate").map(|f| &f.field_type),
        Some(&TemplateFieldType::Date)
    );
    assert_eq!(
        template.field("notes").map(|f| &f.field_type),
        Some(&TemplateFieldType::Textarea)
    );
    let listing_type = template.field("listingType").expect("the select field");
    assert_eq!(listing_type.field_type, TemplateFieldType::Select);
    assert_eq!(listing_type.options.len(), 2);
    assert!(
        !listing_type.required,
        "required defaults to false when the attribute is absent"
    );
    assert!(
        template.field("noSuchField").is_none(),
        "an unknown name does not resolve"
    );
    assert_eq!(
        template.required_fields(),
        vec!["sellerName", "listPrice"],
        "the required registry reads in declaration order"
    );

    // 2. Every canonical binding is accepted as a field's `source`.
    for binding in CANONICAL_BINDINGS {
        let xml = format!(
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text" source="{binding}"/></form>"#
        );
        let parsed = parse_template_xml(&xml).expect("a canonical binding parses");
        assert_eq!(
            parsed.fields[0].binding.as_deref(),
            Some(binding),
            "the binding is kept on the field"
        );
    }

    // 3. NEGATIVE/REFUSAL CASES — the registry is closed: anything outside it is refused with a reason.
    let refusals: [(&str, &str); 5] = [
        // An unknown field type.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="richtext"/></form>"#,
            "unknown type",
        ),
        // A source binding outside the canonical registry.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text" source="nowhere"/></form>"#,
            "unknown source binding",
        ),
        // A select with no options.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="select"/></form>"#,
            "non-empty options list",
        ),
        // A duplicated field id.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text"/><field id="a" label="A2" type="text"/></form>"#,
            "Duplicate field id",
        ),
        // A malformed when gate on a field.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text" when="novalue"/></form>"#,
            "when must be",
        ),
    ];
    for (xml, expected) in refusals {
        let error = parse_template_xml(xml).expect_err("the field must be refused");
        assert!(
            error.message.contains(expected),
            "expected a refusal mentioning \"{expected}\", got \"{}\"",
            error.message
        );
    }

    // 4. THE REAL REGISTRY: sweep every authored template — field ids are unique per template, every
    //    declared binding is canonical, and the registry is genuinely exercised (the sweep can never pass
    //    vacuously over an empty set).
    let root = source::workspace_root();
    let library = TemplateLibrary::load_from_dir(&root.join("middle/model/forms/templates"))
        .expect("the repository's templates load");
    let mut bound_fields = 0usize;
    let mut required_total = 0usize;
    for template in library.all() {
        let mut seen: Vec<&str> = Vec::new();
        for field in &template.fields {
            assert!(
                !seen.contains(&field.name.as_str()),
                "{} v{}: duplicate field id \"{}\" in the authored registry",
                template.id,
                template.version,
                field.name
            );
            seen.push(field.name.as_str());
            if let Some(binding) = field.binding.as_deref() {
                bound_fields += 1;
                assert!(
                    CANONICAL_BINDINGS.contains(&binding),
                    "{} v{}: field \"{}\" binds to non-canonical source \"{binding}\"",
                    template.id,
                    template.version,
                    field.name
                );
            }
        }
        required_total += template.required_fields().len();
    }
    assert!(
        bound_fields >= 8,
        "the authored templates really use the binding registry ({bound_fields} bound fields)"
    );
    assert!(
        required_total > 0,
        "the authored templates really use the required registry"
    );

    // And the canonical LISTING-01 v4 registry itself: the asking price is required money, the listing type
    // is a closed select.
    let listing = library.version("LISTING-01", 4).expect("LISTING-01 v4");
    let price = listing.field("listPrice").expect("listPrice is registered");
    assert_eq!(price.field_type, TemplateFieldType::Money);
    assert!(price.required);
    let options = &listing
        .field("listingType")
        .expect("listingType is registered")
        .options;
    assert!(
        options.contains(&"Exclusive Right to Sell".to_string()),
        "the select's options are the registry's, not a caller's"
    );
}
