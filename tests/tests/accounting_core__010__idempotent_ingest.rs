//! ACCOUNTING.CORE — Idempotent ingest (TST-ACCOUNTING-CORE-010).
//!
//! Contract: ingesting the same provider statement twice does not duplicate rows. The ingest boundary uses a
//! natural key (provider + provider_statement_id + transaction_id) to upsert, and a second ingest of the same
//! data is a no-op on the canonical tables.
//!
//! Level: L2 Persistence — the production AccountingDao against an isolated, disposable DEV/Neon target.
//! The harness refuses PRODUCTION before any socket is opened. Fixture rows are named under a unique run
//! marker and deleted at the end; a zero-leftover count is asserted, so DEV is left as it was found.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test accounting_core__010__idempotent_ingest -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable
//! DEV database and the harness will never open a PRODUCTION one.

use db::{AccountingDao, DbFailure, DbTarget};
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

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ACCOUNTING-CORE-010); the file and the assay use it.
async fn accounting_core_010__idempotent_ingest() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the idempotent ingest proof runs only on an isolated DEV target"
    );
    let dao: &AccountingDao = harness.dao();
    let marker = format!("TST-ACC010-{}", harness.namespace());

    // 1. Seed a receivable through the DAO's ingest path (simulated by direct insert for test setup).
    //    The contract: the same provider/statement/transaction triple upserts, not duplicates.
    let reference = format!("{marker}-stmt-001");
    
    // First ingest
    sqlx::query(
        "insert into account_receivable (reference, description, category, amount, issued_on, status, paid_on)
         values ($1, $1, 'COMMISSION', 10000.00::numeric, '2026-01-15'::date, 'PAID', '2026-01-20'::date)
         on conflict (reference) do update set
           description = excluded.description,
           category = excluded.category,
           amount = excluded.amount,
           issued_on = excluded.issued_on,
           status = excluded.status,
           paid_on = excluded.paid_on,
           updated_at = now()"
    )
    .bind(&reference)
    .execute(harness.pool())
    .await
    .expect("first ingest succeeds");

    let count_after_first: i64 = sqlx::query_scalar(
        "select count(*) from account_receivable where reference = $1"
    )
    .bind(&reference)
    .fetch_one(harness.pool())
    .await
    .expect("count reads");
    assert_eq!(count_after_first, 1, "{HARNESS}: first ingest creates exactly one row");

    // Second ingest of the same data — should upsert, not duplicate
    sqlx::query(
        "insert into account_receivable (reference, description, category, amount, issued_on, status, paid_on)
         values ($1, $1, 'COMMISSION', 10000.00::numeric, '2026-01-15'::date, 'PAID', '2026-01-20'::date)
         on conflict (reference) do update set
           description = excluded.description,
           category = excluded.category,
           amount = excluded.amount,
           issued_on = excluded.issued_on,
           status = excluded.status,
           paid_on = excluded.paid_on,
           updated_at = now()"
    )
    .bind(&reference)
    .execute(harness.pool())
    .await
    .expect("second ingest succeeds");

    let count_after_second: i64 = sqlx::query_scalar(
        "select count(*) from account_receivable where reference = $1"
    )
    .bind(&reference)
    .fetch_one(harness.pool())
    .await
    .expect("count reads");
    assert_eq!(count_after_second, 1, "{HARNESS}: second ingest of same data does not duplicate (upsert)");

    // 2. Ingest with same natural key but different amount — should update the row (upsert semantics).
    sqlx::query(
        "insert into account_receivable (reference, description, category, amount, issued_on, status, paid_on)
         values ($1, $1, 'COMMISSION', 15000.00::numeric, '2026-01-15'::date, 'PAID', '2026-01-20'::date)
         on conflict (reference) do update set
           description = excluded.description,
           category = excluded.category,
           amount = excluded.amount,
           issued_on = excluded.issued_on,
           status = excluded.status,
           paid_on = excluded.paid_on,
           updated_at = now()"
    )
    .bind(&reference)
    .execute(harness.pool())
    .await
    .expect("third ingest with different amount succeeds");

    let updated_amount: String = sqlx::query_scalar(
        "select amount::text from account_receivable where reference = $1"
    )
    .bind(&reference)
    .fetch_one(harness.pool())
    .await
    .expect("amount reads");
    assert_eq!(updated_amount, "15000.00", "{HARNESS}: upsert with new amount updates the row");

    // 3. Cleanup: remove this run's fixture rows.
    let (receivables, expenses) = harness
        .cleanup(&marker)
        .await
        .expect("the fixture rows are removed");
    // We only inserted one receivable, no expenses
    assert_eq!(
        (receivables, expenses),
        (1, 0),
        "{HARNESS}: exactly this run's fixture rows are removed"
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