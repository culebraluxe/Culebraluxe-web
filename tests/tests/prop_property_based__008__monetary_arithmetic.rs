//! PROP.PROPERTY_BASED — monetary arithmetic (TST-PROP-PROPERTY-BASED-008).
//!
//! Contract: money is a decimal string, never an `f64`. `Money::parse`
//! accepts an optional sign, digits and at most one decimal point — and
//! nothing else (`1e3`, `12,50`, bare `.` all refuse with `AMOUNT_INVALID`);
//! what parses is kept verbatim for the wire; negativity is the leading `-`
//! and nothing else; display groups whole-part digits in threes under `$`
//! while non-numeric input passes through unchanged.
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
        // Fixed display: grouped under `$`, fraction kept verbatim, passthrough.
        prop_assert_eq!(format_money("1250000"), "$1,250,000");
        prop_assert_eq!(format_money("1250000.5"), "$1,250,000.5");
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

        // Property: display of a digit string groups the whole part in
        // threes and keeps the typed fraction verbatim.
        let digits: String = amount.chars().filter(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && !amount.starts_with('-') && !amount.starts_with('+') {
            let shown = format_money(&amount);
            prop_assert!(shown.starts_with('$'), "display must carry $: {}", shown);
            let commas = shown.chars().filter(|c| *c == ',').count();
            let whole_len = amount.split('.').next().unwrap_or("").len();
            prop_assert_eq!(commas, whole_len.saturating_sub(1) / 3);
        }
    }
}
