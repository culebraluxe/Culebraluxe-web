use yew::prelude::*;

use crate::icons::icon_html;
use crate::model::{
    Msg, PortalClientDetail, PortalClientSummary, PortalClientsPage, PortalCommsAggregate,
    PortalCommsSource,
};
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
                <a href="/portal/clients" class="mb-3 inline-flex text-sm font-light text-black/45">{"← Clients"}</a>
                { selected_workspace(&props.model) }
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
                <div class="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
                    <div class="mb-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                        { format!("People · {total}") }
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
                            { if model.loading { "Loading…" } else { "No matching clients." } }
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
                { selected_workspace(model) }
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
                "flex","w-full","items-center","gap-2","border-b","border-[var(--portal-panel-border)]","px-2.5","py-2","text-left","transition",
                if selected {
                    "border-l-2 border-l-[var(--portal-gold)] bg-white/40"
                } else {
                    "border-l-2 border-l-transparent hover:bg-white/25"
                }
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

fn selected_workspace(model: &crate::model::Model) -> Html {
    let Some(data) = payload(model) else {
        return empty_client(model.loading);
    };
    let Some(client) = data.selected.as_ref() else {
        return empty_client(model.loading);
    };

    html! {
        <div class="flex h-full min-h-0 flex-col gap-3">
            { command_status_band(model, client) }

            <div class="grid min-h-0 flex-1 gap-3 lg:grid-cols-2 lg:gap-4">
                <main class="min-h-0 overflow-y-auto">
                    <div class="flex flex-col gap-4">
                        { identity_panel(client, data) }
                        { property_panel(data) }
                        { notes_panel(client) }
                    </div>
                </main>

                { relationship_panel(client, data) }
            </div>
        </div>
    }
}

fn empty_client(loading: bool) -> Html {
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
            <p class="text-sm font-light text-black/45">
                { if loading { "Loading selected client…" } else { "Select a client." } }
            </p>
        </section>
    }
}

fn command_status_band(model: &crate::model::Model, client: &PortalClientDetail) -> Html {
    let status_text = if model.loading {
        "Loading client…".to_string()
    } else if let Some(error) = model.error.as_ref() {
        error.clone()
    } else {
        format!("Ready · {}", client.display_name)
    };
    let tone = if model.error.is_some() {
        "bg-[var(--portal-archive)]"
    } else if model.loading {
        "bg-black/25"
    } else {
        "bg-[var(--portal-success)]"
    };

    html! {
        <div class="grid grid-cols-1 gap-3 lg:grid-cols-2 lg:gap-4">
            <section class="portal-glass-panel portal-glass-panel-lifted flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="shrink-0 border-b border-[var(--portal-panel-border)] px-4 py-2.5">
                    <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
                        {"Grok"}
                    </p>
                </div>
                <div class="flex min-h-0 flex-1 items-center gap-2 px-4 py-2.5">
                    <input
                        type="text"
                        disabled=true
                        placeholder={format!("Ask Grok about {}…", client.display_name)}
                        class="h-10 min-w-0 flex-1 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55 px-3 font-serif text-[15px] font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35 disabled:cursor-not-allowed disabled:opacity-70"
                    />
                    <button type="button" disabled=true
                        class="inline-flex h-10 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:cursor-not-allowed disabled:opacity-45">
                        {"Go"}
                    </button>
                </div>
            </section>

            <section aria-label="Status"
                class="portal-glass-panel portal-glass-panel-lifted flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="flex shrink-0 items-center justify-between gap-2 border-b border-[var(--portal-panel-border)] px-4 py-2.5">
                    <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{"Status"}</p>
                    <span aria-hidden=true class={classes!("h-2","w-2","shrink-0","rounded-full",tone)}></span>
                </div>
                <div aria-live="polite"
                    class="min-h-0 flex-1 overflow-hidden px-4 py-2.5 font-serif text-[15px] font-light leading-6 text-[var(--portal-navy)] line-clamp-3">
                    { status_text }
                </div>
            </section>
        </div>
    }
}

fn identity_panel(client: &PortalClientDetail, data: &PortalClientsPage) -> Html {
    let budget = budget_label(client);
    let aggregate = data.comms.as_ref().map(|comms| &comms.aggregate);

    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
            <div class="flex items-start justify-between gap-3 border-b border-[var(--portal-panel-border)] pb-4">
                <div>
                    <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">{"Client"}</div>
                    <h2 class="mt-2 font-serif text-2xl font-light text-[var(--portal-navy)]">{ client.display_name.clone() }</h2>
                    <div class="mt-1 flex items-center gap-2 text-xs font-light text-black/50">
                        <span>{ client.role.clone() }</span><span>{"·"}</span><span>{ client.status.clone() }</span>
                    </div>
                </div>
                if let Some(summary) = aggregate {
                    <div class="text-right text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
                        <div>{ format!("{} observed", summary.observed_count.to_string()) }</div>
                        <div>{ format!("{} / {} sources", summary.active_source_count, summary.source_count) }</div>
                    </div>
                }
            </div>

            <div class="mt-4 grid gap-4 sm:grid-cols-2">
                { detail_field("Phone", client.phone.as_deref()) }
                { detail_field("Email", client.email.as_deref()) }
                { detail_field("Agent", client.assigned_agent.as_deref()) }
                { detail_field("Timeline", client.timeline.as_deref()) }
                { detail_field("Budget", Some(budget.as_str())) }
            </div>
        </section>
    }
}

fn detail_field(label: &str, value: Option<&str>) -> Html {
    let value = value.filter(|value| !value.trim().is_empty()).unwrap_or("—");
    let populated = value != "—";
    html! {
        <div>
            <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">{ label }</div>
            <div class="mt-1 flex items-start gap-2 text-sm font-light leading-5 text-black/70">
                <span aria-hidden=true class={classes!(
                    "mt-1.5","h-2","w-2","shrink-0","rounded-full","ring-1","ring-black/35",
                    if populated { "bg-[var(--portal-gold)]" } else { "bg-transparent" }
                )}></span>
                <span class="min-w-0 break-words">{ value }</span>
            </div>
        </div>
    }
}

fn property_panel(data: &PortalClientsPage) -> Html {
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
            <div class="flex items-center justify-between">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Property Context"}</h2>
                <span class="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
                    { format!("{} properties", data.properties.len()) }
                </span>
            </div>
            <div class="mt-4 space-y-2">
                if data.properties.is_empty() {
                    <p class="text-sm font-light text-black/45">
                        {"No Property relationship recorded for this client yet."}
                    </p>
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
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
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

fn relationship_panel(client: &PortalClientDetail, data: &PortalClientsPage) -> Html {
    let comms = data.comms.as_ref();
    let observed = comms.map(|value| value.aggregate.observed_count).unwrap_or(0);
    let active_sources = comms
        .map(|value| value.aggregate.active_source_count)
        .unwrap_or(0);
    let source_count = comms.map(|value| value.aggregate.source_count).unwrap_or(6);

    html! {
        <section class="portal-glass-panel portal-glass-panel-feature flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
            <header class="flex shrink-0 items-center justify-between gap-3 px-4 py-4">
                <h2 class="font-serif text-2xl font-light text-white">{"Contact History"}</h2>
                <span class="text-xs font-light text-white/50">{ format!("{observed} observed") }</span>
            </header>

            if let Some(comms) = comms {
                { compact_relationship_header(&comms.aggregate) }
            }

            <div class="min-h-0 flex-1 overflow-auto">
                <ol class="divide-y divide-white/10">
                    { source_activity_row("call", "Phone", comms.and_then(|value| source_for(&value.sources, "call")), &client.display_name) }
                    { source_activity_row("imessage", "iMessage", comms.and_then(|value| source_for(&value.sources, "imessage")), &client.display_name) }
                    { source_activity_row("whatsapp", "WhatsApp", comms.and_then(|value| source_for(&value.sources, "whatsapp")), &client.display_name) }
                    { source_activity_row("email", "Email", comms.and_then(|value| source_for(&value.sources, "email")), &client.display_name) }
                    { source_activity_row("facetime", "FaceTime", comms.and_then(|value| source_for(&value.sources, "facetime")), &client.display_name) }
                    { source_activity_row("calendar", "Apple Calendar", comms.and_then(|value| source_for(&value.sources, "calendar")), &client.display_name) }
                </ol>
            </div>

            <div class="border-t border-white/10 px-3 py-1.5 text-center text-[10px] font-light uppercase tracking-[0.12em] text-white/45">
                { format!("{active_sources} of {source_count} sources connected") }
            </div>
            { quick_action_dock(client) }
        </section>
    }
}

fn compact_relationship_header(aggregate: &PortalCommsAggregate) -> Html {
    html! {
        <div class="border-b border-white/10 px-4 py-2">
            <p class="truncate text-[11px] font-light text-white/70">
                { format!(
                    "{} observed · {} inbound · {} outbound{}{}",
                    aggregate.observed_count,
                    aggregate.inbound_count,
                    aggregate.outbound_count,
                    if aggregate.two_way { " · two-way" } else { "" },
                    aggregate.first_observed_at.as_deref().map(|value| format!(" · since {}", short_date(value))).unwrap_or_default()
                ) }
            </p>
            <div class="mt-1 flex flex-wrap gap-x-5 gap-y-0.5 text-[10px] font-light text-white/50">
                <span>{ format!("Last outbound: {}", aggregate.last_outbound_at.as_deref().map(short_date).unwrap_or_else(|| "—".into())) }</span>
                <span>{ format!("Last inbound: {}", aggregate.last_inbound_at.as_deref().map(short_date).unwrap_or_else(|| "—".into())) }</span>
            </div>
            <div class="mt-0.5 flex flex-wrap gap-x-5 gap-y-0.5 text-[10px] font-light text-white/50">
                <span>{ format!("First observed: {}", aggregate.first_observed_at.as_deref().map(short_date).unwrap_or_else(|| "—".into())) }</span>
                <span>{ format!("Active sources: {} of {}", aggregate.active_source_count, aggregate.source_count) }</span>
            </div>
        </div>
    }
}

fn source_for<'a>(sources: &'a [PortalCommsSource], channel: &str) -> Option<&'a PortalCommsSource> {
    sources
        .iter()
        .filter(|source| source.channel == channel)
        .max_by_key(|source| source.total_count)
}

fn source_activity_row(
    channel: &str,
    label: &str,
    source: Option<&PortalCommsSource>,
    client_name: &str,
) -> Html {
    let connected = source.is_some_and(|value| value.total_count > 0);
    let preview = source
        .and_then(|value| value.last_context.clone())
        .or_else(|| {
            source
                .filter(|value| value.total_count > 0)
                .map(|value| format!("{} observed {}", value.total_count, channel_noun(channel)))
        })
        .unwrap_or_else(|| "No activity connected".into());
    let direction = source.map(|value| {
        if value.two_way {
            format!("{client_name} ↔ Lisa")
        } else {
            "Activity observed".into()
        }
    });
    let timestamp = source
        .and_then(|value| value.last_contact_at.as_deref())
        .map(short_timestamp)
        .unwrap_or_else(|| "—".into());

    html! {
        <li class="relative flex min-h-[3.35rem] items-center gap-3 py-2 pl-10 pr-4">
            <span aria-hidden=true
                class={classes!(
                    "absolute","left-[14px]","top-1/2","h-2.5","w-2.5","-translate-y-1/2","rounded-full","ring-2","ring-[var(--portal-navy-deep)]",
                    if connected { "bg-[var(--portal-gold)]" } else { "bg-white/20" }
                )}>
            </span>

            <div class="flex w-[7.25rem] shrink-0 items-center gap-2 text-[10px] font-medium uppercase tracking-[0.12em] text-white/70">
                { source_icon(channel) }
                <span class="truncate">{ label }</span>
            </div>

            <div class="min-w-0 flex-1">
                <p class={classes!(
                    "truncate","text-xs","font-light",
                    if connected { "text-white/90" } else { "text-white/35" }
                )}>
                    { preview }
                </p>
                if let Some(direction) = direction {
                    <p class="mt-0.5 truncate text-[10px] font-light text-white/45">
                        { format!(
                            "{}{}",
                            direction,
                            source.filter(|value| value.last_context.is_some() && value.total_count > 0)
                                .map(|value| format!(" · {} total", value.total_count))
                                .unwrap_or_default()
                        ) }
                    </p>
                }
            </div>

            <time class="w-[6.75rem] shrink-0 text-right text-[10px] font-light text-white/50">
                { timestamp }
            </time>
        </li>
    }
}

fn source_icon(channel: &str) -> Html {
    let name = match channel {
        "call" => "phone",
        "email" => "mail",
        "facetime" => "video",
        "calendar" => "calendar",
        _ => "message-circle",
    };
    icon_html(name, "h-3.5 w-3.5 shrink-0 text-white/55", "1.75").unwrap_or_default()
}

fn quick_action_dock(client: &PortalClientDetail) -> Html {
    let digits = client
        .phone
        .as_deref()
        .map(|phone| {
            phone
                .chars()
                .filter(|character| character.is_ascii_digit() || *character == '+')
                .collect::<String>()
        })
        .filter(|digits| !digits.is_empty());

    let tel = digits.as_ref().map(|digits| format!("tel:{digits}"));
    let sms = digits.as_ref().map(|digits| format!("sms:{digits}"));
    let mail = client
        .email
        .as_deref()
        .filter(|email| !email.trim().is_empty())
        .map(|email| format!("mailto:{email}"));

    html! {
        <div class="grid grid-cols-4 gap-2 border-t border-white/10 px-3 py-2">
            { dock_link(tel.as_deref(), "phone", "Call", "No phone on file") }
            { dock_link(mail.as_deref(), "mail", "Email", "No email on file") }
            { dock_link(sms.as_deref(), "message-circle", "Message", "No phone on file") }
            <a href={format!("/portal/clients/{}", client.id)} class={dock_action_class()}>
                { icon_html("globe", "h-3.5 w-3.5", "1.75").unwrap_or_default() }
                {"More"}
            </a>
        </div>
    }
}

fn dock_link(href: Option<&str>, icon_name: &str, label: &str, title: &str) -> Html {
    if let Some(href) = href {
        html! {
            <a href={href.to_string()} class={dock_action_class()}>
                { icon_html(icon_name, "h-3.5 w-3.5", "1.75").unwrap_or_default() }
                { label }
            </a>
        }
    } else {
        html! {
            <button type="button" disabled=true title={title.to_string()} class={dock_action_class()}>
                { icon_html(icon_name, "h-3.5 w-3.5", "1.75").unwrap_or_default() }
                { label }
            </button>
        }
    }
}

fn dock_action_class() -> Classes {
    classes!(
        "inline-flex","min-h-8","flex-1","items-center","justify-center","gap-1.5",
        "rounded-[var(--portal-tab-radius)]","border","border-white/15","px-2",
        "text-[10px]","font-medium","uppercase","tracking-[0.12em]","text-white/75",
        "transition","hover:border-[var(--portal-gold)]","hover:text-white","disabled:cursor-not-allowed","disabled:opacity-35"
    )
}

fn channel_noun(channel: &str) -> &'static str {
    match channel {
        "email" => "emails",
        "imessage" => "iMessages",
        "whatsapp" => "messages",
        _ => "communications",
    }
}

fn short_date(value: &str) -> String {
    let date = value.get(0..10).unwrap_or(value);
    let mut parts = date.split('-');
    let year = parts.next().unwrap_or_default();
    let month = parts.next().unwrap_or_default();
    let day = parts.next().unwrap_or_default().trim_start_matches('0');
    let month = match month {
        "01" => "Jan",
        "02" => "Feb",
        "03" => "Mar",
        "04" => "Apr",
        "05" => "May",
        "06" => "Jun",
        "07" => "Jul",
        "08" => "Aug",
        "09" => "Sep",
        "10" => "Oct",
        "11" => "Nov",
        "12" => "Dec",
        _ => month,
    };
    if year.is_empty() || day.is_empty() {
        value.to_string()
    } else {
        format!("{month} {day}, {year}")
    }
}

fn short_timestamp(value: &str) -> String {
    if value.len() >= 16 {
        format!("{} {}", short_date(value), &value[11..16])
    } else {
        short_date(value)
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
