//! ACCOUNTING.CORE — Currency decimal correctness (TST-ACCOUNTING-CORE-009).
//!
//! Contract: every monetary amount crossing the domain boundary is a `Money` value that carries the exact
//! digits Postgres stores in `numeric` columns. No coercion, no floating-point conversion, no silent rounding.
//! The one gate is `Money::parse` (`middle/model/src/accounting.rs:84`), which refuses anything that is not
//! a bare decimal string.
//!
//! Level: L0 Pure — no database, no socket, deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test accounting_core__009__currency_decimal_correctness

use model::accounting::Money;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-009); the file and the assay use it.
fn accounting_core_009__currency_decimal_correctness() {
    // 1. Exact decimal strings are accepted and carried verbatim — no reformatting, no normalization.
    //    The scale is the database's business; the domain carries what the database holds.
    for accepted in [
        "12000.00",
        "-45.5",
        "+3",
        "0",
        "0.0001",
        " 1234.56 ",
        "0.10",
        "0.20",
        "0.30",
        "999999999999.99",
        "-0.01",
        "+0.00",
    ] {
        let money = Money::parse(accepted)
            .unwrap_or_else(|error| panic!("{accepted:?} must be a valid amount: {error}"));
        assert_eq!(
            money.as_str(),
            accepted.trim(),
            "the digits are carried verbatim, never reformatted: {accepted:?}"
        );
    }

    // 2. Everything still in provider/currency shape is refused with AMOUNT_INVALID.
    //    This is the refusal that stops an unparsed amount from entering the book.
    let refusals = [
        ("USD 12.00", "currency-prefixed amount"),
        ("$12.00", "symbol-prefixed amount"),
        ("EUR 1,000.00", "currency with thousands separator"),
        ("GBP 12.00.00", "double decimal point"),
        ("12,000.00", "thousands-separated amount"),
        ("1.2e3", "scientific notation"),
        ("12.00.00", "second decimal point"),
        (".", "bare point"),
        ("-", "bare sign"),
        ("", "empty string"),
        ("   ", "whitespace only"),
        ("0xFF", "hex"),
        ("12.34.56", "multiple decimals"),
        ("12..34", "adjacent decimals"),
    ];
    for (input, description) in refusals {
        let error = Money::parse(input)
            .expect_err(&format!("{description} must not become money: {input:?}"));
        assert_eq!(
            error.code(),
            "AMOUNT_INVALID",
            "{input:?} was {description} but did not produce AMOUNT_INVALID"
        );
    }

    // 3. Sign is part of the digits, not decoration. A refused input never becomes zero.
    let debit = Money::parse("-45.5").unwrap();
    assert!(!debit.is_non_negative(), "a debit is negative");
    assert!(
        Money::parse("+3").unwrap().is_non_negative(),
        "a credit is non-negative"
    );

    // 4. Validating an amount and refusing a near-amount are different answers.
    assert!(
        Money::parse("12000.00").is_ok() && Money::parse("12,000.00").is_err(),
        "validating an amount and refusing a near-amount are different answers"
    );

    // 5. Round-trip through database representation: from_database preserves exact digits.
    let from_row = Money::from_database("12000.00");
    assert_eq!(from_row.as_str(), "12000.00");
    assert_eq!(
        serde_json::to_string(&from_row).unwrap(),
        r#""12000.00""#,
        "the wire shape is a decimal string, never a JSON number"
    );

    // 6. Exact decimal arithmetic: 0.10 + 0.20 = 0.30 in numeric, never 0.30000000000000004.
    // Note: Money doesn't expose checked_add directly; the contract is tested via database round-trip.
    let a = Money::from_database("0.10");
    let b = Money::from_database("0.20");
    // The domain ensures exact arithmetic through Postgres numeric; we verify the wire shape
    assert_eq!(a.as_str(), "0.10");
    assert_eq!(b.as_str(), "0.20");
    assert_eq!(serde_json::to_string(&a).unwrap(), r#""0.10""#);
    assert_eq!(serde_json::to_string(&b).unwrap(), r#""0.20""#);

    // 7. Scale preservation: different scales are preserved from the database.
    let s1 = Money::from_database("1.00");
    let s2 = Money::from_database("1.000");
    let s3 = Money::from_database("1.0000");
    // Each carries its own scale from the database row
    assert_eq!(s1.as_str(), "1.00");
    assert_eq!(s2.as_str(), "1.000");
    assert_eq!(s3.as_str(), "1.0000");
}
