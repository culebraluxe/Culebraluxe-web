//! DOCS.FORMS.TEMPLATE — show-all semantics (TST-DOCS-FORMS-TEMPLATE-004).
//!
//! Contract: `SHOW_ALL_VALUE` ("Show All") is the one value that satisfies every visibility gate at
//! once — a field or section gated on a controlling field shows when that field holds "Show All",
//! whatever the gate's own value list says. The production boundary is
//! `TemplateDefinition::when_satisfied` (`middle/model/src/forms_template/show_all_value.rs`), which
//! the renderer (`web/src/vault/forms_render/page_width.rs`) and the WASM editor consult for every
//! gated field and section. The sentinel is never a live transaction value: it exists only so a
//! reader can choose to see everything a template can show.
//!
//! Level: L0 Pure — the production gate function, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_template__004__show_all_semantics

use std::collections::BTreeMap;

use model::forms_template::{TemplateDefinition, TemplateWhen, SHOW_ALL_VALUE};

fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-TEMPLATE-004); the file and the assay use it.
fn docs_forms_template_004__show_all_semantics() {
    // 1. The sentinel itself: the one value that shows every gated field and section at once.
    assert_eq!(SHOW_ALL_VALUE, "Show All");

    let gate = TemplateWhen {
        field: "financing".to_string(),
        values: vec!["Cash".to_string(), "Blend".to_string()],
    };

    // 2. THE SHOW-ALL SHORTCUT: "Show All" satisfies the gate even though it is not in the gate's
    //    own value list — that is what makes it the value that shows everything.
    assert!(
        TemplateDefinition::when_satisfied(Some(&gate), &values(&[("financing", "Show All")])),
        "Show All must satisfy a gate whose own values do not list it"
    );

    // 3. The match is case-insensitive, the same rule the gate's own values use.
    for spelling in ["show all", "SHOW ALL", "sHoW aLl"] {
        assert!(
            TemplateDefinition::when_satisfied(Some(&gate), &values(&[("financing", spelling)])),
            "{spelling:?} must satisfy the gate"
        );
    }

    // 4. A listed value still satisfies the gate — show-all is a shortcut, not a replacement.
    assert!(
        TemplateDefinition::when_satisfied(Some(&gate), &values(&[("financing", "Cash")])),
        "a listed value must satisfy the gate"
    );
    assert!(
        !TemplateDefinition::when_satisfied(Some(&gate), &values(&[("financing", "Bank")])),
        "an unlisted value must not satisfy the gate"
    );

    // 5. NEGATIVE: an unset gate is hidden, never shown. A gate the controlling field has not
    //    answered must not open the field or section it guards.
    assert!(
        !TemplateDefinition::when_satisfied(Some(&gate), &values(&[])),
        "an unset gate is hidden rather than shown"
    );
    assert!(
        !TemplateDefinition::when_satisfied(Some(&gate), &values(&[("financing", "")])),
        "a blank gate value is hidden rather than shown"
    );
    assert!(
        !TemplateDefinition::when_satisfied(Some(&gate), &values(&[("financing", "   ")])),
        "a whitespace-only gate value is hidden rather than shown"
    );

    // 6. NEGATIVE: the gate reads the field it NAMES, not any field that happens to hold Show All.
    assert!(
        !TemplateDefinition::when_satisfied(Some(&gate), &values(&[("other", "Show All")])),
        "a different field holding Show All must not open this gate"
    );

    // 7. No gate at all: everything shows. The sentinel only matters where a gate exists.
    assert!(
        TemplateDefinition::when_satisfied(None, &values(&[])),
        "an ungated field or section is always visible"
    );
}
