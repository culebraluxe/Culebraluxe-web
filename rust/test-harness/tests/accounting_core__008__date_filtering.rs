//! ACCOUNTING.CORE — date filtering (TST-ACCOUNTING-CORE-008).
//!
//! Contract: the P&L period is a **filter**, and the filter is exactly the range the caller named — no wider, no
//! shifted, no clamped. It selects each side of the book by the date that side is settled on:
//!
//! - income is `paid_on` inside `[from, to]` (`INCOME_LINES_SELECT`,
//!   `rust/core/db/src/accounting/recent_expenses_select.rs:95-104`) — **not** `issued_on`;
//! - cost is `expense_on` inside `[from, to]` (`EXPENSE_LINES_SELECT`, `:107-116`) — **not** `created_at`;
//! - both endpoints are **inclusive** (`>= $1::date and <= $2::date`), so a row dated exactly `from` or exactly `to`
//!   is in the period and a row one day outside it is not (`RANGE_NET_SELECT`, `:119-130`).
//!
//! WHY THIS IS A SEPARATE CONTRACT. Aggregation (TST-ACCOUNTING-CORE-007) proves the arithmetic and the status
//! thresholds. This file proves the *predicate*: the two ways a date filter goes quietly wrong are (a) an endpoint
//! that should have been inclusive being treated as exclusive — the boundary row vanishes from a statement that still
//! looks plausible — and (b) filtering on the wrong column, so a receivable issued in the period but paid in the next
//! one is reported as this period's income. Both failures print a number; neither announces itself.
//!
//! MONEY IS EXACT. The totals are Postgres `numeric` sums read back as `::text` (`Money`,
//! `rust/core/domain/src/accounting.rs:76-134`); this file's sums are chosen so a float implementation would show.
//!
//! Negative/refusal cases: a receivable paid one day before `from` or one day after `to` contributes no income, a
//! receivable *issued* in the period but paid outside it contributes no income, an `OPEN` receivable contributes no
//! income, an expense dated one day outside the period or dated long before it (but created now) contributes no cost,
//! a `DRAFT` expense contributes no cost, a period with no rows totals zero rather than nothing, and a backwards or
//! malformed range is REFUSED (`PnlRequest::validate`, `rust/core/domain/src/accounting.rs:296-313`) rather than
//! silently emptied or reinterpreted.
//!
//! THE PERIOD IS FAR OUTSIDE CANONICAL HISTORY, ON PURPOSE. The P&L sums the whole table for the range, so the test
//! picks a period no operator's book can reach and asserts it is empty *before* it seeds — a boundary-true period is
//! the only way the exact totals below are the fixture alone.
//!
//! Level: L2 Persistence — the production `AccountingDao` against an isolated, disposable DEV/Neon target. The
//! harness refuses PRODUCTION before any socket is opened (`rust/test-harness/src/database.rs:68-75`). Fixture rows
//! are named under a unique run marker and deleted at the end; a zero-leftover count is asserted, so DEV is left as
//! it was found.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path rust/Cargo.toml -p test-harness \
//!     --test accounting_core__008__date_filtering -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{AccountingDao, DbFailure, DbTarget};
use domain::accounting::{PnlLine, PnlRequest};
use sqlx::PgConnection;
use test_harness::AccountingHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "AccountingHarness/L2 Persistence";

/// The period under test: a month far outside any operator's book, so the whole-table SUM is this fixture alone.
const FROM: &str = "2099-03-01";
const TO: &str = "2099-03-31";

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

/// The period the test asks for.
fn period() -> PnlRequest {
    PnlRequest {
        from: FROM.into(),
        to: TO.into(),
    }
}

/// Seed the whole fixture: six receivables and six expenses, each sitting on one side of one of the filter's
/// boundaries. Every row here is placed so the exact totals below can only come out right if the filter is inclusive
/// at both ends and keyed on the settled-on date of each side.
async fn seed_fixture(
    harness: &AccountingHarness,
    marker: &str,
) -> Result<(), test_harness::HarnessDbError> {
    // INCOME — filtered on `paid_on`. R1 and R2 are the inclusive boundary rows (R1 also proves `issued_on` is
    // ignored: it was issued in 2098). R3 and R4 are one day outside each end. R5 is the wrong-column trap: issued
    // inside the period but paid in the next month. R6 is OPEN, so it is not income at all.
    harness
        .seed_receivable(
            &format!("{marker}-r1"),
            "COMMISSION",
            "100.00",
            "2098-12-15",
            "PAID",
            Some(FROM),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r2"),
            "LEASING_FEE",
            "200.00",
            "2099-04-15",
            "PAID",
            Some(TO),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r3"),
            "COMMISSION",
            "400.00",
            "2099-03-15",
            "PAID",
            Some("2099-02-28"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r4"),
            "COMMISSION",
            "800.00",
            "2099-03-01",
            "PAID",
            Some("2099-04-01"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r5"),
            "MISC_INCOME",
            "1600.00",
            "2099-03-10",
            "PAID",
            Some("2099-04-10"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r6"),
            "COMMISSION",
            "3200.00",
            "2099-03-05",
            "OPEN",
            None,
        )
        .await?;

    // COST — filtered on `expense_on`. E1 and E2 are the inclusive boundary rows. E3 and E4 are one day outside each
    // end. E5 is dated long before the period but created now: if the filter used `created_at` it would leak in. E6 is
    // DRAFT, so it is not cost at all.
    harness
        .seed_expense(&format!("{marker}-e1"), "Office", "10.00", FROM, "POSTED")
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e2"),
            "Marketing & Advertising",
            "20.00",
            TO,
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e3"),
            "Office",
            "40.00",
            "2099-02-28",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e4"),
            "Office",
            "80.00",
            "2099-04-01",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e5"),
            "Travel & Entertainment",
            "160.00",
            "2098-01-01",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e6"),
            "Office",
            "320.00",
            "2099-03-15",
            "DRAFT",
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
         values ($1, $1, 'COMMISSION', 1000000.00::numeric, '2099-03-15'::date, 'PAID', '2099-03-15'::date)",
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

    // 1. THE FILTER IS BOUND, NOT ASSUMED. Before seeding, the period must be empty, or its totals below would be
    //    somebody else's book. A boundary-true period is the precondition for an exact assertion.
    let before_seed = dao.pnl(&period()).await.expect("the P&L projects");
    assert!(
        before_seed.income.is_empty() && before_seed.expenses.is_empty(),
        "{HARNESS}: the chosen period must be empty before seeding, so the totals are this fixture alone; \
         found {} income and {} expense lines",
        before_seed.income.len(),
        before_seed.expenses.len()
    );
    assert_eq!(
        (
            before_seed.total_income.as_str(),
            before_seed.total_expenses.as_str()
        ),
        ("0", "0"),
        "{HARNESS}: an empty period totals zero rather than nothing"
    );

    seed_fixture(&harness, &marker)
        .await
        .expect("the boundary fixture seeds");

    let statement = dao.pnl(&period()).await.expect("the P&L projects");

    // 2. THE PERIOD IS THE REQUEST. Both ends come back exactly as asked: a filter that widened or shifted the range
    //    would be reporting a period nobody chose.
    assert_eq!(
        (statement.from.as_str(), statement.to.as_str()),
        (FROM, TO),
        "{HARNESS}: the statement echoes the period it was asked for"
    );

    // 3. INCOME — `paid_on` inside `[from, to]`, inclusive at both ends. Only R1 (paid exactly on `from`) and R2
    //    (paid exactly on `to`) are income. R1 was issued in 2098, which is what proves the filter is `paid_on`.
    assert_eq!(
        statement.total_income.as_str(),
        "300.00",
        "{HARNESS}: income is the two boundary rows (100.00 on `from` + 200.00 on `to`)"
    );
    assert_eq!(
        statement.income.len(),
        2,
        "{HARNESS}: the day-before, day-after, wrong-date-column and OPEN receivables contribute no income line"
    );
    assert_eq!(
        amount_of(&statement.income, "COMMISSION"),
        "100.00",
        "{HARNESS}: the receivable paid exactly on `from` is included (inclusive lower bound)"
    );
    assert_eq!(
        amount_of(&statement.income, "LEASING_FEE"),
        "200.00",
        "{HARNESS}: the receivable paid exactly on `to` is included (inclusive upper bound)"
    );

    // 4. COST — `expense_on` inside `[from, to]`, inclusive at both ends. E5 was dated in 2098 but created now; if
    //    the filter used `created_at` it would appear here and the total would not be 30.00.
    assert_eq!(
        statement.total_expenses.as_str(),
        "30.00",
        "{HARNESS}: cost is the two boundary rows (10.00 on `from` + 20.00 on `to`)"
    );
    assert_eq!(
        statement.expenses.len(),
        2,
        "{HARNESS}: the day-before, day-after, long-ago and DRAFT expenses contribute no cost line"
    );
    assert_eq!(
        amount_of(&statement.expenses, "Office"),
        "10.00",
        "{HARNESS}: the expense dated exactly on `from` is included (inclusive lower bound)"
    );
    assert_eq!(
        amount_of(&statement.expenses, "Marketing & Advertising"),
        "20.00",
        "{HARNESS}: the expense dated exactly on `to` is included (inclusive upper bound)"
    );

    // 5. NET — the two filtered sides subtracted by Postgres in `numeric` (300.00 - 30.00).
    assert_eq!(
        statement.net_income.as_str(),
        "270.00",
        "{HARNESS}: net income is the exact difference of the two filtered totals"
    );

    // 6. NEGATIVE / EMPTY PERIOD — a range with no rows totals zero, not NULL and not a missing line. "Nothing
    //    happened this period" is a fact the filter must be able to report.
    let may = PnlRequest {
        from: "2099-05-01".into(),
        to: "2099-05-31".into(),
    };
    let empty = dao.pnl(&may).await.expect("an empty period projects");
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
        ("2099-05-01", "2099-05-31"),
        "{HARNESS}: the empty statement still echoes its period"
    );

    // 7. REFUSAL — a date filter that cannot be honoured is refused, not silently reinterpreted. A backwards range is
    //    a mistake, not an empty report; a non-date is not a date. Returning zeroes here would look like a business
    //    fact, so the domain refuses it before the DAO is ever called.
    assert_eq!(
        PnlRequest {
            from: "2099-04-01".into(),
            to: "2099-03-31".into(),
        }
        .validate()
        .expect_err("a backwards period must be refused")
        .code(),
        "PNL_RANGE_INVALID",
        "{HARNESS}: a backwards filter is refused, not silently emptied or swapped"
    );
    assert_eq!(
        PnlRequest {
            from: "nonsense".into(),
            to: "2099-03-31".into(),
        }
        .validate()
        .expect_err("an unparseable start must be refused")
        .code(),
        "PNL_FROM_INVALID",
        "{HARNESS}: the start of the filter has to be a date"
    );
    assert_eq!(
        PnlRequest {
            from: "2099-03-01".into(),
            to: "2099-02-30".into(),
        }
        .validate()
        .expect_err("an impossible end must be refused")
        .code(),
        "PNL_TO_INVALID",
        "{HARNESS}: the end of the filter has to be a real date"
    );

    // 8. COMMITTED TRUTH — the filter reads what is committed, and a rolled-back probe changes nothing. The probe
    //    inserts an enormous PAID receivable with `paid_on` inside the period, inside a transaction the harness can
    //    only roll back; inside the transaction it is visible, after the rollback the projection sees the committed
    //    fixture again.
    let probe = format!("{marker}-probe");
    let before = dao
        .pnl(&period())
        .await
        .expect("the committed P&L projects");
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
        .pnl(&period())
        .await
        .expect("the P&L projects after rollback");
    assert_eq!(
        after, before,
        "{HARNESS}: a rolled-back probe changes no filtered total — the projection reads committed truth only"
    );

    // 9. CLEANUP / ROLLBACK. This run's fixture rows are deleted; a non-zero leftover count is a failed rollback and
    //    fails the proof, because DEV must be left as it was found.
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
