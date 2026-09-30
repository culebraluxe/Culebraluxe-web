//! ACCOUNTING.CORE — bank transaction normalization (TST-ACCOUNTING-CORE-002).
//!
//! Contract: a line that enters the book from outside — a bank feed, an import, a form — is normalised by exactly
//! three rules and no others: text is **trimmed**, a blank optional becomes **absent** (`trimmed_or_none`,
//! `rust/core/domain/src/accounting.rs:477`) rather than an empty string, and the amount keeps **the digits it was
//! given** (`CreateReceivableCommand::normalised`, `rust/core/domain/src/accounting.rs:417`;
//! `CreateExpenseCommand::normalised`, `:366`).
//!
//! Normalisation is NOT a repair step. It never rounds, re-scales or re-formats an amount — Postgres `numeric` holds
//! what it is given and the module note at `rust/core/domain/src/accounting.rs:7-11` says why (an f64 cannot
//! represent 0.1, and a summary that is a cent out is a summary nobody can reconcile) — and it never rewrites a
//! value it could not validate: a malformed amount is refused by `validate` before `normalised` can store it.
//!
//! The negative case is that ordering: `1,250.00`, `-125.50` and a whitespace-only description are refused, so a
//! line cannot reach the DAO in a shape nobody normalised.
//!
//! Level: L0 Pure — no database, no socket, deterministic.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test accounting_core__002__bank_transaction_normalization

use domain::accounting::{trimmed_or_none, CreateExpenseCommand, CreateReceivableCommand, Money};

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-002); the file and the assay use it.
fn accounting_core_002__bank_transaction_normalization() {
    // 1. A receivable fed from outside: padded text is trimmed, blank optionals become absent, digits survive.
    let command = CreateReceivableCommand {
        reference: Some("  INV-1 ".into()),
        description: "  Commission - 42 Palm ".into(),
        category: "commission".into(),
        amount: " 12000.00 ".into(),
        issued_on: " 2026-03-01 ".into(),
        due_on: Some("   ".into()),
        deal_id: None,
        property_id: Some(" ".into()),
        person_id: None,
    };
    command
        .validate()
        .expect("a well-formed line must validate before it is normalised");

    let normalised = command.normalised();
    assert_eq!(normalised.reference.as_deref(), Some("INV-1"));
    assert_eq!(normalised.description, "Commission - 42 Palm");
    assert_eq!(normalised.amount, "12000.00", "the scale is preserved");
    assert_eq!(normalised.issued_on, "2026-03-01");
    assert_eq!(
        normalised.due_on, None,
        "a blank optional is absent, not an empty string"
    );
    assert_eq!(normalised.property_id, None);
    assert_eq!(
        normalised.category, "COMMISSION",
        "the receivable category rule is kept: uppercased, defaulting to COMMISSION"
    );

    // The blank-optional rule itself, both ways.
    assert_eq!(trimmed_or_none(&Some("  x  ".into())).as_deref(), Some("x"));
    assert_eq!(trimmed_or_none(&Some("   ".into())), None);
    assert_eq!(trimmed_or_none(&None), None);

    // 2. The same three rules on the expense side.
    let expense = CreateExpenseCommand {
        vendor: " Sunrise Fuel ".into(),
        category: "Office".into(),
        amount: "125.50".into(),
        expense_on: " 2026-03-04 ".into(),
        memo: Some("   ".into()),
        ..Default::default()
    };
    expense
        .validate()
        .expect("a well-formed expense must validate");
    let normalised_expense = expense.normalised();
    assert_eq!(normalised_expense.vendor, "Sunrise Fuel");
    assert_eq!(normalised_expense.expense_on, "2026-03-04");
    assert_eq!(normalised_expense.memo, None);

    // 3. Negative cases: what cannot be normalised is refused, in that order.
    let thousands = CreateExpenseCommand {
        amount: "1,250.00".into(),
        ..expense.clone()
    };
    assert_eq!(
        thousands.validate().unwrap_err().code(),
        "AMOUNT_INVALID",
        "a thousands-separated amount is not a decimal"
    );

    let negative = CreateExpenseCommand {
        amount: "-125.50".into(),
        ..expense.clone()
    };
    assert_eq!(
        negative.validate().unwrap_err().code(),
        "AMOUNT_INVALID",
        "an expense is a non-negative amount"
    );

    let blank_description = CreateReceivableCommand {
        description: "   ".into(),
        amount: "1.00".into(),
        issued_on: "2026-03-01".into(),
        ..Default::default()
    };
    assert_eq!(
        blank_description.validate().unwrap_err().code(),
        "DESCRIPTION_REQUIRED",
        "whitespace is not a description"
    );

    // 4. And the digits that did normalise are still the digits, bit for bit.
    assert_eq!(
        Money::parse(&normalised.amount).unwrap().as_str(),
        "12000.00",
        "normalisation never rounds, scales or re-formats an amount"
    );
}
