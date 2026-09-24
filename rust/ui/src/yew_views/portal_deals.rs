//! CORE Contracts / Deals workspace.
//!
//! The navigation label is Contracts, while the canonical transaction record is Deal. This view keeps both truths:
//! the portfolio is made of Deals, and form-created Contract artifacts are shown beside them. All state and commands
//! are reducer-owned; the browser only renders callbacks into Msg.

use yew::prelude::*;

use crate::model::{
    Msg, PortalDeal, PortalDealContract, PortalDealPersonCandidate, PortalDealsPage,
};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct DealsProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Deals;
pub struct DealRecord;

impl Component for Deals {
    type Message = ();
    type Properties = DealsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("deals").expect("deals screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { portfolio(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

impl Component for DealRecord {
    type Message = ();
    type Properties = DealsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("deal-record").expect("deal record exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { deal_workspace(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

fn payload(model: &crate::model::Model) -> Option<&PortalDealsPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.deals.as_ref())
}

fn portfolio(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let data = payload(model);
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
                                    <td colspan="7" class="px-4 py-12 text-center text-sm font-light text-black/40">
                                        {"Loading contracts…"}
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

fn create_panel(
    model: &crate::model::Model,
    data: Option<&PortalDealsPage>,
    on_msg: &Callback<Msg>,
) -> Html {
    let state = &model.deal_create;
    let toggle = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealCreateToggled))
    };
    let property_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value();
            on_msg.emit(Msg::DealCreatePropertyChanged(value));
        })
    };
    let client_input = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::DealCreateClientQueryChanged(value));
        })
    };
    let owner_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value();
            on_msg.emit(Msg::DealCreateOwnerChanged(value));
        })
    };
    let notes_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlTextAreaElement>()
                .value();
            on_msg.emit(Msg::DealCreateNotesChanged(value));
        })
    };
    let create = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealCreateRequested))
    };

    let properties = data.map(|page| page.properties.as_slice()).unwrap_or(&[]);
    let users = data.map(|page| page.users.as_slice()).unwrap_or(&[]);
    let can_create = !state.property_id.trim().is_empty()
        && !state.client_person_id.trim().is_empty()
        && !state.submitting;

    html! {
        <section class="portal-glass-panel mb-4 overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-4 py-3">
                <div>
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"New contract"}</h2>
                    <p class="mt-0.5 text-xs font-light text-black/40">
                        {"Create the canonical Deal and its client/owner participant rows in Rust."}
                    </p>
                </div>
                <button type="button" onclick={toggle}
                    class="inline-flex min-h-9 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.13em] text-white transition hover:opacity-90">
                    { if state.open { "Close" } else { "New contract" } }
                </button>
            </div>

            if state.open {
                <div class="grid gap-3 p-4 md:grid-cols-2">
                    <label class="text-[10px] font-medium uppercase tracking-[0.12em] text-black/45">
                        {"Property *"}
                        <select value={state.property_id.clone()} onchange={property_change} class={field_class()}>
                            <option value="">{"Choose an active property…"}</option>
                            { for properties.iter().map(|property| html! {
                                <option value={property.id.clone()}>
                                    {
                                        property.location.as_deref()
                                            .map(|location| format!("{} — {}", property.name, location))
                                            .unwrap_or_else(|| property.name.clone())
                                    }
                                </option>
                            }) }
                        </select>
                    </label>

                    <label class="relative text-[10px] font-medium uppercase tracking-[0.12em] text-black/45">
                        {"Client person *"}
                        <input
                            type="search"
                            value={state.client_query.clone()}
                            oninput={client_input}
                            placeholder="Search existing people…"
                            autocomplete="off"
                            class={field_class()}
                        />
                        if state.searching {
                            <span class="mt-1 block text-[10px] font-light normal-case tracking-normal text-black/35">
                                {"Searching…"}
                            </span>
                        }
                        if !state.people.is_empty() {
                            <div class="absolute z-20 mt-1 max-h-56 w-full overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white shadow-xl">
                                { for state.people.iter().map(|person| person_choice(person, on_msg)) }
                            </div>
                        }
                        if !state.client_person_id.is_empty() {
                            <span class="mt-1 block text-[10px] font-light normal-case tracking-normal text-[var(--portal-success)]">
                                { format!("Selected: {}", state.client_label) }
                            </span>
                        }
                    </label>

                    <label class="text-[10px] font-medium uppercase tracking-[0.12em] text-black/45">
                        {"Owner"}
                        <select value={state.owner_user_id.clone()} onchange={owner_change} class={field_class()}>
                            <option value="">{"Unassigned…"}</option>
                            { for users.iter().map(|user| html! {
                                <option value={user.id.clone()}>
                                    {
                                        user.email.as_deref()
                                            .map(|email| format!("{} — {}", user.display_name, email))
                                            .unwrap_or_else(|| user.display_name.clone())
                                    }
                                </option>
                            }) }
                        </select>
                    </label>

                    <label class="text-[10px] font-medium uppercase tracking-[0.12em] text-black/45">
                        {"Notes"}
                        <textarea
                            value={state.notes.clone()}
                            oninput={notes_change}
                            rows="3"
                            placeholder="Optional deal notes…"
                            class="mt-1 block min-h-[5.5rem] w-full resize-y rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 py-2 text-sm font-light normal-case tracking-normal text-black/70 outline-none focus:border-[var(--portal-navy)]"
                        />
                    </label>

                    <div class="md:col-span-2">
                        <button type="button" onclick={create} disabled={!can_create}
                            class="inline-flex min-h-10 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-5 text-[10px] font-medium uppercase tracking-[0.13em] text-white transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-35">
                            { if state.submitting { "Creating…" } else { "Create contract" } }
                        </button>
                    </div>
                </div>
            }
        </section>
    }
}

fn person_choice(person: &PortalDealPersonCandidate, on_msg: &Callback<Msg>) -> Html {
    let id = person.id.clone();
    let label = person.display_name.clone();
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealCreateClientSelected {
                id: id.clone(),
                label: label.clone(),
            })
        })
    };
    let detail = person
        .email
        .clone()
        .or_else(|| person.phone.clone())
        .or_else(|| person.location.clone())
        .unwrap_or_else(|| person.role.clone());

    html! {
        <button type="button" {onclick}
            class="block w-full border-b border-[var(--portal-panel-border)] px-3 py-2 text-left last:border-b-0 hover:bg-[var(--portal-mist)]/45">
            <span class="block text-sm font-medium normal-case tracking-normal text-[var(--portal-navy)]">
                { person.display_name.clone() }
            </span>
            <span class="mt-0.5 block text-[11px] font-light normal-case tracking-normal text-black/45">
                { detail }
            </span>
        </button>
    }
}

fn stage_filter(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    const STAGES: [(&str, &str); 7] = [
        ("all", "All"),
        ("new_lead", "New Lead"),
        ("qualified", "Qualified"),
        ("showing", "Showing"),
        ("offer", "Offer"),
        ("under_contract", "Under Contract"),
        ("closed", "Closed"),
    ];
    let active = model.controls.filter.as_deref().unwrap_or("all");

    html! {
        <div class="mb-4 flex flex-wrap gap-1.5">
            { for STAGES.into_iter().map(|(key, label)| {
                let selected = active == key;
                let value = key.to_string();
                let onclick = {
                    let on_msg = on_msg.clone();
                    Callback::from(move |_: MouseEvent| on_msg.emit(Msg::FilterChanged(value.clone())))
                };
                html! {
                    <button type="button" {onclick}
                        class={classes!(
                            "rounded-full","border","px-3","py-1.5","text-[10px]","font-light","uppercase","tracking-[0.12em]","transition",
                            if selected {
                                "border-[var(--portal-navy)] bg-[var(--portal-navy)] text-white"
                            } else {
                                "border-[var(--portal-panel-border)] bg-white/40 text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)]"
                            }
                        )}>
                        { label }
                    </button>
                }
            }) }
        </div>
    }
}

fn deal_row(deal: &PortalDeal) -> Html {
    let price = deal
        .latest_offer_amount
        .or(deal.offer_price)
        .or(deal.list_price);
    html! {
        <tr class="border-b border-[var(--portal-panel-border)] last:border-b-0 hover:bg-white/25">
            <td class="px-4 py-3">
                <div class="flex items-center gap-3">
                    if let Some(media_id) = deal.hero_media_id.as_ref() {
                        <img src={format!("/api/media/{media_id}")} alt={deal.property_name.clone()}
                            class="h-10 w-14 shrink-0 rounded-md object-cover" />
                    } else {
                        <div class="h-10 w-14 shrink-0 rounded-md bg-gradient-to-br from-[var(--portal-blue-pale)] to-[var(--portal-navy-soft)]"></div>
                    }
                    <div class="min-w-0">
                        <a href={format!("/portal/deals/{}", deal.id)}
                            class="block max-w-56 truncate text-sm font-medium text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]">
                            { deal.property_name.clone() }
                        </a>
                        <div class="max-w-56 truncate text-xs font-light text-black/45">
                            { deal.property_location.clone() }
                        </div>
                    </div>
                </div>
            </td>
            <td class="px-4 py-3 text-sm font-light text-black/65">{ deal.client_name.clone() }</td>
            <td class="px-4 py-3">
                <span class={format!("inline-flex rounded-full px-2.5 py-1 text-[10px] font-light uppercase tracking-[0.1em] {}", stage_class(&deal.stage))}>
                    { stage_label(&deal.stage) }
                </span>
            </td>
            <td class="px-4 py-3 text-sm font-light tabular-nums text-black/70">{ format_currency(price) }</td>
            <td class="px-4 py-3">
                <div class="text-sm font-light text-black/65">
                    { deal.next_milestone.clone().unwrap_or_else(|| "—".into()) }
                </div>
                <div class="text-xs font-light text-black/35">
                    { deal.next_milestone_at.clone().unwrap_or_default() }
                </div>
            </td>
            <td class="px-4 py-3 text-sm font-light text-black/65">{ deal.owner.clone() }</td>
            <td class="px-4 py-3 text-right">
                <a href={format!("/portal/deals/{}", deal.id)}
                    class="inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)]">
                    {"Open"}
                </a>
            </td>
        </tr>
    }
}

fn table_head(label: &str) -> Html {
    html! {
        <th class="px-4 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-black/40">
            { label }
        </th>
    }
}

fn contracts_panel(contracts: &[PortalDealContract]) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex items-center justify-between border-b border-[var(--portal-panel-border)] px-4 py-3">
                <div>
                    <p class="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
                        {"Artifacts"}
                    </p>
                    <h2 class="mt-0.5 font-serif text-xl font-light text-[var(--portal-navy)]">
                        {"Contracts from Forms"}
                    </h2>
                </div>
                <span class="text-xs font-light text-black/35">{ contracts.len() }</span>
            </div>
            if contracts.is_empty() {
                <p class="px-4 py-8 text-sm font-light text-black/40">
                    {"No contract artifacts yet. One appears here as soon as a form creates it."}
                </p>
            } else {
                <div class="overflow-x-auto">
                    <table class="w-full min-w-[720px] border-collapse text-left">
                        <thead>
                            <tr class="border-b border-[var(--portal-panel-border)] bg-white/20">
                                { table_head("Contract") }
                                { table_head("Type") }
                                { table_head("Property") }
                                { table_head("Status") }
                                { table_head("Workflow") }
                                { table_head("Executed") }
                            </tr>
                        </thead>
                        <tbody>
                            { for contracts.iter().map(contract_row) }
                        </tbody>
                    </table>
                </div>
            }
        </section>
    }
}

fn contract_row(contract: &PortalDealContract) -> Html {
    html! {
        <tr class="border-b border-[var(--portal-panel-border)] last:border-b-0">
            <td class="px-4 py-3 text-sm font-light text-black/65">{ contract.form_template_id.clone() }</td>
            <td class="px-4 py-3 text-sm font-light text-black/65">{ title_case(&contract.contract_type) }</td>
            <td class="px-4 py-3 text-sm font-light text-black/65">
                { contract.property_label.clone().unwrap_or_else(|| contract.property_id.clone()) }
            </td>
            <td class="px-4 py-3 text-sm font-light text-black/65">{ title_case(&contract.status) }</td>
            <td class="px-4 py-3 text-sm font-light text-black/50">
                { if contract.process_instance_id.is_some() { "Attached" } else { "—" } }
            </td>
            <td class="px-4 py-3 text-sm font-light text-black/50">
                { contract.executed_at.clone().unwrap_or_else(|| contract.created_at.clone()) }
            </td>
        </tr>
    }
}

fn deal_workspace(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let Some(data) = payload(model) else {
        return empty_workspace(model.loading, "Loading contract workspace…");
    };
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

fn property_card(property: Option<&crate::model::PortalDealWorkspaceProperty>) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="border-b border-[var(--portal-panel-border)] px-5 py-4">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Property"}</h2>
            </div>
            <div class="px-5 py-4">
                if let Some(property) = property {
                    <a href={format!("/portal/property-admin/{}", property.id)}
                        class="font-serif text-xl font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]">
                        { property.name.clone() }
                    </a>
                    <p class="mt-2 text-xs font-light text-black/45">
                        { property.location.clone().unwrap_or_else(|| "—".into()) }
                    </p>
                    <p class="mt-2 text-xs font-light text-black/45">
                        { property_descriptor(property) }
                    </p>
                } else {
                    <p class="text-sm font-light text-black/40">{"No property on record."}</p>
                }
            </div>
        </section>
    }
}

fn client_card(client: Option<&crate::model::PortalDealWorkspaceClient>) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="border-b border-[var(--portal-panel-border)] px-5 py-4">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Client"}</h2>
            </div>
            <div class="px-5 py-4">
                if let Some(client) = client {
                    <a href={format!("/portal/clients/{}", client.id)}
                        class="font-serif text-xl font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]">
                        { client.display_name.clone() }
                    </a>
                    if let Some(email) = client.email.as_ref() {
                        <p class="mt-2 text-xs font-light text-black/45">{ email.clone() }</p>
                    }
                    if let Some(phone) = client.phone.as_ref() {
                        <p class="mt-1 text-xs font-light text-black/45">{ phone.clone() }</p>
                    }
                } else {
                    <p class="text-sm font-light text-black/40">{"No client on record."}</p>
                }
            </div>
        </section>
    }
}

fn participants_card(
    model: &crate::model::Model,
    workspace: &crate::model::PortalDealWorkspace,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="border-b border-[var(--portal-panel-border)] px-5 py-4">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Participants"}</h2>
                <p class="mt-1 text-xs font-light text-black/40">{"Canonical deal_participant roles."}</p>
            </div>
            if workspace.participants.is_empty() {
                <p class="px-5 py-6 text-sm font-light text-black/40">{"No participants on record."}</p>
            } else {
                <div>
                    { for workspace.participants.iter().map(|participant| participant_row(model, participant, on_msg, busy)) }
                </div>
            }
            { add_participant_form(model, on_msg, busy) }
            { structural_participant_form(model, workspace, on_msg, busy) }
        </section>
    }
}

fn participant_row(
    model: &crate::model::Model,
    participant: &crate::model::PortalDealWorkspaceParticipant,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let role = participant
        .role_label
        .as_deref()
        .unwrap_or_else(|| participant_role_label(&participant.role_category));
    let participant_id = participant.id.clone();
    let end_other = {
        let on_msg = on_msg.clone();
        let id = participant_id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceEndOtherRequested {
                participant_id: id.clone(),
            })
        })
    };
    let end_structural = {
        let on_msg = on_msg.clone();
        let id = participant_id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceEndStructuralRequested {
                participant_id: id.clone(),
            })
        })
    };

    html! {
        <div class="border-b border-[var(--portal-panel-border)] px-5 py-4 last:border-b-0">
            <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                    <div class="text-[10px] font-light uppercase tracking-[0.15em] text-[var(--portal-navy-soft)]">
                        { role }
                    </div>
                    <div class="mt-1 font-serif text-lg font-light text-[var(--portal-navy)]">
                        { participant.name.clone() }
                    </div>
                    if let Some(detail) = participant.detail.as_ref() {
                        <div class="mt-1 truncate text-xs font-light text-black/40">{ detail.clone() }</div>
                    }
                </div>
                <span class="text-[10px] font-light uppercase tracking-[0.12em] text-black/30">
                    { participant.kind.clone() }
                </span>
            </div>
            if participant.role_category == "other" {
                { other_participant_controls(model, participant, on_msg, end_other, busy) }
            } else if participant.role_category != "client" {
                <button type="button" onclick={end_structural} disabled={busy}
                    class="mt-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-archive)] disabled:opacity-35">
                    {"End role"}
                </button>
            }
        </div>
    }
}

fn other_participant_controls(
    model: &crate::model::Model,
    participant: &crate::model::PortalDealWorkspaceParticipant,
    on_msg: &Callback<Msg>,
    end_other: Callback<MouseEvent>,
    busy: bool,
) -> Html {
    let id = participant.id.clone();
    let value = model
        .deal_workspace
        .other_role_labels
        .get(&id)
        .cloned()
        .unwrap_or_else(|| participant.role_label.clone().unwrap_or_default());
    let oninput = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |event: InputEvent| {
            let value = event.target_unchecked_into::<web_sys::HtmlInputElement>().value();
            on_msg.emit(Msg::DealWorkspaceOtherRoleChanged {
                participant_id: id.clone(),
                value,
            });
        })
    };
    let save = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceUpdateOtherRequested {
                participant_id: id.clone(),
            })
        })
    };

    html! {
        <div class="mt-3 flex flex-wrap items-center gap-2">
            <input value={value} {oninput} class="h-8 min-w-0 flex-1 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/65 px-2 text-xs font-light outline-none focus:border-[var(--portal-navy)]" />
            <button type="button" onclick={save} disabled={busy}
                class="h-8 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2 text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-navy-soft)] disabled:opacity-35">
                {"Save"}
            </button>
            <button type="button" onclick={end_other} disabled={busy}
                class="h-8 px-1 text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-archive)] disabled:opacity-35">
                {"End"}
            </button>
        </div>
    }
}

fn add_participant_form(model: &crate::model::Model, on_msg: &Callback<Msg>, busy: bool) -> Html {
    let query_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceParticipantQueryChanged(
                event.target_unchecked_into::<web_sys::HtmlInputElement>().value(),
            ))
        })
    };
    let role_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceParticipantRoleChanged(
                event.target_unchecked_into::<web_sys::HtmlInputElement>().value(),
            ))
        })
    };
    let add = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceAddParticipantRequested))
    };

    html! {
        <div class="border-t border-[var(--portal-panel-border)] bg-white/20 px-5 py-4">
            <p class="text-[10px] font-medium uppercase tracking-[0.13em] text-black/40">{"Add participant"}</p>
            <div class="relative mt-2">
                <input type="search" value={model.deal_workspace.participant_query.clone()} oninput={query_change}
                    placeholder="Search person…" class={field_class()} />
                if !model.deal_workspace.participant_people.is_empty() {
                    <div class="absolute z-20 mt-1 max-h-48 w-full overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white shadow-xl">
                        { for model.deal_workspace.participant_people.iter().map(|person| workspace_person_choice(person, "participant", on_msg)) }
                    </div>
                }
            </div>
            <input value={model.deal_workspace.participant_role_label.clone()} oninput={role_change}
                placeholder="Role: lender, inspector, notario…" class={field_class()} />
            <button type="button" onclick={add} disabled={busy || model.deal_workspace.participant_person_id.is_empty()}
                class="mt-2 min-h-9 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[9px] font-medium uppercase tracking-[0.12em] text-white disabled:opacity-35">
                {"Add"}
            </button>
        </div>
    }
}

fn structural_participant_form(
    model: &crate::model::Model,
    workspace: &crate::model::PortalDealWorkspace,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let role_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            on_msg.emit(Msg::DealWorkspaceStructuralRoleChanged(
                event.target_unchecked_into::<web_sys::HtmlSelectElement>().value(),
            ))
        })
    };
    let query_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceStructuralQueryChanged(
                event.target_unchecked_into::<web_sys::HtmlInputElement>().value(),
            ))
        })
    };
    let owner_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            on_msg.emit(Msg::DealWorkspaceStructuralOwnerChanged(
                event.target_unchecked_into::<web_sys::HtmlSelectElement>().value(),
            ))
        })
    };
    let save = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceSetStructuralRequested))
    };
    let role = model.deal_workspace.structural_role.as_str();

    html! {
        <div class="border-t border-[var(--portal-panel-border)] bg-white/20 px-5 py-4">
            <p class="text-[10px] font-medium uppercase tracking-[0.13em] text-black/40">{"Replace structural role"}</p>
            <select value={model.deal_workspace.structural_role.clone()} onchange={role_change} class={field_class()}>
                <option value="">{"Choose role…"}</option>
                <option value="client">{"Client"}</option>
                <option value="owner">{"Owner"}</option>
                <option value="seller">{"Seller"}</option>
            </select>
            if role == "owner" {
                <select value={model.deal_workspace.structural_owner_user_id.clone()} onchange={owner_change} class={field_class()}>
                    <option value="">{"Choose active user…"}</option>
                    { for workspace.owner_candidates.iter().map(|user| html! {
                        <option value={user.id.clone()}>{ user.display_name.clone() }</option>
                    }) }
                </select>
            } else if matches!(role, "client" | "seller") {
                <div class="relative">
                    <input type="search" value={model.deal_workspace.structural_query.clone()} oninput={query_change}
                        placeholder="Search person…" class={field_class()} />
                    if !model.deal_workspace.structural_people.is_empty() {
                        <div class="absolute z-20 mt-1 max-h-48 w-full overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white shadow-xl">
                            { for model.deal_workspace.structural_people.iter().map(|person| workspace_person_choice(person, "structural", on_msg)) }
                        </div>
                    }
                </div>
            }
            <button type="button" onclick={save} disabled={busy || role.is_empty()}
                class="mt-2 min-h-9 rounded-[var(--portal-tab-radius)] border border-[var(--portal-navy)] px-3 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)] disabled:opacity-35">
                {"Set / replace"}
            </button>
        </div>
    }
}

fn workspace_person_choice(
    person: &PortalDealPersonCandidate,
    purpose: &'static str,
    on_msg: &Callback<Msg>,
) -> Html {
    let id = person.id.clone();
    let label = person.display_name.clone();
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| {
            if purpose == "participant" {
                on_msg.emit(Msg::DealWorkspaceParticipantSelected {
                    id: id.clone(),
                    label: label.clone(),
                });
            } else {
                on_msg.emit(Msg::DealWorkspaceStructuralPersonSelected {
                    id: id.clone(),
                    label: label.clone(),
                });
            }
        })
    };
    html! {
        <button type="button" {onclick}
            class="block w-full border-b border-[var(--portal-panel-border)] px-3 py-2 text-left last:border-b-0 hover:bg-[var(--portal-mist)]/45">
            <span class="block text-sm font-medium text-[var(--portal-navy)]">{ person.display_name.clone() }</span>
            <span class="text-[11px] font-light text-black/40">
                { person.email.clone().or(person.phone.clone()).or(person.location.clone()).unwrap_or_else(|| person.role.clone()) }
            </span>
        </button>
    }
}

fn tasks_card(
    model: &crate::model::Model,
    workspace: &crate::model::PortalDealWorkspace,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let title_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceTaskTitleChanged(
                event.target_unchecked_into::<web_sys::HtmlInputElement>().value(),
            ))
        })
    };
    let detail_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceTaskDetailChanged(
                event.target_unchecked_into::<web_sys::HtmlInputElement>().value(),
            ))
        })
    };
    let due_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceTaskDueChanged(
                event.target_unchecked_into::<web_sys::HtmlInputElement>().value(),
            ))
        })
    };
    let create = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceCreateTaskRequested))
    };

    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex items-center justify-between border-b border-[var(--portal-panel-border)] px-5 py-4">
                <div>
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Open Tasks"}</h2>
                    <p class="mt-1 text-xs font-light text-black/40">{"Milestones and follow-ups on this deal."}</p>
                </div>
                <span class="text-xs font-light text-black/35">{ workspace.open_tasks.len() }</span>
            </div>
            if workspace.open_tasks.is_empty() {
                <p class="px-5 py-6 text-sm font-light text-black/40">{"No open tasks on this deal."}</p>
            } else {
                <div>
                    { for workspace.open_tasks.iter().map(|task| {
                        let task_id = task.id.clone();
                        let complete = {
                            let on_msg = on_msg.clone();
                            Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceCompleteTaskRequested { task_id: task_id.clone() }))
                        };
                        html! {
                            <div class="border-b border-[var(--portal-panel-border)] px-5 py-4 last:border-b-0">
                                <div class="flex items-start justify-between gap-3">
                                    <div>
                                        <div class="font-serif text-lg font-light text-[var(--portal-navy)]">{ task.title.clone() }</div>
                                        if let Some(detail) = task.detail.as_ref() {
                                            <p class="mt-1 text-sm font-light text-black/50">{ detail.clone() }</p>
                                        }
                                        <p class="mt-2 text-xs font-light text-black/40">{ task.due_at_label.clone().unwrap_or_else(|| "Unscheduled".into()) }</p>
                                    </div>
                                    if task.is_overdue {
                                        <span class="rounded-full bg-[var(--portal-archive-pale)] px-2 py-1 text-[9px] uppercase tracking-[0.1em] text-[var(--portal-archive)]">{"Overdue"}</span>
                                    }
                                </div>
                                <button type="button" onclick={complete} disabled={busy}
                                    class="mt-3 min-h-8 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-35">
                                    {"Complete"}
                                </button>
                            </div>
                        }
                    }) }
                </div>
            }
            <div class="border-t border-[var(--portal-panel-border)] bg-white/20 px-5 py-4">
                <input value={model.deal_workspace.task_title.clone()} oninput={title_change} placeholder="New task title…" class={field_class()} />
                <input value={model.deal_workspace.task_detail.clone()} oninput={detail_change} placeholder="Detail (optional)…" class={field_class()} />
                <input type="datetime-local" value={model.deal_workspace.task_due_at.clone()} oninput={due_change} class={field_class()} />
                <button type="button" onclick={create} disabled={busy || model.deal_workspace.task_title.trim().is_empty()}
                    class="mt-2 min-h-9 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[9px] font-medium uppercase tracking-[0.12em] text-white disabled:opacity-35">
                    {"Add task"}
                </button>
            </div>
        </section>
    }
}

fn activity_card(workspace: &crate::model::PortalDealWorkspace) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex items-center justify-between border-b border-[var(--portal-panel-border)] px-5 py-4">
                <div>
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Recent Contract Activity"}</h2>
                    <p class="mt-1 text-xs font-light text-black/40">{"Interactions tied to this contract."}</p>
                </div>
                <span class="text-xs font-light text-black/35">{ workspace.activity.len() }</span>
            </div>
            if workspace.activity.is_empty() {
                <p class="px-5 py-6 text-sm font-light text-black/40">{"No deal activity yet."}</p>
            } else {
                <div>
                    { for workspace.activity.iter().map(|item| html! {
                        <div class="border-b border-[var(--portal-panel-border)] px-5 py-4 last:border-b-0">
                            <div class="flex flex-wrap items-center gap-2 text-[10px] font-light uppercase tracking-[0.11em] text-[var(--portal-navy-soft)]">
                                <span>{ channel_label(&item.channel) }</span>
                                if let Some(direction) = item.direction.as_ref() {
                                    <span class="text-black/30">{ direction.clone() }</span>
                                }
                                <span class="ml-auto normal-case tracking-normal text-black/35">{ item.occurred_at_label.clone() }</span>
                            </div>
                            <div class="mt-1 text-sm font-medium text-[var(--portal-navy)]">
                                { item.person_name.clone().unwrap_or_else(|| "—".into()) }
                            </div>
                            <p class="mt-1 text-sm font-light text-black/55">
                                { item.summary.clone().or(item.title.clone()).unwrap_or_else(|| "Interaction".into()) }
                            </p>
                        </div>
                    }) }
                </div>
            }
        </section>
    }
}

fn offers_card(
    model: &crate::model::Model,
    workspace: &crate::model::PortalDealWorkspace,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex items-center justify-between border-b border-[var(--portal-panel-border)] px-5 py-4">
                <div>
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Offers"}</h2>
                    <p class="mt-1 text-xs font-light text-black/40">{"Offer history and counter lineage for this deal."}</p>
                </div>
                <span class="text-xs font-light text-black/35">{ workspace.offers.len() }</span>
            </div>
            if workspace.offers.is_empty() {
                <p class="px-5 py-6 text-sm font-light text-black/40">{"No offers on record for this deal."}</p>
            } else {
                <div>
                    { for workspace.offers.iter().map(|offer| offer_row(model, offer, on_msg, busy)) }
                </div>
            }
            if workspace.client.is_some() {
                <div class="border-t border-[var(--portal-panel-border)] bg-white/20 px-5 py-4">
                    { offer_form(model, None, "Submit offer", on_msg, busy) }
                </div>
            }
        </section>
    }
}

fn offer_row(
    model: &crate::model::Model,
    offer: &crate::model::PortalDealWorkspaceOffer,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let withdraw = {
        let on_msg = on_msg.clone();
        let id = offer.id.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceWithdrawOfferRequested { offer_id: id.clone() }))
    };
    let reject = {
        let on_msg = on_msg.clone();
        let id = offer.id.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceRejectOfferRequested { offer_id: id.clone() }))
    };
    html! {
        <div class="border-b border-[var(--portal-panel-border)] px-5 py-4 last:border-b-0">
            <div class="flex flex-wrap items-center gap-2">
                <span class="font-serif text-2xl font-light text-[var(--portal-navy)]">{ format_currency(Some(offer.amount)) }</span>
                <span class="rounded-full bg-[var(--portal-blue-pale)] px-2.5 py-1 text-[9px] uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">{ title_case(&offer.status) }</span>
                if offer.is_counter {
                    <span class="rounded-full border border-[var(--portal-panel-border)] px-2.5 py-1 text-[9px] uppercase tracking-[0.1em] text-black/45">{"Counter"}</span>
                }
            </div>
            <p class="mt-2 text-xs font-light text-black/45">
                { offer.person_name.clone().unwrap_or_else(|| "—".into()) }
                {" · Submitted "}
                { offer.submitted_at_label.clone() }
                if let Some(responded) = offer.responded_at_label.as_ref() {
                    {" · Responded "}
                    { responded.clone() }
                }
            </p>
            if let Some(note) = offer.note.as_ref() {
                <p class="mt-2 text-sm font-light text-black/55">{ note.clone() }</p>
            }
            if offer.status == "submitted" {
                <div class="mt-3 flex flex-wrap gap-2">
                    <button type="button" onclick={withdraw} disabled={busy} class={small_action_class()}>{"Withdraw"}</button>
                    <button type="button" onclick={reject} disabled={busy} class={small_action_class()}>{"Reject"}</button>
                </div>
                <div class="mt-3 max-w-sm">
                    { offer_form(model, Some(offer.id.clone()), "Counter", on_msg, busy) }
                </div>
            }
        </div>
    }
}

fn offer_form(
    model: &crate::model::Model,
    parent_offer_id: Option<String>,
    label: &'static str,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let key = parent_offer_id.clone().unwrap_or_else(|| "root".into());
    let value = model.deal_workspace.offer_amounts.get(&key).cloned().unwrap_or_default();
    let amount_change = {
        let on_msg = on_msg.clone();
        let key = key.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceOfferAmountChanged {
                key: key.clone(),
                value: event.target_unchecked_into::<web_sys::HtmlInputElement>().value(),
            })
        })
    };
    let submit = {
        let on_msg = on_msg.clone();
        let parent_offer_id = parent_offer_id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceSubmitOfferRequested {
                parent_offer_id: parent_offer_id.clone(),
            })
        })
    };
    html! {
        <div class="flex flex-wrap items-center gap-2">
            <input type="number" min="1" step="1" value={value} oninput={amount_change}
                placeholder="Amount" class="h-9 min-w-[140px] flex-1 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-sm font-light outline-none focus:border-[var(--portal-navy)]" />
            <button type="button" onclick={submit} disabled={busy}
                class="h-9 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[9px] font-medium uppercase tracking-[0.12em] text-white disabled:opacity-35">
                { label }
            </button>
        </div>
    }
}

fn showings_card(
    model: &crate::model::Model,
    workspace: &crate::model::PortalDealWorkspace,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let create = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceCreateShowingRequested))
    };
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-5 py-4">
                <div>
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Showings"}</h2>
                    <p class="mt-1 text-xs font-light text-black/40">{"Showing history for this deal."}</p>
                </div>
                <div class="flex items-center gap-3">
                    <span class="text-xs font-light text-black/35">{ workspace.showings.len() }</span>
                    <a href="/portal/showings" class="text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)]">{"View all →"}</a>
                    if workspace.client.is_some() {
                        <button type="button" onclick={create} disabled={busy} class={small_action_class()}>{"Request showing"}</button>
                    }
                </div>
            </div>
            if workspace.showings.is_empty() {
                <p class="px-5 py-6 text-sm font-light text-black/40">{"No showings on record for this deal."}</p>
            } else {
                <div>
                    { for workspace.showings.iter().map(|showing| showing_row(model, showing, on_msg, busy)) }
                </div>
            }
        </section>
    }
}

fn showing_row(
    model: &crate::model::Model,
    showing: &crate::model::PortalDealWorkspaceShowing,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let id = showing.id.clone();
    let time = model.deal_workspace.showing_times.get(&id).cloned().unwrap_or_default();
    let time_change = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceShowingTimeChanged {
                showing_id: id.clone(),
                value: event.target_unchecked_into::<web_sys::HtmlInputElement>().value(),
            })
        })
    };
    let schedule = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceScheduleShowingRequested { showing_id: id.clone() }))
    };
    let cancel = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceCancelShowingRequested { showing_id: id.clone() }))
    };
    let complete = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceCompleteShowingRequested { showing_id: id.clone() }))
    };
    html! {
        <div class="border-b border-[var(--portal-panel-border)] px-5 py-4 last:border-b-0">
            <div class="flex flex-wrap items-center gap-2">
                <a href={format!("/portal/clients/{}", showing.person_id)}
                    class="font-serif text-lg font-light text-[var(--portal-navy)]">{ showing.person_name.clone() }</a>
                <span class="rounded-full bg-[var(--portal-blue-pale)] px-2.5 py-1 text-[9px] uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">{ title_case(&showing.status) }</span>
            </div>
            <p class="mt-2 text-xs font-light text-black/45">
                { format_showing_dates(showing) }
            </p>
            if let Some(feedback) = showing.feedback.as_ref() {
                <p class="mt-2 text-sm font-light text-black/55">{ feedback.clone() }</p>
            }
            if showing.status == "requested" {
                <div class="mt-3 flex flex-wrap items-center gap-2">
                    <input type="datetime-local" value={time} oninput={time_change}
                        class="h-9 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-2 text-xs font-light outline-none focus:border-[var(--portal-navy)]" />
                    <button type="button" onclick={schedule} disabled={busy} class={small_action_class()}>{"Schedule"}</button>
                    <button type="button" onclick={complete.clone()} disabled={busy} class={small_action_class()}>{"Complete"}</button>
                    <button type="button" onclick={cancel.clone()} disabled={busy} class={small_action_class()}>{"Cancel"}</button>
                </div>
            } else if showing.status == "scheduled" {
                <div class="mt-3 flex flex-wrap gap-2">
                    <button type="button" onclick={complete} disabled={busy} class={small_action_class()}>{"Complete"}</button>
                    <button type="button" onclick={cancel} disabled={busy} class={small_action_class()}>{"Cancel"}</button>
                </div>
            }
        </div>
    }
}

fn property_descriptor(property: &crate::model::PortalDealWorkspaceProperty) -> String {
    let mut parts = Vec::new();
    if let Some(kind) = property.property_type.as_ref().filter(|value| !value.trim().is_empty()) {
        parts.push(kind.clone());
    }
    if let Some(bedrooms) = property.bedrooms {
        parts.push(format!("{} bed", format_number(bedrooms)));
    }
    if let Some(bathrooms) = property.bathrooms {
        parts.push(format!("{} bath", format_number(bathrooms)));
    }
    if let Some(square_feet) = property.square_feet {
        parts.push(format!("{} SF", group_integer(square_feet)));
    }
    if parts.is_empty() { "No details on file".into() } else { parts.join(" · ") }
}

fn format_showing_dates(showing: &crate::model::PortalDealWorkspaceShowing) -> String {
    let mut parts = vec![format!("Requested {}", showing.requested_at_label)];
    if let Some(value) = showing.scheduled_at_label.as_ref() {
        parts.push(format!("Scheduled {value}"));
    }
    if let Some(value) = showing.completed_at_label.as_ref() {
        parts.push(format!("Completed {value}"));
    }
    if let Some(value) = showing.cancelled_at_label.as_ref() {
        parts.push(format!("Cancelled {value}"));
    }
    parts.join(" · ")
}

fn small_action_class() -> Classes {
    classes!(
        "min-h-8","rounded-[var(--portal-tab-radius)]","border",
        "border-[var(--portal-panel-border)]","px-2.5","text-[9px]","font-medium",
        "uppercase","tracking-[0.11em]","text-[var(--portal-navy-soft)]","disabled:opacity-35"
    )
}

fn format_number(value: f64) -> String {
    if value.fract().abs() < 0.000_001 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn group_integer(value: i64) -> String {
    let negative = value < 0;
    let digits = value.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let grouped = grouped.chars().rev().collect::<String>();
    format!("{}{}", if negative { "-" } else { "" }, grouped)
}

fn empty_workspace(loading: bool, text: &str) -> Html {
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] px-8 py-12 text-center">
            <h1 class="font-serif text-2xl font-light text-[var(--portal-navy)]">
                { if loading { "Contract workspace" } else { "Contract unavailable" } }
            </h1>
            <p class="mt-2 text-sm font-light text-black/45">{ text }</p>
            if !loading {
                <a href="/portal/deals" class="mt-5 inline-flex text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy-soft)]">
                    {"← Back to Contracts"}
                </a>
            }
        </section>
    }
}

fn detail(label: &str, value: &str) -> Html {
    html! {
        <div>
            <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">{ label }</div>
            <div class="mt-1 text-sm font-light leading-5 text-black/70">{ value }</div>
        </div>
    }
}

fn metric(label: &str, value: i64) -> Html {
    html! {
        <div class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-3">
            <div class="font-serif text-2xl font-light text-[var(--portal-navy)]">{ value }</div>
            <div class="mt-1 text-[10px] font-light uppercase tracking-[0.12em] text-black/35">{ label }</div>
        </div>
    }
}

fn offer_summary(deal: &PortalDeal) -> String {
    if deal.offer_count == 0 {
        return "No offers".into();
    }
    let amount = format_currency(deal.latest_offer_amount.or(deal.offer_price));
    match deal.latest_offer_status.as_deref() {
        Some(status) => format!("{} offer{} · {} · {}", deal.offer_count, if deal.offer_count == 1 { "" } else { "s" }, title_case(status), amount),
        None => format!("{} offer{} · {}", deal.offer_count, if deal.offer_count == 1 { "" } else { "s" }, amount),
    }
}

fn field_class() -> Classes {
    classes!(
        "mt-1","block","h-10","w-full","rounded-[var(--portal-tab-radius)]","border",
        "border-[var(--portal-panel-border)]","bg-white/70","px-3","text-sm","font-light",
        "normal-case","tracking-normal","text-black/70","outline-none","focus:border-[var(--portal-navy)]"
    )
}

fn participant_role_label(role: &str) -> &str {
    match role {
        "client" => "Client",
        "owner" => "Owner",
        "seller" => "Seller",
        "other" => "Other",
        value => value,
    }
}

fn channel_label(channel: &str) -> &str {
    match channel {
        "website" => "Website",
        "email" => "Email",
        "call" => "Phone Call",
        "imessage" => "iMessage",
        "sms" => "SMS",
        "meeting" => "Meeting",
        "showing" => "Showing",
        "document" => "Document",
        "manual" => "Manual Entry",
        "whatsapp" => "WhatsApp",
        value => value,
    }
}

fn stage_label(stage: &str) -> &str {
    match stage {
        "new_lead" => "New Lead",
        "qualified" => "Qualified",
        "showing" => "Showing",
        "offer" => "Offer",
        "under_contract" => "Under Contract",
        "closed" => "Closed",
        other => other,
    }
}

fn stage_class(stage: &str) -> &'static str {
    match stage {
        "new_lead" => "bg-black/5 text-black/50",
        "qualified" => "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]",
        "showing" => "bg-[var(--portal-mist-2)] text-[var(--portal-navy-soft)]",
        "offer" => "bg-[var(--portal-mist)] text-[var(--portal-navy)]",
        "under_contract" => "bg-[var(--portal-success-pale)] text-[var(--portal-success)]",
        "closed" => "bg-[var(--portal-navy)] text-white",
        _ => "bg-black/5 text-black/55",
    }
}

fn title_case(value: &str) -> String {
    value
        .split(|ch: char| ch == '_' || ch.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_currency(value: Option<f64>) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    let rounded = value.round() as i64;
    let negative = rounded < 0;
    let digits = rounded.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let grouped = grouped.chars().rev().collect::<String>();
    format!("{}{}{}", if negative { "-" } else { "" }, "$", grouped)
}
