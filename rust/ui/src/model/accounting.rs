//! Accounting shapes: the book, commission forecast, lines, shares, receivables, expenses, P&L.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingPage {
    /// `/portal/accounting` — the projections over the two tables.
    pub dashboard: Option<PortalAccountingDashboard>,
    /// `/portal/accounting` — expected OPEN commissions by strongest timing fact.
    pub commission_forecast: Option<PortalCommissionForecast>,
    /// `/portal/accounting/expenses`.
    pub expenses: Vec<PortalAccountingExpense>,
    /// `/portal/accounting/expenses` — every posted expense by category with its share, so the screen's ring is drawn from
    /// one aggregation instead of a second sum computed in the browser.
    pub expense_categories: Vec<PortalAccountingShare>,
    /// `/portal/accounting/receivables`.
    pub receivables: Vec<PortalAccountingReceivable>,
    /// `/portal/accounting/pnl` — the period the caller asked for, echoed back.
    pub pnl: Option<PortalAccountingPnl>,
    /// The database's idea of today, so a new record's date field starts on the book's day rather than the browser's.
    pub today: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCommissionForecast {
    pub next_30_days: String,
    pub next_60_days: String,
    pub next_90_days: String,
    pub undated_or_past: String,
    pub items: Vec<PortalCommissionForecastItem>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCommissionForecastItem {
    pub receivable_id: String,
    pub amount: String,
    pub expected_on: String,
    pub expected_on_label: String,
    pub timing_source: String,
    pub deal_id: Option<String>,
    pub property_name: Option<String>,
    pub person_name: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingLine {
    pub label: String,
    pub amount: String,
}

/// A category's total and its share of the month, both computed by the database.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingShare {
    pub label: String,
    pub amount: String,
    pub percent: i64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingTrendPoint {
    pub month: String,
    pub income: String,
    pub expenses: String,
    pub net: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingDashboard {
    pub receivables_outstanding: String,
    pub expenses_this_month: String,
    pub net_income: String,
    pub open_count: i64,
    pub overdue_count: i64,
    pub pnl_trend: Vec<PortalAccountingTrendPoint>,
    pub trend_income: String,
    pub trend_expenses: String,
    pub trend_net: String,
    pub recent_expenses: Vec<PortalAccountingExpense>,
    pub recent_activity: Vec<PortalAccountingReceivable>,
    pub expense_categories: Vec<PortalAccountingShare>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingReceivable {
    pub id: String,
    pub reference: Option<String>,
    pub description: String,
    pub category: String,
    pub amount: String,
    pub issued_on: String,
    pub due_on: Option<String>,
    /// `OPEN`, `PAID` or `VOID`.
    pub status: String,
    pub paid_on: Option<String>,
    pub deal_id: Option<String>,
    pub deal_name: Option<String>,
    pub property_id: Option<String>,
    pub property_name: Option<String>,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingExpense {
    pub id: String,
    pub vendor: String,
    pub category: String,
    pub amount: String,
    pub expense_on: String,
    /// `DRAFT`, `POSTED` or `VOID`.
    pub status: String,
    pub memo: Option<String>,
    pub deal_id: Option<String>,
    pub deal_name: Option<String>,
    pub property_id: Option<String>,
    pub property_name: Option<String>,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingPnl {
    pub from: String,
    pub to: String,
    pub income: Vec<PortalAccountingLine>,
    pub total_income: String,
    pub expenses: Vec<PortalAccountingLine>,
    pub total_expenses: String,
    pub net_income: String,
}
