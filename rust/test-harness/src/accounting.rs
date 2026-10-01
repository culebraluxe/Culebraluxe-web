//! The accounting read boundary, executable against one isolated, disposable DEV database.
//!
//! WHY THIS EXISTS. Accounting's summaries — the dashboard and the P&L — are not computed in Rust at all: every total
//! is a Postgres `numeric` SUM over the two canonical tables, read back as `::text` (`rust/core/db/src/accounting/
//! receivable_row.rs:365-385`, the line/total/net queries in `rust/core/db/src/accounting/recent_expenses_select.rs:
//! 91-130`). A unit test cannot see any of it, because the arithmetic happens inside the database and the exact digits
//! are the contract. The only honest way to prove a P&L aggregation contract is to run the production `AccountingDao`
//! against a real Postgres and read back what committed.
//!
//! This module wraps production types; it does not re-implement them. The aggregation under test is always
//! [`AccountingDao::pnl`](dao), and every assertion is read back from the pool the DAO wrote to
//! ([`pool`](AccountingHarness::pool)). It adds the two seams an accounting contract test needs: a way to seed the two
//! canonical tables with rows named under a unique marker, and a cleanup that removes exactly those rows.
//!
//! Level: L2 Persistence — the database contract against an isolated, disposable DEV/Neon target. The wrapped
//! [`TestDatabase`] refuses PRODUCTION before any socket is opened.

use db::{AccountingDao, DbFailure};
use sqlx::PgPool;

use crate::database::{HarnessDbError, TestDatabase};

/// The production accounting DAO on an isolated, disposable DEV database.
#[derive(Clone)]
pub struct AccountingHarness {
    database: TestDatabase,
    dao: AccountingDao,
}

impl AccountingHarness {
    /// Read the declared environment, refuse PRODUCTION, connect, and wrap the production accounting DAO.
    ///
    /// This is the entry point an accounting contract test should use: it resolves `VERCEL_ENV`/`APP_ENV` exactly as
    /// production does and returns the harness's refusal — never a production pool — when the declaration is
    /// production. It therefore runs only where a disposable DEV target is declared and `DATABASE_URL_DEV` is set.
    pub async fn connect_from_env() -> Result<Self, HarnessDbError> {
        let database = TestDatabase::connect_from_env().await?;
        Ok(Self::wrap(database))
    }

    /// Like [`connect_from_env`](Self::connect_from_env), with the environment declaration supplied by the caller.
    pub async fn connect_declared(
        vercel_env: Option<&str>,
        app_env: Option<&str>,
    ) -> Result<Self, HarnessDbError> {
        let database = TestDatabase::connect_declared(vercel_env, app_env).await?;
        Ok(Self::wrap(database))
    }

    fn wrap(database: TestDatabase) -> Self {
        let dao = AccountingDao::new(database.database().clone());
        Self { database, dao }
    }

    /// The disposable database every statement in this harness runs against.
    pub fn database(&self) -> &TestDatabase {
        &self.database
    }

    /// The production accounting DAO under test. The receivables, expenses and P&L projections live here.
    pub fn dao(&self) -> &AccountingDao {
        &self.dao
    }

    /// The pool the DAO wrote to, for reading the committed truth back on a connection the DAO does not own.
    pub fn pool(&self) -> &PgPool {
        self.database.database().pool()
    }

    /// This test database's unique namespace, safe to use as a marker prefix for disposable rows.
    pub fn namespace(&self) -> &str {
        self.database.namespace()
    }

    /// Insert one receivable, committed, under a caller-supplied `reference` marker.
    ///
    /// Fixture setup only: it writes the canonical table, not a projection, so the DAO under test still computes every
    /// total from the rows. `paid_on` must be present when `status` is `PAID`, exactly as the table's
    /// `check (status <> 'PAID' or paid_on is not null)` demands (`db/migrations/087_accounting.sql:28`).
    pub async fn seed_receivable(
        &self,
        reference: &str,
        category: &str,
        amount: &str,
        issued_on: &str,
        status: &str,
        paid_on: Option<&str>,
    ) -> Result<String, HarnessDbError> {
        let id = sqlx::query_scalar(
            "insert into account_receivable (reference, description, category, amount, issued_on, status, paid_on)
             values ($1, $1, $2, $3::numeric, $4::date, $5, $6::date)
             returning id::text",
        )
        .bind(reference)
        .bind(category)
        .bind(amount)
        .bind(issued_on)
        .bind(status)
        .bind(paid_on)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.accounting.seed_receivable",
                &error,
            ))
        })?;
        Ok(id)
    }

    /// Insert one expense, committed, under a caller-supplied `vendor` marker.
    ///
    /// Fixture setup only, the expense half of [`seed_receivable`](Self::seed_receivable).
    pub async fn seed_expense(
        &self,
        vendor: &str,
        category: &str,
        amount: &str,
        expense_on: &str,
        status: &str,
    ) -> Result<String, HarnessDbError> {
        let id = sqlx::query_scalar(
            "insert into account_expense (vendor, category, amount, expense_on, status)
             values ($1, $2, $3::numeric, $4::date, $5)
             returning id::text",
        )
        .bind(vendor)
        .bind(category)
        .bind(amount)
        .bind(expense_on)
        .bind(status)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.accounting.seed_expense",
                &error,
            ))
        })?;
        Ok(id)
    }

    /// Delete every row this run seeded under `marker`; returns `(receivables, expenses)` removed.
    ///
    /// Scoped to the marker so a concurrent run of another accounting contract has its own marker and is never
    /// touched. The marker lives in `reference` (receivable) and `vendor` (expense), columns no projection reads.
    pub async fn cleanup(&self, marker: &str) -> Result<(u64, u64), HarnessDbError> {
        let pattern = format!("{marker}%");
        let receivables = sqlx::query("delete from account_receivable where reference like $1")
            .bind(&pattern)
            .execute(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx(
                    "test-harness.accounting.cleanup_receivables",
                    &error,
                ))
            })?
            .rows_affected();
        let expenses = sqlx::query("delete from account_expense where vendor like $1")
            .bind(&pattern)
            .execute(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx(
                    "test-harness.accounting.cleanup_expenses",
                    &error,
                ))
            })?
            .rows_affected();
        Ok((receivables, expenses))
    }

    /// How many rows this run seeded under `marker` still remain, across both canonical tables.
    pub async fn leftover_count(&self, marker: &str) -> Result<i64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let receivables: i64 =
            sqlx::query_scalar("select count(*) from account_receivable where reference like $1")
                .bind(&pattern)
                .fetch_one(self.pool())
                .await
                .map_err(|error| {
                    HarnessDbError::from(DbFailure::from_sqlx(
                        "test-harness.accounting.leftover_receivables",
                        &error,
                    ))
                })?;
        let expenses: i64 =
            sqlx::query_scalar("select count(*) from account_expense where vendor like $1")
                .bind(&pattern)
                .fetch_one(self.pool())
                .await
                .map_err(|error| {
                    HarnessDbError::from(DbFailure::from_sqlx(
                        "test-harness.accounting.leftover_expenses",
                        &error,
                    ))
                })?;
        Ok(receivables + expenses)
    }
}
