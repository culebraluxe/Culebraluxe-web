//! `/portal/accounting/pnl` — the profit-and-loss statement for a period the operator chooses.
//!
//! THE PERIOD IS THE SCREEN'S, AND IT IS THE POINT OF THE SCREEN. The live page was a GET form that reloaded
//! `/portal/accounting/pnl?from=…&to=…`, and the generic rows renderer before this replaced it projected a hard-coded
//! `2020-01-01 → today` — a period nobody chose. Here the two dates live in the reducer, Apply asks Rust for EXACTLY that
//! period, and the statement echoes back the period it covers, so the figures on screen and the range above them cannot
//! disagree.
//!
//! WHY THIS IS AN EFFECT RATHER THAN A FORM POST: applying a range is a request for a projection, not a page load. A reload
//! would throw away the rest of the screen's state and make the range a URL a person could edit into something the service
//! never validated.

use yew::prelude::*;

use crate::format::format_money;
use crate::model::{Msg, PortalAccountingLine, PortalAccountingPnl};
use crate::yew_views::portal_accounting_shell::{AccountingShell, GlassPanel};
use crate::yew_views::portal_shell::PortalShell;

const DATE_INPUT: &str =
    "mt-1 block w-44 rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white \
                          focus:border-[var(--portal-gold)] focus:outline-none";
const LABEL: &str = "block text-[11px] font-medium uppercase tracking-wide text-white/60";

#[derive(Properties, PartialEq)]
pub struct PnlProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Pnl;

impl Component for Pnl {
    type Message = ();
    type Properties = PnlProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen =
            crate::model::screen("accounting-pnl").expect("the P&L screen is in the registry");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                <AccountingShell eyebrow="Accounting" title="P&L Statement">
                    { self.body(&props.model, &props.on_msg) }
                </AccountingShell>
            </PortalShell>
        }
    }
}

impl Pnl {
    fn body(&self, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let statement = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.accounting.as_ref())
            .and_then(|accounting| accounting.pnl.clone());
        let Some(statement) = statement else {
            return html! {
                <p class="text-sm font-light text-white/40">{"Loading the period…"}</p>
            };
        };
        let lines = statement.income.len() + statement.expenses.len();
        let negative = statement.net_income.trim().starts_with('-');
        html! {
            <div class="space-y-4">
                { self.range_form(model, on_msg) }
                <div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
                    { self.income_panel(&statement) }
                    { self.expense_panel(&statement) }
                </div>
                <div class={classes!("rounded-[var(--portal-panel-radius)]", "border", "p-5",
                    if negative { "border-rose-400/30 bg-rose-400/10" } else { "border-emerald-400/30 bg-emerald-400/10" })}>
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div>
                            <p class="text-[10px] font-medium uppercase tracking-[0.2em] text-white/60">{"Net Income"}</p>
                            <p class="mt-1 text-xs font-light text-white/50">
                                { format!("Realized income − posted expenses · {lines} line items") }
                            </p>
                        </div>
                        <p class={classes!("font-serif", "text-3xl", "font-light",
                            if negative { "text-rose-300" } else { "text-emerald-300" })}>
                            { format_money(&statement.net_income) }
                        </p>
                    </div>
                </div>
            </div>
        }
    }

    /// The period, and the control that asks for it.
    ///
    /// The two fields are the reducer's, seeded from the period the statement covers. Applying sends exactly what they hold:
    /// an empty field means "the current month", which is what the bridge projects and what the echo will show, so the
    /// screen is never displaying a period it did not ask for.
    fn range_form(&self, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let draft = &model.accounting;
        let field = |makes: fn(String) -> Msg| {
            let on_msg = on_msg.clone();
            Callback::from(move |event: Event| {
                let input: web_sys::HtmlInputElement = event.target_unchecked_into();
                on_msg.emit(makes(input.value()));
            })
        };
        let submit = {
            let on_msg = on_msg.clone();
            Callback::from(move |event: SubmitEvent| {
                event.prevent_default();
                on_msg.emit(Msg::PnlApplied);
            })
        };
        html! {
            <GlassPanel title="Date Range">
                <form onsubmit={submit} class="flex flex-wrap items-end gap-3">
                    <label class={LABEL}>
                        {"From"}
                        <input type="date" value={draft.pnl_from.clone()} onchange={field(Msg::PnlFromChanged)}
                            class={DATE_INPUT} />
                    </label>
                    <label class={LABEL}>
                        {"To"}
                        <input type="date" value={draft.pnl_to.clone()} onchange={field(Msg::PnlToChanged)}
                            class={DATE_INPUT} />
                    </label>
                    <button type="submit"
                        class="rounded-md border border-[var(--portal-gold)]/60 bg-[var(--portal-gold)]/15 px-4 py-2 text-xs font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)] transition hover:bg-[var(--portal-gold)]/25">
                        {"Apply"}
                    </button>
                </form>
            </GlassPanel>
        }
    }
}

impl Pnl {
    /// The income side: each category as the database grouped it, then the period's total.
    ///
    /// The labels are the categories as stored, with their underscores shown as spaces — `MISC_INCOME` reads as
    /// `MISC INCOME`, which is what the live statement printed.
    fn income_panel(&self, statement: &PortalAccountingPnl) -> Html {
        html! {
            <GlassPanel title="Income">
                { self.lines(&statement.income, "No income in this range.", "text-white") }
                <div class="mt-2 flex items-center justify-between border-t border-white/15 pt-3 text-sm">
                    <span class="font-medium uppercase tracking-wide text-white/70">{"Total Income"}</span>
                    <span class="font-serif text-lg text-white">{ format_money(&statement.total_income) }</span>
                </div>
            </GlassPanel>
        }
    }

    /// The cost side, the same shape, in the sky blue the expenses screens use.
    fn expense_panel(&self, statement: &PortalAccountingPnl) -> Html {
        html! {
            <GlassPanel title="Expenses">
                { self.lines(&statement.expenses, "No expenses in this range.", "text-sky-300") }
                <div class="mt-2 flex items-center justify-between border-t border-white/15 pt-3 text-sm">
                    <span class="font-medium uppercase tracking-wide text-white/70">{"Total Expenses"}</span>
                    <span class="font-serif text-lg text-white">{ format_money(&statement.total_expenses) }</span>
                </div>
            </GlassPanel>
        }
    }

    /// One side of the statement, or the sentence that says there is nothing in the period.
    ///
    /// AN EMPTY PERIOD IS A REAL ANSWER, not a failure: a month with no paid commission says so rather than drawing a table
    /// with no rows, which is what the live statement did.
    fn lines(&self, lines: &[PortalAccountingLine], empty: &str, tone: &'static str) -> Html {
        if lines.is_empty() {
            return html! { <p class="py-2 text-sm font-light text-white/40">{ empty }</p> };
        }
        html! {
            <ul class="divide-y divide-white/10">
                { for lines.iter().map(|line| html! {
                    <li class="flex items-center justify-between py-2.5 text-sm">
                        <span class="text-white/75">{ line.label.replace('_', " ") }</span>
                        <span class={classes!("font-medium", tone)}>{ format_money(&line.amount) }</span>
                    </li>
                }) }
            </ul>
        }
    }
}
