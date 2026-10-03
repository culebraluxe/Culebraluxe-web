//! The forms rail: the list of forms, new-form entry, and the empty state.

#[allow(unused_imports)]
use super::*;

pub(super) fn forms_rail(
    model: &Model,
    page: &FormsPage,
    form: &FormItem,
    template: &FormTemplate,
    link: &Link<Msg>,
) -> Html {
    let needle = model.session_query.trim().to_lowercase();
    let visible = page
        .items
        .iter()
        .filter(|item| item.template_id == form.template_id)
        .filter(|item| {
            if needle.is_empty() {
                return true;
            }
            session_label(item).to_lowercase().contains(&needle)
        })
        .collect::<Vec<_>>();
    let query_changed = link.callback(|event: InputEvent| {
        Msg::SessionQueryChanged(crate::app::exec::input_value(&event))
    });
    let template_changed =
        link.callback(|event: Event| Msg::TemplateSelected(crate::app::exec::select_value(&event)));

    html! {
        <aside class="portal-glass-panel flex max-h-72 min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)] lg:max-h-none">
            <div class="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
                <div class="mb-2 flex items-center justify-between gap-2">
                    <span class="text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                        { format!("Forms · {}", visible.len()) }
                    </span>
                    <button
                        type="button"
                        disabled={model.busy}
                        onclick={link.callback(|_: MouseEvent| Msg::NewForm)}
                        class="inline-flex min-h-7 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:opacity-40"
                    >
                        {"New"}
                    </button>
                </div>
                if model.new_form_template.is_some() {
                    <div class="mb-2 space-y-1.5 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/60 p-2">
                        <p class="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">{"Who is this form for?"}</p>
                        <input value={model.new_seller.clone()} placeholder="Seller, as on the contract"
                            oninput={link.callback(|event: InputEvent| Msg::NewSellerChanged(crate::app::exec::input_value(&event)))}
                            class="block h-8 w-full rounded-md border border-[var(--portal-panel-border)] bg-white px-2 text-[12px] text-[var(--portal-navy)]" />
                        <input value={model.new_catastro.clone()} placeholder="Catastro number (optional)"
                            oninput={link.callback(|event: InputEvent| Msg::NewCatastroChanged(crate::app::exec::input_value(&event)))}
                            class="block h-8 w-full rounded-md border border-[var(--portal-panel-border)] bg-white px-2 text-[12px] text-[var(--portal-navy)]" />
                        <div class="flex justify-end gap-1.5">
                            <button type="button" onclick={link.callback(|_: MouseEvent| Msg::NewFormCancel)}
                                class="h-7 rounded-md px-2 text-[10px] uppercase tracking-[0.1em] text-black/50 hover:text-black/80">{"Cancel"}</button>
                            <button type="button" disabled={model.busy} onclick={link.callback(|_: MouseEvent| Msg::NewFormCreate)}
                                class="h-7 rounded-md bg-[var(--portal-navy)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-white disabled:opacity-40">{"Create"}</button>
                        </div>
                    </div>
                }
                <input
                    type="search"
                    value={model.session_query.clone()}
                    oninput={query_changed}
                    placeholder="Search…"
                    class="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
                />
                <select
                    value={model.selected_template.clone()}
                    disabled={model.busy}
                    onchange={template_changed}
                    class="mt-2 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none focus:border-[var(--portal-navy)] disabled:opacity-40"
                >
                    {
                        for page.template_choices.iter().map(|item| html! {
                            <option value={item.id.clone()}>
                                { &item.display_name }
                            </option>
                        })
                    }
                </select>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto">
                {
                    if visible.is_empty() {
                        html! {
                            <p class="px-3 py-6 text-sm font-light text-black/40">
                                { format!("No matching {} forms{}",
                                    template.display_name.to_lowercase(),
                                    if needle.is_empty() { " yet." } else { "." }
                                ) }
                            </p>
                        }
                    } else {
                        html! {
                            <>
                                {
                                    for visible.into_iter().map(|item| {
                                        let selected = item.id == form.id;
                                        let id = item.id.clone();
                                        let open = link.callback(move |_: MouseEvent| Msg::OpenForm(id.clone()));
                                        html! {
                                            <button
                                                type="button"
                                                disabled={model.busy}
                                                onclick={open}
                                                class={format!(
                                                    "flex w-full items-center gap-2 border-b border-[var(--portal-panel-border)] px-2.5 py-2 text-left transition {}",
                                                    if selected {
                                                        "border-l-2 border-l-[var(--portal-gold)] bg-white/40"
                                                    } else {
                                                        "border-l-2 border-l-transparent hover:bg-white/25"
                                                    }
                                                )}
                                            >
                                                <span
                                                    class={format!(
                                                        "h-1.5 w-1.5 shrink-0 rounded-full {}",
                                                        if selected && model.dirty {
                                                            "bg-[var(--portal-gold)]"
                                                        } else {
                                                            status_dot_class(&item.status)
                                                        }
                                                    )}
                                                />
                                                <div class="min-w-0 flex-1">
                                                    <div class="truncate text-[13px] font-medium text-[var(--portal-navy)]">
                                                        { party_name(item) }
                                                    </div>
                                                    <div class="truncate text-[11px] font-light text-black/45">
                                                        { session_secondary(item, template) }
                                                    </div>
                                                </div>
                                            </button>
                                        }
                                    })
                                }
                            </>
                        }
                    }
                }
            </div>
        </aside>
    }
}

pub(super) fn empty_forms_view(page: &FormsPage, model: &Model, _link: &Link<Msg>) -> Html {
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-8">
            { error_band(model.error.as_deref()) }
            <h1 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"Forms"}</h1>
            {
                if page.items.is_empty() {
                    html! {
                        <p class="mt-3 text-sm font-light text-black/45">{"No saved forms yet."}</p>
                    }
                } else {
                    html! {
                        <p class="mt-3 text-sm font-light text-black/45">
                            {"Select a saved form to open the working surface."}
                        </p>
                    }
                }
            }
        </section>
    }
}
