//! ACCOUNTING.CORE — date filtering (TST-ACCOUNTING-CORE-008).
//!
//! Contract: the P&L projection is a **date filter**, and the date it filters on is the business date of each side,
//! not a row's creation time and not a date that merely happens to correlate:
//!
//! - income is the receivables whose `paid_on` falls inside `[from, to]` — the date the money was actually received,
//!   NOT the date the receivable was issued (`INCOME_LINES_SELECT`,
//!   `db/src/accounting/recent_expenses_select.rs:95-104`);
//! - cost is the expenses whose `expense_on` falls inside `[from, to]` (`EXPENSE_LINES_SELECT`, `:107-116`);
//! - BOTH ENDS ARE INCLUSIVE. `paid_on >= $1::date and paid_on <= $2::date` is the boundary production applies, so a
//!   row dated exactly `from` or exactly `to` is in the period and a row one day outside is not.
//! - the projection echoes the period it was asked for (`AccountingDao::pnl`,
//!   `db/src/accounting/receivable_row.rs:365-385`), so a screen that asked for a window can see it got that
//!   window and not a widened one.
//!
//! WHY THIS IS ITS OWN CONTRACT. A date filter is the place a projection goes quietly wrong: it clamps an inclusive
//! end to exclusive (`<` for `<=`), it filters on `issued_on` because that is the column the row carries, or it keeps
//! an out-of-period row because a status check passed. None of those announce themselves — the screen prints a
//! plausible number for the wrong rows. This file proves the boundary by placing a row exactly on each edge, a row on
//! each side of each edge, and a row whose issue date and paid date disagree about which side of the period it is on.
//!
//! THE DISAGREEING ROW IS THE COLUMN PROOF. `{marker}-r-in-from` is issued in December but paid on the first day of
//! the period, and `{marker}-r-out-before` is issued inside the period but paid in December. The projection must count
//! the first as income and drop the second — which it can only do by filtering income on `paid_on`, not `issued_on`.
//!
//! Negative/refusal cases: a PAID receivable dated outside the period, an OPEN and a VOID receivable inside it, a
//! POSTED expense dated outside, and a DRAFT and a VOID expense inside it all contribute nothing; a period with no
//! rows totals `0` rather than nothing; and a backwards or malformed period is REFUSED
//! (`PnlRequest::validate`, `middle/model/src/accounting.rs:296-313`) rather than reported as an empty report.
//!
//! Level: L2 Persistence — the production `AccountingDao` against an isolated, disposable DEV/Neon target. The
//! harness refuses PRODUCTION before any socket is opened (`rust/test-harness/src/database.rs:68-75`). Fixture rows
//! are named under a unique run marker and deleted at the end; a zero-leftover count is asserted, so DEV is left as
//! it was found.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test accounting_core__008__date_filtering -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{AccountingDao, DbFailure, DbTarget};
use model::accounting::{PnlLine, PnlRequest};
use sqlx::PgConnection;
use test_harness::AccountingHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "AccountingHarness/L2 Persistence";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// Infrastructure, not the contract: `TestDatabase` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> AccountingHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match AccountingHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; the harness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

/// The amount on the line labelled `label`, or a marker that makes a missing line visible.
fn amount_of(lines: &[PnlLine], label: &str) -> String {
    lines
        .iter()
        .find(|line| line.label == label)
        .map(|line| line.amount.as_str().to_owned())
        .unwrap_or_else(|| format!("<no {label} line>"))
}

/// True when `lines` carries a line labelled `label`.
fn has_line(lines: &[PnlLine], label: &str) -> bool {
    lines.iter().any(|line| line.label == label)
}

/// Seed the date-filter fixture: six receivables and six expenses, one on each edge and each side of the period.
///
/// The period under test is January 2026. Every seeded row sits on exactly one side of one of the filter's
/// thresholds — the `from` edge, the `to` edge, the status gate, or the issue-vs-paid date disagreement — so the
/// expected totals can only come out right if the projection's date filter is respected on both ends and on the
/// correct column.
async fn seed_fixture(
    harness: &AccountingHarness,
    marker: &str,
) -> Result<(), test_harness::HarnessDbError> {
    // Income — filter is on `paid_on`, inclusive both ends. The first two are the boundary rows; the next two are one
    // day outside on each side; the last two are OPEN/VOID inside the period and are not income at all.
    //
    // r-in-from proves the COLUMN: issued in December, paid on the period's first day, so it is in only if the filter
    // reads `paid_on`. r-out-before proves the same from the other side: issued inside the period, paid in December.
    harness
        .seed_receivable(
            &format!("{marker}-r-in-from"),
            "COMMISSION",
            "100.00",
            "2025-12-01",
            "PAID",
            Some("2026-01-01"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r-in-to"),
            "LEASING_FEE",
            "200.00",
            "2026-01-15",
            "PAID",
            Some("2026-01-31"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r-out-before"),
            "COMMISSION",
            "1000.00",
            "2026-01-10",
            "PAID",
            Some("2025-12-31"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r-out-after"),
            "COMMISSION",
            "2000.00",
            "2026-01-10",
            "PAID",
            Some("2026-02-01"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r-open"),
            "COMMISSION",
            "4000.00",
            "2026-01-10",
            "OPEN",
            None,
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r-void"),
            "COMMISSION",
            "8000.00",
            "2026-01-10",
            "VOID",
            None,
        )
        .await?;

    // Cost — filter is on `expense_on`, inclusive both ends. Two boundary rows, two outside, a DRAFT and a VOID.
    harness
        .seed_expense(
            &format!("{marker}-e-in-from"),
            "Office",
            "10.00",
            "2026-01-01",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e-in-to"),
            "Office",
            "20.00",
            "2026-01-31",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e-out-before"),
            "Office",
            "1000.00",
            "2025-12-31",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e-out-after"),
            "Office",
            "2000.00",
            "2026-02-01",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e-draft"),
            "Office",
            "4000.00",
            "2026-01-15",
            "DRAFT",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e-void"),
            "Office",
            "8000.00",
            "2026-01-15",
            "VOID",
        )
        .await?;
    Ok(())
}

/// Insert a PAID receivable inside `conn`'s transaction, so the caller can see it and then roll it back.
async fn probe_insert_receivable(
    conn: &mut PgConnection,
    reference: &str,
) -> Result<(), DbFailure> {
    sqlx::query(
        "insert into account_receivable (reference, description, category, amount, issued_on, status, paid_on)
         values ($1, $1, 'COMMISSION', 1000000.00::numeric, '2026-01-15'::date, 'PAID', '2026-01-15'::date)",
    )
    .bind(reference)
    .execute(&mut *conn)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.accounting.probe_insert", &error))?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-008); the file and the assay use it.
async fn accounting_core_008__date_filtering() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the date-filter proof runs only on an isolated DEV target"
    );
    let dao: &AccountingDao = harness.dao();
    let marker = format!("TST-ACC008-{}", harness.namespace());

    seed_fixture(&harness, &marker)
        .await
        .expect("the January date-filter fixture seeds");

    // The period the filter is asked for. Both ends inclusive.
    let january = PnlRequest {
        from: "2026-01-01".into(),
        to: "2026-01-31".into(),
    };
    let statement = dao.pnl(&january).await.expect("the P&L projects");

    // 1. THE PERIOD IS THE REQUEST. Both ends come back exactly as asked: a filter that clamped or widened the window
    //    would be reporting a period nobody chose.
    assert_eq!(
        (statement.from.as_str(), statement.to.as_str()),
        ("2026-01-01", "2026-01-31"),
        "{HARNESS}: the statement echoes the period it was asked for"
    );

    // 2. THE `from` EDGE IS INCLUSIVE. A row dated exactly 2026-01-01 is in the period, on both sides.
    assert!(
        has_line(&statement.income, "COMMISSION"),
        "{HARNESS}: a receivable PAID on the first day of the period is income (the `from` edge is inclusive)"
    );
    assert_eq!(
        amount_of(&statement.income, "COMMISSION"),
        "100.00",
        "{HARNESS}: the boundary income row is counted at its own amount"
    );
    assert_eq!(
        amount_of(&statement.expenses, "Office"),
        "30.00",
        "{HARNESS}: an expense dated exactly on the `from` and `to` edges is cost (10.00 + 20.00)"
    );

    // 3. THE `to` EDGE IS INCLUSIVE. A receivable PAID on the last day of the period is income, and the expense on the
    //    same day is already folded into the Office line above (10.00 + 20.00 = 30.00).
    assert!(
        has_line(&statement.income, "LEASING_FEE"),
        "{HARNESS}: a receivable PAID on the last day of the period is income (the `to` edge is inclusive)"
    );
    assert_eq!(
        amount_of(&statement.income, "LEASING_FEE"),
        "200.00",
        "{HARNESS}: the closing boundary row is counted at its own amount"
    );

    // 4. THE COLUMN IS THE BUSINESS DATE, NOT THE ISSUE DATE. These two rows disagree about which side of the period
    //    they are on: one is issued in December and paid inside, the other is issued inside and paid in December. The
    //    projection must include the first and exclude the second, which it can only do by filtering `paid_on`.
    assert_eq!(
        statement.total_income.as_str(),
        "300.00",
        "{HARNESS}: income is the two PAID receivables inside the period (100.00 + 200.00), \
         not the issued-inside/paid-outside row"
    );
    assert_eq!(
        statement.income.len(),
        2,
        "{HARNESS}: the OPEN, VOID and out-of-period receivables contribute no income line"
    );

    // 5. OUTSIDE THE PERIOD IS OUT, AND STATUS IS NOT A SUBSTITUTE FOR THE DATE. The two PAID-but-outside rows
    //    (1000.00 + 2000.00) are excluded by the date filter, and the OPEN (4000.00) and VOID (8000.00) rows inside
    //    the period are excluded by status — four distinct ways to be the wrong row, none of them in the totals. The
    //    DRAFT (4000.00) and VOID (8000.00) expenses inside the period are likewise excluded from cost.
    assert_eq!(
        statement.total_expenses.as_str(),
        "30.00",
        "{HARNESS}: cost is POSTED expenses inside the period only (10.00 + 20.00), \
         not the outside, DRAFT or VOID rows"
    );
    assert_eq!(
        statement.expenses.len(),
        1,
        "{HARNESS}: the outside, DRAFT and VOID expenses contribute no cost line"
    );

    // 6. NET IS THE FILTERED DIFFERENCE, subtracted by Postgres in `numeric`.
    assert_eq!(
        statement.net_income.as_str(),
        "270.00",
        "{HARNESS}: net income is the filtered difference, to the cent (300.00 - 30.00)"
    );

    // 7. NEGATIVE / EMPTY PERIOD — narrowing the window past every row totals zero, not NULL and not a full-book sum.
    //    A filter that ignored an edge would still show the boundary rows here; this is the narrowing proof.
    let march = PnlRequest {
        from: "2026-03-01".into(),
        to: "2026-03-31".into(),
    };
    let empty = dao.pnl(&march).await.expect("an empty period projects");
    assert!(
        empty.income.is_empty() && empty.expenses.is_empty(),
        "{HARNESS}: a period with no rows has no lines"
    );
    assert_eq!(
        (
            empty.total_income.as_str(),
            empty.total_expenses.as_str(),
            empty.net_income.as_str()
        ),
        ("0", "0", "0"),
        "{HARNESS}: an empty period totals zero rather than nothing"
    );
    assert_eq!(
        (empty.from.as_str(), empty.to.as_str()),
        ("2026-03-01", "2026-03-31"),
        "{HARNESS}: the empty statement still echoes its period"
    );

    // 8. NEGATIVE / REFUSAL — a period that runs backwards is a mistake, not an empty report, and a date that is not a
    //    date cannot be a filter bound at all. The domain refuses both before the DAO is ever called; returning zeroes
    //    or binding garbage would look like a business fact.
    assert_eq!(
        PnlRequest {
            from: "2026-04-01".into(),
            to: "2026-03-31".into(),
        }
        .validate()
        .expect_err("a backwards period must be refused")
        .code(),
        "PNL_RANGE_INVALID",
        "{HARNESS}: a backwards period is refused, not silently emptied"
    );
    assert_eq!(
        PnlRequest {
            from: "not-a-date".into(),
            to: "2026-03-31".into(),
        }
        .validate()
        .expect_err("an unparseable start must be refused")
        .code(),
        "PNL_FROM_INVALID",
        "{HARNESS}: the start date has to be a date"
    );
    assert_eq!(
        PnlRequest {
            from: "2026-01-01".into(),
            to: "2026-13-40".into(),
        }
        .validate()
        .expect_err("an impossible end must be refused")
        .code(),
        "PNL_TO_INVALID",
        "{HARNESS}: the end date has to be a date"
    );

    // 9. COMMITTED TRUTH — the filter reads what is committed, and a rolled-back probe changes nothing. The probe
    //    inserts an enormous in-period PAID receivable inside a transaction the harness can only roll back; inside the
    //    transaction it is visible, after the rollback the pool and the projection see the committed fixture again.
    let probe = format!("{marker}-probe");
    let before = dao.pnl(&january).await.expect("the committed P&L projects");
    let probe_reference = probe.clone();
    let visible_inside = harness
        .database()
        .with_rollback(move |conn| {
            Box::pin(async move {
                probe_insert_receivable(conn, &probe_reference).await?;
                let count: i64 = sqlx::query_scalar(
                    "select count(*) from account_receivable where reference = $1",
                )
                .bind(&probe_reference)
                .fetch_one(&mut *conn)
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx("test-harness.accounting.probe_read", &error)
                })?;
                Ok(count)
            })
        })
        .await
        .expect("the rolled-back probe runs");
    assert_eq!(
        visible_inside, 1,
        "{HARNESS}: inside its own transaction the probe sees the row it inserted"
    );
    let after = dao
        .pnl(&january)
        .await
        .expect("the P&L projects after rollback");
    assert_eq!(
        after, before,
        "{HARNESS}: a rolled-back probe changes no total — the filter reads committed truth only"
    );

    // 10. CLEANUP / ROLLBACK. This run's fixture rows are deleted; a non-zero leftover count is a failed rollback and
    //     fails the proof, because DEV must be left as it was found.
    let (receivables, expenses) = harness
        .cleanup(&marker)
        .await
        .expect("the fixture rows are removed");
    assert_eq!(
        (receivables, expenses),
        (6, 6),
        "{HARNESS}: exactly this run's six receivables and six expenses are removed"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("the leftover count reads"),
        0,
        "{HARNESS}: the proof leaves no accounting row behind"
    );
}
