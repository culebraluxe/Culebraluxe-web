//! Moved from `accounting.rs` (move only): ReceivableRow, into_domain, ExpenseRow, TextValue, money, CountsRow, ShareRow, into_share, TrendRow, ForecastTotalRow, ForecastItemRow, PaidRow, IdRow, MONTH_LABELS, AccountingDao, new, RECEIVABLE_SELECT, EXPENSE_SELECT, OUTSTANDING_SELECT, EXPENSES_THIS_MONTH_SELECT, COUNTS_SELECT, NET_INCOME_SELECT, TREND_SELECT.

#[allow(unused_imports)]
use super::*;

/// One receivable row, with its associations already resolved to names.
#[derive(Debug, Clone, FromRow)]
pub(super) struct ReceivableRow {
    pub(super) id: String,
    pub(super) reference: Option<String>,
    pub(super) description: String,
    pub(super) category: String,
    pub(super) amount: String,
    pub(super) issued_on: String,
    pub(super) due_on: Option<String>,
    pub(super) status: String,
    pub(super) paid_on: Option<String>,
    pub(super) deal_id: Option<String>,
    pub(super) deal_name: Option<String>,
    pub(super) property_id: Option<String>,
    pub(super) property_name: Option<String>,
    pub(super) person_id: Option<String>,
    pub(super) person_name: Option<String>,
}

impl ReceivableRow {
    pub(super) fn into_domain(self) -> Receivable {
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
pub(super) struct ExpenseRow {
    pub(super) id: String,
    pub(super) vendor: String,
    pub(super) category: String,
    pub(super) amount: String,
    pub(super) expense_on: String,
    pub(super) status: String,
    pub(super) memo: Option<String>,
    pub(super) deal_id: Option<String>,
    pub(super) deal_name: Option<String>,
    pub(super) property_id: Option<String>,
    pub(super) property_name: Option<String>,
    pub(super) person_id: Option<String>,
    pub(super) person_name: Option<String>,
}

impl ExpenseRow {
    pub(super) fn into_domain(self) -> Expense {
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
pub(super) struct TextValue {
    pub(super) v: Option<String>,
}

impl TextValue {
    pub(super) fn money(&self) -> Money {
        Money::from_database(self.v.clone().unwrap_or_else(|| "0".to_owned()))
    }
}

/// The open/overdue counts, which are counts rather than amounts.
#[derive(Debug, Clone, FromRow)]
pub(super) struct CountsRow {
    pub(super) open_count: i64,
    pub(super) overdue_count: i64,
}

/// One category in the dashboard's breakdown: its total and its share, both from the query.
#[derive(Debug, Clone, FromRow)]
pub(super) struct ShareRow {
    pub(super) label: String,
    pub(super) amount: String,
    pub(super) percent: i64,
}

impl ShareRow {
    pub(super) fn into_share(self) -> CategoryShare {
        CategoryShare {
            label: self.label,
            amount: Money::from_database(self.amount),
            percent: self.percent,
        }
    }
}

/// One month of the trend: two sums, their difference and the six-month totals, all computed by Postgres.
#[derive(Debug, Clone, FromRow)]
pub(super) struct TrendRow {
    pub(super) month: String,
    pub(super) income: String,
    pub(super) expenses: String,
    pub(super) net: String,
    pub(super) total_income: Option<String>,
    pub(super) total_expenses: Option<String>,
    pub(super) total_net: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct ForecastTotalRow {
    pub(super) next_30_days: String,
    pub(super) next_60_days: String,
    pub(super) next_90_days: String,
    pub(super) undated_or_past: String,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct ForecastItemRow {
    pub(super) receivable_id: String,
    pub(super) amount: String,
    pub(super) expected_on: String,
    pub(super) expected_on_label: String,
    pub(super) timing_source: String,
    pub(super) deal_id: Option<String>,
    pub(super) property_name: Option<String>,
    pub(super) person_name: Option<String>,
    pub(super) description: String,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct PaidRow {
    pub(super) id: String,
    pub(super) status: String,
    pub(super) paid_on: String,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct IdRow {
    pub(super) id: String,
}

/// The month abbreviations the trend is drawn with. Fixed rather than locale-derived: a chart's axis labels must not
/// change shape with the server's locale, and the live screen's appeared in this form.
pub(super) const MONTH_LABELS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[derive(Clone)]
pub struct AccountingDao {
    pub(super) db: Database,
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

    /// OPEN commission receivables projected onto their strongest known timing fact.
    /// Deal closing date wins; due date and issue date are fallbacks. Every total is a numeric SUM in Postgres.
    pub async fn commission_forecast(&self) -> DbResult<CommissionForecast> {
        let pool = self.db.pool();
        let totals = sqlx::query_as::<_, ForecastTotalRow>(
            r#"
            with forecast as (
              select
                r.amount,
                coalesce(d.closing_date, r.due_on, r.issued_on) as expected_on
              from account_receivable r
              left join deal d on d.id=r.deal_id
              where r.status='OPEN'
                and upper(coalesce(r.category, ''))='COMMISSION'
            )
            select
              coalesce(sum(amount) filter (
                where expected_on >= current_date and expected_on <= current_date + 30
              ), 0)::text as next_30_days,
              coalesce(sum(amount) filter (
                where expected_on >= current_date and expected_on <= current_date + 60
              ), 0)::text as next_60_days,
              coalesce(sum(amount) filter (
                where expected_on >= current_date and expected_on <= current_date + 90
              ), 0)::text as next_90_days,
              coalesce(sum(amount) filter (
                where expected_on < current_date or expected_on is null
              ), 0)::text as undated_or_past
            from forecast
            "#,
        )
        .fetch_one(pool);
        let items = sqlx::query_as::<_, ForecastItemRow>(
            r#"
            select
              r.id::text as receivable_id,
              r.amount::text as amount,
              coalesce(d.closing_date, r.due_on, r.issued_on)::text as expected_on,
              to_char(coalesce(d.closing_date, r.due_on, r.issued_on), 'Mon FMDD, YYYY') as expected_on_label,
              case
                when d.closing_date is not null then 'closing'
                when r.due_on is not null then 'due'
                else 'issued'
              end as timing_source,
              r.deal_id::text as deal_id,
              coalesce(dp.name, p.name) as property_name,
              person.display_name as person_name,
              r.description
            from account_receivable r
            left join deal d on d.id=r.deal_id
            left join property dp on dp.id=d.property_id
            left join property p on p.id=r.property_id
            left join person on person.id=r.person_id
            where r.status='OPEN'
              and upper(coalesce(r.category, ''))='COMMISSION'
            order by coalesce(d.closing_date, r.due_on, r.issued_on) asc, r.created_at asc
            limit 50
            "#,
        )
        .fetch_all(pool);
        let (totals, items) = tokio::try_join!(totals, items)
            .map_err(|error| DbFailure::from_sqlx("accounting.commission_forecast", &error))?;
        Ok(CommissionForecast {
            next_30_days: Money::from_database(totals.next_30_days),
            next_60_days: Money::from_database(totals.next_60_days),
            next_90_days: Money::from_database(totals.next_90_days),
            undated_or_past: Money::from_database(totals.undated_or_past),
            items: items
                .into_iter()
                .map(|row| CommissionForecastItem {
                    receivable_id: row.receivable_id,
                    amount: Money::from_database(row.amount),
                    expected_on: row.expected_on,
                    expected_on_label: row.expected_on_label,
                    timing_source: row.timing_source,
                    deal_id: row.deal_id,
                    property_name: row.property_name,
                    person_name: row.person_name,
                    description: row.description,
                })
                .collect(),
        })
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
pub(super) const RECEIVABLE_SELECT: &str = r#"
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
pub(super) const EXPENSE_SELECT: &str = r#"
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
pub(super) const OUTSTANDING_SELECT: &str = r#"
select coalesce(sum(amount), 0)::text as v from account_receivable where status = 'OPEN'
"#;

/// This month's posted cost.
pub(super) const EXPENSES_THIS_MONTH_SELECT: &str = r#"
select coalesce(sum(amount), 0)::text as v
from account_expense
where status = 'POSTED'
  and date_trunc('month', expense_on) = date_trunc('month', current_date)
"#;

/// Open and overdue receivables. A receivable with no due date is open, never overdue — the live screen's rule, kept,
/// because "no deadline agreed" is not the same as "late".
pub(super) const COUNTS_SELECT: &str = r#"
select
  count(*) filter (where status = 'OPEN' and (due_on is null or due_on >= current_date)) as open_count,
  count(*) filter (where status = 'OPEN' and due_on is not null and due_on < current_date) as overdue_count
from account_receivable
"#;

/// Paid income minus posted cost, all time.
pub(super) const NET_INCOME_SELECT: &str = r#"
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
pub(super) const TREND_SELECT: &str = r#"
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
