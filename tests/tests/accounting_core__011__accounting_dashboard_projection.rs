//! ACCOUNTING.CORE — Accounting dashboard projection (TST-ACCOUNTING-CORE-011).
//!
//! Contract: the dashboard projection returns the exact aggregates the domain computes:
//! - total receivables (all statuses)
//! - total expenses (all statuses)
//! - income (PAID receivables in period)
//! - cost (POSTED expenses in period)
//! - net (income - cost)
//! All computed in Postgres `numeric` and returned as exact decimal strings.
//!
//! Level: L2 Persistence — the production AccountingDao against an isolated, disposable DEV/Neon target.
//! The harness refuses PRODUCTION before any socket is opened. Fixture rows are named under a unique run
//! marker and deleted at the end; a zero-leftover count is asserted, so DEV is left as it was found.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test accounting_core__011__accounting_dashboard_projection -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable
//! DEV database and the harness will never open a PRODUCTION one.

use db::{AccountingDao, DbFailure, DbTarget};
use model::accounting::{PnlLine, PnlRequest};
use sqlx::PgConnection;
use test_harness::AccountingHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "AccountingHarness/L2 Persistence";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
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

async fn seed_dashboard_fixture(
    harness: &AccountingHarness,
    marker: &str,
) -> Result<(), test_harness::HarnessDbError> {
    // Receivables: PAID in period, OPEN, VOID, PAID outside period
    harness
        .seed_receivable(
            &format!("{marker}-r1"),
            "COMMISSION",
            "25000.00",
            "2026-01-10",
            "PAID",
            Some("2026-01-15"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r2"),
            "LEASING_FEE",
            "3000.00",
            "2026-01-20",
            "PAID",
            Some("2026-01-25"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r3"),
            "COMMISSION",
            "10000.00",
            "2026-01-22",
            "OPEN",
            None,
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r4"),
            "COMMISSION",
            "5000.00",
            "2026-01-23",
            "VOID",
            None,
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r5"),
            "COMMISSION",
            "8000.00",
            "2025-12-28",
            "PAID",
            Some("2025-12-31"),
        )
        .await?;

    // Expenses: POSTED in period, DRAFT, VOID, POSTED outside period
    harness
        .seed_expense(
            &format!("{marker}-e1"),
            "Office",
            "500.00",
            "2026-01-12",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e2"),
            "Marketing & Advertising",
            "1200.00",
            "2026-01-18",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e3"),
            "Office",
            "2000.00",
            "2026-01-24",
            "DRAFT",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e4"),
            "Travel & Entertainment",
            "3000.00",
            "2026-01-26",
            "VOID",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e5"),
            "Office",
            "400.00",
            "2025-12-29",
            "POSTED",
        )
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-011); the file and the assay use it.
async fn accounting_core_011__accounting_dashboard_projection() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the dashboard projection proof runs only on an isolated DEV target"
    );
    let dao: &AccountingDao = harness.dao();
    let marker = format!("TST-ACC011-{}", harness.namespace());

    seed_dashboard_fixture(&harness, &marker)
        .await
        .expect("the January fixture seeds");

    // The period under test is January 2026
    let january = PnlRequest {
        from: "2026-01-01".into(),
        to: "2026-01-31".into(),
    };
    let statement = dao.pnl(&january).await.expect("the dashboard projects");

    // 1. THE PERIOD IS THE REQUEST. Both ends come back exactly as asked.
    assert_eq!(
        (statement.from.as_str(), statement.to.as_str()),
        ("2026-01-01", "2026-01-31"),
        "{HARNESS}: the statement echoes the period it was asked for"
    );

    // 2. INCOME — PAID receivables in the period (25000 + 3000 = 28000)
    assert_eq!(
        statement.total_income.as_str(),
        "28000.00",
        "{HARNESS}: income is the two PAID receivables in the period"
    );
    assert_eq!(
        statement.income.len(),
        2,
        "{HARNESS}: OPEN, VOID and out-of-period receivables contribute no income line"
    );
    assert_eq!(
        amount_of(&statement.income, "COMMISSION"),
        "25000.00",
        "{HARNESS}: commission line amount"
    );
    assert_eq!(
        amount_of(&statement.income, "LEASING_FEE"),
        "3000.00",
        "{HARNESS}: leasing fee line amount"
    );

    // 3. COST — POSTED expenses in the period (500 + 1200 = 1700)
    assert_eq!(
        statement.total_expenses.as_str(),
        "1700.00",
        "{HARNESS}: cost is POSTED expenses only"
    );
    assert_eq!(
        statement.expenses.len(),
        2,
        "{HARNESS}: DRAFT and VOID expenses contribute no cost line"
    );
    assert_eq!(
        amount_of(&statement.expenses, "Office"),
        "500.00",
        "{HARNESS}: office line amount"
    );
    assert_eq!(
        amount_of(&statement.expenses, "Marketing & Advertising"),
        "1200.00",
        "{HARNESS}: marketing line amount"
    );

    // 4. NET — exact decimal arithmetic (28000.00 - 1700.00 = 26300.00)
    assert_eq!(
        statement.net_income.as_str(),
        "26300.00",
        "{HARNESS}: net income is exact difference in numeric"
    );

    // 5. EMPTY PERIOD — February has no rows, totals zero
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

    // 6. CLEANUP / ROLLBACK
    let (receivables, expenses) = harness
        .cleanup(&marker)
        .await
        .expect("the fixture rows are removed");
    assert_eq!(
        (receivables, expenses),
        (5, 5),
        "{HARNESS}: exactly this run's five receivables and five expenses are removed"
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
