//! A property's people: owners and parties, found, linked and edited.

use super::*;

pub(super) fn property_person_editor(
    model: &Vm<'_>,
    property: &PortalOpsProperty,
    on_msg: &Callback<Msg>,
) -> Html {
    let selected = model.ops.selected_person.as_ref();
    let linked_id = value(model, "sellerPersonId");
    let name = selected
        .map(|person| person.display_name.as_str())
        .or(property.seller_name.as_deref())
        .unwrap_or("No Person linked");
    let phone = selected
        .and_then(|person| person.phone.as_deref())
        .or(property.seller_phone.as_deref())
        .unwrap_or("—");
    let email = selected
        .and_then(|person| person.email.as_deref())
        .or(property.seller_email.as_deref())
        .unwrap_or("—");
    let location = selected
        .and_then(|person| person.location.as_deref())
        .or(property.seller_location.as_deref())
        .unwrap_or("—");

    let on_query = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = crate::app::exec::input_value(&event);
            on_msg.emit(Msg::OpsPersonQueryChanged(value));
        })
    };
    let clear = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsPersonSelected(String::new())))
    };

    html! {
        <div class="space-y-4">
            {section_intro(
                "Person",
                "Link the Property to a canonical Person using the identity humans actually know: name, phone or email. The UUID stays visible for diagnostics, not as the picker.",
            )}
            <div class="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(320px,0.9fr)]">
                <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-4">
                    <div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                        {"Linked seller / owner"}
                    </div>
                    <div class="mt-2 font-serif text-2xl font-light text-[var(--portal-navy)]">{name}</div>
                    <div class="mt-4 grid gap-3 sm:grid-cols-2">
                        {readonly_card("Phone", phone)}
                        {readonly_card("Email", email)}
                        {readonly_card("Location", location)}
                        {readonly_card("Person ID", if linked_id.is_empty() { "—" } else { linked_id.as_str() })}
                    </div>
                </section>

                <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-4">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                                {"Find Person"}
                            </div>
                            <p class="mt-1 text-[11px] font-light text-black/45">{"Search name, phone or email."}</p>
                        </div>
                        if !linked_id.is_empty() {
                            <button
                                type="button"
                                onclick={clear}
                                class="text-[10px] font-semibold uppercase tracking-[0.11em] text-black/45 hover:text-[var(--portal-navy)]"
                            >
                                {"Clear link"}
                            </button>
                        }
                    </div>
                    <input
                        type="search"
                        value={model.ops.person_query.clone()}
                        oninput={on_query}
                        placeholder="Lisa · 787… · lisa@…"
                        class="mt-3 h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light outline-none focus:border-[var(--portal-navy)]"
                    />
                    if model.ops.person_searching {
                        <div class="px-1 py-3 text-[11px] font-light text-black/40">{"Searching…"}</div>
                    } else if !model.ops.person_people.is_empty() {
                        <div class="mt-2 max-h-64 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55">
                            {for model.ops.person_people.iter().map(|person| {
                                let id = person.id.clone();
                                let onclick = {
                                    let on_msg = on_msg.clone();
                                    Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsPersonSelected(id.clone())))
                                };
                                let detail = person
                                    .phone
                                    .as_deref()
                                    .or(person.email.as_deref())
                                    .or(person.location.as_deref())
                                    .unwrap_or("No contact identity");
                                html! {
                                    <button
                                        type="button"
                                        {onclick}
                                        class="flex w-full items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-3 py-2.5 text-left last:border-b-0 hover:bg-white/75"
                                    >
                                        <span class="min-w-0">
                                            <span class="block truncate text-[13px] font-medium text-[var(--portal-navy)]">
                                                {person.display_name.clone()}
                                            </span>
                                            <span class="mt-0.5 block truncate text-[11px] font-light text-black/45">{detail}</span>
                                        </span>
                                        <span class="shrink-0 text-[9px] uppercase tracking-[0.1em] text-black/35">
                                            {person.role.clone()}
                                        </span>
                                    </button>
                                }
                            })}
                        </div>
                    } else if model.ops.person_query.trim().len() >= 2 {
                        <div class="px-1 py-3 text-[11px] font-light text-black/40">{"No matching people."}</div>
                    }
                </section>
            </div>

            <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/25 p-4">
                <div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">{"Property record"}</div>
                <div class="mt-1 grid gap-x-6 sm:grid-cols-3">
                    {read_line("Property ID", &property.id)}
                    {read_line("Created", property.created_at.as_deref().unwrap_or("—"))}
                    {read_line("Updated", property.updated_at.as_deref().unwrap_or("—"))}
                </div>
            </section>
        </div>
    }
}
