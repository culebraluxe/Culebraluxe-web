//! The Accounting DAO: the two canonical tables, and the projections over them.
//!
//! EVERY SUM IS POSTGRES'S. The amounts are `numeric`; the summary figures are computed by the database on `numeric` and
//! read back as `::text`, so no total in this codebase is an approximation and no decimal arithmetic happens in Rust.
//! That is the whole reason `Money` is a string (see `domain::accounting`) and it is why the queries below cast the way
//! they do.
//!
//! THE PROJECTIONS ARE QUERIES, NOT MODELS. The dashboard and the P&L are read here as joins of the same two tables the
//! lists read. Nothing in this file writes a derived row, which is what keeps "the numbers can disagree with the rows"
//! from being a possible state.

use domain::{
    AccountingDashboard, CategoryShare, CreateExpenseCommand, CreateReceivableCommand, Expense,
    MarkReceivablePaidCommand, MarkReceivablePaidOutcome, Money, PnlLine, PnlRequest, PnlStatement,
    PnlTrendPoint, Receivable,
};
use sqlx::FromRow;

use crate::{Database, DbFailure, DbResult};

/// One receivable row, with its associations already resolved to names.
#[derive(Debug, Clone, FromRow)]
struct ReceivableRow {
    id: String,
    reference: Option<String>,
    description: String,
    category: String,
    amount: String,
    issued_on: String,
    due_on: Option<String>,
    status: String,
    paid_on: Option<String>,
    deal_id: Option<String>,
    deal_name: Option<String>,
    property_id: Option<String>,
    property_name: Option<String>,
    person_id: Option<String>,
    person_name: Option<String>,
}

impl ReceivableRow {
    fn into_domain(self) -> Receivable {
        Receivable {
            id: self.id,
            reference: self.reference,
            description: self.description,
            category: self.category,
            amount: Money::from_database(self.amount),
            issued_on: self.issued_on,
            due_on: self.due_on,
            status: self.status,
            paid_on: self.paid_on,
            deal_id: self.deal_id,
            deal_name: self.deal_name,
            property_id: self.property_id,
            property_name: self.property_name,
            person_id: self.person_id,
            person_name: self.person_name,
        }
    }
}

/// One expense row, with its associations already resolved to names.
#[derive(Debug, Clone, FromRow)]
struct ExpenseRow {
    id: String,
    vendor: String,
    category: String,
    amount: String,
    expense_on: String,
    status: String,
    memo: Option<String>,
    deal_id: Option<String>,
    deal_name: Option<String>,
    property_id: Option<String>,
    property_name: Option<String>,
    person_id: Option<String>,
    person_name: Option<String>,
}

impl ExpenseRow {
    fn into_domain(self) -> Expense {
        Expense {
            id: self.id,
            vendor: self.vendor,
            category: self.category,
            amount: Money::from_database(self.amount),
            expense_on: self.expense_on,
            status: self.status,
            memo: self.memo,
            deal_id: self.deal_id,
            deal_name: self.deal_name,
            property_id: self.property_id,
            property_name: self.property_name,
            person_id: self.person_id,
            person_name: self.person_name,
        }
    }
}

/// A single aggregate read back as digits (`0` when there is nothing to sum, never NULL).
#[derive(Debug, Clone, FromRow)]
struct TextValue {
    v: Option<String>,
}

impl TextValue {
    fn money(&self) -> Money {
        Money::from_database(self.v.clone().unwrap_or_else(|| "0".to_owned()))
    }
}

/// The open/overdue counts, which are counts rather than amounts.
#[derive(Debug, Clone, FromRow)]
struct CountsRow {
    open_count: i64,
    overdue_count: i64,
}

/// One category in the dashboard's breakdown: its total and its share, both from the query.
#[derive(Debug, Clone, FromRow)]
struct ShareRow {
    label: String,
    amount: String,
    percent: i64,
}

impl ShareRow {
    fn into_share(self) -> CategoryShare {
        CategoryShare {
            label: self.label,
            amount: Money::from_database(self.amount),
            percent: self.percent,
        }
    }
}

/// One month of the trend: two sums, their difference and the six-month totals, all computed by Postgres.
#[derive(Debug, Clone, FromRow)]
struct TrendRow {
    month: String,
    income: String,
    expenses: String,
    net: String,
    total_income: Option<String>,
    total_expenses: Option<String>,
    total_net: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct PaidRow {
    id: String,
    status: String,
    paid_on: String,
}

#[derive(Debug, Clone, FromRow)]
struct IdRow {
    id: String,
}

/// The month abbreviations the trend is drawn with. Fixed rather than locale-derived: a chart's axis labels must not
/// change shape with the server's locale, and the live screen's appeared in this form.
const MONTH_LABELS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[derive(Clone)]
pub struct AccountingDao {
    db: Database,
}

impl AccountingDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Every receivable, newest first — the ordering the live screen used (`issued_on desc, created_at desc`).
    pub async fn receivables(&self) -> DbResult<Vec<Receivable>> {
        let rows = sqlx::query_as::<_, ReceivableRow>(RECEIVABLE_SELECT)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("accounting.list_receivables", &error))?;
        Ok(rows.into_iter().map(ReceivableRow::into_domain).collect())
    }

    /// Every expense, newest first.
    pub async fn expenses(&self) -> DbResult<Vec<Expense>> {
        let rows = sqlx::query_as::<_, ExpenseRow>(EXPENSE_SELECT)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("accounting.list_expenses", &error))?;
        Ok(rows.into_iter().map(ExpenseRow::into_domain).collect())
    }

    /// Every posted expense by category, with each one's share of the total.
    pub async fn expense_categories(&self) -> DbResult<Vec<CategoryShare>> {
        let rows = share_rows(self.db.pool(), EXPENSE_CATEGORY_SHARES_SELECT)
            .await
            .map_err(|error| DbFailure::from_sqlx("accounting.expense_categories", &error))?;
        Ok(rows.into_iter().map(ShareRow::into_share).collect())
    }

    /// The dashboard's figures.
    ///
    /// EIGHT READS, CONCURRENTLY, against the same two tables the lists read — the shape the live screen used. They are
    /// two `try_join!`s nested inside a third so all eight are in flight at once while each pair still reports a single
    /// operation name: a dashboard that went out and back eight times would be eight round trips of latency for one
    /// screen.
    pub async fn dashboard(&self) -> DbResult<AccountingDashboard> {
        let pool = self.db.pool();
        // Eight reads in one `try_join!`: tokio's join macros await internally, so nesting two of them would run the
        // first group to completion before the second started — concurrent-looking code that is secretly serial.
        let (
            outstanding,
            this_month,
            counts,
            net,
            trend,
            recent_expenses,
            recent_activity,
            categories,
        ) = tokio::try_join!(
            text_one(pool, OUTSTANDING_SELECT),
            text_one(pool, EXPENSES_THIS_MONTH_SELECT),
            count_rows(pool),
            text_one(pool, NET_INCOME_SELECT),
            trend_rows(pool),
            expense_rows(pool, RECENT_EXPENSES_SELECT),
            receivable_rows(pool, RECENT_RECEIVABLES_SELECT),
            share_rows(pool, CATEGORY_BREAKDOWN_SELECT),
        )
        .map_err(|error| DbFailure::from_sqlx("accounting.dashboard", &error))?;

        Ok(AccountingDashboard {
            receivables_outstanding: outstanding.money(),
            expenses_this_month: this_month.money(),
            net_income: net.money(),
            open_count: counts.open_count,
            overdue_count: counts.overdue_count,
            // The totals ride on every trend row; whichever arrived is the same figure, and an empty series has none.
            trend_income: trend
                .first()
                .and_then(|row| row.total_income.clone())
                .map(Money::from_database)
                .unwrap_or_else(|| Money::from_database("0")),
            trend_expenses: trend
                .first()
                .and_then(|row| row.total_expenses.clone())
                .map(Money::from_database)
                .unwrap_or_else(|| Money::from_database("0")),
            trend_net: trend
                .first()
                .and_then(|row| row.total_net.clone())
                .map(Money::from_database)
                .unwrap_or_else(|| Money::from_database("0")),
            pnl_trend: trend.into_iter().map(trend_point).collect(),
            recent_expenses: recent_expenses
                .into_iter()
                .map(ExpenseRow::into_domain)
                .collect(),
            recent_activity: recent_activity
                .into_iter()
                .map(ReceivableRow::into_domain)
                .collect(),
            expense_categories: categories.into_iter().map(ShareRow::into_share).collect(),
        })
    }

    /// The profit-and-loss projection for a period the caller chose.
    ///
    /// THE PERIOD IS BOUND, NOT ASSUMED. The range arrives in the request, is bound into every query here as a date, and
    /// is echoed back on the statement — so a screen that filters to March gets March and can see that it did. The row
    /// the old generic bridge rendered came from a hard-coded `2020-01-01 → today`, which is not a period anybody chose.
    pub async fn pnl(&self, request: &PnlRequest) -> DbResult<PnlStatement> {
        let pool = self.db.pool();
        let (income, expenses, net) = tokio::try_join!(
            line_total_rows(pool, INCOME_LINES_SELECT, &request.from, &request.to),
            line_total_rows(pool, EXPENSE_LINES_SELECT, &request.from, &request.to),
            range_net(pool, &request.from, &request.to),
        )
        .map_err(|error| DbFailure::from_sqlx("accounting.pnl", &error))?;

        Ok(PnlStatement {
            // Echoed from the request, which is the only way a caller can tell that the period it asked for is the
            // period it got.
            from: request.from.clone(),
            to: request.to.clone(),
            total_income: income.total(),
            total_expenses: expenses.total(),
            income: income.into_iter().map(LineTotalRow::into_line).collect(),
            expenses: expenses.into_iter().map(LineTotalRow::into_line).collect(),
            net_income: net.money(),
        })
    }

    /// Record an expense and hand back its id.
    ///
    /// THE COMMAND IS ASSUMED VALIDATED — it is the service's job to have checked it, and the amount is bound as text and
    /// cast to `numeric` in SQL so the digits Postgres stores are the digits the operator typed rather than a float that
    /// travelled through the wire.
    pub async fn create_expense(&self, command: &CreateExpenseCommand) -> DbResult<String> {
        let row = sqlx::query_as::<_, IdRow>(
            r#"
            insert into account_expense (
              vendor, category, amount, expense_on, memo, deal_id, property_id, person_id
            ) values (
              $1, $2, $3::numeric, $4::date, $5, $6::uuid, $7::uuid, $8::uuid
            )
            returning id::text as id
            "#,
        )
        .bind(&command.vendor)
        .bind(&command.category)
        .bind(&command.amount)
        .bind(&command.expense_on)
        .bind(command.memo.as_deref())
        .bind(command.deal_id.as_deref())
        .bind(command.property_id.as_deref())
        .bind(command.person_id.as_deref())
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("accounting.create_expense", &error))?;
        Ok(row.id)
    }

    /// Record a receivable and hand back its id.
    pub async fn create_receivable(&self, command: &CreateReceivableCommand) -> DbResult<String> {
        let row = sqlx::query_as::<_, IdRow>(
            r#"
            insert into account_receivable (
              reference, description, category, amount, issued_on, due_on, deal_id, property_id, person_id
            ) values (
              $1, $2, $3, $4::numeric, $5::date, $6::date, $7::uuid, $8::uuid, $9::uuid
            )
            returning id::text as id
            "#,
        )
        .bind(command.reference.as_deref())
        .bind(&command.description)
        .bind(&command.category)
        .bind(&command.amount)
        .bind(&command.issued_on)
        .bind(command.due_on.as_deref())
        .bind(command.deal_id.as_deref())
        .bind(command.property_id.as_deref())
        .bind(command.person_id.as_deref())
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("accounting.create_receivable", &error))?;
        Ok(row.id)
    }

    /// Mark a receivable paid, or report that it could not be.
    ///
    /// THE TRANSITION IS THE WHERE CLAUSE. `status <> 'VOID'` is the invariant: a voided receivable is not payable, and a
    /// receivable that does not exist is not payable either — both come back as no row, and both are the same conflict
    /// the live screen reported. Nothing transitions a status in the browser, and nothing here reads the row first:
    /// read-then-write would let a receivable be voided between the two statements and be paid anyway.
    ///
    /// The id is matched as text (`id::text = $1`) so a malformed id is "no such receivable" rather than a cast error
    /// that reports a 500 for what is really a conflict. This table is a brokerage's own book and not large enough for
    /// the index this gives up to matter.
    pub async fn mark_receivable_paid(
        &self,
        command: &MarkReceivablePaidCommand,
    ) -> DbResult<Option<MarkReceivablePaidOutcome>> {
        let row = sqlx::query_as::<_, PaidRow>(
            r#"
            update account_receivable
            set status = 'PAID', paid_on = $2::date, updated_at = now()
            where id::text = $1 and status <> 'VOID'
            returning id::text as id, status, paid_on::text as paid_on
            "#,
        )
        .bind(&command.receivable_id)
        .bind(&command.paid_on)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("accounting.mark_receivable_paid", &error))?;

        Ok(row.map(|row| MarkReceivablePaidOutcome {
            id: row.id,
            status: row.status,
            paid_on: row.paid_on,
        }))
    }
}

/// The receivable projection every read shares: associations resolved to names, amounts as digits.
const RECEIVABLE_SELECT: &str = r#"
select
  t.id::text as id,
  t.reference,
  t.description,
  t.category,
  t.amount::text as amount,
  t.issued_on::text as issued_on,
  t.due_on::text as due_on,
  t.status,
  t.paid_on::text as paid_on,
  t.deal_id::text as deal_id,
  t.property_id::text as property_id,
  t.person_id::text as person_id,
  person.display_name as person_name,
  property.name as property_name,
  deal_property.name as deal_name
from account_receivable t
left join person on person.id = t.person_id
left join property on property.id = t.property_id
left join deal on deal.id = t.deal_id
left join property deal_property on deal_property.id = deal.property_id
order by t.issued_on desc, t.created_at desc
"#;

/// The expense projection, the same shape and the same joins.
const EXPENSE_SELECT: &str = r#"
select
  t.id::text as id,
  t.vendor,
  t.category,
  t.amount::text as amount,
  t.expense_on::text as expense_on,
  t.status,
  t.memo,
  t.deal_id::text as deal_id,
  t.property_id::text as property_id,
  t.person_id::text as person_id,
  person.display_name as person_name,
  property.name as property_name,
  deal_property.name as deal_name
from account_expense t
left join person on person.id = t.person_id
left join property on property.id = t.property_id
left join deal on deal.id = t.deal_id
left join property deal_property on deal_property.id = deal.property_id
order by t.expense_on desc, t.created_at desc
"#;

/// Sum of every open receivable — what is owed to the brokerage right now.
const OUTSTANDING_SELECT: &str = r#"
select coalesce(sum(amount), 0)::text as v from account_receivable where status = 'OPEN'
"#;

/// This month's posted cost.
const EXPENSES_THIS_MONTH_SELECT: &str = r#"
select coalesce(sum(amount), 0)::text as v
from account_expense
where status = 'POSTED'
  and date_trunc('month', expense_on) = date_trunc('month', current_date)
"#;

/// Open and overdue receivables. A receivable with no due date is open, never overdue — the live screen's rule, kept,
/// because "no deadline agreed" is not the same as "late".
const COUNTS_SELECT: &str = r#"
select
  count(*) filter (where status = 'OPEN' and (due_on is null or due_on >= current_date)) as open_count,
  count(*) filter (where status = 'OPEN' and due_on is not null and due_on < current_date) as overdue_count
from account_receivable
"#;

/// Paid income minus posted cost, all time.
const NET_INCOME_SELECT: &str = r#"
select (
  (select coalesce(sum(amount), 0) from account_receivable where status = 'PAID')
  - (select coalesce(sum(amount), 0) from account_expense where status = 'POSTED')
)::text as v
"#;

/// The six months ending with this one, each with its income, cost and difference.
///
/// THE MONTHS COME FROM `generate_series`, so a month with no activity is a row of zeroes rather than a gap the caller
/// has to notice, and the series is exactly six long whatever the data says. The arithmetic is the database's, on
/// `numeric`, which is why the difference is a column and not something Rust works out.
const TREND_SELECT: &str = r#"
with months as (
  select generate_series(
    date_trunc('month', current_date) - interval '5 months',
    date_trunc('month', current_date),
    interval '1 month'
  ) as month
),
points as (
  select
    to_char(m.month, 'YYYY-MM') as month,
    coalesce((
      select sum(r.amount) from account_receivable r
      where r.status = 'PAID' and date_trunc('month', r.paid_on) = m.month
    ), 0) as income,
    coalesce((
      select sum(e.amount) from account_expense e
      where e.status = 'POSTED' and date_trunc('month', e.expense_on) = m.month
    ), 0) as expenses
  from months m
)
select
  month,
  income::text as income,
  expenses::text as expenses,
  (income - expenses)::text as net,
  -- The three totals the chart's header prints, added up by the database over the same six months rather than by the
  -- screen: a figure beside a chart must be the chart's own figure, to the cent.
  (sum(income) over ())::text as total_income,
  (sum(expenses) over ())::text as total_expenses,
  (sum(income - expenses) over ())::text as total_net
from points
order by month
"#;

/// The five most recent posted expenses, spelled out rather than parameterised: a `limit` cannot be bound into a
/// projection that also has to keep the join list, and two copies of a projection drift.
const RECENT_EXPENSES_SELECT: &str = r#"
select
  t.id::text as id,
  t.vendor,
  t.category,
  t.amount::text as amount,
  t.expense_on::text as expense_on,
  t.status,
  t.memo,
  t.deal_id::text as deal_id,
  t.property_id::text as property_id,
  t.person_id::text as person_id,
  person.display_name as person_name,
  property.name as property_name,
  deal_property.name as deal_name
from account_expense t
left join person on person.id = t.person_id
left join property on property.id = t.property_id
left join deal on deal.id = t.deal_id
left join property deal_property on deal_property.id = deal.property_id
where t.status = 'POSTED'
order by t.expense_on desc
limit 5
"#;

/// The six most recent receivables that are not void.
const RECENT_RECEIVABLES_SELECT: &str = r#"
select
  t.id::text as id,
  t.reference,
  t.description,
  t.category,
  t.amount::text as amount,
  t.issued_on::text as issued_on,
  t.due_on::text as due_on,
  t.status,
  t.paid_on::text as paid_on,
  t.deal_id::text as deal_id,
  t.property_id::text as property_id,
  t.person_id::text as person_id,
  person.display_name as person_name,
  property.name as property_name,
  deal_property.name as deal_name
from account_receivable t
left join person on person.id = t.person_id
left join property on property.id = t.property_id
left join deal on deal.id = t.deal_id
left join property deal_property on deal_property.id = deal.property_id
where t.status <> 'VOID'
order by t.issued_on desc
limit 6
"#;

/// Every posted expense by category — the breakdown the Expenses screen draws, all time, largest first, with each
/// category's share.
///
/// THE LIVE SCREEN ADDED THESE UP IN THE BROWSER, filtering the rows it had already fetched for `POSTED` and summing them
/// as floats. That is a float sum of money, and it is a second aggregation that can disagree with the rows beneath it —
/// so it is a query here instead, in `numeric`, with the share computed at the same time.
const EXPENSE_CATEGORY_SHARES_SELECT: &str = r#"
select
  t.category as label,
  coalesce(sum(t.amount), 0)::text as amount,
  coalesce(round(100 * sum(t.amount) / nullif(sum(sum(t.amount)) over (), 0)), 0)::bigint as percent
from account_expense t
where t.status = 'POSTED'
group by t.category
order by sum(t.amount) desc
"#;
/// month. The share is `numeric` arithmetic in the database and arrives as a rounded percentage, so the ring the screen
/// draws and the figures printed beside it come from the same calculation.
const CATEGORY_BREAKDOWN_SELECT: &str = r#"
select
  t.category as label,
  coalesce(sum(t.amount), 0)::text as amount,
  coalesce(round(100 * sum(t.amount) / nullif(sum(sum(t.amount)) over (), 0)), 0)::bigint as percent
from account_expense t
where t.status = 'POSTED'
  and date_trunc('month', t.expense_on) = date_trunc('month', current_date)
group by t.category
order by sum(t.amount) desc
"#;

/// The income lines of a P&L, grouped by category, with the period's total carried on every row.
///
/// The total is a window function rather than a second round trip and rather than a sum in Rust: the figure that has to
/// agree with the database is computed by the database, in `numeric`, once.
const INCOME_LINES_SELECT: &str = r#"
select
  category as label,
  coalesce(sum(amount), 0)::text as amount,
  coalesce(sum(sum(amount)) over (), 0)::text as total
from account_receivable
where status = 'PAID' and paid_on >= $1::date and paid_on <= $2::date
group by category
order by sum(amount) desc
"#;

/// The expense lines of a P&L, the same shape.
const EXPENSE_LINES_SELECT: &str = r#"
select
  category as label,
  coalesce(sum(amount), 0)::text as amount,
  coalesce(sum(sum(amount)) over (), 0)::text as total
from account_expense
where status = 'POSTED' and expense_on >= $1::date and expense_on <= $2::date
group by category
order by sum(amount) desc
"#;

/// The period's net income: paid income in the range minus posted cost in the range.
const RANGE_NET_SELECT: &str = r#"
select (
  coalesce((
    select sum(amount) from account_receivable
    where status = 'PAID' and paid_on >= $1::date and paid_on <= $2::date
  ), 0)
  - coalesce((
    select sum(amount) from account_expense
    where status = 'POSTED' and expense_on >= $1::date and expense_on <= $2::date
  ), 0)
)::text as v
"#;

/// One category line as the P&L queries return it: the line, and the period's total beside it.
#[derive(Debug, Clone, FromRow)]
struct LineTotalRow {
    label: String,
    amount: String,
    total: Option<String>,
}

impl LineTotalRow {
    fn into_line(self) -> PnlLine {
        PnlLine {
            label: self.label,
            amount: Money::from_database(self.amount),
        }
    }
}

/// The total on a set of P&L lines.
///
/// Whenever there are no lines the window function never ran, so the total is absent; "nothing was earned in this
/// period" is zero and not a missing value.
trait LineTotals {
    fn total(&self) -> Money;
}

impl LineTotals for Vec<LineTotalRow> {
    fn total(&self) -> Money {
        self.first()
            .and_then(|row| row.total.clone())
            .map(Money::from_database)
            .unwrap_or_else(|| Money::from_database("0"))
    }
}

/// The month as the chart draws it: `2026-03` becomes `Mar`.
///
/// A key that is not a month is passed through rather than dropped. The series comes from `generate_series` and is always
/// well formed, so this cannot happen from the database — but if it ever did, a visible `2026-13` on a chart is a bug
/// somebody can see, where an empty label is one they cannot.
fn month_label(month: &str) -> String {
    month
        .split_once('-')
        .and_then(|(_, number)| number.parse::<usize>().ok())
        .and_then(|number| MONTH_LABELS.get(number.saturating_sub(1)))
        .map(|label| (*label).to_owned())
        .unwrap_or_else(|| month.to_owned())
}

fn trend_point(row: TrendRow) -> PnlTrendPoint {
    PnlTrendPoint {
        month: month_label(&row.month),
        income: Money::from_database(row.income),
        expenses: Money::from_database(row.expenses),
        net: Money::from_database(row.net),
    }
}

/// The read helpers behind `dashboard` and `pnl`, returning `sqlx` errors so the caller can name the operation once for
/// the whole projection rather than once per query.
async fn text_one(pool: &sqlx::PgPool, sql: &'static str) -> Result<TextValue, sqlx::Error> {
    sqlx::query_as::<_, TextValue>(sql).fetch_one(pool).await
}

async fn count_rows(pool: &sqlx::PgPool) -> Result<CountsRow, sqlx::Error> {
    sqlx::query_as::<_, CountsRow>(COUNTS_SELECT)
        .fetch_one(pool)
        .await
}

async fn trend_rows(pool: &sqlx::PgPool) -> Result<Vec<TrendRow>, sqlx::Error> {
    sqlx::query_as::<_, TrendRow>(TREND_SELECT)
        .fetch_all(pool)
        .await
}

async fn expense_rows(
    pool: &sqlx::PgPool,
    sql: &'static str,
) -> Result<Vec<ExpenseRow>, sqlx::Error> {
    sqlx::query_as::<_, ExpenseRow>(sql).fetch_all(pool).await
}

async fn receivable_rows(
    pool: &sqlx::PgPool,
    sql: &'static str,
) -> Result<Vec<ReceivableRow>, sqlx::Error> {
    sqlx::query_as::<_, ReceivableRow>(sql)
        .fetch_all(pool)
        .await
}

async fn share_rows(pool: &sqlx::PgPool, sql: &'static str) -> Result<Vec<ShareRow>, sqlx::Error> {
    sqlx::query_as::<_, ShareRow>(sql).fetch_all(pool).await
}

async fn line_total_rows(
    pool: &sqlx::PgPool,
    sql: &'static str,
    from: &str,
    to: &str,
) -> Result<Vec<LineTotalRow>, sqlx::Error> {
    sqlx::query_as::<_, LineTotalRow>(sql)
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await
}

async fn range_net(pool: &sqlx::PgPool, from: &str, to: &str) -> Result<TextValue, sqlx::Error> {
    sqlx::query_as::<_, TextValue>(RANGE_NET_SELECT)
        .bind(from)
        .bind(to)
        .fetch_one(pool)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trend_months_are_labelled_the_way_the_chart_draws_them() {
        assert_eq!(month_label("2026-01"), "Jan");
        assert_eq!(month_label("2026-09"), "Sep");
        assert_eq!(month_label("2026-12"), "Dec");
        // A key that is not a month shows itself rather than disappearing.
        assert_eq!(month_label("2026-13"), "2026-13");
        assert_eq!(month_label("nonsense"), "nonsense");
    }

    #[test]
    fn a_trend_row_becomes_a_point_with_the_databases_own_digits() {
        let point = trend_point(TrendRow {
            month: "2026-03".into(),
            income: "12000.00".into(),
            expenses: "125.50".into(),
            net: "11874.50".into(),
            total_income: Some("24000.00".into()),
            total_expenses: Some("1000.00".into()),
            total_net: Some("23000.00".into()),
        });
        assert_eq!(point.month, "Mar");
        assert_eq!(point.income.as_str(), "12000.00");
        assert_eq!(point.net.as_str(), "11874.50");
    }

    #[test]
    fn a_period_with_no_lines_totals_zero_rather_than_nothing() {
        let empty: Vec<LineTotalRow> = Vec::new();
        assert_eq!(empty.total().as_str(), "0");
    }

    #[test]
    fn a_line_carries_the_period_total_it_was_read_with() {
        let lines = vec![LineTotalRow {
            label: "COMMISSION".into(),
            amount: "12000.00".into(),
            total: Some("12000.00".into()),
        }];
        assert_eq!(lines.total().as_str(), "12000.00");
        assert_eq!(lines[0].clone().into_line().label, "COMMISSION");
    }
}
