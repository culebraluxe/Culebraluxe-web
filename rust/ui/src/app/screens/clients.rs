//! `/portal/clients` and `/portal/clients/:personId` — the people the firm works with. THE REFERENCE LIST SCREEN.
//!
//! The list is the `ListState` building block (search with a pause, paging) in the standard rail; the selected person
//! is in the URL (`?selected=`), so a selection can be reloaded, shared and gone back to. The record route is the same
//! screen with the list set aside. The detail panels are pure functions of the payload.

use yew::prelude::*;

use crate::app::api::ClientsRead;
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::list::{self, ListChange, ListMsg, ListState};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::icons::icon_html;
use crate::model::{
    PortalClientDetail, PortalClientSummary, PortalClientsPage, PortalCommsAggregate,
    PortalCommsSource, PortalPage,
};

/// The directory with the selected person beside it.
pub struct Clients;
/// One person, reached from the directory (or a link).
pub struct ClientRecord;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub list: ListState,
    pub data: Remote<PortalClientsPage>,
    /// A read is in flight while the previous answer is still on screen (a search, a page, another selection).
    pub refreshing: bool,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    List(ListMsg),
    Loaded(Result<PortalPage, ApiError>),
    /// Just the selected person, merged into the list already on screen.
    PersonLoaded(Result<PortalPage, ApiError>),
}

fn read(model: &Model, ctx: &ScreenCtx, record: Option<String>) -> Cmd<Msg> {
    Cmd::request(
        ClientsRead {
            record,
            selected: ctx.query("selected").map(str::to_owned),
            search: model.list.search.clone(),
            page: model.list.page,
        },
        Msg::Loaded,
    )
}

/// A new selection reads ONLY that person and keeps the list on screen: the list did not change, so reading it again
/// (as the relay does for a full read) would double the wait for nothing.
fn read_person(id: &str) -> Cmd<Msg> {
    Cmd::request(
        ClientsRead {
            record: Some(id.to_owned()),
            selected: None,
            search: String::new(),
            page: 1,
        },
        Msg::PersonLoaded,
    )
}

fn merge_person(model: &mut Model, answer: Result<PortalPage, ApiError>) {
    model.refreshing = false;
    let person = answer.and_then(|page| {
        page.clients
            .ok_or_else(|| ApiError::decode("The answer had no client in it."))
    });
    match (person, &mut model.data) {
        (Ok(person), Remote::Loaded(list)) => {
            list.selected_id = person.selected_id;
            list.selected = person.selected;
            list.comms = person.comms;
            list.properties = person.properties;
        }
        (Ok(person), _) => model.data = Remote::Loaded(person),
        (Err(error), _) => model.data = Remote::Failed(error),
    }
}

fn apply(model: &mut Model, answer: Result<PortalPage, ApiError>) {
    model.refreshing = false;
    model.data = Remote::from_result(answer.and_then(|page| {
        page.clients
            .ok_or_else(|| ApiError::decode("The answer had no clients in it."))
    }));
}

impl Screen for Clients {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let model = Model {
            data: Remote::Loading,
            ..Model::default()
        };
        let cmd = read(&model, ctx, None);
        (model, cmd)
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::List(msg) => {
                let pages = model
                    .data
                    .loaded()
                    .map(|data| list::pages(data.total, data.page_size))
                    .unwrap_or(1);
                let (change, cmd) = model.list.update(msg, pages);
                let cmd = cmd.map(Msg::List);
                if change == ListChange::Reload {
                    model.refreshing = true;
                    return Cmd::batch([cmd, read(model, ctx, None)]);
                }
                cmd
            }
            Msg::Loaded(answer) => {
                apply(model, answer);
                Cmd::none()
            }
            Msg::PersonLoaded(answer) => {
                merge_person(model, answer);
                Cmd::none()
            }
        }
    }

    /// Another person selected (`?selected=`): read just them; the list stays as it is.
    fn url_changed(model: &mut Model, ctx: &ScreenCtx) -> Cmd<Msg> {
        let current = model
            .data
            .loaded()
            .and_then(|data| data.selected_id.as_deref());
        match ctx.query("selected") {
            Some(id) if current != Some(id) => {
                model.refreshing = true;
                read_person(id)
            }
            _ => Cmd::none(),
        }
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let data = model.data.loaded();
        let rows = data.map(|page| page.rows.as_slice()).unwrap_or(&[]);
        let selected = data.and_then(|page| page.selected_id.as_deref());
        let pages = data
            .map(|page| list::pages(page.total, page.page_size))
            .unwrap_or(1);
        let rail = list::rail(
            "People",
            data.map(|page| page.total).unwrap_or(0),
            &model.list,
            pages,
            model.data.is_loading() || model.refreshing,
            "No matching clients.",
            rows.iter()
                .map(|row| row_view(row, selected, ctx))
                .collect(),
            !rows.is_empty(),
            &link.callback(Msg::List),
        );
        html! {
            <div class="grid min-h-0 gap-4 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[200px_minmax(0,1fr)]">
                { rail }
                <div class="min-h-0 overflow-hidden">{ workspace(model) }</div>
            </div>
        }
    }
}

impl Screen for ClientRecord {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let model = Model {
            data: Remote::Loading,
            ..Model::default()
        };
        let cmd = read(&model, ctx, ctx.id.clone());
        (model, cmd)
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        if let Msg::Loaded(answer) = msg {
            apply(model, answer);
        }
        Cmd::none()
    }

    fn view(model: &Model, ctx: &ScreenCtx, _link: &Link<Msg>) -> Html {
        html! {
            <div class="flex flex-col gap-3">
                <div>{ template::back_link(ctx) }</div>
                { workspace(model) }
            </div>
        }
    }
}

/// A person in the list. Choosing one is a link (`?selected=`), not a message: the URL holds the selection.
fn row_view(row: &PortalClientSummary, selected_id: Option<&str>, ctx: &ScreenCtx) -> Html {
    let selected = selected_id == Some(row.id.as_str());
    let secondary = row
        .primary_phone
        .clone()
        .or_else(|| row.primary_email.clone())
        .unwrap_or_else(|| row.role.clone());
    let classes = classes!(
        "flex",
        "w-full",
        "items-center",
        "gap-2",
        "border-b",
        "border-[var(--portal-panel-border)]",
        "px-2.5",
        "py-2",
        "text-left",
        "transition",
        if selected {
            "border-l-2 border-l-[var(--portal-gold)] bg-white/40"
        } else {
            "border-l-2 border-l-transparent hover:bg-white/25"
        }
    );
    html! {
        <crate::app::chrome::AppLink href={template::with_query(ctx, "selected", Some(&row.id))} classes={classes} current={selected}>
            <span class={format!("h-1.5 w-1.5 shrink-0 rounded-full {}", status_dot(&row.status))}></span>
            <div class="min-w-0 flex-1">
                <div class="truncate text-[13px] font-medium text-[var(--portal-navy)]">
                    { if row.name_resolved { row.display_name.clone() } else { "Unknown contact".into() } }
                </div>
                <div class="truncate text-[11px] font-light text-black/45">{ secondary }</div>
                if row.observed_count > 0 {
                    <div class="truncate text-[10px] font-light text-black/35">
                        { format!("{} observed{}", row.observed_count, if row.two_way { " \u{b7} two-way" } else { "" }) }
                    </div>
                }
            </div>
        </crate::app::chrome::AppLink>
    }
}

/// The selected person, or the standard loading / failure / nothing-selected states.
fn workspace(model: &Model) -> Html {
    let Some(data) = model.data.loaded() else {
        return template::remote(&model.data, "clients", |_| Html::default());
    };
    let Some(client) = data.selected.as_ref() else {
        return template::empty_panel(if model.refreshing {
            "Loading the selected client\u{2026}"
        } else {
            "Select a client."
        });
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

fn command_status_band(model: &Model, client: &PortalClientDetail) -> Html {
    let status_text = if model.refreshing {
        "Loading client\u{2026}".to_string()
    } else {
        format!("Ready \u{b7} {}", client.display_name)
    };
    let tone = if model.refreshing {
        "bg-black/25"
    } else {
        "bg-[var(--portal-success)]"
    };
    html! {
        <div class="grid grid-cols-1 gap-3 lg:grid-cols-2 lg:gap-4">
            <section class="portal-glass-panel portal-glass-panel-lifted flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="shrink-0 border-b border-[var(--portal-panel-border)] px-4 py-2.5">
                    <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{"Grok"}</p>
                </div>
                <div class="flex min-h-0 flex-1 items-center gap-2 px-4 py-2.5">
                    <input type="text" disabled=true placeholder={format!("Ask Grok about {}\u{2026}", client.display_name)}
                        class="h-10 min-w-0 flex-1 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55 px-3 font-serif text-[15px] font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35 disabled:cursor-not-allowed disabled:opacity-70" />
                    <button type="button" disabled=true
                        class="inline-flex h-10 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:cursor-not-allowed disabled:opacity-45">
                        {"Go"}
                    </button>
                </div>
            </section>
            <section aria-label="Status" class="portal-glass-panel portal-glass-panel-lifted flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="flex shrink-0 items-center justify-between gap-2 border-b border-[var(--portal-panel-border)] px-4 py-2.5">
                    <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{"Status"}</p>
                    <span aria-hidden=true class={classes!("h-2","w-2","shrink-0","rounded-full",tone)}></span>
                </div>
                <div aria-live="polite" class="min-h-0 flex-1 overflow-hidden px-4 py-2.5 font-serif text-[15px] font-light leading-6 text-[var(--portal-navy)] line-clamp-3">
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
    let value = value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("—");
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
    let observed = comms
        .map(|value| value.aggregate.observed_count)
        .unwrap_or(0);
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

fn source_for<'a>(
    sources: &'a [PortalCommsSource],
    channel: &str,
) -> Option<&'a PortalCommsSource> {
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
        "inline-flex",
        "min-h-8",
        "flex-1",
        "items-center",
        "justify-center",
        "gap-1.5",
        "rounded-[var(--portal-tab-radius)]",
        "border",
        "border-white/15",
        "px-2",
        "text-[10px]",
        "font-medium",
        "uppercase",
        "tracking-[0.12em]",
        "text-white/75",
        "transition",
        "hover:border-[var(--portal-gold)]",
        "hover:text-white",
        "disabled:cursor-not-allowed",
        "disabled:opacity-35"
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A REAL answer (captured by `pnpm ui:fixtures`, personal data scrubbed) decodes into what the screen reads.
    #[test]
    fn the_real_clients_answer_decodes_and_shows_its_selected_person() {
        let ctx = ScreenCtx {
            path: "/portal/clients".into(),
            ..ScreenCtx::default()
        };
        let (mut model, cmd) = Clients::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(
            request.path,
            "/api/portal/rust-ui/clients?screen=clients&page=0&search="
        );
        let answer: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/clients-list.json")).unwrap();
        Clients::update(&mut model, request.respond(Ok(answer)), &ctx);
        let data = model.data.loaded().expect("the real payload decodes");
        assert!(!data.rows.is_empty());
        assert_eq!(
            data.selected_id.as_deref(),
            data.selected.as_ref().map(|client| client.id.as_str())
        );
    }

    #[test]
    fn searching_reads_page_one_and_a_selection_reads_that_person() {
        let ctx = ScreenCtx {
            path: "/portal/clients".into(),
            ..ScreenCtx::default()
        };
        let (mut model, _) = Clients::init(&ctx);
        model.list.page = 3;
        Clients::update(&mut model, Msg::List(ListMsg::Typed("ale x".into())), &ctx);
        let cmd = Clients::update(&mut model, Msg::List(ListMsg::Paused(1)), &ctx);
        assert_eq!(
            cmd.into_requests().remove(0).path,
            "/api/portal/rust-ui/clients?screen=clients&page=0&search=ale%20x"
        );
        let selected = ScreenCtx {
            query: crate::app::screen::parse_query("?selected=p-9"),
            ..ctx
        };
        let path = Clients::url_changed(&mut model, &selected)
            .into_requests()
            .remove(0)
            .path;
        assert_eq!(
            path, "/api/portal/rust-ui/clients?screen=client-record&scope=p-9",
            "a selection reads only the person"
        );
        assert!(model.refreshing);
    }

    #[test]
    fn the_record_reads_one_person_by_its_route_id() {
        let ctx = ScreenCtx {
            path: "/portal/clients/p-9".into(),
            id: Some("p-9".into()),
            ..ScreenCtx::default()
        };
        let path = ClientRecord::init(&ctx).1.into_requests().remove(0).path;
        assert_eq!(
            path,
            "/api/portal/rust-ui/clients?screen=client-record&scope=p-9"
        );
    }
}
