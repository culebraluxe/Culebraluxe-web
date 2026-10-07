//! DOCS.FORMS.TEMPLATE — section parser (TST-DOCS-FORMS-TEMPLATE-002).
//!
//! Contract: a template `<section>` is the contract prose, and the production section parser
//! (`parse_section` in `middle/model/src/forms_template/parse_section.rs`, reached through
//! `parse_template_xml`) keeps it exact: literal text and `<value field="…"/>` interpolations stay in
//! authored order as `TemplateSectionSegment`s, the interpolated field names are recorded in `values` in the
//! order they appear, `editable` and the `when` visibility gate are honored, and anything the prose cannot
//! mean — an unknown element, a value naming no declared field, a duplicated section id — is refused with a
//! reason rather than rendered as different terms.
//!
//! Level: L0 Pure — the production parser on in-memory documents and the repository's real templates;
//! no database, no network, no providers.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_template__002__section_parser

use std::collections::BTreeMap;

use model::forms_template::{
    parse_template_xml, TemplateDefinition, TemplateLibrary, TemplateSectionSegment, TemplateWhen,
};
use test_harness::source;

fn values_of(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-TEMPLATE-002); the file and the assay use it.
fn docs_forms_template_002__section_parser() {
    // 1. A section keeps its prose EXACTLY: literal text and field interpolations in authored order, the
    //    interpolated field names in `values` in the same order, and the editable flag honored.
    let template = parse_template_xml(
        r#"<form id="T" version="1" title="T">
             <field id="sellerName" label="Seller" type="text"/>
             <field id="listPrice" label="Price" type="money"/>
             <section id="terms" title="Terms" editable="true">The Seller, <value field="sellerName"/>, asks <value field="listPrice"/>.</section>
             <section id="fixed" title="Fixed">No edits.</section>
           </form>"#,
    )
    .expect("the sections parse");
    let terms = &template.sections[0];
    assert_eq!(terms.name, "terms");
    assert_eq!(terms.label, "Terms");
    assert!(terms.editable, "editable=\"true\" is honored");
    assert_eq!(
        terms.segments,
        vec![
            TemplateSectionSegment::Text("The Seller, ".to_string()),
            TemplateSectionSegment::Value("sellerName".to_string()),
            TemplateSectionSegment::Text(", asks ".to_string()),
            TemplateSectionSegment::Value("listPrice".to_string()),
            TemplateSectionSegment::Text(".".to_string()),
        ],
        "text and interpolations stay in authored order"
    );
    assert_eq!(
        terms.values,
        vec!["sellerName".to_string(), "listPrice".to_string()],
        "the interpolated fields are recorded in the order they appear"
    );
    assert!(!template.sections[1].editable, "editable defaults to false");

    // An empty section is legal and carries no segments.
    let empty = parse_template_xml(
        r#"<form id="T" version="1" title="T">
             <field id="a" label="A" type="text"/>
             <section id="blank" title="Blank" editable="true"></section>
           </form>"#,
    )
    .expect("an empty section is allowed");
    assert!(empty.sections[0].segments.is_empty());
    assert!(empty.sections[0].values.is_empty());

    // 2. NEGATIVE/REFUSAL CASES — prose that cannot mean what it says is refused with a reason.
    let refusals: [(&str, &str); 4] = [
        // A value naming a field the template never declared.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text"/><section id="s" title="S"><value field="missing"/></section></form>"#,
            "references unknown field",
        ),
        // An element the prose grammar does not own.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text"/><section id="s" title="S"><b>bold</b></section></form>"#,
            "Unknown element <b> inside <section",
        ),
        // A duplicated section id.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text"/><section id="s" title="S">x</section><section id="s" title="S2">y</section></form>"#,
            "Duplicate section id",
        ),
        // A when gate without "field:Value".
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text"/><section id="s" title="S" when="novalue">x</section></form>"#,
            "when must be",
        ),
    ];
    for (xml, expected) in refusals {
        let error = parse_template_xml(xml).expect_err("the section must be refused");
        assert!(
            error.message.contains(expected),
            "expected a refusal mentioning \"{expected}\", got \"{}\"",
            error.message
        );
    }

    // 3. The when gate on a section parses and evaluates through the production rule
    //    (`TemplateDefinition::when_satisfied`): case-insensitive, an unset field HIDES the section, and the
    //    "Show All" sentinel shows everything.
    let gated = parse_template_xml(
        r#"<form id="T" version="1" title="T">
             <field id="financing" label="Financing" type="select" options="Cash,Bank"/>
             <section id="cashTerms" title="Cash Terms" when="financing:Cash,Blend">Cash prose.</section>
           </form>"#,
    )
    .expect("a gated section parses");
    let gate = gated.sections[0]
        .when
        .as_ref()
        .expect("the gate is recorded");
    assert_eq!(
        gate,
        &TemplateWhen {
            field: "financing".to_string(),
            values: vec!["Cash".to_string(), "Blend".to_string()],
        }
    );
    assert!(TemplateDefinition::when_satisfied(
        Some(gate),
        &values_of(&[("financing", "cash")])
    ));
    assert!(TemplateDefinition::when_satisfied(
        Some(gate),
        &values_of(&[("financing", "Show All")])
    ));
    assert!(
        !TemplateDefinition::when_satisfied(Some(gate), &values_of(&[("financing", "Bank")])),
        "a value the gate does not list hides the section"
    );
    assert!(
        !TemplateDefinition::when_satisfied(Some(gate), &values_of(&[])),
        "an unset gate field HIDES the section rather than showing it"
    );
    assert!(
        TemplateDefinition::when_satisfied(None, &values_of(&[])),
        "an ungated section always shows"
    );

    // 4. The real templates: the production LISTING-01 v4 prose interpolates the seller's name through this
    //    exact seam — the partnership section names `sellerName` as a Value segment and records it in values.
    let root = source::workspace_root();
    let library = TemplateLibrary::load_from_dir(&root.join("middle/model/forms/templates"))
        .expect("the repository's templates load");
    let listing = library.version("LISTING-01", 4).expect("LISTING-01 v4");
    let partnership = listing
        .sections
        .iter()
        .find(|section| section.name == "partnership")
        .expect("the partnership section exists");
    assert!(
        partnership
            .segments
            .iter()
            .any(|segment| *segment == TemplateSectionSegment::Value("sellerName".into())),
        "the prose interpolates the seller's name"
    );
    assert!(partnership.values.contains(&"sellerName".to_string()));
    assert!(
        listing
            .sections
            .iter()
            .all(|section| !section.label.is_empty()),
        "every authored section carries its label"
    );
}
