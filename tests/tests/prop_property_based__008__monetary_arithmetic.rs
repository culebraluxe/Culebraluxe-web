//! PROP.PROPERTY_BASED — monetary arithmetic (TST-PROP-PROPERTY-BASED-008).
//!
//! Contract: money is a decimal string, never an `f64`. `Money::parse`
//! accepts an optional sign, digits and at most one decimal point — and
//! nothing else (`1e3`, `12,50`, bare `.` all refuse with `AMOUNT_INVALID`);
//! what parses is kept verbatim for the wire; negativity is the leading `-`
//! and nothing else; display is spreadsheet money — `$`, whole-part digits in
//! threes, **always two decimals** (`model::forms_format`, forms v5 `c483e8f81`,
//! pinned by `tests/tests/docs_forms_template__007__field_formatting.rs:42-43`)
//! — while non-numeric input passes through unchanged.
//!
//! The two display assertions below — and the display property behind them — were
//! authored before forms v5 (batch 36, `17bf934a6`) against the pre-`c483e8f81` rule
//! that kept the typed decimal and echoed the typed digits, so they went red the
//! moment the formatter moved to two decimals (and the property failed on leading
//! zeros, `"0000"` → `"$0.00"`, which the two assertions had been failing in front
//! of). The formatter is the canonical half: its own doc says two decimals and a
//! second test agrees (`docs_forms_template__007…rs:42-43`).
//!
//! Level: L0 Pure — the executable boundary is
//! `model::{accounting, forms_format}`, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test prop_property_based__008__monetary_arithmetic

use model::accounting::Money;
use model::forms_format::format_money;
use proptest::prelude::*;

/// Well-formed decimal strings: optional sign, digits, one optional point.
fn decimal_text() -> impl Strategy<Value = String> {
    (
        prop_oneof![Just(""), Just("+"), Just("-")],
        prop::collection::vec("[0-9]", 1..7).prop_map(|d| d.concat()),
        prop::option::of(prop::collection::vec("[0-9]", 1..4).prop_map(|d| d.concat())),
    )
        .prop_map(|(sign, whole, fraction)| match fraction {
            Some(fraction) => format!("{sign}{whole}.{fraction}"),
            None => format!("{sign}{whole}"),
        })
}

/// Hostile almost-numbers: exponents, separators, letters, bare punctuation.
fn hostile_amount() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("1e3".to_string()),
        Just("12,50".to_string()),
        Just(".".to_string()),
        Just("+".to_string()),
        Just("-".to_string()),
        Just("".to_string()),
        Just("  ".to_string()),
        Just("$12.50".to_string()),
        Just("12.5.6".to_string()),
        Just("abc".to_string()),
        Just("NaN".to_string()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Well-formed decimals parse verbatim and report their sign honestly;
    /// hostile almost-numbers refuse; display groups under `$` and never
    /// invents digits for non-numeric input.
    #[test]
    #[allow(non_snake_case)]
    fn prop_property_based_008__monetary_arithmetic(
        amount in decimal_text(),
        hostile in hostile_amount(),
    ) {
        // Fixed positives: the documented wire forms.
        let wire_amount = Money::parse(" 125.50 ").unwrap();
        prop_assert_eq!(wire_amount.as_str(), "125.50");
        let zero = Money::parse("0").unwrap();
        prop_assert_eq!(zero.as_str(), "0");
        prop_assert!(Money::parse("0").unwrap().is_non_negative());
        prop_assert!(!Money::parse("-0.01").unwrap().is_non_negative());
        prop_assert!(Money::parse("+4.25").unwrap().is_non_negative());
        // Fixed negatives: not a decimal number, whatever the browser sent.
        prop_assert_eq!(Money::parse("1e3").unwrap_err().code(), "AMOUNT_INVALID");
        prop_assert_eq!(Money::parse("12,50").unwrap_err().code(), "AMOUNT_INVALID");
        prop_assert_eq!(Money::parse("").unwrap_err().code(), "AMOUNT_INVALID");
        prop_assert_eq!(Money::parse(".").unwrap_err().code(), "AMOUNT_INVALID");
        // Fixed display: spreadsheet money — grouped under `$`, always two decimals
        // (rounded to the cent), and passthrough for input with no digits in it.
        prop_assert_eq!(format_money("1250000"), "$1,250,000.00");
        prop_assert_eq!(format_money("1250000.5"), "$1,250,000.50");
        prop_assert_eq!(format_money("not a number"), "not a number");
        prop_assert_eq!(format_money(""), "");

        // Property: a well-formed decimal parses, keeps its trimmed text
        // verbatim, and is non-negative exactly when it has no leading `-`.
        let parsed = Money::parse(&amount).expect("generated decimal must parse");
        prop_assert_eq!(parsed.as_str(), amount.trim());
        prop_assert_eq!(parsed.is_non_negative(), !amount.starts_with('-'));
        // Property: surrounding whitespace is trimmed, never kept.
        let padded = format!("  {amount}  ");
        let padded_parsed = Money::parse(&padded).unwrap();
        prop_assert_eq!(padded_parsed.as_str(), amount.trim());

        // Property: every hostile almost-number refuses with AMOUNT_INVALID.
        prop_assert_eq!(
            Money::parse(&hostile).unwrap_err().code(),
            "AMOUNT_INVALID",
            "hostile amount parsed: {:?}",
            hostile
        );

        // Property: display of a digit string is spreadsheet money over the NUMBER it parses to —
        // the whole part grouped in threes, and always exactly two decimals. `format_money`
        // normalises rather than echoing: leading zeros are the number's, and a third fraction
        // digit rounds (so `999.999` renders `$1,000.00`).
        let digits: String = amount.chars().filter(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && !amount.starts_with('-') && !amount.starts_with('+') {
            let shown = format_money(&amount);
            prop_assert!(shown.starts_with('$'), "display must carry $: {}", shown);
            let whole = shown
                .trim_start_matches('$')
                .split('.')
                .next()
                .expect("a `$`-prefixed display has a whole part");
            let whole_digits = whole.chars().filter(char::is_ascii_digit).count();
            prop_assert_eq!(
                shown.chars().filter(|c| *c == ',').count(),
                whole_digits.saturating_sub(1) / 3,
                "grouping must be threes of the rendered whole part: {}",
                shown
            );
            prop_assert_eq!(
                shown.split('.').nth(1).map(str::len),
                Some(2),
                "money is always two decimals: {}",
                shown
            );
        }
    }
}
