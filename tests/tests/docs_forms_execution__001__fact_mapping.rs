//! DOCS.FORMS.EXECUTION — fact mapping (TST-DOCS-FORMS-EXECUTION-001).
//!
//! Contract: a field's `source` attribute declares which FACT prefills it, and the mapping is closed at both
//! ends. At the grammar end, the production parser (`parse_field` in
//! `middle/model/src/forms_template/show_all_value.rs`) accepts only the eight canonical `BINDINGS` and
//! refuses any other source. At the composition end, the production prefill (`binding_value` in
//! `web/src/api/portal_bridge/forms_templates.rs`, driven by `prefill_form_values` in
//! `web/src/api/portal_bridge/forms_values.rs`) resolves exactly those bindings against the deal facts, the
//! person and the property — so a binding the parser accepts has a mapper (never a silent empty prefill) and
//! a mapper case the parser refuses is dead drift (never a shadow registry).
//!
//! Specified level: L3 Composition / DocumentVaultHarness. The reachable deterministic seam is the one
//! TST-DOCS-CONTRACT-005 (Complete) established for this taxonomy: the production parser on real templates
//! plus source-structure assertions over the composition boundary; no database, no network, no providers.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_execution__001__fact_mapping

use model::forms_template::{parse_template_xml, TemplateLibrary};
use test_harness::source;

/// Extract the quoted entries of a `CONST` string array from production source.
fn const_string_entries(text: &str, const_name: &str) -> Vec<String> {
    let start = text
        .find(const_name)
        .unwrap_or_else(|| panic!("{const_name} exists in the production source"));
    let body = &text[start..];
    // The array itself starts at its `= [`: a type annotation like `[&str; 8]` closes earlier, and must
    // never be mistaken for the entries.
    let open = body
        .find("= [")
        .unwrap_or_else(|| panic!("{const_name}'s array opens"));
    let body = &body[open..];
    let end = body
        .find(']')
        .unwrap_or_else(|| panic!("{const_name}'s array closes"));
    body[..end]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// Extract the match-arm patterns of one `pub(super) fn` from production source: the quoted string before
/// each top-level `=>` inside the function body.
fn match_arm_keys(text: &str, function: &str) -> Vec<String> {
    let start = text
        .find(function)
        .unwrap_or_else(|| panic!("{function} exists in the production source"));
    let body = &text[start..];
    // The function ends at the first line that closes it at column zero.
    let end = body[1..]
        .find("\n}\n")
        .map(|index| index + 1)
        .unwrap_or(body.len());
    body[..end]
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.ends_with('{') && trimmed.contains("=>") {
                trimmed.split('"').nth(1).map(str::to_string)
            } else {
                None
            }
        })
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-EXECUTION-001); the file and the assay use it.
fn docs_forms_execution_001__fact_mapping() {
    let root = source::workspace_root();

    // 1. THE GRAMMAR END: the parser's canonical registry is exactly the eight fact bindings.
    let model_source =
        source::read(&root.join("middle/model/src/forms_template/show_all_value.rs"));
    let mut bindings = const_string_entries(&model_source, "const BINDINGS");
    bindings.sort();
    assert_eq!(
        bindings,
        vec![
            "deal.client.name",
            "deal.closing.date",
            "deal.financing.type",
            "deal.offer.amount",
            "deal.property.label",
            "person.displayName",
            "property.location",
            "property.name",
        ],
        "the parser's canonical fact registry is the declared eight"
    );

    // 2. NEGATIVE (grammar): a field binding outside the registry is refused — a fact source the mapping
    //    does not own can never enter through the template.
    let error = parse_template_xml(
        r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text" source="deal.made.up"/></form>"#,
    )
    .expect_err("a non-canonical fact source is refused");
    assert!(
        error.message.contains("unknown source binding"),
        "the refusal names the binding: {}",
        error.message
    );

    // 3. THE COMPOSITION END: the production mapper resolves exactly the same registry — no canonical
    //    binding without a mapper arm, no mapper arm the registry refuses.
    let web_source = source::read(&root.join("web/src/api/portal_bridge/forms_templates.rs"));
    let mut mapped = match_arm_keys(&web_source, "pub(super) fn binding_value");
    mapped.sort();
    assert_eq!(
        mapped, bindings,
        "fact mapping is closed: the mapper's arms are exactly the parser's canonical bindings"
    );
    // Each mapper arm reads the fact it names (the deal facts carry the deal.* sources).
    for fact_field in [
        "facts.client_name",
        "facts.property_label",
        "facts.offer_amount",
        "facts.financing_type",
        "facts.closing_date",
    ] {
        assert!(
            web_source.contains(fact_field),
            "the deal-facts mapping resolves {fact_field}"
        );
    }

    // 4. The prefill drives the mapping through the template, never around it: the binding is consulted
    //    FIRST, the form default is the fallback, and an unanswered date field gets the date default.
    let values_source = source::read(&root.join("web/src/api/portal_bridge/forms_values.rs"));
    let prefill = {
        let start = values_source
            .find("pub(super) fn prefill_form_values")
            .expect("prefill_form_values exists");
        let body = &values_source[start..];
        let end = body[1..]
            .find("\n}\n")
            .map(|index| index + 1)
            .unwrap_or(body.len());
        &body[..end]
    };
    let binding_first = prefill
        .find("binding_value(binding, facts, person, property)")
        .expect("the binding is mapped");
    let default_second = prefill
        .find("form_default(&template.id, &field.name)")
        .expect("the form default is the fallback");
    assert!(
        binding_first < default_second,
        "the fact binding is consulted before the default"
    );
    assert!(
        prefill.contains("date_default(&field.name)"),
        "an unanswered date field is filled by the date default"
    );

    // 5. THE REAL TEMPLATES: the authored forms really use the registry (the sweep cannot pass vacuously),
    //    and every binding they declare is canonical.
    let library = TemplateLibrary::load_from_dir(&root.join("middle/model/forms/templates"))
        .expect("the repository's templates load");
    let mut bound = 0usize;
    for template in library.all() {
        for field in &template.fields {
            if let Some(binding) = field.binding.as_deref() {
                bound += 1;
                assert!(
                    bindings.contains(&binding.to_string()),
                    "{} v{}: field \"{}\" binds outside the canonical registry",
                    template.id,
                    template.version,
                    field.name
                );
            }
        }
    }
    assert!(
        bound >= 8,
        "the authored templates genuinely prefill from facts ({bound} bound fields)"
    );
}
