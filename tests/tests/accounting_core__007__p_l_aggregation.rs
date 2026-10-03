//! ACCOUNTING.CORE — P&L aggregation (TST-ACCOUNTING-CORE-007).
//!
//! Contract: the profit-and-loss projection for a caller-chosen period is an aggregation over the two canonical
//! tables, computed by Postgres on `numeric`, and it counts **exactly**:
//!
//! - income is `sum(amount)` of receivables whose `status = 'PAID'` and whose `paid_on` falls inside the period
//!   (`INCOME_LINES_SELECT`, `db/src/accounting/recent_expenses_select.rs:95-104`);
//! - cost is `sum(amount)` of expenses whose `status = 'POSTED'` and whose `expense_on` falls inside the period
//!   (`EXPENSE_LINES_SELECT`, `:107-116`);
//! - net income is income minus cost, subtracted by the database in `numeric` (`RANGE_NET_SELECT`, `:119-130`);
//! - the statement echoes the period it was asked for, so a caller can see that the range it requested is the range
//!   it got (`AccountingDao::pnl`, `db/src/accounting/receivable_row.rs:365-385`).
//!
//! THE THRESHOLD IS THE CONTRACT. A receivable that is `OPEN` is not income, a receivable that is `VOID` is not
//! income, and a `PAID` receivable outside the period is not income in this period. An expense that is `DRAFT` or
//! `VOID` is not cost. This file proves the boundary respects all four, because the failure mode that does not
//! announce itself is a projection that quietly sums the wrong rows and prints a plausible number.
//!
//! MONEY IS EXACT. The amounts are `numeric` on both sides; the totals cross back as the digits Postgres holds
//! (`Money`, `middle/model/src/accounting.rs:69-134`). `0.10 + 0.20` in this projection is `0.30`, never
//! `0.30000000000000004`, which is the whole reason no total is a float.
//!
//! Negative/refusal cases: a `DRAFT` expense, a `VOID` expense, an `OPEN` receivable, a `VOID` receivable and rows
//! dated outside the period are all excluded from the totals; an empty period totals `0` rather than nothing; and a
//! backwards or malformed period is REFUSED (`PnlRequest::validate`,
//! `middle/model/src/accounting.rs:296-313`) rather than reported as an empty report.
//!
//! Level: L2 Persistence — the production `AccountingDao` against an isolated, disposable DEV/Neon target. The
//! harness refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`). Fixture rows
//! are named under a unique run marker and deleted at the end; a zero-leftover count is asserted, so DEV is left as
//! it was found.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test accounting_core__007__p_l_aggregation -- --ignored
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

/// Seed the whole fixture: five receivables, seven expenses, across every status and both sides of the period.
///
/// The period under test is January 2026. Every seeded row sits on one side of one of the projection's four
/// thresholds, so the expected totals below can only come out right if the projection respects all of them.
async fn seed_fixture(
    harness: &AccountingHarness,
    marker: &str,
) -> Result<(), test_harness::HarnessDbError> {
    // Income. Two PAID receivables in the period; an OPEN, a VOID and a PAID-in-December must all stay out.
    harness
        .seed_receivable(
            &format!("{marker}-r1"),
            "COMMISSION",
            "12000.00",
            "2026-01-05",
            "PAID",
            Some("2026-01-10"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r2"),
            "LEASING_FEE",
            "1500.00",
            "2026-01-06",
            "PAID",
            Some("2026-01-20"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r3"),
            "COMMISSION",
            "9999.00",
            "2026-01-07",
            "OPEN",
            None,
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r4"),
            "COMMISSION",
            "500.00",
            "2026-01-08",
            "VOID",
            None,
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r5"),
            "COMMISSION",
            "777.00",
            "2025-12-20",
            "PAID",
            Some("2025-12-31"),
        )
        .await?;

    // Cost. Two POSTED expenses in the period (plus two tiny ones to pin exact decimals), a DRAFT, a VOID and a
    // POSTED-in-December must all stay out.
    harness
        .seed_expense(
            &format!("{marker}-e1"),
            "Office",
            "125.50",
            "2026-01-15",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e2"),
            "Marketing & Advertising",
            "74.50",
            "2026-01-16",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e3"),
            "Office",
            "5000.00",
            "2026-01-17",
            "DRAFT",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e4"),
            "Travel & Entertainment",
            "9000.00",
            "2026-01-18",
            "VOID",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e5"),
            "Office",
            "400.00",
            "2025-12-31",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e6"),
            "Office",
            "0.10",
            "2026-01-19",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e7"),
            "Office",
            "0.20",
            "2026-01-20",
            "POSTED",
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-007); the file and the assay use it.
async fn accounting_core_007__p_l_aggregation() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the P&L proof runs only on an isolated DEV target"
    );
    let dao: &AccountingDao = harness.dao();
    let marker = format!("TST-ACC007-{}", harness.namespace());

    seed_fixture(&harness, &marker)
        .await
        .expect("the January fixture seeds");

    // The period the aggregation is asked for. The DAO must echo it back: a projection that silently widened or
    // shifted the range would be reporting a period nobody chose.
    let january = PnlRequest {
        from: "2026-01-01".into(),
        to: "2026-01-31".into(),
    };
    let statement = dao.pnl(&january).await.expect("the P&L projects");

    // 1. THE PERIOD IS THE REQUEST. Both ends come back exactly as asked.
    assert_eq!(
        (statement.from.as_str(), statement.to.as_str()),
        ("2026-01-01", "2026-01-31"),
        "{HARNESS}: the statement echoes the period it was asked for"
    );

    // 2. INCOME — PAID receivables in the period, and nothing else.
    assert_eq!(
        statement.total_income.as_str(),
        "13500.00",
        "{HARNESS}: income is the two PAID receivables in the period (12000.00 + 1500.00)"
    );
    assert_eq!(
        statement.income.len(),
        2,
        "{HARNESS}: the OPEN, VOID and out-of-period receivables contribute no income line"
    );
    assert_eq!(
        amount_of(&statement.income, "COMMISSION"),
        "12000.00",
        "{HARNESS}: the commission line carries its own amount"
    );
    assert_eq!(
        amount_of(&statement.income, "LEASING_FEE"),
        "1500.00",
        "{HARNESS}: the leasing-fee line carries its own amount"
    );

    // 3. COST — POSTED expenses in the period, and nothing else. The DRAFT (5000.00) and the VOID (9000.00) are
    //    excluded, which is the difference between a real profit figure and an alarming one.
    assert_eq!(
        statement.total_expenses.as_str(),
        "200.30",
        "{HARNESS}: cost is POSTED expenses only (125.50 + 74.50 + 0.10 + 0.20)"
    );
    assert_eq!(
        statement.expenses.len(),
        2,
        "{HARNESS}: the DRAFT and VOID expenses contribute no cost line"
    );
    assert_eq!(
        amount_of(&statement.expenses, "Office"),
        "125.80",
        "{HARNESS}: the Office line sums its three POSTED rows exactly (125.50 + 0.10 + 0.20)"
    );
    assert_eq!(
        amount_of(&statement.expenses, "Marketing & Advertising"),
        "74.50",
        "{HARNESS}: the advertising line carries its own amount"
    );

    // 4. NET — income minus cost, subtracted by Postgres in `numeric`. A float sum of the same rows would print
    //    200.30000000000001 / 13299.699999999999, which is the defect this assertion catches.
    assert_eq!(
        statement.net_income.as_str(),
        "13299.70",
        "{HARNESS}: net income is the exact difference, to the cent (13500.00 - 200.30)"
    );

    // 5. NEGATIVE / REFUSAL — a period that runs backwards is a mistake, not an empty report. Returning zeroes
    //    for it would look like a business fact; the domain refuses it before the DAO is ever called.
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
            from: "nonsense".into(),
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

    // 6. NEGATIVE / EMPTY PERIOD — no rows in range totals zero, not NULL and not a missing line. "Nothing was
    //    earned this period" is a real, reportable fact.
    let february = PnlRequest {
        from: "2026-02-01".into(),
        to: "2026-02-28".into(),
    };
    let empty = dao.pnl(&february).await.expect("an empty period projects");
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
        ("2026-02-01", "2026-02-28"),
        "{HARNESS}: the empty statement still echoes its period"
    );

    // 7. COMMITTED TRUTH — the aggregation reads what is committed, and a rolled-back probe changes nothing. The
    //    probe inserts an enormous PAID receivable inside a transaction the harness can only roll back; inside the
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
        "{HARNESS}: a rolled-back probe changes no total — the projection reads committed truth only"
    );

    // 8. CLEANUP / ROLLBACK. This run's fixture rows are deleted; a non-zero leftover count is a failed rollback
    //    and fails the proof, because DEV must be left as it was found.
    let (receivables, expenses) = harness
        .cleanup(&marker)
        .await
        .expect("the fixture rows are removed");
    assert_eq!(
        (receivables, expenses),
        (5, 7),
        "{HARNESS}: exactly this run's five receivables and seven expenses are removed"
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
