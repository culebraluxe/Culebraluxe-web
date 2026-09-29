//! The entity switcher, the record selector rail, and creating a property.

use super::*;

pub(super) fn entity_switcher(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
    html! {
        <div class="portal-glass-panel flex flex-wrap items-center justify-between gap-3 rounded-[var(--portal-panel-radius)] px-3 py-2">
            <div class="flex min-w-0 items-center gap-3">
                <div class="hidden sm:block">
                    <div class="text-[10px] font-semibold uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{"OPPS"}</div>
                    <div class="font-serif text-lg font-light text-[var(--portal-navy)]">{"Data Workbench"}</div>
                </div>
                <div class="flex rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-1">
                    { entity_button(model, on_msg, "property", "⌂", "Property") }
                    { entity_button(model, on_msg, "person", "♙", "Person") }
                    { entity_button(model, on_msg, "project", "◇", "Project") }
                </div>
            </div>
            <div class="text-right">
                <div class="text-[10px] uppercase tracking-[0.14em] text-black/35">{"Canonical data"}</div>
                <div class="text-[12px] font-light text-black/55">{"Edit once · project downstream"}</div>
            </div>
        </div>
    }
}

pub(super) fn entity_button(
    model: &Vm<'_>,
    on_msg: &Callback<Msg>,
    key: &'static str,
    icon: &'static str,
    label: &'static str,
) -> Html {
    let active = model.ops.entity == key;
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsEntitySelected(key.into())))
    };
    html! {
        <button
            type="button"
            {onclick}
            aria-pressed={active.to_string()}
            class={classes!(
                "inline-flex","h-9","items-center","gap-1.5","rounded-[calc(var(--portal-tab-radius)-2px)]","px-3","text-[12px]","font-medium","transition",
                if active { "bg-[var(--portal-navy)] text-white shadow-sm" } else { "text-[var(--portal-navy)] hover:bg-white/60" }
            )}
        >
            <span class="text-[15px] leading-none" aria-hidden="true">{icon}</span>
            <span>{label}</span>
        </button>
    }
}

pub(super) fn selector_rail(
    model: &Vm<'_>,
    on_msg: &Callback<Msg>,
    total: i64,
    current: i64,
    pages: i64,
) -> Html {
    let data = payload(model);
    let rows = data.map(|data| data.rows.as_slice()).unwrap_or(&[]);
    let collapsed = model.ops.rail_collapsed;

    let search = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = crate::app::exec::input_value(&event);
            on_msg.emit(Msg::QueryChanged(value));
        })
    };
    let toggle = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsRailToggled))
    };
    let previous = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PageChanged(-1)))
    };
    let next = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PageChanged(1)))
    };
    let create = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsCreateToggled))
    };

    html! {
        <aside class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="shrink-0 border-b border-[var(--portal-panel-border)] p-2">
                <div class="flex items-center justify-between gap-2">
                    if !collapsed {
                        <div class="truncate text-[10px] font-semibold uppercase tracking-[0.14em] text-black/40">
                            { format!("{} · {total}", entity_plural(&model.ops.entity)) }
                        </div>
                    }
                    <button
                        type="button"
                        onclick={toggle}
                        title={if collapsed { "Expand selector" } else { "Collapse selector" }}
                        class="ml-auto grid h-8 w-8 shrink-0 place-items-center rounded-full border border-[var(--portal-panel-border)] bg-white/45 text-[var(--portal-navy)] transition hover:bg-white/75"
                    >
                        { if collapsed { "›" } else { "‹" } }
                    </button>
                </div>
                if !collapsed {
                    <input
                        type="search"
                        oninput={search}
                        value={model.controls.query.clone()}
                        disabled={model.ops.dirty}
                        placeholder={format!("Search {}…", entity_plural(&model.ops.entity).to_lowercase())}
                        class="mt-2 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-2.5 py-2 text-[13px] font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)] disabled:opacity-45"
                    />
                    if model.ops.entity == "property" {
                        <button
                            type="button"
                            onclick={create}
                            disabled={model.ops.dirty || !model.can("property.write")}
                            class="mt-2 flex h-8 w-full items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy)] hover:bg-white/60 disabled:opacity-35"
                        >
                            { if model.ops.creating { "Cancel new property" } else { "+ New property" } }
                        </button>
                        if model.ops.creating {
                            { create_property(model, on_msg) }
                        }
                    }
                }
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto">
                if rows.is_empty() {
                    <p class={if collapsed { "px-2 py-6 text-center text-xs text-black/35" } else { "px-3 py-6 text-sm font-light text-black/40" }}>
                        { if model.loading { "…" } else if collapsed { "—" } else { "No matching records." } }
                    </p>
                } else {
                    { for rows.iter().map(|row| {
                        let selected = data.and_then(|data| data.selected_id.as_deref()) == Some(row.id.as_str());
                        let id = row.id.clone();
                        let onclick = {
                            let on_msg = on_msg.clone();
                            Callback::from(move |_: MouseEvent| on_msg.emit(Msg::RowSelected(id.clone())))
                        };
                        html! {
                            <button
                                type="button"
                                {onclick}
                                title={if collapsed { row.title.clone() } else { String::new() }}
                                class={classes!(
                                    "w-full","border-b","border-[var(--portal-panel-border)]","text-left","transition",
                                    if collapsed { "grid h-11 place-items-center px-1" } else { "flex items-start gap-2 px-2.5 py-2.5" },
                                    if selected { "border-l-2 border-l-[var(--portal-gold)] bg-white/45" } else { "border-l-2 border-l-transparent hover:bg-white/25" }
                                )}
                            >
                                if collapsed {
                                    <span class={format!("h-2 w-2 rounded-full {}", status_dot(&row.status))}></span>
                                } else {
                                    <span class={format!("mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full {}", status_dot(&row.status))}></span>
                                    <span class="min-w-0 flex-1">
                                        <span class="block truncate text-[13px] font-medium text-[var(--portal-navy)]">{ row.title.clone() }</span>
                                        <span class="mt-0.5 block truncate text-[11px] font-light text-black/45">
                                            { compact_meta(row.subtitle.as_deref(), row.meta.as_deref(), &row.status) }
                                        </span>
                                    </span>
                                }
                            </button>
                        }
                    }) }
                }
            </div>

            if !collapsed {
                <div class="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-1.5">
                    <button type="button" onclick={previous} disabled={current <= 1 || model.ops.dirty}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">
                        {"← Prev"}
                    </button>
                    <span class="text-[10px] font-light text-black/40">{ format!("{current} / {pages}") }</span>
                    <button type="button" onclick={next} disabled={current >= pages || model.ops.dirty}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">
                        {"Next →"}
                    </button>
                </div>
            }
        </aside>
    }
}

pub(super) fn create_property(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
    let change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = crate::app::exec::input_value(&event);
            on_msg.emit(Msg::OpsCreateNameChanged(value));
        })
    };
    let change_type = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = crate::app::exec::input_value(&event);
            on_msg.emit(Msg::OpsCreateTypeChanged(value));
        })
    };
    let create = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsCreateRequested))
    };
    html! {
        <div class="mt-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-2">
            <input
                value={model.ops.new_name.clone()}
                oninput={change}
                placeholder="Property name"
                class="h-8 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/65 px-2 text-[12px] outline-none"
            />
            <input type="text" oninput={change_type} value={model.ops.new_property_type.clone()}
                placeholder="Property type (for example, Land)"
                class="mt-2 h-9 w-full rounded border border-[var(--portal-panel-border)] bg-white/65 px-2 text-[12px]" />
            <button
                type="button"
                onclick={create}
                disabled={model.loading || !model.can("property.write")}
                class="mt-2 h-8 w-full rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] text-[10px] font-semibold uppercase tracking-[0.12em] text-white disabled:opacity-40"
            >
                { if model.loading { "Creating…" } else { "Create & open" } }
            </button>
        </div>
    }
}
