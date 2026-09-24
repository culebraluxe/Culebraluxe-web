//! `/portal/accounting/receivables` — what is owed, and the two ways to change it.
//!
//! PARITY WITH `components/portal/accounting/accounting-receivables.tsx`: the count, the New Receivable control, the form
//! with its six fields, and the table with its reference, client/property, description and category, amount, issue date,
//! due date, status — including the OVERDUE presentation, which is a status the database does not carry — and the per-row
//! Mark Paid control with its own date.
//!
//! TWO COMMANDS, ONE REDUCER. Creating a receivable and marking one paid are both effects dispatched from this state; the
//! inline date input beside each row is the reducer's too, keyed by that receivable's id, because "the date this one was
//! paid" is not one value shared across a table.
//!
//! THE TRANSITION IS NOT THIS FILE'S. Mark Paid asks Rust to make the change, and the answer is the refreshed screen: a
//! voided or vanished receivable comes back as the conflict the service reported, and the row is still there saying what it
//! really is.

use yew::prelude::*;

use crate::format::{format_date, format_money};
use crate::model::{Msg, PortalAccountingReceivable};
use crate::yew_views::portal_accounting_shell::{AccountingShell, GlassPanel};
use crate::yew_views::portal_shell::PortalShell;

/// The controlled input styling the live form used.
const INPUT: &str = "mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white \
                     placeholder:text-white/30 focus:border-[var(--portal-gold)] focus:outline-none";
const LABEL: &str = "block text-[11px] font-medium uppercase tracking-wide text-white/60";

/// The receivable categories the form offers, labelled the way the live form labelled them (`_` as a space).
const RECEIVABLE_CATEGORIES: [&str; 4] = ["COMMISSION", "LEASING_FEE", "MISC_INCOME", "OTHER"];

#[derive(Properties, PartialEq)]
pub struct ReceivablesProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Receivables;

impl Component for Receivables {
    type Message = ();
    type Properties = ReceivablesProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("accounting-receivables")
            .expect("the receivables screen is in the registry");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                <AccountingShell eyebrow="Accounting" title="Receivables">
                    { self.body(&props.model, &props.on_msg) }
                </AccountingShell>
            </PortalShell>
        }
    }
}

impl Receivables {
    fn body(&self, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let rows = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.accounting.as_ref())
            .map(|accounting| accounting.receivables.clone())
            .unwrap_or_default();
        html! {
            <div class="space-y-4">
                { self.header(rows.len(), model, on_msg) }
                if model.can("accounting.write") && model.accounting.receivable_open {
                    { self.form(model, on_msg) }
                }
                { self.table(&rows, model, on_msg) }
            </div>
        }
    }

    fn header(&self, count: usize, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let open = model.accounting.receivable_open;
        let onclick = {
            let on_msg = on_msg.clone();
            Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ReceivableFormToggled))
        };
        html! {
            <div class="flex flex-wrap items-center justify-between gap-3">
                <p class="text-sm font-light text-white/60">
                    { format!("{count} receivable{}", if count == 1 { "" } else { "s" }) }
                </p>
                if model.can("accounting.write") {
                <button type="button" {onclick}
                    class="rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/60 bg-[var(--portal-gold)]/15 px-4 py-2 text-xs font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)] transition hover:bg-[var(--portal-gold)]/25">
                    { if open { "Cancel" } else { "+ New Receivable" } }
                </button>
                }
            </div>
        }
    }
}


impl Receivables {
    /// The create form. The same shape as the expense form's: a `<form>` whose submit dispatches a message, every field bound
    /// to the reducer, and no validation that Rust would then have to agree with.
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
                on_msg.emit(Msg::ReceivableSubmitted);
            })
        };
        let pending = draft.submitting;
        html! {
            <GlassPanel title="New Receivable">
                <form onsubmit={submit} class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
                    <label class={LABEL}>
                        {"Reference"}
                        <input value={draft.receivable_reference.clone()} oninput={field(Msg::ReceivableReferenceChanged)}
                            placeholder="e.g. 2026-014 · Commission" class={INPUT} />
                    </label>
                    <label class={LABEL}>
                        {"Description *"}
                        <input value={draft.receivable_description.clone()}
                            oninput={field(Msg::ReceivableDescriptionChanged)} class={INPUT} />
                    </label>
                    <label class={LABEL}>
                        {"Category"}
                        <select onchange={change(Msg::ReceivableCategoryChanged)}
                            class="mt-1 w-full rounded-md border border-white/15 bg-[#0b1220] px-3 py-2 text-sm text-white focus:border-[var(--portal-gold)] focus:outline-none">
                            { for RECEIVABLE_CATEGORIES.iter().map(|category| html! {
                                <option value={*category} selected={draft.receivable_category == *category}>
                                    { category.replace('_', " ") }
                                </option>
                            }) }
                        </select>
                    </label>
                    <label class={LABEL}>
                        {"Amount (USD) *"}
                        <input value={draft.receivable_amount.clone()} oninput={field(Msg::ReceivableAmountChanged)}
                            inputmode="decimal" placeholder="0.00" class={INPUT} />
                    </label>
                    <label class={LABEL}>
                        {"Issue Date"}
                        <input value={draft.receivable_issued_on.clone()}
                            onchange={change(Msg::ReceivableIssuedOnChanged)} type="date" class={INPUT} />
                    </label>
                    <label class={LABEL}>
                        {"Due Date"}
                        <input value={draft.receivable_due_on.clone()}
                            onchange={change(Msg::ReceivableDueOnChanged)} type="date" class={INPUT} />
                    </label>
                    <div class="flex items-end gap-3 sm:col-span-2 lg:col-span-3">
                        <button type="submit" disabled={pending}
                            class="rounded-md bg-[var(--portal-gold)] px-4 py-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy-deep)] transition hover:brightness-110 disabled:opacity-50">
                            { if pending { "Creating…" } else { "Create Receivable" } }
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


impl Receivables {
    /// The book, with each row's status and its Mark Paid control.
    fn table(
        &self,
        rows: &[PortalAccountingReceivable],
        model: &crate::model::Model,
        on_msg: &Callback<Msg>,
    ) -> Html {
        html! {
            <GlassPanel title="All Receivables">
                <div class="overflow-x-auto">
                    <table class="w-full min-w-[760px] text-left text-sm">
                        <thead>
                            <tr class="border-b border-white/10 text-[10px] font-medium uppercase tracking-[0.16em] text-white/45">
                                <th class="py-2 pr-3">{"Reference"}</th>
                                <th class="py-2 pr-3">{"Client / Property"}</th>
                                <th class="py-2 pr-3">{"Description / Category"}</th>
                                <th class="py-2 pr-3 text-right">{"Amount"}</th>
                                <th class="py-2 pr-3">{"Issue"}</th>
                                <th class="py-2 pr-3">{"Due"}</th>
                                <th class="py-2 pr-3">{"Status"}</th>
                                <th class="py-2 text-right">{"Action"}</th>
                            </tr>
                        </thead>
                        <tbody>
                            { for rows.iter().map(|receivable| self.row(receivable, model, on_msg)) }
                            if rows.is_empty() {
                                <tr>
                                    <td colspan="8" class="py-6 text-center text-sm font-light text-white/40">
                                        {"No receivables yet."}
                                    </td>
                                </tr>
                            }
                        </tbody>
                    </table>
                </div>
            </GlassPanel>
        }
    }

    fn row(
        &self,
        receivable: &PortalAccountingReceivable,
        model: &crate::model::Model,
        on_msg: &Callback<Msg>,
    ) -> Html {
        // OVERDUE IS A PRESENTATION, NOT A STATUS. The database knows OPEN, PAID and VOID; "past its due date" is what the
        // screen says about an open one, and the live screen decided it exactly this way — an ISO date compares as text.
        let today = crate::update::accounting_today(model);
        let overdue = receivable.status == "OPEN"
            && receivable
                .due_on
                .as_deref()
                .is_some_and(|due_on| !today.is_empty() && due_on < today.as_str());
        // The row's own date: the operator's edit, else the book's today — which is what its input shows, so the date sent
        // is the date on screen.
        let paid_on = model
            .accounting
            .paid_on
            .get(&receivable.id)
            .cloned()
            .unwrap_or_else(|| today.clone());
        let pending = model.accounting.submitting;
        let id = receivable.id.clone();
        let on_date = {
            let on_msg = on_msg.clone();
            let id = id.clone();
            Callback::from(move |event: Event| {
                let input: web_sys::HtmlInputElement = event.target_unchecked_into();
                on_msg.emit(Msg::ReceivablePaidDateChanged {
                    id: id.clone(),
                    value: input.value(),
                });
            })
        };
        let on_paid = {
            let on_msg = on_msg.clone();
            Callback::from(move |event: SubmitEvent| {
                event.prevent_default();
                on_msg.emit(Msg::ReceivablePaidSubmitted { id: id.clone() });
            })
        };
        html! {
            <tr class="border-b border-white/[0.06] hover:bg-white/[0.03]">
                <td class="py-2.5 pr-3 text-white/85">
                    { receivable.reference.clone().unwrap_or_else(|| "—".to_string()) }
                </td>
                <td class="py-2.5 pr-3 text-white/70">{ belonging(receivable) }</td>
                <td class="py-2.5 pr-3">
                    <p class="text-white/85">{ receivable.description.clone() }</p>
                    <p class="text-[11px] text-white/45">{ receivable.category.replace('_', " ") }</p>
                </td>
                <td class="py-2.5 pr-3 text-right font-medium text-white">{ format_money(&receivable.amount) }</td>
                <td class="py-2.5 pr-3 text-white/60">{ format_date(Some(receivable.issued_on.as_str())) }</td>
                <td class="py-2.5 pr-3 text-white/60">{ due_label(receivable.due_on.as_deref()) }</td>
                <td class="py-2.5 pr-3">{ status_badge(&receivable.status, overdue) }</td>
                <td class="py-2.5 text-right">
                    if receivable.status == "OPEN" {
                        <form onsubmit={on_paid} class="flex items-center justify-end gap-2">
                            <input type="date" value={paid_on} onchange={on_date}
                                class="w-[7.5rem] rounded border border-white/15 bg-white/5 px-2 py-1 text-xs text-white focus:border-[var(--portal-gold)] focus:outline-none" />
                            <button type="submit" disabled={pending}
                                class="rounded border border-emerald-400/50 px-2 py-1 text-[10px] font-medium uppercase tracking-wide text-emerald-300 transition hover:bg-emerald-400/15 disabled:opacity-50">
                                {"Mark Paid"}
                            </button>
                        </form>
                    }
                </td>
            </tr>
        }
    }
}


/// The name that identifies a receivable: the person, then the property, then the deal — the live screen's order.
fn belonging(receivable: &PortalAccountingReceivable) -> String {
    receivable
        .person_name
        .clone()
        .or_else(|| receivable.property_name.clone())
        .or_else(|| receivable.deal_name.clone())
        .unwrap_or_else(|| "—".to_string())
}

/// A due date, or the dash the live screen put in its place.
fn due_label(due_on: Option<&str>) -> String {
    let label = format_date(due_on);
    if label.is_empty() {
        "—".to_string()
    } else {
        label
    }
}

/// The status pill: overdue is amber, paid is emerald, void is grey, and open is gold.
fn status_badge(status: &str, overdue: bool) -> Html {
    let tone = match status {
        "PAID" => "border-emerald-400/40 text-emerald-300",
        "VOID" => "border-white/20 text-white/40",
        _ if overdue => "border-amber-400/50 text-amber-300",
        _ => "border-[var(--portal-gold)]/50 text-[var(--portal-gold)]",
    };
    html! {
        <span class={classes!("inline-flex", "rounded-full", "border", "px-2", "py-0.5", "text-[10px]",
            "font-medium", "uppercase", "tracking-wide", tone)}>
            { if overdue { "Overdue" } else { status } }
        </span>
    }
}

