//! ACCOUNTING.CORE — duplicate import (TST-ACCOUNTING-CORE-006).
//!
//! Contract: **this layer does not dedupe, and must not pretend to.** Identity in the accounting book is the row id
//! alone — `id uuid primary key default gen_random_uuid()` (`db/migrations/087_accounting.sql:11`) — and the imported
//! half is not an identity: `reference text` (`db/migrations/087_accounting.sql:13`) has no unique constraint, and
//! `account_expense` has none either (`db/migrations/087_accounting.sql:31-40`). The DAO writes with a plain
//! `insert ... returning id::text` and **no** `on conflict` (`rust/core/db/src/accounting/receivable_row.rs:418-426`;
//! the expense path at `:392-400`). So a statement line imported twice is two rows unless the importer decides
//! otherwise before it calls this boundary — which is the truth worth pinning, because the opposite belief (that the
//! domain silently absorbs a re-import) is how a real invoice disappears.
//!
//! There is no importer in Rust to test: the only bank/import code in the tree is dead TypeScript
//! (`legacy/workflow_app/tests/bank-transaction.test.ts`), which `AGENTS.md` forbids repairing. What exists, and what
//! this file proves, is the command boundary the importer must use: a command carries no id at all
//! (`CreateReceivableCommand`, `rust/core/domain/src/accounting.rs:383-393`), two identical imports normalise to
//! byte-identical commands, and no rule folds or fuzzes content into a key.
//!
//! Negative case: `reference` is not case-folded (`INV-1` and `inv-1` stay two different references — two genuinely
//! different invoices must never be quietly merged), equal amounts on different dates are not the same line, and a
//! duplicated line still VALIDATES: a duplicate is the writer's conflict to detect, not a validation error, and a
//! test that expected a refusal here would be inventing a dedupe that does not exist.
//!
//! Level: L0 Pure — no database, no socket, deterministic.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test accounting_core__006__duplicate_import

use domain::accounting::{CreateExpenseCommand, CreateReceivableCommand};

fn line() -> CreateReceivableCommand {
    CreateReceivableCommand {
        reference: Some("INV-1".into()),
        description: "Commission - 42 Palm".into(),
        category: "COMMISSION".into(),
        amount: "12000.00".into(),
        issued_on: "2026-03-01".into(),
        due_on: Some("2026-03-31".into()),
        ..Default::default()
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-006); the file and the assay use it.
fn accounting_core_006__duplicate_import() {
    // 1. Imported twice, the same line yields the same command: nothing at this boundary distinguishes a re-import,
    //    so the decision to treat it as one cannot be made here — and is not made silently.
    let first = line();
    let second = line();
    first.validate().expect("the first import is a valid line");
    second.validate().expect("the re-import is just as valid");
    assert_eq!(
        first.normalised(),
        second.normalised(),
        "identical input normalises identically; the layer derives no identity from content"
    );

    // 2. The command carries no id, so the row that a duplicate would collide with is minted by the database
    //    (`returning id::text`, receivable_row.rs:400/426) — the payload cannot name an existing receivable at all.
    let normalised = first.normalised();
    assert!(
        normalised.reference.is_some() && normalised.amount.as_str() == "12000.00",
        "what the import carries is content, and content is not a key"
    );

    // 3. Negative case: nothing folds content into a key. A reference differing only in case stays a different
    //    reference, so two real invoices are never merged by normalisation.
    let upper = CreateReceivableCommand {
        reference: Some("INV-1".into()),
        ..line()
    };
    let lower = CreateReceivableCommand {
        reference: Some("inv-1".into()),
        ..line()
    };
    assert_ne!(
        upper.normalised().reference,
        lower.normalised().reference,
        "reference is carried, not canonicalised"
    );

    // 4. Equal amounts on different lines are different lines: no collapsing by amount.
    let same_amount_other_date = CreateReceivableCommand {
        issued_on: "2026-03-02".into(),
        ..line()
    };
    assert_ne!(
        line().normalised().issued_on,
        same_amount_other_date.normalised().issued_on
    );
    assert_eq!(
        line().normalised().amount,
        same_amount_other_date.normalised().amount
    );

    // 5. The expense side behaves the same way: two identical expenses are indistinguishable here.
    let expense = CreateExpenseCommand {
        vendor: "Sunrise Fuel".into(),
        category: "Office".into(),
        amount: "125.50".into(),
        expense_on: "2026-03-04".into(),
        ..Default::default()
    };
    let reimported = CreateExpenseCommand {
        amount: " 125.50 ".into(),
        ..expense.clone()
    };
    assert_eq!(expense.normalised(), reimported.normalised());
    reimported
        .validate()
        .expect("a re-imported expense is a valid expense, not a validation error");
}
