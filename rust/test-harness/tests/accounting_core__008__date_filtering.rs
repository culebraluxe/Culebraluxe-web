//! ACCOUNTING.CORE — date filtering (TST-ACCOUNTING-CORE-008).
//!
//! Contract: the P&L projection filters its two canonical tables by the caller's period, and the filter is on the
//! *economic* date of each row, not on any other date the row happens to carry:
//!
//! - income is `sum(amount)` of receivables whose `status = 'PAID'` and whose `paid_on` is inside the period
//!   (`INCOME_LINES_SELECT`, `rust/core/db/src/accounting/recent_expenses_select.rs:95-104`);
//! - cost is `sum(amount)` of expenses whose `status = 'POSTED'` and whose `expense_on` is inside the period
//!   (`EXPENSE_LINES_SELECT`, `:107-116`);
//! - the period bounds are INCLUSIVE on both ends — a row dated exactly `from` or exactly `to` is in, a row one day
//!   outside is out (`paid_on >= $1::date and paid_on <= $2::date`, `:101`; `expense_on >= $1::date and expense_on <=
//!   $2::date`, `:113`);
//! - net income is the same two filtered sets, subtracted by Postgres in `numeric` (`RANGE_NET_SELECT`, `:119-130`).
//!
//! THE FILTER COLUMN IS THE CONTRACT. A receivable carries up to three dates: `issued_on`, `due_on` and `paid_on`.
//! The P&L filters income on `paid_on`, because "earned this period" is when the money was received, not when the
//! invoice was raised or when it fell due. This file proves that directly: a receivable issued inside the period but
//! paid outside it is NOT income in this period, and a receivable issued outside the period but paid inside it IS.
//! Swapping `paid_on` for `issued_on` in the query would leave the totals looking plausible while reporting a
//! different quarter's cash — the failure mode that does not announce itself.
//!
//! MONEY IS EXACT. Both sides are Postgres `numeric` and the totals read back as the digits the database holds
//! (`Money`, `rust/core/domain/src/accounting.rs:69-134`). `0.10 + 0.20` here is `0.30`, never
//! `0.30000000000000004`.
//!
//! Negative/refusal cases: a row dated one day before `from`, one day after `to`, a `PAID` receivable whose
//! `issued_on` is inside but whose `paid_on` is outside, and a `PAID` receivable whose `due_on` is inside but whose
//! `paid_on` is outside are all excluded; a backwards or malformed period is REFUSED
//! (`PnlRequest::validate`, `rust/core/domain/src/accounting.rs:296-313`) rather than reported as an empty report; a
//! single-day period is a legal window, not a backwards one; and a period with no rows totals `0` rather than nothing.
//!
//! THE PERIOD IS ISOLATED. The proof runs over calendar year 2099, a window no other accounting fixture can occupy,
//! so the totals cannot be disturbed by rows that were already committed in DEV. Every seeded row is named under a
//! unique run marker and deleted at the end; a zero-leftover count is asserted, so DEV is left as it was found.
//!
//! Level: L2 Persistence — the production `AccountingDao` against an isolated, disposable DEV/Neon target. The
//! harness refuses PRODUCTION before any socket is opened (`rust/test-harness/src/database.rs:68-75`).
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

/// The period under test: all of June 2099, a window with no other accounting fixture in it.
const JUNE_FROM: &str = "2099-06-01";
const JUNE_TO: &str = "2099-06-30";

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

/// Seed every row that sits on one side of a filter boundary in June 2099, plus the single-day row in September.
///
/// Each row is placed so the June totals can only come out right if the filter uses the right date column and the
/// right inclusive bounds:
///
/// - `r1`/`r2` sit exactly on `from` and exactly on `to` — inclusive bounds;
/// - `r3` was issued in May but paid inside June — income by `paid_on`;
/// - `r4` was issued inside June but paid in July — NOT income by `paid_on`;
/// - `r5`/`r6` are PAID one day outside the period — excluded;
/// - `r7` is OPEN inside the period — no income (status);
/// - `e1`/`e2` sit exactly on the expense bounds, `e3`/`e4` one day outside;
/// - `r8`/`e5` sit on a single day in September, for the `from == to` period;
/// - `r9` is due in October but paid in November and `r10` is due in December but paid in November, so the November
///   period has both and the October period has neither — the proof that income is filtered on `paid_on`, not `due_on`.
async fn seed_fixture(
    harness: &AccountingHarness,
    marker: &str,
) -> Result<(), test_harness::HarnessDbError> {
    // Income. Only r1, r2 and r3 are June income; every other receivable is a filter boundary that must stay out.
    harness
        .seed_receivable(
            &format!("{marker}-r1"),
            "COMMISSION",
            "100.00",
            JUNE_FROM,
            "PAID",
            Some(JUNE_FROM),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r2"),
            "LEASING_FEE",
            "200.00",
            JUNE_TO,
            "PAID",
            Some(JUNE_TO),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r3"),
            "SERVICE_FEE",
            "50.00",
            "2099-05-20",
            "PAID",
            Some("2099-06-10"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r4"),
            "CLOSING_FEE",
            "99999.00",
            "2099-06-15",
            "PAID",
            Some("2099-07-05"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r5"),
            "COMMISSION",
            "1111.00",
            "2099-05-10",
            "PAID",
            Some("2099-05-31"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r6"),
            "COMMISSION",
            "2222.00",
            "2099-06-20",
            "PAID",
            Some("2099-07-01"),
        )
        .await?;
    harness
        .seed_receivable(
            &format!("{marker}-r7"),
            "COMMISSION",
            "3333.00",
            "2099-06-15",
            "OPEN",
            None,
        )
        .await?;

    // Cost. Only e1 and e2 are June cost; e3 and e4 are one day outside.
    harness
        .seed_expense(
            &format!("{marker}-e1"),
            "Office",
            "10.00",
            JUNE_FROM,
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e2"),
            "Office",
            "20.00",
            JUNE_TO,
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e3"),
            "Office",
            "4000.00",
            "2099-05-31",
            "POSTED",
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e4"),
            "Office",
            "5000.00",
            "2099-07-01",
            "POSTED",
        )
        .await?;

    // The single-day period's rows: `from == to` must be a legal one-day filter, and inclusive.
    harness
        .seed_receivable(
            &format!("{marker}-r8"),
            "COMMISSION",
            "7.00",
            "2099-09-15",
            "PAID",
            Some("2099-09-15"),
        )
        .await?;
    harness
        .seed_expense(
            &format!("{marker}-e5"),
            "Office",
            "3.00",
            "2099-09-15",
            "POSTED",
        )
        .await?;

    // The `due_on` probe — income is filtered on `paid_on`, not on the receivable's due date. Both rows are placed
    // across November 2099 so no other assertion's period is disturbed:
    //  - r9 is DUE in October but PAID in November: not October income, but November income;
    //  - r10 is DUE in December but PAID in November: November income, even though its due date is outside.
    // A query that filtered income on `due_on` instead of `paid_on` would report the exact opposite for both.
    harness
        .seed_receivable_dated(
            &format!("{marker}-r9"),
            "DUE_WINDOW",
            "44444.00",
            "2099-05-25",
            Some("2099-10-05"),
            "PAID",
            Some("2099-11-10"),
        )
        .await?;
    harness
        .seed_receivable_dated(
            &format!("{marker}-r10"),
            "PAID_WINDOW",
            "55555.00",
            "2099-05-25",
            Some("2099-12-05"),
            "PAID",
            Some("2099-11-20"),
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
         values ($1, $1, 'COMMISSION', 1000000.00::numeric, '2099-06-15'::date, 'PAID', '2099-06-15'::date)",
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
        "{HARNESS}: the date-filtering proof runs only on an isolated DEV target"
    );
    let dao: &AccountingDao = harness.dao();
    let marker = format!("TST-ACC008-{}", harness.namespace());

    // SELF-HEAL. The totals below are exact over their periods, so a previous run of THIS story that failed before
    // its own cleanup would leave rows in June/September 2099 and make every later run red for the wrong reason. The
    // `TST-ACC008-` prefix belongs to this story alone, so clearing it first makes the periods deterministic and the
    // contract repeatable even immediately after a genuine failure. A first, clean run deletes nothing.
    harness
        .cleanup("TST-ACC008-")
        .await
        .expect("stale TST-ACCOUNTING-CORE-008 rows are cleared before seeding");

    seed_fixture(&harness, &marker)
        .await
        .expect("the June/September fixture seeds");

    let june = PnlRequest {
        from: JUNE_FROM.into(),
        to: JUNE_TO.into(),
    };
    let statement = dao.pnl(&june).await.expect("the P&L projects");

    // 1. THE PERIOD IS THE REQUEST. Both ends come back exactly as asked, so a filter that silently widened or
    //    shifted the range is visible.
    assert_eq!(
        (statement.from.as_str(), statement.to.as_str()),
        (JUNE_FROM, JUNE_TO),
        "{HARNESS}: the statement echoes the period it was asked for"
    );

    // 2. INCOME — only PAID receivables whose `paid_on` is in June. r1 and r2 are the inclusive bounds; r3 was issued
    //    in May but paid in June, so the filter is on `paid_on`, not `issued_on`.
    assert_eq!(
        statement.total_income.as_str(),
        "350.00",
        "{HARNESS}: June income is exactly r1 + r2 + r3 (100.00 + 200.00 + 50.00)"
    );
    assert_eq!(
        statement.income.len(),
        3,
        "{HARNESS}: the boundary rows r4/r5/r6/r7 contribute no June income line"
    );
    // The out-of-range PAID receivables share the COMMISSION category with r1, so if the filter leaked them in, this
    // line would read 100.00 + 1111.00 + 2222.00 + 3333.00 = 6666.00 rather than 100.00.
    assert_eq!(
        amount_of(&statement.income, "COMMISSION"),
        "100.00",
        "{HARNESS}: only the in-period COMMISSION receivable is summed into its line"
    );
    assert_eq!(
        amount_of(&statement.income, "LEASING_FEE"),
        "200.00",
        "{HARNESS}: the upper-bound receivable is included on `paid_on = to`"
    );
    assert_eq!(
        amount_of(&statement.income, "SERVICE_FEE"),
        "50.00",
        "{HARNESS}: a receivable issued before the period but paid inside it is income"
    );
    // r4 was issued inside June but paid in July: if the query filtered on `issued_on`, this line would appear.
    assert_eq!(
        amount_of(&statement.income, "CLOSING_FEE"),
        "<no CLOSING_FEE line>",
        "{HARNESS}: income is filtered on paid_on — an inside-issued but outside-paid receivable is excluded"
    );

    // 3. COST — only POSTED expenses whose `expense_on` is in June, inclusive of both bounds. The out-of-range Office
    //    rows share a category with the in-range ones, so a leaked row would make the Office line 3030.00, not 30.00.
    assert_eq!(
        statement.total_expenses.as_str(),
        "30.00",
        "{HARNESS}: June cost is exactly e1 + e2 (10.00 + 20.00)"
    );
    assert_eq!(
        statement.expenses.len(),
        1,
        "{HARNESS}: both June expenses share one Office line"
    );
    assert_eq!(
        amount_of(&statement.expenses, "Office"),
        "30.00",
        "{HARNESS}: only the in-period Office expenses are summed (e3/e4 one day outside are excluded)"
    );

    // 4. NET — the exact difference of the two filtered sets, subtracted by Postgres in `numeric`.
    assert_eq!(
        statement.net_income.as_str(),
        "320.00",
        "{HARNESS}: net income is the filtered income minus filtered cost (350.00 - 30.00)"
    );

    // 5. NEGATIVE / REFUSAL — a period that runs backwards, or names a non-date, is a mistake and is refused before
    //    the DAO is ever called. Returning zeroes would look like a business fact.
    assert_eq!(
        PnlRequest {
            from: JUNE_TO.into(),
            to: JUNE_FROM.into(),
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
            to: JUNE_TO.into(),
        }
        .validate()
        .expect_err("an unparseable start must be refused")
        .code(),
        "PNL_FROM_INVALID",
        "{HARNESS}: the start date has to be a date"
    );
    assert_eq!(
        PnlRequest {
            from: JUNE_FROM.into(),
            to: "2099-13-40".into(),
        }
        .validate()
        .expect_err("an impossible end must be refused")
        .code(),
        "PNL_TO_INVALID",
        "{HARNESS}: the end date has to be a date"
    );

    // 6. NEGATIVE / EMPTY PERIOD — no rows in range totals zero, not NULL and not a missing line.
    let empty = dao
        .pnl(&PnlRequest {
            from: "2099-02-01".into(),
            to: "2099-02-28".into(),
        })
        .await
        .expect("an empty period projects");
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
        ("2099-02-01", "2099-02-28"),
        "{HARNESS}: even an empty statement echoes the period it was asked for"
    );

    // 7. NEGATIVE / SINGLE-DAY PERIOD — `from == to` is a legal one-day filter, and it is inclusive on that day. A
    //    filter that treated a same-day period as backwards, or excluded the endpoint, would not return r8/e5.
    let one_day = dao
        .pnl(&PnlRequest {
            from: "2099-09-15".into(),
            to: "2099-09-15".into(),
        })
        .await
        .expect("a single-day period projects");
    assert_eq!(
        one_day.total_income.as_str(),
        "7.00",
        "{HARNESS}: a from == to period includes the receivable paid on that day"
    );
    assert_eq!(
        one_day.total_expenses.as_str(),
        "3.00",
        "{HARNESS}: a from == to period includes the expense dated on that day"
    );
    assert_eq!(
        one_day.net_income.as_str(),
        "4.00",
        "{HARNESS}: the single-day net is filtered income minus filtered cost (7.00 - 3.00)"
    );
    assert_eq!(
        (one_day.from.as_str(), one_day.to.as_str()),
        ("2099-09-15", "2099-09-15"),
        "{HARNESS}: a single-day statement echoes both ends of the one-day period"
    );
    // The refusal is a strict ordering: equal ends are a legal one-day window, not a backwards range. If the
    // validator treated `from == to` as invalid, the callers that guard on `validate()` would reject it.
    assert!(
        PnlRequest {
            from: "2099-09-15".into(),
            to: "2099-09-15".into(),
        }
        .validate()
        .is_ok(),
        "{HARNESS}: a single-day period is valid, not backwards"
    );

    // 7c. WHICH DATE COLUMN — a receivable carries `issued_on`, `due_on` and `paid_on`; the P&L filters income on
    //     `paid_on`. r9 is due in October but paid in November and r10 is due in December but paid in November, so
    //     October must have neither and November must have both. A query filtering on `due_on` would report r9 in
    //     October and misplace r10 out of November — a plausible-looking quarter that is simply the wrong cash.
    let october = dao
        .pnl(&PnlRequest {
            from: "2099-10-01".into(),
            to: "2099-10-31".into(),
        })
        .await
        .expect("October projects");
    assert_eq!(
        amount_of(&october.income, "DUE_WINDOW"),
        "<no DUE_WINDOW line>",
        "{HARNESS}: income is filtered on paid_on — a receivable due in-period but paid outside it is not income"
    );
    assert_eq!(
        october.total_income.as_str(),
        "0",
        "{HARNESS}: October has no PAID receivable whose paid_on is in October"
    );
    let november = dao
        .pnl(&PnlRequest {
            from: "2099-11-01".into(),
            to: "2099-11-30".into(),
        })
        .await
        .expect("November projects");
    assert_eq!(
        amount_of(&november.income, "DUE_WINDOW"),
        "44444.00",
        "{HARNESS}: the same receivable is income in the period it was paid, not the period it fell due"
    );
    assert_eq!(
        amount_of(&november.income, "PAID_WINDOW"),
        "55555.00",
        "{HARNESS}: a receivable paid in-period is income even though its due date is outside the period"
    );
    assert_eq!(
        november.total_income.as_str(),
        "99999.00",
        "{HARNESS}: November income is exactly r9 + r10 (44444.00 + 55555.00)"
    );

    // 8. COMMITTED TRUTH — the filter reads what is committed, and a rolled-back probe changes nothing. The probe
    //    inserts an enormous PAID receivable inside June inside a transaction the harness can only roll back; inside
    //    the transaction it is visible, after the rollback the projection sees the committed fixture again.
    let before = dao.pnl(&june).await.expect("the committed P&L projects");
    let probe_reference = format!("{marker}-probe");
    let visible_inside = harness
        .database()
        .with_rollback(move |conn| {
            let probe_reference = probe_reference.clone();
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
        .pnl(&june)
        .await
        .expect("the P&L projects after rollback");
    assert_eq!(
        after, before,
        "{HARNESS}: a rolled-back probe changes no total — the filter reads committed truth only"
    );

    // 9. CLEANUP / ROLLBACK. This run's fixture rows are deleted; a non-zero leftover count is a failed rollback and
    //    fails the proof, because DEV must be left as it was found.
    let (receivables, expenses) = harness
        .cleanup(&marker)
        .await
        .expect("the fixture rows are removed");
    assert_eq!(
        (receivables, expenses),
        (10, 5),
        "{HARNESS}: exactly this run's ten receivables and five expenses are removed"
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
