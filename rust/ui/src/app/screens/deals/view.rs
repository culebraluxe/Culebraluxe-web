//! CORE Contracts / Deals workspace.
//!
//! The navigation label is Contracts, while the canonical transaction record is Deal. This view keeps both truths:
//! the portfolio is made of Deals, and form-created Contract artifacts are shown beside them. All state and commands
//! are reducer-owned; the browser only renders callbacks into Msg.

use yew::prelude::*;

use crate::model::{PortalDeal, PortalDealContract, PortalDealPersonCandidate, PortalDealsPage};

use super::{Msg, Vm};
mod portfolio;
mod cards;
mod participants;
mod tasks_activity;
mod offers;
mod showings;
mod format;
#[allow(unused_imports)]
pub(super) use portfolio::*;
#[allow(unused_imports)]
pub(super) use cards::*;
#[allow(unused_imports)]
pub(super) use participants::*;
#[allow(unused_imports)]
pub(super) use tasks_activity::*;
#[allow(unused_imports)]
pub(super) use offers::*;
#[allow(unused_imports)]
pub(super) use showings::*;
#[allow(unused_imports)]
pub(super) use format::*;


pub(super) fn portfolio(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
    let data = Some(model.data);
    let filter = model.controls.filter.as_deref().unwrap_or("all");
    let deals = data
        .map(|page| {
            page.deals
                .iter()
                .filter(|deal| filter == "all" || deal.stage == filter)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let total = data.map(|page| page.deals.len()).unwrap_or(0);
    let contracts = data.map(|page| page.contracts.as_slice()).unwrap_or(&[]);

    html! {
        <div>
            <header class="mb-5 flex flex-wrap items-end justify-between gap-3">
                <div>
                    <p class="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">
                        {"Portfolio"}
                    </p>
                    <h1 class="mt-1 font-serif text-3xl font-light text-[var(--portal-navy)]">
                        {"Contracts"}
                    </h1>
                    <p class="mt-1 text-sm font-light text-black/50">
                        {"Transactions in motion with their form-created contract artifacts."}
                    </p>
                </div>
                <span class="text-xs font-light text-black/40">
                    { format!("{} shown · {} total", deals.len(), total) }
                </span>
            </header>

            if model.can("deal.write") {
                { create_panel(model, data, on_msg) }
            }
            { stage_filter(model, on_msg) }

            <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="overflow-x-auto">
                    <table class="w-full min-w-[900px] border-collapse text-left">
                        <thead>
                            <tr class="border-b border-[var(--portal-panel-border)] bg-white/25">
                                { table_head("Property") }
                                { table_head("Client") }
                                { table_head("Stage") }
                                { table_head("Price / Offer") }
                                { table_head("Next") }
                                { table_head("Owner") }
                                { table_head("") }
                            </tr>
                        </thead>
                        <tbody>
                            if model.loading && data.is_none() {
                                <tr>
                                    <td colspan="7" class="px-4 py-12 text-center">
                                        { crate::app::template::loading_line("contracts") }
                                    </td>
                                </tr>
                            } else if deals.is_empty() {
                                <tr>
                                    <td colspan="7" class="px-4 py-12 text-center text-sm font-light text-black/40">
                                        {"No contracts found for this stage."}
                                    </td>
                                </tr>
                            } else {
                                { for deals.into_iter().map(deal_row) }
                            }
                        </tbody>
                    </table>
                </div>
            </section>

            <div class="mt-4">
                { contracts_panel(contracts) }
            </div>
        </div>
    }
}

pub(super) fn deal_workspace(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
    let data = model.data;
    let Some(workspace) = data.workspace.as_ref() else {
        return empty_workspace(model.loading, "Contract not found.");
    };
    let Some(deal) = workspace.deal.as_ref() else {
        return empty_workspace(false, "Contract not found.");
    };
    let property = workspace.property.as_ref();
    let client = workspace.client.as_ref();
    let busy = model.deal_workspace.busy_action.is_some();
    let deal_busy = busy || !model.can("deal.write");
    let showing_busy = busy || !model.can("showing.write");

    let next_action = workspace
        .open_tasks
        .first()
        .map(|task| {
            task.due_at_label
                .as_deref()
                .map(|due| format!("{} · {}", task.title, due))
                .unwrap_or_else(|| task.title.clone())
        })
        .unwrap_or_else(|| "No open task".into());
    let offer_state = workspace
        .offers
        .last()
        .map(|offer| {
            format!(
                "{} offer{} · latest {}",
                workspace.offers.len(),
                if workspace.offers.len() == 1 { "" } else { "s" },
                title_case(&offer.status)
            )
        })
        .unwrap_or_else(|| "No offers".into());

    html! {
        <div>
            <header class="mb-5 flex flex-wrap items-start justify-between gap-4">
                <div>
                    <a href="/portal/deals" class="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy-soft)]">
                        {"← Contracts"}
                    </a>
                    <p class="mt-4 text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">
                        {"Contract"}
                    </p>
                    <h1 class="mt-1 font-serif text-3xl font-light text-[var(--portal-navy)]">
                        { property.map(|item| item.name.clone()).unwrap_or_else(|| "Contract".into()) }
                    </h1>
                    if let Some(location) = property.and_then(|item| item.location.as_ref()) {
                        <p class="mt-1 text-sm font-light text-black/45">{ location.clone() }</p>
                    }
                </div>
                <span class={format!("inline-flex rounded-full px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] {}", stage_class(&deal.stage))}>
                    { stage_label(&deal.stage) }
                </span>
            </header>

            <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)] p-4">
                <div class="flex items-center justify-between gap-3">
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Operating summary"}</h2>
                    if model.loading {
                        <span class="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">{"Refreshing…"}</span>
                    }
                </div>
                <div class="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                    { detail("Current gate", deal.closed_at_label.as_deref().map(|closed| format!("Closed · {closed}")).as_deref().unwrap_or_else(|| stage_label(&deal.stage))) }
                    { detail("Next action", &next_action) }
                    { detail("Offer state", &offer_state) }
                    { detail("Closing", deal.closing_date_label.as_deref().or(deal.closed_at_label.as_deref()).unwrap_or("Not recorded")) }
                </div>
                <div class="mt-4 grid gap-4 border-t border-[var(--portal-panel-border)] pt-4 md:grid-cols-3">
                    { detail("Client", client.map(|item| item.display_name.as_str()).unwrap_or("—")) }
                    { detail("Property", property.map(|item| item.name.as_str()).unwrap_or("—")) }
                    { detail("Created", &deal.created_at_label) }
                </div>
            </section>

            <div class="mt-4">
                { deal_health_card(&workspace.health) }
            </div>

            <section class="portal-glass-panel mt-4 overflow-hidden rounded-[var(--portal-panel-radius)] p-4">
                <div class="grid gap-6 md:grid-cols-2 xl:grid-cols-4">
                    { detail("List Price", &format_currency(deal.list_price)) }
                    { detail("Offer Price", &format_currency(deal.offer_price)) }
                    { detail("Closing", deal.closing_date_label.as_deref().unwrap_or("—")) }
                    { detail("Last Updated", &deal.updated_at_label) }
                </div>
            </section>

            <div class="mt-4 grid gap-4 lg:grid-cols-3">
                { property_card(property) }
                { client_card(client) }
                { participants_card(model, workspace, on_msg, deal_busy) }
            </div>

            <div class="mt-4 grid gap-4 lg:grid-cols-2">
                { tasks_card(model, workspace, on_msg, deal_busy) }
                { activity_card(workspace) }
            </div>

            <div class="mt-4">
                { offers_card(model, workspace, on_msg, deal_busy) }
            </div>

            <div class="mt-4">
                { showings_card(model, workspace, on_msg, showing_busy) }
            </div>

            if let Some(notes) = deal.notes.as_ref().filter(|value| !value.trim().is_empty()) {
                <section class="portal-glass-panel mt-4 overflow-hidden rounded-[var(--portal-panel-radius)] p-5">
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Contract Notes"}</h2>
                    <p class="mt-3 whitespace-pre-wrap text-sm font-light leading-7 text-black/55">{ notes.clone() }</p>
                </section>
            }

            <div class="mt-4">
                { contracts_panel(&workspace.contracts) }
            </div>
        </div>
    }
}
