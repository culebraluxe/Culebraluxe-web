//! ACCOUNTING.CORE — OFX/QBO parsing (TST-ACCOUNTING-CORE-001).
//!
//! Contract: text that arrives from an institution export (OFX, QBO/QuickBooks) is **not money** until it has been
//! reduced to the decimal digits this book stores. The one gate every amount crosses is `Money::parse`
//! (`rust/core/domain/src/accounting.rs:84`), and it REFUSES anything that is not a bare decimal rather than
//! coercing it, defaulting it to zero or truncating it.
//!
//! Where the parse itself lives today, said plainly: **nowhere in Rust.** The only OFX code in the tree is dead
//! TypeScript (`legacy/workflow_app/tests/bank-ofx.test.ts`, `legacy/workflow_app/tests/bank-transaction.test.ts`),
//! which `AGENTS.md` forbids repairing, and no Rust crate carries an `ofx`/`qbo` identifier. Production's ingest
//! boundary is therefore the edge plus this gate: whatever parses the provider's file must hand the domain digits,
//! and the domain refuses everything that is still provider-shaped. That refusal is the invariant a broken importer
//! cannot quietly cross — an element, a currency prefix or a thousands separator cannot become an amount, a zero, or
//! a rounded number on the way in.
//!
//! The negative case is the payload itself: a raw `<STMTTRN>` line, an OFX header, `<TRNAMT>12000.00</TRNAMT>`, a
//! currency-prefixed or thousands-separated string, scientific notation and a second decimal point are all refused
//! with `AMOUNT_INVALID`, and each refusal is asserted so the test cannot pass without exercising the gate.
//!
//! Level: L0 Pure — no database, no socket, deterministic.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test accounting_core__001__ofx_qbo_parsing

use domain::accounting::Money;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-001); the file and the assay use it.
fn accounting_core_001__ofx_qbo_parsing() {
    // 1. What a parser may hand over: the digits Postgres holds, unchanged. The scale is the database's business, so
    //    `12000` is NOT reformatted to `12000.00` and `0.0001` is not rounded away.
    for accepted in ["12000.00", "-45.5", "+3", "0", "0.0001", " 1234.56 "] {
        let money = Money::parse(accepted)
            .unwrap_or_else(|error| panic!("{accepted:?} must be a valid amount: {error}"));
        assert_eq!(
            money.as_str(),
            accepted.trim(),
            "the digits are carried verbatim, never reformatted"
        );
    }

    // 2. Everything still in provider shape is refused. This is the refusal that stops an unparsed statement line
    //    from entering the book as a value nobody can reconcile.
    let refusals = [
        (
            "<STMTTRN><TRNTYPE>DEBIT</TRNTYPE><TRNAMT>-125.50</TRNAMT></STMTTRN>",
            "a raw OFX statement line",
        ),
        ("OFXHEADER:100", "an OFX header"),
        ("<TRNAMT>12000.00</TRNAMT>", "an OFX/QBO amount element"),
        ("USD 12.00", "a currency-prefixed amount"),
        ("$12.00", "a symbol-prefixed amount"),
        ("12,000.00", "a thousands-separated amount"),
        ("1.2e3", "scientific notation"),
        ("12.00.00", "a second decimal point"),
        (".", "a bare point"),
        ("-", "a bare sign"),
        ("", "an empty string"),
        ("   ", "whitespace"),
    ];
    for (input, description) in refusals {
        let error = Money::parse(input)
            .expect_err(&format!("{description} must not become money: {input:?}"));
        assert_eq!(error.code(), "AMOUNT_INVALID", "{input:?}");
    }

    // 3. The sign is part of the digits, not decoration, and a refused input never becomes zero.
    let debit = Money::parse("-45.5").unwrap();
    assert!(!debit.is_non_negative(), "a debit is negative");
    assert!(Money::parse("+3").unwrap().is_non_negative());
    assert!(
        Money::parse("12000.00").is_ok() && Money::parse("12,000.00").is_err(),
        "validating an amount and refusing a near-amount are different answers"
    );

    // 4. A value read back out of a `numeric` column crosses the wire as the same digits
    //    (`Money::from_database`, `rust/core/domain/src/accounting.rs:126`).
    let from_row = Money::from_database("12000.00");
    assert_eq!(from_row.as_str(), "12000.00");
    assert_eq!(
        serde_json::to_string(&from_row).unwrap(),
        r#""12000.00""#,
        "the wire shape is a decimal string, never a JSON number"
    );
}
