//! The generic field renderers and the small shared cards and labels.

use super::*;

/// The first pane, each box as wide as what it holds: a name-sized name, the catastro fits
/// 476-000-005-19-000 with FIND beside it (opens the property that already has that catastro, filled from the property
/// table), and status and price are short.
pub(super) fn property_pane(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
    let [name, catastro, status, price] = [0, 1, 2, 3].map(|index| &PROPERTY_IDENTITY[index]);
    let find = on_msg.reform(|_: MouseEvent| Msg::FindByCatastro);
    let can_find = !value(model, "catastroNumber").trim().is_empty() && !model.loading;
    html! {
        <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-4">
            <div class="mb-3 text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--portal-gold-muted)]">{"Property"}</div>
            <div class="flex flex-wrap items-end gap-3">
                {sized_field(model, on_msg, name, "min-w-[14rem] flex-1")}
                <div class="flex items-end gap-1.5">
                    {sized_field(model, on_msg, catastro, "w-[13rem]")}
                    <button type="button" onclick={find} disabled={!can_find}
                        title="Open the property that already has this catastro number"
                        class="mb-px h-10 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[10px] font-semibold uppercase tracking-[0.12em] text-white disabled:opacity-40">
                        {"Find"}
                    </button>
                </div>
                {sized_field(model, on_msg, status, "w-[9.5rem]")}
                {sized_field(model, on_msg, price, "w-[10rem]")}
            </div>
        </section>
    }
}

pub(super) fn field_panel(model: &Vm<'_>, on_msg: &Callback<Msg>, title: &str, fields: &[FieldSpec]) -> Html {
    html! {
        <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-4">
            <div class="mb-3 text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--portal-gold-muted)]">{title}</div>
            {field_grid(model, on_msg, fields)}
        </section>
    }
}

pub(super) fn field_grid(model: &Vm<'_>, on_msg: &Callback<Msg>, fields: &[FieldSpec]) -> Html {
    html! {
        <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            {for fields.iter().map(|field| editor_field(model, on_msg, field))}
        </div>
    }
}

pub(super) fn editor_field(model: &Vm<'_>, on_msg: &Callback<Msg>, field: &FieldSpec) -> Html {
    sized_field(model, on_msg, field, if field.wide { "sm:col-span-2 xl:col-span-4" } else { "" })
}

/// A field whose box width the caller sets (the first pane sizes each box to what it holds).
pub(super) fn sized_field(model: &Vm<'_>, on_msg: &Callback<Msg>, field: &FieldSpec, wrapper: &'static str) -> Html {
    let field_value = value(model, field.key);
    let disabled = model.ops.saving;

    let control = match field.kind {
        FieldKind::Textarea(rows) => {
            let key = field.key.to_string();
            let on_msg = on_msg.clone();
            html! {
                <textarea
                    value={field_value}
                    rows={rows.to_string()}
                    disabled={disabled}
                    oninput={Callback::from(move |event: InputEvent| {
                        let value = crate::app::exec::textarea_value(&event);
                        on_msg.emit(Msg::OpsFieldChanged { key: key.clone(), value });
                    })}
                    class="mt-1.5 w-full resize-y rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 py-2 text-[13px] font-light leading-relaxed text-black/75 outline-none focus:border-[var(--portal-navy)] disabled:opacity-50"
                />
            }
        }
        FieldKind::Toggle => {
            let key = field.key.to_string();
            let on_msg = on_msg.clone();
            html! {
                <div class="mt-2 flex h-10 items-center">
                    <input
                        type="checkbox"
                        checked={field_value == "true"}
                        disabled={disabled}
                        onchange={Callback::from(move |event: Event| {
                            let checked = crate::app::exec::checked(&event);
                            on_msg.emit(Msg::OpsFieldChanged { key: key.clone(), value: checked.to_string() });
                        })}
                        class="h-4 w-4 rounded border-[var(--portal-panel-border)]"
                    />
                </div>
            }
        }
        FieldKind::Select(options) => {
            let key = field.key.to_string();
            let on_msg = on_msg.clone();
            html! {
                <select
                    value={field_value.clone()}
                    disabled={disabled}
                    onchange={Callback::from(move |event: Event| {
                        let value = crate::app::exec::select_value(&event);
                        on_msg.emit(Msg::OpsFieldChanged { key: key.clone(), value });
                    })}
                    class="mt-1.5 h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light text-black/75 outline-none focus:border-[var(--portal-navy)] disabled:opacity-50"
                >
                    {for options.iter().map(|(value, label)| html! {
                        <option value={*value} selected={field_value.as_str() == *value}>{*label}</option>
                    })}
                </select>
            }
        }
        FieldKind::Money => {
            let key = field.key.to_string();
            let on_msg = on_msg.clone();
            html! {
                <input
                    type="text"
                    inputmode="decimal"
                    placeholder="$0"
                    value={usd(&field_value)}
                    disabled={disabled}
                    oninput={Callback::from(move |event: InputEvent| {
                        let typed = crate::app::exec::input_value(&event);
                        let digits: String = typed.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
                        on_msg.emit(Msg::OpsFieldChanged { key: key.clone(), value: digits });
                    })}
                    class="mt-1.5 h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light text-black/75 outline-none focus:border-[var(--portal-navy)] disabled:opacity-50"
                />
            }
        }
        FieldKind::Date | FieldKind::Number | FieldKind::Text => {
            let input_type = match field.kind {
                FieldKind::Date => "date",
                FieldKind::Number => "number",
                _ => "text",
            };
            let step = if matches!(field.kind, FieldKind::Number) {
                "any"
            } else {
                ""
            };
            let key = field.key.to_string();
            let on_msg = on_msg.clone();
            html! {
                <input
                    type={input_type}
                    step={step}
                    value={field_value}
                    disabled={disabled}
                    oninput={Callback::from(move |event: InputEvent| {
                        let value = crate::app::exec::input_value(&event);
                        on_msg.emit(Msg::OpsFieldChanged { key: key.clone(), value });
                    })}
                    class="mt-1.5 h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light text-black/75 outline-none focus:border-[var(--portal-navy)] disabled:opacity-50"
                />
            }
        }
    };

    html! {
        <div class={wrapper}>
            <label class="block text-[10px] font-semibold uppercase tracking-[0.11em] text-[var(--portal-blue-gray)]">
                {field.label}
                {control}
            </label>
            if let Some(hint) = field.hint {
                <p class="mt-1 text-[10px] font-light leading-snug text-black/40">{hint}</p>
            }
        </div>
    }
}

pub(super) fn section_intro(title: &str, body: &str) -> Html {
    html! {
        <div>
            <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">{title}</div>
            <p class="mt-1 max-w-4xl text-[13px] font-light leading-relaxed text-black/55">{body}</p>
        </div>
    }
}

pub(super) fn inherited(label: &str, value: String) -> Html {
    html! {
        <div class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] p-3">
            <div class="text-[9px] font-semibold uppercase tracking-[0.13em] text-black/35">{label}</div>
            <div class="mt-1 truncate text-[12px] font-medium text-[var(--portal-navy)]">
                {if value.trim().is_empty() { "—".into() } else { value }}
            </div>
            <div class="mt-1 text-[9px] uppercase tracking-[0.1em] text-black/30">{"Inherited from Property"}</div>
        </div>
    }
}

pub(super) fn count_card(label: &str, count: i64) -> Html {
    html! {
        <div class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-4">
            <div class="font-serif text-3xl font-light text-[var(--portal-navy)]">{count}</div>
            <div class="mt-1 text-[10px] font-semibold uppercase tracking-[0.13em] text-black/40">{label}</div>
        </div>
    }
}

pub(super) fn readonly_card(label: &str, value: &str) -> Html {
    html! {
        <div class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-4">
            <div class="text-[9px] font-semibold uppercase tracking-[0.13em] text-black/35">{label}</div>
            <div class="mt-1 break-words text-[13px] font-light text-[var(--portal-navy)]">{value}</div>
        </div>
    }
}

pub(super) fn read_line(label: &str, value: &str) -> Html {
    html! {
        <div class="mt-2 flex gap-3 text-[12px]">
            <span class="w-20 shrink-0 text-black/35">{label}</span>
            <span class="min-w-0 break-all font-light text-[var(--portal-navy)]">{value}</span>
        </div>
    }
}

pub(super) fn empty_editor(loading: bool) -> Html {
    html! {
        <section class="portal-glass-panel grid h-full min-h-64 place-items-center rounded-[var(--portal-panel-radius)] p-8 text-center">
            <div>
                <div class="font-serif text-xl font-light text-[var(--portal-navy)]">
                    {if loading { crate::app::template::loading_words("the workbench") } else { "Select a record".to_owned() }}
                </div>
                <p class="mt-2 text-[12px] font-light text-black/45">
                    {"The same workspace edits every major entity."}
                </p>
            </div>
        </section>
    }
}

pub(super) fn empty_record(label: &str) -> Html {
    html! {
        <div class="py-12 text-center text-sm font-light text-black/45">
            {format!("No {label} record is loaded.")}
        </div>
    }
}

pub(super) fn entity_plural(entity: &str) -> &'static str {
    match entity {
        "person" => "People",
        "project" => "Projects",
        _ => "Properties",
    }
}

pub(super) fn status_dot(status: &str) -> &'static str {
    match status {
        "active" | "doing" | "open" => "bg-[var(--portal-success)]",
        "archived" | "done" | "sold" => "bg-black/30",
        "coming_soon" | "warm" => "bg-[var(--portal-gold)]",
        _ => "bg-[var(--portal-blue-gray)]",
    }
}

pub(super) fn compact_meta(subtitle: Option<&str>, meta: Option<&str>, status: &str) -> String {
    [Some(status), subtitle, meta]
        .into_iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.replace('_', " "))
        .collect::<Vec<_>>()
        .join(" · ")
}
