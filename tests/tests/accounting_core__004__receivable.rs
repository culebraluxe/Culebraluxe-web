//! ACCOUNTING.CORE — receivable (TST-ACCOUNTING-CORE-004).
//!
//! Contract: a receivable is a description, a non-negative amount and an issue date that exists —
//! `CreateReceivableCommand::validate` (`middle/model/src/accounting.rs:396`). The amount is a decimal string
//! (`Money::parse`, `middle/model/src/accounting.rs:84`), the dates are `YYYY-MM-DD` and nothing else
//! (`date`, `middle/model/src/accounting.rs:486`), and a date that does not exist (`2026-02-30`) is refused
//! rather than rolled forward.
//!
//! The category is the documented exception: unlike expenses it is NOT enforced, because tightening it would reject
//! rows that already exist (`middle/model/src/accounting.rs:42-45`) — `normalise_receivable_category`
//! (`:462`) uppercases what it is given and falls back to `COMMISSION`.
//!
//! The status vocabulary is closed (`RECEIVABLE_STATUSES`, `middle/model/src/accounting.rs:21`), and the database
//! enforces the same three plus "a PAID receivable must say when" (`db/migrations/087_accounting.sql:20-21` and
//! `:32-33`) — the domain's `PAID_ON_INVALID` rule is what keeps that constraint reachable rather than violated.
//!
//! Negative cases: a blank description, a negative amount, a thousands-separated amount, a non-date issue date, an
//! impossible calendar date and an unparseable due date are all refused, each with its own code.
//!
//! Level: L0 Pure — no database, no socket, deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test accounting_core__004__receivable

use model::accounting::{
    normalise_receivable_category, CreateReceivableCommand, RECEIVABLE_CATEGORIES,
    RECEIVABLE_STATUSES,
};

fn command() -> CreateReceivableCommand {
    CreateReceivableCommand {
        reference: Some("INV-1".into()),
        description: "Commission - 42 Palm".into(),
        category: "commission".into(),
        amount: "12000.00".into(),
        issued_on: "2026-03-01".into(),
        due_on: Some("2026-03-31".into()),
        ..Default::default()
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-004); the file and the assay use it.
fn accounting_core_004__receivable() {
    // 1. A well-formed receivable validates, and normalisation trims without touching the digits.
    let ok = command();
    ok.validate()
        .expect("a description, a non-negative amount and a real date are enough");
    let normalised = ok.normalised();
    assert_eq!(normalised.amount, "12000.00");
    assert_eq!(normalised.issued_on, "2026-03-01");
    assert_eq!(normalised.due_on.as_deref(), Some("2026-03-31"));
    assert_eq!(normalised.category, "COMMISSION");

    // 2. Negative cases, one rule each.
    let cases: [(CreateReceivableCommand, &str); 6] = [
        (
            CreateReceivableCommand {
                description: "   ".into(),
                ..command()
            },
            "DESCRIPTION_REQUIRED",
        ),
        (
            CreateReceivableCommand {
                amount: "-0.01".into(),
                ..command()
            },
            "AMOUNT_INVALID",
        ),
        (
            CreateReceivableCommand {
                amount: "12,000.00".into(),
                ..command()
            },
            "AMOUNT_INVALID",
        ),
        (
            CreateReceivableCommand {
                amount: String::new(),
                ..command()
            },
            "AMOUNT_INVALID",
        ),
        (
            CreateReceivableCommand {
                issued_on: "2026-02-30".into(),
                ..command()
            },
            "ISSUED_ON_INVALID",
        ),
        (
            CreateReceivableCommand {
                issued_on: "03/01/2026".into(),
                ..command()
            },
            "ISSUED_ON_INVALID",
        ),
    ];
    for (case, expected) in cases {
        let error = case
            .validate()
            .expect_err(&format!("expected {expected}: {case:?}"));
        assert_eq!(error.code(), expected, "{case:?}");
    }

    // A due date that cannot be parsed is refused too, and it is a different rule from the issue date.
    let bad_due = CreateReceivableCommand {
        due_on: Some("soon".into()),
        ..command()
    };
    assert_eq!(bad_due.validate().unwrap_err().code(), "DUE_ON_INVALID");

    // 3. A blank due date is optional, not invalid — and it normalises to absent.
    let undated = CreateReceivableCommand {
        due_on: Some("   ".into()),
        ..command()
    };
    assert!(undated.validate().is_ok());
    assert_eq!(undated.normalised().due_on, None);

    // 4. The category rule is the seam's, kept: uppercased, defaulting to COMMISSION (never enforced).
    assert_eq!(
        normalise_receivable_category(" leasing_fee "),
        "LEASING_FEE"
    );
    assert_eq!(normalise_receivable_category("   "), "COMMISSION");
    assert_eq!(normalise_receivable_category("Anything"), "ANYTHING");
    assert_eq!(
        RECEIVABLE_CATEGORIES,
        ["COMMISSION", "LEASING_FEE", "MISC_INCOME", "OTHER"]
    );

    // 5. The closed status vocabulary the row is checked against in migration 087.
    assert_eq!(RECEIVABLE_STATUSES, ["OPEN", "PAID", "VOID"]);
}
