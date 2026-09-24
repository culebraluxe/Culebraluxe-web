//! Accounting V1 — receivables, expenses, and the projections derived from them.
//!
//! SMALL ON PURPOSE. The canonical tables are `account_receivable` and `account_expense` and nothing else; the dashboard
//! and the P&L are DERIVED PROJECTIONS over those two, never persisted as their own models. There is no ledger, no
//! journal, no reconciliation engine and no chart of accounts here, and this module must not grow one.
//!
//! MONEY IS A DECIMAL STRING, NOT AN f64. Postgres stores these amounts as `numeric`; `f64` cannot represent 0.1, and a
//! summary that is a cent out is a summary nobody can reconcile. Every amount crosses the wire as the digits Postgres
//! holds (`amount::text`), every sum is computed BY POSTGRES on `numeric`, and Rust's job is to validate the digits an
//! operator typed rather than to do the arithmetic. That keeps precision without adding a decimal dependency to the
//! workspace: the exact type is the one the database already has.
//!
//! VALIDATION IS HERE, NOT IN THE BROWSER. The rules below are the ones the TypeScript seam enforced
//! (`legacy/db/accounting.ts`), moved into the domain so the same rules apply to every caller — the web bridge, a CLI or
//! a test — and so a caller cannot skip them by not being the UI.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// The three states a receivable can be in. `VOID` is terminal and is never transitioned out of.
pub const RECEIVABLE_STATUSES: [&str; 3] = ["OPEN", "PAID", "VOID"];
/// The three states an expense can be in. Only `POSTED` expenses count as cost in any projection.
pub const EXPENSE_STATUSES: [&str; 3] = ["DRAFT", "POSTED", "VOID"];

/// The controlled expense categories — `lib/accounting/categories.ts`, kept in step with it.
///
/// A closed list rather than a category table: this is a brokerage's own bookkeeping, and a category-administration
/// subsystem would be a schema this domain does not have. Membership is EXACT, as it is in the TypeScript seam, so a
/// spelling this list does not carry is rejected rather than silently stored.
pub const EXPENSE_CATEGORIES: [&str; 9] = [
    "Marketing & Advertising",
    "Professional Fees",
    "Office",
    "Insurance",
    "MLS & Memberships",
    "Travel & Entertainment",
    "Merchant / Bank Fees",
    "Property / Deal Expense",
    "Other",
];

/// The controlled receivable categories. Unlike expenses these are NOT enforced: the seam uppercases what it is given and
/// falls back to `COMMISSION`, and that behaviour is preserved here rather than tightened, because tightening it would
/// reject rows that already exist.
pub const RECEIVABLE_CATEGORIES: [&str; 4] = ["COMMISSION", "LEASING_FEE", "MISC_INCOME", "OTHER"];

/// What can be wrong with an Accounting request. The code is for the client; the message is for a human.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AccountingError {
    #[error("{code}: {message}")]
    Invalid { code: &'static str, message: String },
}

impl AccountingError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self::Invalid {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid { code, .. } => code,
        }
    }
}

/// An amount of money, exactly as Postgres holds it.
///
/// The inner string is the digits: an optional sign, digits, and at most one decimal point. It is deliberately not a
/// number — see the module note — and `parse` is the only way to build one, so a value that reached this type has been
/// checked to be a number at all.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(String);

impl Money {
    /// Validate a decimal the operator (or a form) supplied.
    ///
    /// The scale is NOT capped: Postgres stores what its column allows and rounding is the database's business, not this
    /// function's. What is rejected is anything that is not a decimal number — which is what `Number.isFinite` used to
    /// catch in the browser, moved to where it can be relied on.
    pub fn parse(value: &str) -> Result<Self, AccountingError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(AccountingError::new(
                "AMOUNT_INVALID",
                "Amount is required.",
            ));
        }
        let digits = trimmed.strip_prefix(['+', '-']).unwrap_or(trimmed);
        let mut seen_point = false;
        let mut digit_count = 0;
        for character in digits.chars() {
            match character {
                '.' if !seen_point => seen_point = true,
                '0'..='9' => digit_count += 1,
                _ => {
                    return Err(AccountingError::new(
                        "AMOUNT_INVALID",
                        format!("'{trimmed}' is not a valid amount."),
                    ))
                }
            }
        }
        if digit_count == 0 {
            return Err(AccountingError::new(
                "AMOUNT_INVALID",
                format!("'{trimmed}' is not a valid amount."),
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The amount as Postgres writes it out, for the wire and for the SQL binding.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A value read back out of a `numeric` column.
    ///
    /// Trusting by design, and this is the one place that trust is recorded: a `numeric` column cannot hold anything but
    /// a decimal number, so re-validating every row would be a check that cannot fail. What it must NOT become is
    /// `unwrap`-with-a-default scattered through a DAO, where a surprise would be silently turned into zero.
    pub fn from_database(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Whether the amount is zero or above. Negative amounts are refused on both commands.
    pub fn is_non_negative(&self) -> bool {
        !self.0.starts_with('-')
    }
}

/// One amount of money with its label, in a P&L line or a category total.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PnlLine {
    pub label: String,
    pub amount: Money,
}

/// A receivable, with its associations resolved to display names — never UUIDs in the UI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Receivable {
    pub id: String,
    pub reference: Option<String>,
    pub description: String,
    pub category: String,
    pub amount: Money,
    pub issued_on: String,
    pub due_on: Option<String>,
    /// One of `RECEIVABLE_STATUSES`.
    pub status: String,
    pub paid_on: Option<String>,
    pub deal_id: Option<String>,
    pub deal_name: Option<String>,
    pub property_id: Option<String>,
    pub property_name: Option<String>,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
}

/// An expense, with its associations resolved to display names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Expense {
    pub id: String,
    pub vendor: String,
    pub category: String,
    pub amount: Money,
    pub expense_on: String,
    /// One of `EXPENSE_STATUSES`.
    pub status: String,
    pub memo: Option<String>,
    pub deal_id: Option<String>,
    pub deal_name: Option<String>,
    pub property_id: Option<String>,
    pub property_name: Option<String>,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
}

/// One month of the six-month trend on the dashboard.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PnlTrendPoint {
    /// The month as it is drawn: `Jan`, `Feb`, …
    pub month: String,
    pub income: Money,
    pub expenses: Money,
    pub net: Money,
}

/// A period's projection: the income and expense lines, their totals, and the difference.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PnlStatement {
    /// The period actually projected, echoed back so the screen can show what it asked for.
    pub from: String,
    pub to: String,
    pub income: Vec<PnlLine>,
    pub total_income: Money,
    pub expenses: Vec<PnlLine>,
    pub total_expenses: Money,
    pub net_income: Money,
}

/// One month of income and cost, as the trend chart draws it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MonthAmounts {
    /// `YYYY-MM`, in the database's own sortable form.
    pub month: String,
    pub amount: Money,
}

/// One category, its total, and its share of the total — the breakdown's three numbers.
///
/// THE SHARE IS COMPUTED BY POSTGRES. The live screen divided `amount / total` in the browser, which is a float division
/// of money; here it is `numeric` arithmetic in the query and travels as a rounded percentage. A ring chart is drawn from
/// shares, and a share that came from a float is a share that can disagree with the figures printed beside it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CategoryShare {
    pub label: String,
    pub amount: Money,
    pub percent: i64,
}

/// The dashboard's numbers, all of them projections over the two tables.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AccountingDashboard {
    /// Sum of every `OPEN` receivable.
    pub receivables_outstanding: Money,
    /// Sum of every `POSTED` expense dated in the current calendar month.
    pub expenses_this_month: Money,
    /// Paid receivables minus posted expenses, all time — the same figure the live screen showed.
    pub net_income: Money,
    /// `OPEN` receivables not yet past their due date, plus those with no due date.
    pub open_count: i64,
    /// `OPEN` receivables whose due date has passed.
    pub overdue_count: i64,
    pub pnl_trend: Vec<PnlTrendPoint>,
    /// The six trend months added up, which the chart's own header prints.
    pub trend_income: Money,
    pub trend_expenses: Money,
    pub trend_net: Money,
    /// The five most recent `POSTED` expenses.
    pub recent_expenses: Vec<Expense>,
    /// The six most recent receivables that are not `VOID`.
    pub recent_activity: Vec<Receivable>,
    /// This month's `POSTED` expenses by category, largest first, with each one's share.
    pub expense_categories: Vec<CategoryShare>,
}

/// The period a P&L projection covers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PnlRequest {
    pub from: String,
    pub to: String,
}

impl PnlRequest {
    /// The range has to be two real dates in order. A backwards range is not an empty report, it is a mistake, and
    /// returning zeroes for it would look like a business fact.
    pub fn validate(&self) -> Result<(), AccountingError> {
        let from = date(&self.from, "PNL_FROM_INVALID", "From date")?;
        let to = date(&self.to, "PNL_TO_INVALID", "To date")?;
        if from > to {
            return Err(AccountingError::new(
                "PNL_RANGE_INVALID",
                format!(
                    "The period starts after it ends: {} to {}.",
                    self.from, self.to
                ),
            ));
        }
        Ok(())
    }
}

/// The result of a receivable's status transition, as the screen reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MarkReceivablePaidOutcome {
    pub id: String,
    pub status: String,
    pub paid_on: String,
}

/// Create an expense. The four required fields are the four the live form collected.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateExpenseCommand {
    pub vendor: String,
    /// Must be one of `EXPENSE_CATEGORIES`, exactly.
    pub category: String,
    pub amount: String,
    pub expense_on: String,
    pub memo: Option<String>,
    /// The optional associations, carried only when the caller has them.
    pub deal_id: Option<String>,
    pub property_id: Option<String>,
    pub person_id: Option<String>,
}

impl CreateExpenseCommand {
    pub fn validate(&self) -> Result<(), AccountingError> {
        if self.vendor.trim().is_empty() {
            return Err(AccountingError::new(
                "VENDOR_REQUIRED",
                "Vendor is required.",
            ));
        }
        if !EXPENSE_CATEGORIES.contains(&self.category.as_str()) {
            return Err(AccountingError::new(
                "EXPENSE_CATEGORY_INVALID",
                format!("'{}' is not an expense category.", self.category),
            ));
        }
        let amount = Money::parse(&self.amount)?;
        if !amount.is_non_negative() {
            return Err(AccountingError::new(
                "AMOUNT_INVALID",
                "Amount must be a non-negative number.",
            ));
        }
        date(&self.expense_on, "EXPENSE_ON_INVALID", "Expense date")?;
        Ok(())
    }

    /// What is stored: trimmed, with blank optional text treated as absent rather than as an empty string.
    pub fn normalised(&self) -> Self {
        Self {
            vendor: self.vendor.trim().to_owned(),
            category: self.category.clone(),
            amount: self.amount.trim().to_owned(),
            expense_on: self.expense_on.trim().to_owned(),
            memo: trimmed_or_none(&self.memo),
            deal_id: trimmed_or_none(&self.deal_id),
            property_id: trimmed_or_none(&self.property_id),
            person_id: trimmed_or_none(&self.person_id),
        }
    }
}

/// Create a receivable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateReceivableCommand {
    pub reference: Option<String>,
    pub description: String,
    pub category: String,
    pub amount: String,
    pub issued_on: String,
    pub due_on: Option<String>,
    pub deal_id: Option<String>,
    pub property_id: Option<String>,
    pub person_id: Option<String>,
}

impl CreateReceivableCommand {
    pub fn validate(&self) -> Result<(), AccountingError> {
        if self.description.trim().is_empty() {
            return Err(AccountingError::new(
                "DESCRIPTION_REQUIRED",
                "Description is required.",
            ));
        }
        let amount = Money::parse(&self.amount)?;
        if !amount.is_non_negative() {
            return Err(AccountingError::new(
                "AMOUNT_INVALID",
                "Amount must be a non-negative number.",
            ));
        }
        date(&self.issued_on, "ISSUED_ON_INVALID", "Issue date")?;
        if let Some(due_on) = trimmed_or_none(&self.due_on) {
            date(&due_on, "DUE_ON_INVALID", "Due date")?;
        }
        Ok(())
    }

    pub fn normalised(&self) -> Self {
        Self {
            reference: trimmed_or_none(&self.reference),
            description: self.description.trim().to_owned(),
            // The seam's rule, kept: uppercased, and `COMMISSION` when the caller gave nothing usable.
            category: normalise_receivable_category(&self.category),
            amount: self.amount.trim().to_owned(),
            issued_on: self.issued_on.trim().to_owned(),
            due_on: trimmed_or_none(&self.due_on),
            deal_id: trimmed_or_none(&self.deal_id),
            property_id: trimmed_or_none(&self.property_id),
            person_id: trimmed_or_none(&self.person_id),
        }
    }
}

/// Mark a receivable paid. Whether it may be is the database's answer, not the caller's — see the DAO.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MarkReceivablePaidCommand {
    pub receivable_id: String,
    pub paid_on: String,
}

impl MarkReceivablePaidCommand {
    pub fn validate(&self) -> Result<(), AccountingError> {
        if self.receivable_id.trim().is_empty() {
            return Err(AccountingError::new(
                "RECEIVABLE_REQUIRED",
                "A receivable is required.",
            ));
        }
        date(&self.paid_on, "PAID_ON_INVALID", "Paid date")?;
        Ok(())
    }

    pub fn normalised(&self) -> Self {
        Self {
            receivable_id: self.receivable_id.trim().to_owned(),
            paid_on: self.paid_on.trim().to_owned(),
        }
    }
}

/// The category spelling a receivable is stored under: uppercased, defaulting to `COMMISSION`.
pub fn normalise_receivable_category(category: &str) -> String {
    let trimmed = category.trim().to_uppercase();
    if trimmed.is_empty() {
        "COMMISSION".to_owned()
    } else {
        trimmed
    }
}

/// Whether a spelling is a canonical expense category. Exact, deliberately: see `EXPENSE_CATEGORIES`.
pub fn is_expense_category(category: &str) -> bool {
    EXPENSE_CATEGORIES.contains(&category)
}

/// A trimmed optional string, with the empty string treated as no value at all.
pub fn trimmed_or_none(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Parse a `YYYY-MM-DD` date, naming the field when it is not one.
fn date(value: &str, code: &'static str, label: &str) -> Result<NaiveDate, AccountingError> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map_err(|_| AccountingError::new(code, format!("{label} must be a date, as YYYY-MM-DD.")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expense() -> CreateExpenseCommand {
        CreateExpenseCommand {
            vendor: "Sunrise Fuel".into(),
            category: "Office".into(),
            amount: "125.50".into(),
            expense_on: "2026-03-04".into(),
            memo: Some("  ".into()),
            ..Default::default()
        }
    }

    fn receivable() -> CreateReceivableCommand {
        CreateReceivableCommand {
            reference: Some("INV-1".into()),
            description: "Closing commission".into(),
            category: String::new(),
            amount: "12000".into(),
            issued_on: "2026-03-01".into(),
            due_on: Some("2026-03-31".into()),
            ..Default::default()
        }
    }

    #[test]
    fn money_accepts_decimals_and_refuses_prose() {
        assert_eq!(Money::parse(" 125.50 ").unwrap().as_str(), "125.50");
        assert_eq!(Money::parse("0").unwrap().as_str(), "0");
        // A scale is kept as written: rounding is the database column's business, not this type's.
        assert_eq!(Money::parse("1.005").unwrap().as_str(), "1.005");
        for bad in ["1e3", "12,50", "", "  ", ".", "+", "twelve"] {
            assert_eq!(Money::parse(bad).unwrap_err().code(), "AMOUNT_INVALID");
        }
    }

    #[test]
    fn negative_amounts_are_not_an_amount_this_domain_accepts() {
        assert!(!Money::parse("-0.01").unwrap().is_non_negative());
        assert!(Money::parse("0.00").unwrap().is_non_negative());
    }

    #[test]
    fn expense_requires_a_vendor() {
        let command = CreateExpenseCommand {
            vendor: "   ".into(),
            ..expense()
        };
        assert_eq!(command.validate().unwrap_err().code(), "VENDOR_REQUIRED");
    }

    #[test]
    fn expense_requires_a_canonical_category() {
        // The list is closed and membership is exact, which is what the TypeScript seam enforced.
        let command = CreateExpenseCommand {
            category: "office".into(),
            ..expense()
        };
        assert_eq!(
            command.validate().unwrap_err().code(),
            "EXPENSE_CATEGORY_INVALID"
        );
        assert!(is_expense_category("Office"));
        assert!(!is_expense_category("Anything Else"));
    }

    #[test]
    fn expense_requires_a_non_negative_amount() {
        for amount in ["-1", "abc", ""] {
            let command = CreateExpenseCommand {
                amount: amount.into(),
                ..expense()
            };
            assert_eq!(command.validate().unwrap_err().code(), "AMOUNT_INVALID");
        }
        assert!(expense().validate().is_ok());
    }

    #[test]
    fn expense_requires_a_real_date() {
        let command = CreateExpenseCommand {
            expense_on: "04/03/2026".into(),
            ..expense()
        };
        assert_eq!(command.validate().unwrap_err().code(), "EXPENSE_ON_INVALID");
    }

    #[test]
    fn receivable_requires_a_description() {
        let command = CreateReceivableCommand {
            description: "  ".into(),
            ..receivable()
        };
        assert_eq!(
            command.validate().unwrap_err().code(),
            "DESCRIPTION_REQUIRED"
        );
    }

    #[test]
    fn receivable_requires_a_non_negative_amount() {
        let command = CreateReceivableCommand {
            amount: "-0.01".into(),
            ..receivable()
        };
        assert_eq!(command.validate().unwrap_err().code(), "AMOUNT_INVALID");
        assert!(receivable().validate().is_ok());
    }

    #[test]
    fn receivable_requires_real_dates_including_the_optional_one() {
        let issued = CreateReceivableCommand {
            issued_on: "March 1".into(),
            ..receivable()
        };
        assert_eq!(issued.validate().unwrap_err().code(), "ISSUED_ON_INVALID");

        let due = CreateReceivableCommand {
            due_on: Some("2026-13-40".into()),
            ..receivable()
        };
        assert_eq!(due.validate().unwrap_err().code(), "DUE_ON_INVALID");

        // An absent due date is a real state, not an error.
        let undated = CreateReceivableCommand {
            due_on: Some("  ".into()),
            ..receivable()
        };
        assert!(undated.validate().is_ok());
        assert_eq!(undated.normalised().due_on, None);
    }

    #[test]
    fn receivable_category_keeps_the_seams_behaviour() {
        // Uppercased, and `COMMISSION` when there is nothing to uppercase — not rejected, because rows like that exist.
        assert_eq!(normalise_receivable_category("leasing_fee"), "LEASING_FEE");
        assert_eq!(normalise_receivable_category("  "), "COMMISSION");
        assert_eq!(normalise_receivable_category("Anything"), "ANYTHING");
        assert_eq!(receivable().normalised().category, "COMMISSION");
    }

    #[test]
    fn mark_paid_requires_a_receivable_and_a_real_date() {
        let missing = MarkReceivablePaidCommand {
            receivable_id: "  ".into(),
            paid_on: "2026-03-10".into(),
        };
        assert_eq!(
            missing.validate().unwrap_err().code(),
            "RECEIVABLE_REQUIRED"
        );

        let bad_date = MarkReceivablePaidCommand {
            receivable_id: "abc".into(),
            paid_on: String::new(),
        };
        assert_eq!(bad_date.validate().unwrap_err().code(), "PAID_ON_INVALID");

        let ok = MarkReceivablePaidCommand {
            receivable_id: "  abc  ".into(),
            paid_on: " 2026-03-10 ".into(),
        };
        assert!(ok.validate().is_ok());
        assert_eq!(ok.normalised().receivable_id, "abc");
        assert_eq!(ok.normalised().paid_on, "2026-03-10");
    }

    #[test]
    fn pnl_range_must_be_two_dates_in_order() {
        assert!(PnlRequest {
            from: "2026-01-01".into(),
            to: "2026-03-31".into(),
        }
        .validate()
        .is_ok());

        // A backwards range is a mistake, not an empty report.
        assert_eq!(
            PnlRequest {
                from: "2026-04-01".into(),
                to: "2026-03-31".into(),
            }
            .validate()
            .unwrap_err()
            .code(),
            "PNL_RANGE_INVALID"
        );

        assert_eq!(
            PnlRequest {
                from: "nonsense".into(),
                to: "2026-03-31".into(),
            }
            .validate()
            .unwrap_err()
            .code(),
            "PNL_FROM_INVALID"
        );

        // A single day is a period.
        assert!(PnlRequest {
            from: "2026-03-31".into(),
            to: "2026-03-31".into(),
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn money_serialises_as_the_databases_own_digits() {
        // The wire shape matters as much as the validation: a client must receive `"12000.00"`, never `12000`.
        let payload = serde_json::to_string(&Receivable {
            amount: Money::parse("12000.00").unwrap(),
            ..Default::default()
        })
        .unwrap();
        assert!(payload.contains(r#""amount":"12000.00""#), "{payload}");
        let round_tripped: Receivable = serde_json::from_str(&payload).unwrap();
        assert_eq!(round_tripped.amount.as_str(), "12000.00");
    }

    #[test]
    fn expense_normalisation_trims_and_blanks_the_empty_optionals() {
        let normalised = expense().normalised();
        assert_eq!(normalised.vendor, "Sunrise Fuel");
        assert_eq!(normalised.amount, "125.50");
        assert_eq!(normalised.memo, None);
    }
}
