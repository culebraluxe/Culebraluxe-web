//! ACCOUNTING.CORE — expense categorization (TST-ACCOUNTING-CORE-003).
//!
//! Contract: an expense carries one of the controlled categories, and membership is **exact** —
//! `EXPENSE_CATEGORIES` (`rust/core/domain/src/accounting.rs:30`), enforced by
//! `CreateExpenseCommand::validate` (`rust/core/domain/src/accounting.rs:347-353`) and readable through
//! `is_expense_category` (`rust/core/domain/src/accounting.rs:472`). A closed list rather than a category table, and
//! a spelling the list does not carry is **rejected**, not stored and not repaired to its nearest neighbour — the
//! behaviour the TypeScript seam had, kept deliberately.
//!
//! Normalisation does not launder a category: `CreateExpenseCommand::normalised`
//! (`rust/core/domain/src/accounting.rs:366`) copies the category through, so the only way a category reaches a row
//! is by having been validated exactly.
//!
//! The negative case is every near miss — lower case, upper case, a trailing space, an abbreviation, the empty
//! string — each refused with `EXPENSE_CATEGORY_INVALID`. Expenses also have a closed status vocabulary
//! (`EXPENSE_STATUSES`, `rust/core/domain/src/accounting.rs:23`), mirrored by the `check (status in ('DRAFT',
//! 'POSTED', 'VOID'))` constraint in `db/migrations/087_accounting.sql:31-35`.
//!
//! Level: L0 Pure — no database, no socket, deterministic.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test accounting_core__003__expense_categorization

use domain::accounting::{
    is_expense_category, CreateExpenseCommand, EXPENSE_CATEGORIES, EXPENSE_STATUSES,
};

fn command(category: &str) -> CreateExpenseCommand {
    CreateExpenseCommand {
        vendor: "Sunrise Fuel".into(),
        category: category.into(),
        amount: "125.50".into(),
        expense_on: "2026-03-04".into(),
        ..Default::default()
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-003); the file and the assay use it.
fn accounting_core_003__expense_categorization() {
    // 1. The list is the whole list, and every member is accepted — a category the UI offers must not be refused.
    assert_eq!(EXPENSE_CATEGORIES.len(), 9);
    for category in EXPENSE_CATEGORIES {
        assert!(
            is_expense_category(category),
            "{category:?} is a canonical category"
        );
        command(category)
            .validate()
            .unwrap_or_else(|error| panic!("{category:?} must validate: {error}"));
    }

    // 2. Negative case: membership is exact, so a near miss is refused rather than silently filed under something
    //    else. The trailing-space entry is the one that proves there is no trimming in the check.
    for near_miss in [
        "marketing & advertising",
        "MARKETING & ADVERTISING",
        "Marketing",
        "Office ",
        " Office",
        "other",
        "MERCHANT / BANK FEES",
        "Consulting",
        "",
    ] {
        assert!(
            !is_expense_category(near_miss),
            "{near_miss:?} must not be canonical"
        );
        let error = command(near_miss)
            .validate()
            .expect_err("a non-canonical category must be refused");
        assert_eq!(error.code(), "EXPENSE_CATEGORY_INVALID", "{near_miss:?}");
    }

    // 3. Normalisation copies the category; it never rewrites it to a canonical spelling.
    assert_eq!(command("Marketing").normalised().category, "Marketing");

    // 4. The status vocabulary the categorization is stored under, mirrored by the CHECK in migration 087.
    assert_eq!(EXPENSE_STATUSES, ["DRAFT", "POSTED", "VOID"]);
}
