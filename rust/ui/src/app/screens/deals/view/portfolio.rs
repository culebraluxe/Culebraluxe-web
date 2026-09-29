//! The contracts portfolio: creating a deal, the stage filter, deal rows and each deal's contracts.

#[allow(unused_imports)]
use super::*;

pub(super) fn create_panel(
    model: &Vm<'_>,
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
            let value = crate::app::exec::select_value(&event);
            on_msg.emit(Msg::DealCreatePropertyChanged(value));
        })
    };
    let client_input = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = crate::app::exec::input_value(&event);
            on_msg.emit(Msg::DealCreateClientQueryChanged(value));
        })
    };
    let owner_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = crate::app::exec::select_value(&event);
            on_msg.emit(Msg::DealCreateOwnerChanged(value));
        })
    };
    let notes_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = crate::app::exec::textarea_value(&event);
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
                            <option value="" selected={state.property_id.is_empty()}>{"Choose an active property…"}</option>
                            { for properties.iter().map(|property| html! {
                                <option value={property.id.clone()} selected={property.id == state.property_id}>
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
                            <option value="" selected={state.owner_user_id.is_empty()}>{"Unassigned…"}</option>
                            { for users.iter().map(|user| html! {
                                <option value={user.id.clone()} selected={user.id == state.owner_user_id}>
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

pub(super) fn person_choice(person: &PortalDealPersonCandidate, on_msg: &Callback<Msg>) -> Html {
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

pub(super) fn stage_filter(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
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

pub(super) fn deal_row(deal: &PortalDeal) -> Html {
    let price = deal
        .latest_offer_amount
        .or(deal.offer_price)
        .or(deal.list_price);
    html! {
        <tr class="border-b border-[var(--portal-panel-border)] last:border-b-0 hover:bg-white/25">
            <td class="px-4 py-3">
                <div class="flex items-center gap-3">
                    if let Some(media_id) = deal.hero_media_id.as_ref() {
                        <img src={crate::app::api::links::media(&media_id)} alt={deal.property_name.clone()}
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

pub(super) fn table_head(label: &str) -> Html {
    html! {
        <th class="px-4 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-black/40">
            { label }
        </th>
    }
}

pub(super) fn contracts_panel(contracts: &[PortalDealContract]) -> Html {
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

pub(super) fn contract_row(contract: &PortalDealContract) -> Html {
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
