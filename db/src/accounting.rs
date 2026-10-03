//! The Accounting DAO: the two canonical tables, and the projections over them.
//!
//! EVERY SUM IS POSTGRES'S. The amounts are `numeric`; the summary figures are computed by the database on `numeric` and
//! read back as `::text`, so no total in this codebase is an approximation and no decimal arithmetic happens in Rust.
//! That is the whole reason `Money` is a string (see `model::accounting`) and it is why the queries below cast the way
//! they do.
//!
//! THE PROJECTIONS ARE QUERIES, NOT MODELS. The dashboard and the P&L are read here as joins of the same two tables the
//! lists read. Nothing in this file writes a derived row, which is what keeps "the numbers can disagree with the rows"
//! from being a possible state.

use model::{
    AccountingDashboard, CategoryShare, CommissionForecast, CommissionForecastItem,
    CreateExpenseCommand, CreateReceivableCommand, Expense, MarkReceivablePaidCommand,
    MarkReceivablePaidOutcome, Money, PnlLine, PnlRequest, PnlStatement, PnlTrendPoint, Receivable,
};
use sqlx::FromRow;

use crate::{Database, DbFailure, DbResult};
mod receivable_row;
mod recent_expenses_select;
#[allow(unused_imports)]
pub use receivable_row::*;
#[allow(unused_imports)]
pub use recent_expenses_select::*;

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
