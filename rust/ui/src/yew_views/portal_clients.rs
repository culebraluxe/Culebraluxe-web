use yew::prelude::*;

use crate::model::{Msg, PortalClientDetail, PortalClientSummary, PortalClientsPage};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct ClientsProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Clients;
pub struct ClientRecord;

impl Component for Clients {
    type Message = ();
    type Properties = ClientsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("clients").expect("clients screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { workspace(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

impl Component for ClientRecord {
    type Message = ();
    type Properties = ClientsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("client-record").expect("client record exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                <a href="/portal/clients" class="mb-5 inline-flex text-sm font-light text-black/45">{"← Clients"}</a>
                { selected_body(&props.model) }
            </PortalShell>
        }
    }
}

fn payload(model: &crate::model::Model) -> Option<&PortalClientsPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.clients.as_ref())
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
        <div class="grid min-h-0 gap-4 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[200px_minmax(0,1fr)]">
            <aside class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="border-b border-[var(--portal-panel-border)] p-2.5">
                    <div class="mb-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                        { format!("People · {total}") }
                    </div>
                    <input type="search" {oninput} value={model.controls.query.clone()} placeholder="Search…"
                        class="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none" />
                </div>
                <div class="min-h-0 flex-1 overflow-y-auto">
                    if rows.is_empty() {
                        <p class="px-3 py-6 text-sm font-light text-black/40">
                            { if model.loading { "Loading…" } else { "No matching clients." } }
                        </p>
                    } else {
                        { for rows.iter().map(|row| row_view(row, data.and_then(|d| d.selected_id.as_deref()), on_msg)) }
                    }
                </div>
                <div class="flex items-center justify-between border-t border-[var(--portal-panel-border)] px-2 py-1.5">
                    <button type="button" onclick={previous} disabled={current <= 1}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] disabled:opacity-30">{"← Prev"}</button>
                    <span class="text-[10px] font-light text-black/40">{ format!("{} / {}", current, pages) }</span>
                    <button type="button" onclick={next} disabled={current >= pages}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] disabled:opacity-30">{"Next →"}</button>
                </div>
            </aside>

            <div class="min-h-0 overflow-y-auto">
                { selected_body(model) }
            </div>
        </div>
    }
}

fn row_view(
    row: &PortalClientSummary,
    selected_id: Option<&str>,
    on_msg: &Callback<Msg>,
) -> Html {
    let selected = selected_id == Some(row.id.as_str());
    let id = row.id.clone();
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::RowSelected(id.clone())))
    };
    let secondary = row
        .primary_phone
        .clone()
        .or_else(|| row.primary_email.clone())
        .unwrap_or_else(|| row.role.clone());

    html! {
        <button type="button" {onclick}
            class={classes!(
                "flex","w-full","items-center","gap-2","border-b","border-[var(--portal-panel-border)]","px-2.5","py-2","text-left",
                if selected { "border-l-2 border-l-[var(--portal-gold)] bg-white/40" } else { "border-l-2 border-l-transparent hover:bg-white/25" }
            )}>
            <span class={format!("h-1.5 w-1.5 shrink-0 rounded-full {}", status_dot(&row.status))}></span>
            <div class="min-w-0 flex-1">
                <div class="truncate text-[13px] font-medium text-[var(--portal-navy)]">
                    { if row.name_resolved { row.display_name.clone() } else { "Unknown contact".into() } }
                </div>
                <div class="truncate text-[11px] font-light text-black/45">{ secondary }</div>
                if row.observed_count > 0 {
                    <div class="truncate text-[10px] font-light text-black/35">
                        { format!("{} observed{}", row.observed_count, if row.two_way { " · two-way" } else { "" }) }
                    </div>
                }
            </div>
        </button>
    }
}

fn selected_body(model: &crate::model::Model) -> Html {
    let Some(data) = payload(model) else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
                <p class="text-sm font-light text-black/45">{ if model.loading { "Loading selected client…" } else { "Select a client." } }</p>
            </section>
        };
    };
    let Some(client) = data.selected.as_ref() else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
                <p class="text-sm font-light text-black/45">{"Select a client."}</p>
            </section>
        };
    };

    html! {
        <div class="flex flex-col gap-4">
            { identity_panel(client, data) }
            <div class="grid gap-4 lg:grid-cols-2">
                <div class="flex flex-col gap-4">
                    { property_panel(data) }
                    { notes_panel(client) }
                </div>
                { relationship_panel(data) }
            </div>
        </div>
    }
}

fn identity_panel(client: &PortalClientDetail, data: &PortalClientsPage) -> Html {
    let budget = budget_label(client);
    let aggregate = data.comms.as_ref().map(|comms| &comms.aggregate);
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
            <div class="flex items-start justify-between gap-4 border-b border-[var(--portal-panel-border)] pb-4">
                <div>
                    <h1 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{ client.display_name.clone() }</h1>
                    <p class="mt-1 text-xs font-light text-black/50">{ format!("{} · {}", client.role, client.status) }</p>
                </div>
                if let Some(summary) = aggregate {
                    <div class="text-right text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
                        <div>{ format!("{} observed", summary.observed_count) }</div>
                        <div>{ format!("{} / {} sources", summary.active_source_count, summary.source_count) }</div>
                    </div>
                }
            </div>
            <div class="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                { field("Phone", client.phone.as_deref()) }
                { field("Email", client.email.as_deref()) }
                { field("Agent", client.assigned_agent.as_deref()) }
                { field("Timeline", client.timeline.as_deref()) }
                { field("Budget", Some(budget.as_str())) }
                { field("Last Contact", aggregate.and_then(|value| value.last_contact_label.as_deref())) }
            </div>
        </section>
    }
}

fn field(label: &str, value: Option<&str>) -> Html {
    let value = value.filter(|value| !value.is_empty()).unwrap_or("—");
    html! {
        <div>
            <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">{ label }</div>
            <div class="mt-1 text-sm font-light leading-5 text-black/70">{ value }</div>
        </div>
    }
}

fn property_panel(data: &PortalClientsPage) -> Html {
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
            <div class="flex items-center justify-between">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Property Context"}</h2>
                <span class="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">{ format!("{} properties", data.properties.len()) }</span>
            </div>
            <div class="mt-4 space-y-2">
                if data.properties.is_empty() {
                    <p class="text-sm font-light text-black/45">{"No Property relationship recorded for this client yet."}</p>
                } else {
                    { for data.properties.iter().map(|property| html! {
                        <div class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-3 py-2.5">
                            <div class="font-serif text-base font-light text-[var(--portal-navy)]">{ property.display_name.clone() }</div>
                            <div class="mt-1 text-xs font-light text-black/55">{ property.address.clone() }</div>
                            <div class="mt-1 text-[9px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                                { property.relation_status.clone().unwrap_or_else(|| property.relation.clone()) }
                            </div>
                        </div>
                    }) }
                }
            </div>
        </section>
    }
}

fn notes_panel(client: &PortalClientDetail) -> Html {
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
            <div class="flex items-baseline justify-between gap-3">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Notes"}</h2>
                <span class="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">{"Read only in Rust API"}</span>
            </div>
            <div class="mt-4 min-h-28 whitespace-pre-wrap rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55 p-3 font-serif text-[15px] font-light leading-6 text-[var(--portal-navy)]">
                { client.notes.clone().filter(|value| !value.is_empty()).unwrap_or_else(|| "No notes yet.".into()) }
            </div>
        </section>
    }
}

fn relationship_panel(data: &PortalClientsPage) -> Html {
    let Some(comms) = data.comms.as_ref() else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Contact History"}</h2>
                <p class="mt-4 text-sm font-light text-black/45">{"No relationship evidence."}</p>
            </section>
        };
    };

    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
            <div class="flex items-baseline justify-between">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Contact History"}</h2>
                <span class="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">{ format!("{} moments", comms.moment_count) }</span>
            </div>
            <div class="mt-4 grid gap-2 sm:grid-cols-2">
                { for comms.sources.iter().filter(|source| source.total_count > 0).map(|source| html! {
                    <div class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 p-3">
                        <div class="flex items-center justify-between">
                            <span class="text-xs font-medium text-[var(--portal-navy)]">{ source.label.clone() }</span>
                            <span class="text-[10px] font-light text-black/40">{ source.total_count }</span>
                        </div>
                        if let Some(context) = source.last_context.clone() {
                            <p class="mt-1 text-xs font-light leading-5 text-black/50">{ context }</p>
                        }
                        if source.two_way {
                            <div class="mt-1 text-[9px] font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">{"two-way"}</div>
                        }
                    </div>
                }) }
            </div>
            <ol class="mt-5 space-y-3 border-t border-[var(--portal-panel-border)] pt-4">
                { for comms.moments.iter().map(|moment| html! {
                    <li class="border-b border-[var(--portal-panel-border)] pb-3 last:border-0">
                        <div class="flex items-center justify-between gap-3">
                            <span class="text-[11px] font-medium text-[var(--portal-navy)]">
                                { moment.channel.clone().unwrap_or_else(|| "Contact".into()) }
                            </span>
                            <span class="text-[10px] font-light text-black/35">{ moment.occurred_at.clone() }</span>
                        </div>
                        <p class="mt-1 text-sm font-light leading-5 text-black/60">
                            { moment.summary.clone().or_else(|| moment.title.clone()).unwrap_or_else(|| "Interaction".into()) }
                        </p>
                    </li>
                }) }
            </ol>
        </section>
    }
}

fn budget_label(client: &PortalClientDetail) -> String {
    match (client.budget_min, client.budget_max) {
        (None, None) => "—".into(),
        (min, max) => format!(
            "{} – {}",
            min.map(currency).unwrap_or_else(|| "—".into()),
            max.map(currency).unwrap_or_else(|| "—".into())
        ),
    }
}

fn currency(value: f64) -> String {
    format!("{}{}", '\u{24}', format!("{value:.0}"))
}

fn status_dot(status: &str) -> &'static str {
    match status {
        "active" => "bg-emerald-500",
        "warm" => "bg-amber-400",
        "new" => "bg-sky-400",
        "referral" => "bg-violet-400",
        _ => "bg-black/20",
    }
}
