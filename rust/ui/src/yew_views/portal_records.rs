//! Records — bounded property ledger.
//!
//! Same shape as `portal_clients.rs`: PortalShell, search rail, selected workspace,
//! intents go out as `Msg`. The reducer owns archive/restore.
//!
//! Not registered in mod.rs until OPS-RECORDS-MEDIA-01 types land in model.rs.

use yew::prelude::*;

use crate::model::{Msg, PortalRecordProperty, PortalRecordsPage};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct RecordsProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Records;

impl Component for Records {
    type Message = ();
    type Properties = RecordsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("property-admin").expect("records screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { workspace(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

fn payload(model: &crate::model::Model) -> Option<&PortalRecordsPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.records.as_ref())
}

fn workspace(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let data = payload(model);
    let rows = data.map(|page| page.rows.as_slice()).unwrap_or(&[]);
    let total = data.map(|page| page.total).unwrap_or(0);
    let current = data.map(|page| page.page).unwrap_or(1);
    let page_size = data.map(|page| page.page_size.max(1)).unwrap_or(50);
    let pages = ((total + page_size - 1) / page_size).max(1);

    let oninput = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::QueryChanged(value));
        })
    };
    let previous = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PageChanged(-1)))
    };
    let next = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PageChanged(1)))
    };

    html! {
        <div class="grid min-h-0 gap-4 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[220px_minmax(0,1fr)]">
            <aside class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
                    <div class="mb-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                        { format!("Properties · {total}") }
                    </div>
                    <input
                        type="search"
                        {oninput}
                        value={model.controls.query.clone()}
                        placeholder="Search…"
                        class="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
                    />
                </div>
                <div class="min-h-0 flex-1 overflow-y-auto">
                    if rows.is_empty() {
                        <p class="px-3 py-6 text-sm font-light text-black/40">
                            { if model.loading { "Loading…" } else { "No matching properties." } }
                        </p>
                    } else {
                        { for rows.iter().map(|row| row_view(row, data.and_then(|d| d.selected_id.as_deref()), on_msg)) }
                    }
                </div>
                <div class="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-1.5">
                    <button type="button" onclick={previous} disabled={current <= 1}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">
                        {"← Prev"}
                    </button>
                    <span class="text-[10px] font-light text-black/40">{ format!("{current} / {pages}") }</span>
                    <button type="button" onclick={next} disabled={current >= pages}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">
                        {"Next →"}
                    </button>
                </div>
            </aside>
            <div class="min-h-0 overflow-hidden">
                { selected_workspace(model, on_msg) }
            </div>
        </div>
    }
}

fn row_view(
    row: &PortalRecordProperty,
    selected_id: Option<&str>,
    on_msg: &Callback<Msg>,
) -> Html {
    let selected = selected_id == Some(row.id.as_str());
    let id = row.id.clone();
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::RowSelected(id.clone())))
    };
    html! {
        <button type="button" {onclick}
            class={classes!(
                "flex","w-full","items-center","gap-2","border-b","border-[var(--portal-panel-border)]","px-2.5","py-2","text-left","transition",
                if selected {
                    "border-l-2 border-l-[var(--portal-gold)] bg-white/40"
                } else {
                    "border-l-2 border-l-transparent hover:bg-white/25"
                }
            )}>
            <span class={format!("h-1.5 w-1.5 shrink-0 rounded-full {}", if row.archived { "bg-black/30" } else { "bg-[var(--portal-success)]" })}></span>
            <div class="min-w-0 flex-1">
                <div class="truncate text-[13px] font-medium text-[var(--portal-navy)]">{ row.name.clone() }</div>
                <div class="truncate text-[11px] font-light text-black/45">
                    { format!("{} · {}", row.status, row.location) }
                </div>
            </div>
        </button>
    }
}

fn selected_workspace(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let Some(data) = payload(model) else {
        return empty(model.loading);
    };
    let Some(property) = data.selected.as_ref() else {
        return empty(model.loading);
    };

    let archived = property.archived;
    let archive = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::RecordArchiveRequested))
    };

    html! {
        <div class="flex h-full min-h-0 flex-col gap-3">
            { status_band(model, property) }
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">{"Property"}</div>
                <h2 class="mt-2 font-serif text-2xl font-light text-[var(--portal-navy)]">{ property.name.clone() }</h2>
                <div class="mt-1 text-xs font-light text-black/50">
                    { format!("{} · {}", property.status, property.location) }
                </div>
                <div class="mt-4 grid gap-4 sm:grid-cols-2">
                    { field("List price", property.list_price.as_deref()) }
                    { field("Slug", property.slug.as_deref()) }
                    { field("Photos", Some(&property.image_count.to_string())) }
                    { field("State", Some(if archived { "Archived" } else { "Active" })) }
                </div>
                <div class="mt-6 flex gap-2">
                    <button type="button" onclick={archive} disabled={model.loading}
                        class="inline-flex h-10 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:opacity-45">
                        { if archived { "Restore" } else { "Archive" } }
                    </button>
                    <a href="/portal/property-media"
                        class="inline-flex h-10 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy)]">
                        {"Photos"}
                    </a>
                </div>
            </section>
        </div>
    }
}

fn empty(loading: bool) -> Html {
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
            <p class="text-sm font-light text-black/45">
                { if loading { "Loading selected property…" } else { "Select a property." } }
            </p>
        </section>
    }
}

fn status_band(model: &crate::model::Model, property: &PortalRecordProperty) -> Html {
    let status_text = if model.loading {
        "Working…".into()
    } else if let Some(error) = model.error.as_ref() {
        error.clone()
    } else {
        format!("Ready · {}", property.name)
    };
    html! {
        <section class="portal-glass-panel px-4 py-2.5 rounded-[var(--portal-panel-radius)]">
            <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{"Status"}</p>
            <p class="mt-1 font-serif text-[15px] font-light text-[var(--portal-navy)]">{ status_text }</p>
        </section>
    }
}

fn field(label: &str, value: Option<&str>) -> Html {
    let value = value.filter(|value| !value.trim().is_empty()).unwrap_or("—");
    html! {
        <div>
            <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">{ label }</div>
            <div class="mt-1 text-sm font-light text-black/70">{ value }</div>
        </div>
    }
}
