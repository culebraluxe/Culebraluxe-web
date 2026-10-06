//! DOCS.FORMS.TEMPLATE — field formatting (TST-DOCS-FORMS-TEMPLATE-007).
//!
//! Contract: a field's value is formatted for display and for rendering by the one set of rules
//! the composer and the editor share — money groups thousands under a `$` and keeps the typed
//! decimal, a date renders as `Month D, YYYY`, and a value of any other shape passes through
//! unchanged. The production boundary is `model::forms_format`
//! (`middle/model/src/forms_format.rs`), ported number for number from `lib/forms/format.ts` so a
//! money field reads the same in the input as it does in the PDF.
//!
//! Level: L0 Pure — the production formatting functions, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_template__007__field_formatting

use model::forms_format::{format_date, format_field_value, format_money};
use model::forms_template::{TemplateFieldDefinition, TemplateFieldType};

fn field(field_type: TemplateFieldType) -> TemplateFieldDefinition {
    TemplateFieldDefinition {
        name: "f".into(),
        label: "F".into(),
        field_type,
        required: false,
        binding: None,
        options: Vec::new(),
        when: None,
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-TEMPLATE-007); the file and the assay use it.
fn docs_forms_template_007__field_formatting() {
    // 1. Money: thousands are grouped, the `$` is prepended, and the typed decimal is kept.
    assert_eq!(format_money("1250000"), "$1,250,000");
    assert_eq!(format_money("1250000.5"), "$1,250,000.5");
    assert_eq!(
        format_money("$1,250,000"),
        "$1,250,000",
        "formatting an already-formatted value is idempotent"
    );

    // 2. NEGATIVE: a value with no digits at all passes through unchanged — it is not money and
    //    must not become `$0` or an empty cell.
    assert_eq!(format_money("not a number"), "not a number");
    assert_eq!(format_money(""), "");

    // 3. Dates: ISO `YYYY-MM-DD` renders as prose.
    assert_eq!(format_date("2026-01-05"), "January 5, 2026");
    assert_eq!(format_date("2026-12-25"), "December 25, 2026");

    // 4. NEGATIVE: a value that is not a valid ISO date passes through unchanged — an impossible
    //    month, an impossible day, a non-padded date and a prose date are all left alone rather
    //    than rendered as a wrong date.
    assert_eq!(format_date("2026-13-05"), "2026-13-05", "an impossible month passes through");
    assert_eq!(format_date("2026-01-32"), "2026-01-32", "an impossible day passes through");
    assert_eq!(format_date("2026-1-5"), "2026-1-5", "a non-padded date passes through");
    assert_eq!(format_date("next week"), "next week");
    assert_eq!(format_date(""), "");

    // 5. The dispatcher: money and dates are formatted by their type, every other type is left
    //    alone, and an empty value is never formatted.
    assert_eq!(
        format_field_value(&field(TemplateFieldType::Money), "1250000"),
        "$1,250,000"
    );
    assert_eq!(
        format_field_value(&field(TemplateFieldType::Date), "2026-01-05"),
        "January 5, 2026"
    );
    assert_eq!(
        format_field_value(&field(TemplateFieldType::Text), "1250000"),
        "1250000",
        "a text field is not money"
    );
    assert_eq!(
        format_field_value(&field(TemplateFieldType::Select), "Cash"),
        "Cash"
    );
    assert_eq!(
        format_field_value(&field(TemplateFieldType::Money), "  "),
        "  ",
        "an empty money value is not formatted"
    );
    assert_eq!(
        format_field_value(&field(TemplateFieldType::Date), ""),
        "",
        "an empty date value is not formatted"
    );
}
