//! ACCOUNTING — five screens on one book: Dashboard, Receivables, Expenses, P&L Statement and Receipt Scanner.
//!
//! ONE MODEL, ONE `update`, FIVE THIN SCREENS. The five read the same payload (`PortalPage.accounting`) and share one form
//! state (`AccountingState`), so the state and every rule about it live here once; each screen supplies only which page it
//! reads, which command body it builds (through `Msg`), and its view.
//!
//! A WRITE ANSWERS WITH THE REFRESHED PAGE. `createExpense`, `createReceivable` and `markReceivablePaid` return the screen
//! the command came from, so the new row is on the page the moment the notice says "Created." — and that is true on every
//! screen that writes, the Receipt Scanner included (the legacy reducer forgot it and left the scanner saying "Creating…").
//!
//! DATES ARE THE BOOK'S. Every date field defaults to the day the database says it is (`accounting.today`), never the
//! browser's: an operator an hour from the server is on a different day than the book.

pub mod dashboard;
pub mod expenses;
pub mod pnl;
pub mod receipt_scanner;
pub mod receivables;
pub mod shell;

use yew::prelude::*;

use crate::app::api::{AccountingCommand, AccountingPnl, PortalScreenPage};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, ScreenCtx};
use crate::app::template;
use crate::model::{
    AccountingState, CommandNotice, PortalAccountingPage, PortalPage, ScannerDraft,
};

pub use dashboard::Dashboard;
pub use expenses::Expenses;
pub use pnl::Pnl;
pub use receipt_scanner::ReceiptScanner;
pub use receivables::Receivables;

/// Everything an Accounting screen remembers: its read of the book, and the forms drawn over it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalAccountingPage>,
    pub accounting: AccountingState,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    /// The screen's page answered (the first read, or a P&L period applied).
    Loaded(Result<PortalPage, ApiError>),
    /// A command answered: the refreshed page, or the service's reason it refused.
    CommandAnswered(Result<PortalPage, ApiError>),

    ExpenseFormToggled,
    ExpenseVendorChanged(String),
    ExpenseCategoryChanged(String),
    ExpenseAmountChanged(String),
    ExpenseDateChanged(String),
    ExpenseMemoChanged(String),
    ExpenseSubmitted,

    ReceivableFormToggled,
    ReceivableReferenceChanged(String),
    ReceivableDescriptionChanged(String),
    ReceivableCategoryChanged(String),
    ReceivableAmountChanged(String),
    ReceivableIssuedOnChanged(String),
    ReceivableDueOnChanged(String),
    ReceivableSubmitted,
    ReceivablePaidDateChanged {
        id: String,
        value: String,
    },
    ReceivablePaidSubmitted {
        id: String,
    },

    PnlFromChanged(String),
    PnlToChanged(String),
    PnlApplied,

    ScannerDragging(bool),
    ScannerFileChosen(String),
    ScannerScanned,
    ScannerCategoryChanged(String),
    ScannerSubmitted,
}

/// The first state and read of the screen with this registry key.
pub(crate) fn init(screen: &'static str) -> (Model, Cmd<Msg>) {
    let read = if screen == "accounting-pnl" {
        // No period yet: the server projects the current month and names it, and the fields take it from the answer.
        Cmd::request(
            AccountingPnl {
                from: String::new(),
                to: String::new(),
            },
            Msg::Loaded,
        )
    } else {
        Cmd::request(PortalScreenPage::of(screen), Msg::Loaded)
    };
    (
        Model {
            read: Remote::Loading,
            accounting: AccountingState::default(),
        },
        read,
    )
}

/// The one reducer. `screen` is the registry key of the screen it runs for; it goes into command bodies, because the
/// command's answer is that screen's page.
pub(crate) fn update(model: &mut Model, msg: Msg, screen: &'static str) -> Cmd<Msg> {
    let state = &mut model.accounting;
    match msg {
        Msg::Loaded(answer) => match book(answer) {
            Ok(book) => {
                model.read = Remote::Loaded(book);
                model.accounting.notice = None;
                take_defaults(model);
            }
            // A failed first read is the screen's failure. A failed re-read (a P&L period the service refused) keeps the
            // statement on screen and says why beside the fields the operator has to change.
            Err(error) if model.read.loaded().is_some() => {
                model.accounting.notice = Some(CommandNotice::failure(error.message));
            }
            Err(error) => model.read = Remote::Failed(error),
        },
        Msg::CommandAnswered(answer) => {
            state.submitting = false;
            match book(answer) {
                Ok(book) => {
                    model.read = Remote::Loaded(book);
                    model.accounting.notice = Some(CommandNotice::success("Created."));
                    take_defaults(model);
                }
                // "Vendor is required.", the 409 for a voided receivable: the service's own words, beside the form.
                Err(error) => {
                    model.accounting.notice = Some(CommandNotice::failure(error.message));
                }
            }
        }

        // ---- the expense form ---------------------------------------------------------------------------------------
        Msg::ExpenseFormToggled => {
            state.expense_open = !state.expense_open;
            state.notice = None;
        }
        Msg::ExpenseVendorChanged(value) => edit(state, |state| state.expense_vendor = value),
        Msg::ExpenseCategoryChanged(value) => edit(state, |state| state.expense_category = value),
        Msg::ExpenseAmountChanged(value) => edit(state, |state| state.expense_amount = value),
        Msg::ExpenseDateChanged(value) => edit(state, |state| state.expense_on = value),
        Msg::ExpenseMemoChanged(value) => edit(state, |state| state.expense_memo = value),
        Msg::ExpenseSubmitted => {
            let body = serde_json::json!({
                "action": "createExpense",
                "screen": screen,
                "vendor": state.expense_vendor,
                "category": state.expense_category,
                "amount": state.expense_amount,
                "expenseOn": state.expense_on,
                "memo": state.expense_memo,
            });
            return command(state, body);
        }

        // ---- the receivable form, and mark-paid ---------------------------------------------------------------------
        Msg::ReceivableFormToggled => {
            state.receivable_open = !state.receivable_open;
            state.notice = None;
        }
        Msg::ReceivableReferenceChanged(value) => {
            edit(state, |state| state.receivable_reference = value)
        }
        Msg::ReceivableDescriptionChanged(value) => {
            edit(state, |state| state.receivable_description = value)
        }
        Msg::ReceivableCategoryChanged(value) => {
            edit(state, |state| state.receivable_category = value)
        }
        Msg::ReceivableAmountChanged(value) => edit(state, |state| state.receivable_amount = value),
        Msg::ReceivableIssuedOnChanged(value) => {
            edit(state, |state| state.receivable_issued_on = value)
        }
        Msg::ReceivableDueOnChanged(value) => edit(state, |state| state.receivable_due_on = value),
        Msg::ReceivableSubmitted => {
            let body = serde_json::json!({
                "action": "createReceivable",
                "screen": screen,
                "reference": state.receivable_reference,
                "description": state.receivable_description,
                "category": state.receivable_category,
                "amount": state.receivable_amount,
                "issuedOn": state.receivable_issued_on,
                "dueOn": state.receivable_due_on,
            });
            return command(state, body);
        }
        // One row's date, not the table's: every row has its own input.
        Msg::ReceivablePaidDateChanged { id, value } => {
            state.paid_on.insert(id, value);
        }
        Msg::ReceivablePaidSubmitted { id } => {
            // The row's own date, else the book's today — the same default its input shows, so what is sent is what is
            // on screen.
            let paid_on = state
                .paid_on
                .get(&id)
                .cloned()
                .unwrap_or_else(|| today(model.read.loaded()));
            let body = serde_json::json!({
                "action": "markReceivablePaid",
                "screen": screen,
                "receivableId": id,
                "paidOn": paid_on,
            });
            return command(&mut model.accounting, body);
        }

        // ---- the P&L's period ---------------------------------------------------------------------------------------
        Msg::PnlFromChanged(value) => edit(state, |state| state.pnl_from = value),
        Msg::PnlToChanged(value) => edit(state, |state| state.pnl_to = value),
        Msg::PnlApplied => {
            // The statement stays on screen while the new period is read; whether the two strings are a valid period is
            // Rust's answer to give.
            state.notice = None;
            return Cmd::request(
                AccountingPnl {
                    from: state.pnl_from.clone(),
                    to: state.pnl_to.clone(),
                },
                Msg::Loaded,
            );
        }

        // ---- the receipt scanner's demonstration --------------------------------------------------------------------
        Msg::ScannerDragging(dragging) => state.scanner.dragging = dragging,
        Msg::ScannerFileChosen(name) => state.scanner.file_name = name,
        Msg::ScannerScanned => {
            // THE DEMONSTRATION'S EXTRACTION, deterministic and cyclical, and deliberately not OCR: pressing Scan again
            // shows the next seed, as the live component did. The date is the book's.
            let book_today = today(model.read.loaded());
            let state = &mut model.accounting;
            let seed = SCANNER_SEEDS[state.scanner.demo_index % SCANNER_SEEDS.len()];
            state.scanner.demo_index += 1;
            if state.scanner.file_name.is_empty() {
                state.scanner.file_name = "demo-receipt.jpg".to_owned();
            }
            state.scanner.draft = Some(ScannerDraft {
                vendor: seed.0.to_owned(),
                amount: seed.1.to_owned(),
                category: seed.2.to_owned(),
                memo: seed.3.to_owned(),
                expense_on: if book_today.is_empty() {
                    seed.4.to_owned()
                } else {
                    book_today
                },
            });
            state.notice = None;
        }
        Msg::ScannerCategoryChanged(value) => edit(state, |state| {
            if let Some(draft) = state.scanner.draft.as_mut() {
                draft.category = value;
            }
        }),
        Msg::ScannerSubmitted => {
            // Nothing reviewed, nothing to save: an expense made of empty strings is never sent.
            let Some(draft) = state.scanner.draft.clone() else {
                return Cmd::none();
            };
            let body = serde_json::json!({
                "action": "createExpense",
                "screen": screen,
                "vendor": draft.vendor,
                "category": draft.category,
                "amount": draft.amount,
                "expenseOn": draft.expense_on,
                // The receipt's note is saved with it: the review step shows it, so the row can explain itself later.
                "memo": draft.memo,
            });
            return command(state, body);
        }
    }
    Cmd::none()
}

/// A field edit: the value changes, and an old notice about the last save no longer describes the form.
fn edit(state: &mut AccountingState, change: impl FnOnce(&mut AccountingState)) {
    change(state);
    state.notice = None;
}

/// Send one command. ONE AT A TIME: a second press while the first is in flight does nothing, which is what keeps a
/// double-click from recording an expense twice.
fn command(state: &mut AccountingState, body: serde_json::Value) -> Cmd<Msg> {
    if state.submitting {
        return Cmd::none();
    }
    state.submitting = true;
    state.notice = None;
    Cmd::request(AccountingCommand { body }, Msg::CommandAnswered)
}

/// The accounting part of an answer. An answer without it is a failure to say so, not an empty book.
fn book(answer: Result<PortalPage, ApiError>) -> Result<PortalAccountingPage, ApiError> {
    answer.and_then(|page| {
        page.accounting
            .ok_or_else(|| ApiError::decode("The answer had no accounting book in it."))
    })
}

/// The database's idea of today, or empty before the book has been read.
fn today(book: Option<&PortalAccountingPage>) -> String {
    book.map(|book| book.today.clone()).unwrap_or_default()
}

/// Defaults the drafts take from the book — only while they have none of their own, so a date the operator chose is
/// never overwritten by a later refresh.
fn take_defaults(model: &mut Model) {
    let Some(book) = model.read.loaded() else {
        return;
    };
    let state = &mut model.accounting;
    if !book.today.is_empty() {
        if state.expense_on.is_empty() {
            state.expense_on = book.today.clone();
        }
        if state.receivable_issued_on.is_empty() {
            state.receivable_issued_on = book.today.clone();
        }
    }
    // Where the live form's category started; uppercased, which is what the seam stores anyway.
    if state.receivable_category.is_empty() {
        state.receivable_category = "COMMISSION".to_owned();
    }
    // The P&L's fields take the period that was projected, so an operator edits a range they can see.
    if let Some(pnl) = book.pnl.as_ref() {
        if state.pnl_from.is_empty() {
            state.pnl_from = pnl.from.clone();
        }
        if state.pnl_to.is_empty() {
            state.pnl_to = pnl.to.clone();
        }
    }
}

/// The demonstration receipts, in the order the live component cycled them: vendor, amount, category, memo, and a date
/// used only if the book's day is unknown. The amounts go through Rust's validation like any an operator types.
const SCANNER_SEEDS: [(&str, &str, &str, &str, &str); 4] = [
    (
        "Metro Maintenance Co.",
        "412.5",
        "Property / Deal Expense",
        "Walkthrough cleanup \u{2014} demo receipt",
        "2026-01-01",
    ),
    (
        "Wells Fargo Merchant Services",
        "89",
        "Merchant / Bank Fees",
        "Monthly processing \u{2014} demo receipt",
        "2026-01-01",
    ),
    (
        "State Insurance Group",
        "1200",
        "Insurance",
        "E&O premium \u{2014} demo receipt",
        "2026-01-01",
    ),
    (
        "Luxe Signage & Print",
        "235.75",
        "Marketing & Advertising",
        "Listing collateral \u{2014} demo receipt",
        "2026-01-01",
    ),
];

/// What the views read: the book (once it has arrived), the forms, and what the signed-in user may do. Read-only.
pub struct Vm<'a> {
    pub book: Option<&'a PortalAccountingPage>,
    pub accounting: &'a AccountingState,
    ctx: &'a ScreenCtx,
}

impl<'a> Vm<'a> {
    pub fn can(&self, action: &str) -> bool {
        self.ctx.can(action)
    }

    pub fn today(&self) -> String {
        today(self.book)
    }
}

/// Every Accounting screen's frame: the navy shell with its title, then the read's loading or failure, or the body.
pub(crate) fn frame(
    title: &'static str,
    model: &Model,
    ctx: &ScreenCtx,
    link: &Link<Msg>,
    body: impl FnOnce(&Vm<'_>, &Callback<Msg>) -> Html,
) -> Html {
    let content = match &model.read {
        Remote::Failed(error) => template::failure(error),
        Remote::Loaded(book) => body(
            &Vm {
                book: Some(book),
                accounting: &model.accounting,
                ctx,
            },
            &link.callback(|msg: Msg| msg),
        ),
        Remote::Loading | Remote::NotAsked => html! {
            <p class="text-sm font-light text-white/40" data-screen-state="loading">{"Reading the book\u{2026}"}</p>
        },
    };
    html! {
        <shell::AccountingShell eyebrow="Accounting" title={title}>
            { content }
        </shell::AccountingShell>
    }
}

/// One Accounting screen on the `Screen` trait: its registry key, its title and its body. The model, messages and rules
/// are the shared ones above.
macro_rules! accounting_screen {
    ($name:ident, $key:literal, $title:literal, $body:path) => {
        pub struct $name;

        impl crate::app::screen::Screen for $name {
            type Model = super::Model;
            type Msg = super::Msg;

            fn init(
                _ctx: &crate::app::screen::ScreenCtx,
            ) -> (super::Model, crate::app::cmd::Cmd<super::Msg>) {
                super::init($key)
            }

            fn update(
                model: &mut super::Model,
                msg: super::Msg,
                _ctx: &crate::app::screen::ScreenCtx,
            ) -> crate::app::cmd::Cmd<super::Msg> {
                super::update(model, msg, $key)
            }

            fn view(
                model: &super::Model,
                ctx: &crate::app::screen::ScreenCtx,
                link: &crate::app::screen::Link<super::Msg>,
            ) -> yew::Html {
                super::frame($title, model, ctx, link, $body)
            }
        }
    };
}
pub(crate) use accounting_screen;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::cmd::Method;
    use crate::app::screen::Screen;
    use serde_json::json;

    fn fixture(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    fn opened<S: Screen<Model = Model, Msg = Msg>>(answer: serde_json::Value) -> Model {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = S::init(&ctx);
        let request = cmd.into_requests().remove(0);
        S::update(&mut model, request.respond(Ok(answer)), &ctx);
        model
    }

    #[test]
    fn each_screen_reads_its_own_page() {
        let path = |cmd: Cmd<Msg>| cmd.into_requests().remove(0).path;
        let ctx = ScreenCtx::default();
        assert_eq!(
            path(Dashboard::init(&ctx).1),
            "/api/portal/rust-ui/page?screen=accounting"
        );
        assert_eq!(
            path(Expenses::init(&ctx).1),
            "/api/portal/rust-ui/page?screen=accounting-expenses"
        );
        assert_eq!(
            path(Receivables::init(&ctx).1),
            "/api/portal/rust-ui/page?screen=accounting-receivables"
        );
        assert_eq!(
            path(ReceiptScanner::init(&ctx).1),
            "/api/portal/rust-ui/page?screen=accounting-receipt-scanner"
        );
        assert_eq!(
            path(Pnl::init(&ctx).1),
            "/api/portal/rust-ui/page?screen=accounting-pnl&from=&to="
        );
    }

    #[test]
    fn the_dashboard_payload_decodes_and_the_drafts_take_the_books_day() {
        let model = opened::<Dashboard>(fixture(include_str!(
            "../../../../fixtures/portal-page-accounting.json"
        )));
        let book = model.read.loaded().expect("the payload decodes");
        assert_eq!(book.dashboard.as_ref().unwrap().overdue_count, 1);
        assert_eq!(model.accounting.expense_on, "2026-09-26");
        assert_eq!(model.accounting.receivable_issued_on, "2026-09-26");
        assert_eq!(model.accounting.receivable_category, "COMMISSION");
    }

    #[test]
    fn an_answer_without_a_book_is_a_failure_not_an_empty_book() {
        let model = opened::<Dashboard>(json!({}));
        assert!(matches!(model.read, Remote::Failed(ref error) if error.code == "DECODE"));
    }

    #[test]
    fn the_pnl_fields_take_the_projected_period_and_apply_asks_for_theirs() {
        let ctx = ScreenCtx::default();
        let mut model = opened::<Pnl>(fixture(include_str!(
            "../../../../fixtures/portal-page-accounting-pnl.json"
        )));
        assert_eq!(
            (
                model.accounting.pnl_from.as_str(),
                model.accounting.pnl_to.as_str()
            ),
            ("2026-09-01", "2026-09-30")
        );
        Pnl::update(&mut model, Msg::PnlFromChanged("2026-01-01".into()), &ctx);
        let request = Pnl::update(&mut model, Msg::PnlApplied, &ctx)
            .into_requests()
            .remove(0);
        assert_eq!(
            request.path,
            "/api/portal/rust-ui/page?screen=accounting-pnl&from=2026-01-01&to=2026-09-30"
        );
        // A refused period keeps the statement and says why.
        Pnl::update(
            &mut model,
            Msg::Loaded(Err(ApiError::network("from must be before to"))),
            &ctx,
        );
        assert!(model.read.loaded().is_some());
        assert_eq!(
            model.accounting.notice,
            Some(CommandNotice::failure("from must be before to"))
        );
    }

    #[test]
    fn an_expense_is_one_command_at_a_time_and_the_answer_ends_it() {
        let ctx = ScreenCtx::default();
        let mut model = opened::<Expenses>(fixture(include_str!(
            "../../../../fixtures/portal-page-accounting.json"
        )));
        Expenses::update(
            &mut model,
            Msg::ExpenseVendorChanged("Vendor B".into()),
            &ctx,
        );
        Expenses::update(&mut model, Msg::ExpenseAmountChanged("10.00".into()), &ctx);
        let request = Expenses::update(&mut model, Msg::ExpenseSubmitted, &ctx)
            .into_requests()
            .remove(0);
        assert_eq!(request.method, Method::Post);
        assert_eq!(request.path, "/api/portal/rust-ui/accounting");
        let body = request.body.clone().unwrap();
        assert_eq!(body["action"], "createExpense");
        assert_eq!(body["screen"], "accounting-expenses");
        assert_eq!(body["vendor"], "Vendor B");
        assert_eq!(body["expenseOn"], "2026-09-26");
        assert!(model.accounting.submitting);
        assert!(
            Expenses::update(&mut model, Msg::ExpenseSubmitted, &ctx)
                .into_requests()
                .is_empty(),
            "a double press sends nothing"
        );

        Expenses::update(
            &mut model,
            request.respond(Ok(fixture(include_str!(
                "../../../../fixtures/portal-page-accounting.json"
            )))),
            &ctx,
        );
        assert!(!model.accounting.submitting);
        assert_eq!(
            model.accounting.notice,
            Some(CommandNotice::success("Created."))
        );

        let request = Expenses::update(&mut model, Msg::ExpenseSubmitted, &ctx)
            .into_requests()
            .remove(0);
        Expenses::update(
            &mut model,
            request.respond(Err(ApiError::network("Vendor is required."))),
            &ctx,
        );
        assert!(!model.accounting.submitting);
        assert_eq!(
            model.accounting.notice,
            Some(CommandNotice::failure("Vendor is required."))
        );
        assert!(
            model.read.loaded().is_some(),
            "a refused write keeps the book"
        );
    }

    #[test]
    fn mark_paid_sends_the_rows_date_or_the_books_day() {
        let ctx = ScreenCtx::default();
        let mut model = opened::<Receivables>(fixture(include_str!(
            "../../../../fixtures/portal-page-accounting.json"
        )));
        let body = |cmd: Cmd<Msg>| cmd.into_requests().remove(0).body.unwrap();
        let sent = body(Receivables::update(
            &mut model,
            Msg::ReceivablePaidSubmitted { id: "rec-1".into() },
            &ctx,
        ));
        assert_eq!(
            (sent["action"].as_str(), sent["paidOn"].as_str()),
            (Some("markReceivablePaid"), Some("2026-09-26"))
        );
        Receivables::update(
            &mut model,
            Msg::CommandAnswered(Err(ApiError::network("x"))),
            &ctx,
        );
        Receivables::update(
            &mut model,
            Msg::ReceivablePaidDateChanged {
                id: "rec-1".into(),
                value: "2026-09-20".into(),
            },
            &ctx,
        );
        let sent = body(Receivables::update(
            &mut model,
            Msg::ReceivablePaidSubmitted { id: "rec-1".into() },
            &ctx,
        ));
        assert_eq!(sent["paidOn"], "2026-09-20");
        assert_eq!(sent["screen"], "accounting-receivables");
    }

    #[test]
    fn the_scanner_cycles_its_seeds_saves_the_memo_and_is_not_left_creating() {
        let ctx = ScreenCtx::default();
        let mut model = opened::<ReceiptScanner>(fixture(include_str!(
            "../../../../fixtures/portal-page-accounting.json"
        )));
        assert!(
            ReceiptScanner::update(&mut model, Msg::ScannerSubmitted, &ctx)
                .into_requests()
                .is_empty(),
            "no draft, nothing sent"
        );
        ReceiptScanner::update(&mut model, Msg::ScannerScanned, &ctx);
        ReceiptScanner::update(&mut model, Msg::ScannerScanned, &ctx);
        let draft = model.accounting.scanner.draft.clone().unwrap();
        assert_eq!(draft.vendor, "Wells Fargo Merchant Services");
        assert_eq!(draft.expense_on, "2026-09-26");
        assert_eq!(model.accounting.scanner.file_name, "demo-receipt.jpg");

        let request = ReceiptScanner::update(&mut model, Msg::ScannerSubmitted, &ctx)
            .into_requests()
            .remove(0);
        let body = request.body.clone().unwrap();
        assert_eq!(body["screen"], "accounting-receipt-scanner");
        assert_eq!(body["memo"], draft.memo);
        ReceiptScanner::update(
            &mut model,
            request.respond(Ok(fixture(include_str!(
                "../../../../fixtures/portal-page-accounting.json"
            )))),
            &ctx,
        );
        assert!(
            !model.accounting.submitting,
            "the legacy scanner stayed on Creating…"
        );
        assert_eq!(
            model.accounting.notice,
            Some(CommandNotice::success("Created."))
        );
    }
}
