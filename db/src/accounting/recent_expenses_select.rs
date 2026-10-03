//! Moved from `accounting.rs` (move only): RECENT_EXPENSES_SELECT, RECENT_RECEIVABLES_SELECT, EXPENSE_CATEGORY_SHARES_SELECT, CATEGORY_BREAKDOWN_SELECT, INCOME_LINES_SELECT, EXPENSE_LINES_SELECT, RANGE_NET_SELECT, LineTotalRow, into_line, LineTotals, total, month_label, trend_point, text_one, count_rows, trend_rows, expense_rows, receivable_rows, share_rows, line_total_rows, range_net.

#[allow(unused_imports)]
use super::*;

/// The five most recent posted expenses, spelled out rather than parameterised: a `limit` cannot be bound into a
/// projection that also has to keep the join list, and two copies of a projection drift.
pub(super) const RECENT_EXPENSES_SELECT: &str = r#"
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
pub(super) const RECENT_RECEIVABLES_SELECT: &str = r#"
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
pub(super) const EXPENSE_CATEGORY_SHARES_SELECT: &str = r#"
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
pub(super) const CATEGORY_BREAKDOWN_SELECT: &str = r#"
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
pub(super) const INCOME_LINES_SELECT: &str = r#"
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
pub(super) const EXPENSE_LINES_SELECT: &str = r#"
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
pub(super) const RANGE_NET_SELECT: &str = r#"
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
pub(super) struct LineTotalRow {
    pub(super) label: String,
    pub(super) amount: String,
    pub(super) total: Option<String>,
}

impl LineTotalRow {
    pub(super) fn into_line(self) -> PnlLine {
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
pub(super) trait LineTotals {
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
pub(super) fn month_label(month: &str) -> String {
    month
        .split_once('-')
        .and_then(|(_, number)| number.parse::<usize>().ok())
        .and_then(|number| MONTH_LABELS.get(number.saturating_sub(1)))
        .map(|label| (*label).to_owned())
        .unwrap_or_else(|| month.to_owned())
}

pub(super) fn trend_point(row: TrendRow) -> PnlTrendPoint {
    PnlTrendPoint {
        month: month_label(&row.month),
        income: Money::from_database(row.income),
        expenses: Money::from_database(row.expenses),
        net: Money::from_database(row.net),
    }
}

/// The read helpers behind `dashboard` and `pnl`, returning `sqlx` errors so the caller can name the operation once for
/// the whole projection rather than once per query.
pub(super) async fn text_one(
    pool: &sqlx::PgPool,
    sql: &'static str,
) -> Result<TextValue, sqlx::Error> {
    sqlx::query_as::<_, TextValue>(sql).fetch_one(pool).await
}

pub(super) async fn count_rows(pool: &sqlx::PgPool) -> Result<CountsRow, sqlx::Error> {
    sqlx::query_as::<_, CountsRow>(COUNTS_SELECT)
        .fetch_one(pool)
        .await
}

pub(super) async fn trend_rows(pool: &sqlx::PgPool) -> Result<Vec<TrendRow>, sqlx::Error> {
    sqlx::query_as::<_, TrendRow>(TREND_SELECT)
        .fetch_all(pool)
        .await
}

pub(super) async fn expense_rows(
    pool: &sqlx::PgPool,
    sql: &'static str,
) -> Result<Vec<ExpenseRow>, sqlx::Error> {
    sqlx::query_as::<_, ExpenseRow>(sql).fetch_all(pool).await
}

pub(super) async fn receivable_rows(
    pool: &sqlx::PgPool,
    sql: &'static str,
) -> Result<Vec<ReceivableRow>, sqlx::Error> {
    sqlx::query_as::<_, ReceivableRow>(sql)
        .fetch_all(pool)
        .await
}

pub(super) async fn share_rows(
    pool: &sqlx::PgPool,
    sql: &'static str,
) -> Result<Vec<ShareRow>, sqlx::Error> {
    sqlx::query_as::<_, ShareRow>(sql).fetch_all(pool).await
}

pub(super) async fn line_total_rows(
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

pub(super) async fn range_net(
    pool: &sqlx::PgPool,
    from: &str,
    to: &str,
) -> Result<TextValue, sqlx::Error> {
    sqlx::query_as::<_, TextValue>(RANGE_NET_SELECT)
        .bind(from)
        .bind(to)
        .fetch_one(pool)
        .await
}
