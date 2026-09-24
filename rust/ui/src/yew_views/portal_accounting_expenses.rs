//! `/portal/accounting/expenses` — the expense book, and the form that adds to it.
//!
//! PARITY WITH `components/portal/accounting/accounting-expenses.tsx`: the count, the New Expense control, the form with its
//! five fields, the category ring, and the full table with its client/property/deal name, status and memo.
//!
//! ALL FORM STATE IS THE REDUCER'S. `useState` for `showNew` and `useActionState` for the submit are gone with the React
//! component: whether the panel is open, what is typed in each field, whether a command is in flight and what the last one
//! said are four kinds of state held in one `AccountingState`, changed only by messages, and rendered from the model. The
//! DOM never owns a second copy of a field, which is what makes the submit path — validate in Rust, refresh on success —
//! something this screen cannot get out of step with.
//!
//! NOTHING HERE VALIDATES. The amount is not parsed, the category is not checked, the vendor is not required by this file:
//! Rust decides all three, and the form shows what it says. A browser that enforced its own copy of those rules would be the
//! copy that goes stale.

use yew::prelude::*;

use crate::format::format_money;
use crate::model::{Msg, PortalAccountingExpense, PortalAccountingShare};
use crate::yew_views::portal_accounting_shell::{AccountingShell, GlassPanel};
use crate::yew_views::portal_shell::PortalShell;

/// The ring's slice colours, as the live screen drew them.
const DONUT_COLOURS: [&str; 9] = [
    "#c6a15b", "#7dd3fc", "#f0abfc", "#86efac", "#fcd34d", "#fca5a5", "#a5b4fc", "#93c5fd", "#e2e8f0",
];

/// The controlled input styling the live form used.
const INPUT: &str = "mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white \
                     placeholder:text-white/30 focus:border-[var(--portal-gold)] focus:outline-none";
const LABEL: &str = "block text-[11px] font-medium uppercase tracking-wide text-white/60";

/// The categories the operator can choose from.
///
/// THE SAME NINE STRINGS RUST VALIDATES AGAINST, spelled once here because a `<select>` needs its options in the browser.
/// They are not a second set of RULES — the rule is Rust's `EXPENSE_CATEGORIES` — and if the two ever disagree the service
/// refuses the value and the form says so, which is the failure mode we want: a refusal, not a silent acceptance.
const EXPENSE_CATEGORIES: [&str; 9] = [
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

#[derive(Properties, PartialEq)]
pub struct ExpensesProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Expenses;

impl Component for Expenses {
    type Message = ();
    type Properties = ExpensesProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen =
            crate::model::screen("accounting-expenses").expect("the expenses screen is in the registry");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                <AccountingShell eyebrow="Accounting" title="Expenses">
                    { self.body(&props.model, &props.on_msg) }
                </AccountingShell>
            </PortalShell>
        }
    }
}

impl Expenses {
    fn body(&self, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let accounting = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.accounting.as_ref());
        let rows = accounting
            .map(|page| page.expenses.clone())
            .unwrap_or_default();
        let categories = accounting
            .map(|page| page.expense_categories.clone())
            .unwrap_or_default();
        html! {
            <div class="space-y-4">
                { self.header(rows.len(), model, on_msg) }
                if model.can("accounting.write") && model.accounting.expense_open {
                    { self.form(model, on_msg) }
                }
                // THE ROW IS 40/60, as the live screen was. The ring gets two columns of five and the table three, so the
                // table has the width its seven columns need and the row is not two-fifths empty.
                <div class="grid grid-cols-1 gap-4 lg:grid-cols-5">
                    { self.category_panel(&categories) }
                    { self.table(&rows) }
                </div>
            </div>
        }
    }

    /// The count and the control that opens the form.
    fn header(&self, count: usize, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let open = model.accounting.expense_open;
        let onclick = {
            let on_msg = on_msg.clone();
            Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ExpenseFormToggled))
        };
        html! {
            <div class="flex flex-wrap items-center justify-between gap-3">
                <p class="text-sm font-light text-white/60">
                    { format!("{count} expense{}", if count == 1 { "" } else { "s" }) }
                </p>
                if model.can("accounting.write") {
                <button type="button" {onclick}
                    class="rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/60 bg-[var(--portal-gold)]/15 px-4 py-2 text-xs font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)] transition hover:bg-[var(--portal-gold)]/25">
                    { if open { "Cancel" } else { "+ New Expense" } }
                </button>
                }
            </div>
        }
    }
}


impl Expenses {
    /// The create form.
    ///
    /// A `<form>` with a submit handler rather than a POST: the command is an effect, so the browser does not navigate and
    /// the page keeps its state. `prevent_default` is the one thing this file decides that the DOM would otherwise own.
    fn form(&self, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let draft = &model.accounting;
        let field = |makes: fn(String) -> Msg| {
            let on_msg = on_msg.clone();
            Callback::from(move |event: InputEvent| {
                let input: web_sys::HtmlInputElement = event.target_unchecked_into();
                on_msg.emit(makes(input.value()));
            })
        };
        let change = |makes: fn(String) -> Msg| {
            let on_msg = on_msg.clone();
            Callback::from(move |event: Event| {
                let select: web_sys::HtmlSelectElement = event.target_unchecked_into();
                on_msg.emit(makes(select.value()));
            })
        };
        let submit = {
            let on_msg = on_msg.clone();
            Callback::from(move |event: SubmitEvent| {
                event.prevent_default();
                on_msg.emit(Msg::ExpenseSubmitted);
            })
        };
        let pending = draft.submitting;
        html! {
            <GlassPanel title="New Expense">
                <form onsubmit={submit} class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
                    <label class={LABEL}>
                        {"Vendor *"}
                        <input value={draft.expense_vendor.clone()} oninput={field(Msg::ExpenseVendorChanged)}
                            placeholder="e.g. Broker Public Portal" class={INPUT} />
                    </label>
                    <label class={LABEL}>
                        {"Category *"}
                        <select onchange={change(Msg::ExpenseCategoryChanged)}
                            class="mt-1 w-full rounded-md border border-white/15 bg-[#0b1220] px-3 py-2 text-sm text-white focus:border-[var(--portal-gold)] focus:outline-none">
                            <option value="" disabled={true} selected={draft.expense_category.is_empty()}>
                                {"Select category…"}
                            </option>
                            { for EXPENSE_CATEGORIES.iter().map(|category| html! {
                                <option value={*category} selected={draft.expense_category == *category}>
                                    { *category }
                                </option>
                            }) }
                        </select>
                    </label>
                    <label class={LABEL}>
                        {"Amount (USD) *"}
                        <input value={draft.expense_amount.clone()} oninput={field(Msg::ExpenseAmountChanged)}
                            inputmode="decimal" placeholder="0.00" class={INPUT} />
                    </label>
                    <label class={LABEL}>
                        {"Date"}
                        <input value={draft.expense_on.clone()} onchange={change(Msg::ExpenseDateChanged)}
                            type="date" class={INPUT} />
                    </label>
                    <label class={classes!(LABEL, "sm:col-span-2")}>
                        {"Memo"}
                        <input value={draft.expense_memo.clone()} oninput={field(Msg::ExpenseMemoChanged)} class={INPUT} />
                    </label>
                    <div class="flex items-end gap-3 sm:col-span-2 lg:col-span-3">
                        <button type="submit" disabled={pending}
                            class="rounded-md bg-[var(--portal-gold)] px-4 py-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy-deep)] transition hover:brightness-110 disabled:opacity-50">
                            { if pending { "Creating…" } else { "Create Expense" } }
                        </button>
                        if let Some(notice) = draft.notice.clone() {
                            <p class={classes!("text-xs", if notice.ok { "text-emerald-300" } else { "text-rose-300" })}>
                                { notice.message }
                            </p>
                        }
                    </div>
                </form>
            </GlassPanel>
        }
    }
}


impl Expenses {
    /// Expenses by category, as the live screen drew it: a ring, and the categories with their totals.
    ///
    /// THE TOTALS ARE THE DATABASE'S. The live screen summed the rows it had in hand, in the browser, in floats; this draws
    /// from the breakdown the service computed in `numeric`. The ring's centre says how many categories there are, which is
    /// what the live ring said — the amount is already in the legend beside it.
    fn category_panel(&self, categories: &[PortalAccountingShare]) -> Html {
        let non_empty = categories
            .iter()
            .any(|category| !crate::format::is_zero(&category.amount));
        html! {
            <GlassPanel title="Expenses by Category" class={classes!("lg:col-span-2")}>
                if non_empty {
                    <div class="flex items-center gap-5">
                        <div class="relative h-32 w-32 flex-none rounded-full"
                            style={format!("background: conic-gradient({})", ring_stops(categories))}>
                            <div class="absolute inset-[22%] flex items-center justify-center rounded-full bg-[var(--portal-navy-deep)]">
                                <span class="text-[10px] font-medium uppercase tracking-wide text-white/50">
                                    { format!("{} cats", categories.len()) }
                                </span>
                            </div>
                        </div>
                        <ul class="min-w-0 flex-1 space-y-1.5">
                            { for categories.iter().enumerate().map(|(index, category)| html! {
                                <li class="flex items-center gap-2 text-xs">
                                    <span class="h-2 w-2 flex-none rounded-full"
                                        style={format!("background: {}", DONUT_COLOURS[index % DONUT_COLOURS.len()])}></span>
                                    <span class="truncate text-white/70">{ category.label.clone() }</span>
                                    <span class="ml-auto flex-none text-white/90">{ format_money(&category.amount) }</span>
                                </li>
                            }) }
                        </ul>
                    </div>
                } else {
                    <p class="py-6 text-sm font-light text-white/40">{"No posted expenses to visualize."}</p>
                }
            </GlassPanel>
        }
    }
}


impl Expenses {
    /// The book: every expense, newest first, with the name that identifies what it belongs to.
    fn table(&self, rows: &[PortalAccountingExpense]) -> Html {
        html! {
            <GlassPanel title="All Expenses" class={classes!("lg:col-span-3")}>
                <div class="overflow-x-auto">
                    <table class="w-full min-w-[640px] text-left text-sm">
                        <thead>
                            <tr class="border-b border-white/10 text-[10px] font-medium uppercase tracking-[0.16em] text-white/45">
                                <th class="py-2 pr-3">{"Date"}</th>
                                <th class="py-2 pr-3">{"Vendor"}</th>
                                <th class="py-2 pr-3">{"Category"}</th>
                                <th class="py-2 pr-3">{"Client / Property"}</th>
                                <th class="py-2 pr-3 text-right">{"Amount"}</th>
                                <th class="py-2 pr-3">{"Status"}</th>
                                <th class="py-2">{"Memo"}</th>
                            </tr>
                        </thead>
                        <tbody>
                            { for rows.iter().map(|expense| html! {
                                <tr class="border-b border-white/[0.06] hover:bg-white/[0.03]">
                                    <td class="py-2.5 pr-3 text-white/60">
                                        { crate::format::format_date(Some(expense.expense_on.as_str())) }
                                    </td>
                                    <td class="py-2.5 pr-3 text-white/85">{ expense.vendor.clone() }</td>
                                    <td class="py-2.5 pr-3 text-white/70">{ expense.category.clone() }</td>
                                    <td class="py-2.5 pr-3 text-white/70">{ belonging(expense) }</td>
                                    <td class="py-2.5 pr-3 text-right font-medium text-sky-300">
                                        { format_money(&expense.amount) }
                                    </td>
                                    <td class="py-2.5 pr-3">{ expense_status_badge(&expense.status) }</td>
                                    <td class="py-2.5 text-white/50">
                                        <span class="block max-w-[12rem] truncate">
                                            { expense.memo.clone().unwrap_or_else(|| "—".to_string()) }
                                        </span>
                                    </td>
                                </tr>
                            }) }
                            if rows.is_empty() {
                                <tr>
                                    <td colspan="7" class="py-6 text-center text-sm font-light text-white/40">
                                        {"No expenses yet."}
                                    </td>
                                </tr>
                            }
                        </tbody>
                    </table>
                </div>
            </GlassPanel>
        }
    }
}

/// The ring's stops, from the shares the database computed.
///
/// Integer arithmetic on rounded percentages: no float division of money happens here, and the last stop landing a degree
/// short of a full turn is the price of that — a degree of a ring drawn from the figures beside it.
fn ring_stops(categories: &[PortalAccountingShare]) -> String {
    let mut cursor = 0_i64;
    categories
        .iter()
        .enumerate()
        .map(|(index, category)| {
            let from = cursor;
            let to = cursor + category.percent.max(0) * 360 / 100;
            cursor = to;
            format!("{} {from}deg {to}deg", DONUT_COLOURS[index % DONUT_COLOURS.len()])
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// What an expense belongs to: the deal, else the property, else the person — the live screen's order, because a cost on a
/// deal is a deal cost before it is a client's.
fn belonging(expense: &PortalAccountingExpense) -> String {
    expense
        .deal_name
        .clone()
        .or_else(|| expense.property_name.clone())
        .or_else(|| expense.person_name.clone())
        .unwrap_or_else(|| "—".to_string())
}

/// The status pill. `POSTED` is money spent, `VOID` is money that does not count, and anything else is a draft.
fn expense_status_badge(status: &str) -> Html {
    let tone = match status {
        "POSTED" => "border-emerald-400/40 text-emerald-300",
        "VOID" => "border-white/20 text-white/40",
        _ => "border-white/25 text-white/60",
    };
    html! {
        <span class={classes!("inline-flex", "rounded-full", "border", "px-2", "py-0.5", "text-[10px]",
            "font-medium", "uppercase", "tracking-wide", tone)}>
            { status }
        </span>
    }
}

