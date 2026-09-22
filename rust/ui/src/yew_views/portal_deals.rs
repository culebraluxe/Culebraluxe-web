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
                { deal_workspace(&props.model) }
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

            { create_panel(model, data, on_msg) }
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

fn deal_workspace(model: &crate::model::Model) -> Html {
    let Some(data) = payload(model) else {
        return empty_workspace(model.loading, "Loading contract workspace…");
    };
    let Some(id) = model.scope.as_deref() else {
        return empty_workspace(false, "No contract was selected.");
    };
    let Some(deal) = data.deals.iter().find(|deal| deal.id == id) else {
        return empty_workspace(model.loading, "Contract not found.");
    };
    let contracts = data
        .contracts
        .iter()
        .filter(|contract| contract.property_id == deal.property_id)
        .collect::<Vec<_>>();

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
                        { deal.property_name.clone() }
                    </h1>
                    <p class="mt-1 text-sm font-light text-black/45">{ deal.property_location.clone() }</p>
                </div>
                <span class={format!("inline-flex rounded-full px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] {}", stage_class(&deal.stage))}>
                    { stage_label(&deal.stage) }
                </span>
            </header>

            <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)] p-4">
                <div class="flex items-center justify-between gap-3">
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Operating summary"}</h2>
                    <span class="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
                        { format!("{} participants", deal.participant_count) }
                    </span>
                </div>
                <div class="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                    { detail("Current gate", stage_label(&deal.stage)) }
                    { detail("Next action", deal.next_milestone.as_deref().unwrap_or("No open milestone")) }
                    { detail("Offer state", &offer_summary(deal)) }
                    { detail("Closing", deal.closing_date.as_deref().unwrap_or("Not recorded")) }
                </div>
                <div class="mt-4 grid gap-4 border-t border-[var(--portal-panel-border)] pt-4 md:grid-cols-3">
                    { detail("Client", &deal.client_name) }
                    { detail("Owner", &deal.owner) }
                    { detail("Last activity", deal.last_activity.as_deref().unwrap_or("No activity yet")) }
                </div>
            </section>

            <div class="mt-4 grid gap-4 lg:grid-cols-3">
                <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                    <p class="text-[10px] font-light uppercase tracking-[0.15em] text-black/35">{"Property"}</p>
                    <a href={format!("/portal/property-admin/{}", deal.property_id)}
                        class="mt-2 block font-serif text-xl font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]">
                        { deal.property_name.clone() }
                    </a>
                    <p class="mt-1 text-sm font-light text-black/45">{ deal.property_location.clone() }</p>
                    if let Some(descriptor) = deal.property_descriptor.as_ref() {
                        <p class="mt-2 text-xs font-light text-black/45">{ descriptor.clone() }</p>
                    }
                </section>

                <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                    <p class="text-[10px] font-light uppercase tracking-[0.15em] text-black/35">{"Client"}</p>
                    <a href={format!("/portal/clients/{}", deal.client_id)}
                        class="mt-2 block font-serif text-xl font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]">
                        { deal.client_name.clone() }
                    </a>
                    <p class="mt-2 text-xs font-light text-black/45">
                        { format!("{} participant{}", deal.participant_count, if deal.participant_count == 1 { "" } else { "s" }) }
                    </p>
                </section>

                <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                    <p class="text-[10px] font-light uppercase tracking-[0.15em] text-black/35">{"Activity"}</p>
                    <div class="mt-3 grid grid-cols-2 gap-3">
                        { metric("Showings", deal.showing_count) }
                        { metric("Offers", deal.offer_count) }
                    </div>
                    <p class="mt-4 text-xs font-light leading-5 text-black/45">
                        {
                            deal.last_activity_at.as_deref()
                                .map(|at| format!("Last activity · {at}"))
                                .unwrap_or_else(|| "No dated activity yet.".into())
                        }
                    </p>
                </section>
            </div>

            <section class="portal-glass-panel mt-4 overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="flex items-center justify-between border-b border-[var(--portal-panel-border)] px-4 py-3">
                    <div>
                        <p class="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
                            {"Artifacts"}
                        </p>
                        <h2 class="mt-0.5 font-serif text-xl font-light text-[var(--portal-navy)]">
                            {"Property contract lineage"}
                        </h2>
                    </div>
                    <span class="text-xs font-light text-black/35">{ contracts.len() }</span>
                </div>
                if contracts.is_empty() {
                    <p class="px-4 py-8 text-sm font-light text-black/40">
                        {"No form-created contract artifacts are attached to this property yet."}
                    </p>
                } else {
                    <div class="divide-y divide-[var(--portal-panel-border)]">
                        { for contracts.into_iter().map(|contract| html! {
                            <div class="grid gap-2 px-4 py-3 md:grid-cols-[1.2fr_1fr_1fr_auto] md:items-center">
                                <div>
                                    <div class="text-sm font-medium text-[var(--portal-navy)]">{ contract.form_template_id.clone() }</div>
                                    <div class="text-xs font-light text-black/40">{ title_case(&contract.contract_type) }</div>
                                </div>
                                <div class="text-sm font-light text-black/60">{ title_case(&contract.status) }</div>
                                <div class="text-xs font-light text-black/45">
                                    { contract.executed_at.clone().unwrap_or_else(|| contract.created_at.clone()) }
                                </div>
                                <div class="text-[10px] font-light uppercase tracking-[0.12em] text-[var(--portal-navy-soft)]">
                                    { if contract.process_instance_id.is_some() { "Workflow attached" } else { "No workflow" } }
                                </div>
                            </div>
                        }) }
                    </div>
                }
            </section>
        </div>
    }
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
