//! ACCOUNTING.CORE — mark paid (TST-ACCOUNTING-CORE-005).
//!
//! Contract: marking a receivable paid requires a receivable and a real date, and nothing else —
//! `MarkReceivablePaidCommand::validate` (`middle/model/src/accounting.rs:442`). A blank id is refused
//! (`RECEIVABLE_REQUIRED`) so a malformed id never reaches the DAO, where the id is compared as text
//! (`where id::text = $1`, `db/src/accounting/receivable_row.rs:462`) precisely so "no such receivable" is a
//! conflict rather than a cast error reported as a 500. A blank or unparseable date is refused (`PAID_ON_INVALID`)
//! before the DAO's `$2::date` bind can fail — and that is what keeps the two database rules reachable rather than
//! violated: `check (status <> 'PAID' or paid_on is not null)` (`db/migrations/087_accounting.sql:32-33`) and the
//! closed status list (`db/migrations/087_accounting.sql:20-21`).
//!
//! Whether the transition is ALLOWED is the database's answer, not the caller's: the DAO transitions in one statement
//! with `status <> 'VOID'` in its WHERE clause (`db/src/accounting/receivable_row.rs:462`), so a voided
//! receivable and a missing one both come back as a conflict. This file proves the half that is pure — the command's
//! own refusals and the shape of the outcome the screen reports.
//!
//! Negative cases: no receivable, no date, a timestamp instead of a date, a locale date, an impossible calendar date,
//! and the word "yesterday" — each refused with its own code.
//!
//! Level: L0 Pure — no database, no socket, deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test accounting_core__005__mark_paid

use model::accounting::{
    MarkReceivablePaidCommand, MarkReceivablePaidOutcome, RECEIVABLE_STATUSES,
};

const RECEIVABLE_ID: &str = "11111111-1111-1111-1111-111111111111";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-005); the file and the assay use it.
fn accounting_core_005__mark_paid() {
    // 1. A well-formed mark-paid, padded the way a form sends it.
    let command = MarkReceivablePaidCommand {
        receivable_id: format!("  {RECEIVABLE_ID}  "),
        paid_on: " 2026-03-10 ".into(),
    };
    command.validate().expect("an id and a date are enough");
    let normalised = command.normalised();
    assert_eq!(normalised.receivable_id, RECEIVABLE_ID);
    assert_eq!(normalised.paid_on, "2026-03-10");

    // 2. Negative case: no receivable, or no real date.
    assert_eq!(
        MarkReceivablePaidCommand {
            receivable_id: "   ".into(),
            paid_on: "2026-03-10".into(),
        }
        .validate()
        .unwrap_err()
        .code(),
        "RECEIVABLE_REQUIRED"
    );

    for not_a_date in [
        "",
        "   ",
        "2026-03-10T00:00:00Z",
        "10/03/2026",
        "2026-02-30",
        "yesterday",
    ] {
        let error = MarkReceivablePaidCommand {
            receivable_id: RECEIVABLE_ID.into(),
            paid_on: not_a_date.into(),
        }
        .validate()
        .expect_err(&format!("{not_a_date:?} is not a paid date"));
        assert_eq!(error.code(), "PAID_ON_INVALID", "{not_a_date:?}");
    }

    // 3. The outcome the screen reads is the paid row: a status from the closed list, and a date that is present —
    //    which is exactly what `check (status <> 'PAID' or paid_on is not null)` demands of the row.
    let outcome = MarkReceivablePaidOutcome {
        id: normalised.receivable_id.clone(),
        status: "PAID".into(),
        paid_on: normalised.paid_on.clone(),
    };
    assert!(
        RECEIVABLE_STATUSES.contains(&outcome.status.as_str()),
        "a transition lands on one of the three statuses"
    );
    assert!(
        !outcome.paid_on.trim().is_empty(),
        "a paid receivable always says when"
    );
    assert_eq!(
        serde_json::to_string(&outcome).unwrap(),
        format!(r#"{{"id":"{RECEIVABLE_ID}","status":"PAID","paidOn":"2026-03-10"}}"#),
        "camelCase on the wire, the shape the screen already parses"
    );
}
