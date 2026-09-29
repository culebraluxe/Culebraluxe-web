//! Catch-Up, drawn in the Projects workspace's own language: the gold label and glass tab rail of a project header,
//! the work-plan table, and the same selected-work editor underneath. The layout follows the TypeScript workbench it
//! replaces (today's schedule strip, the Today/Unscheduled worklists with status and area filters, Complete in the
//! row, open-in-project at the end), and adds People: the relationship queue that used to be its own CORE screen.

use yew::prelude::*;

use crate::app::cmd::Remote;
use crate::model::{PortalCatchUpItem, PortalCatchUpPage, PortalProjectWorkItem, PortalProjectsPage};

use super::super::catch_up::{buckets, effective_tab, overdue, project_name, status_matches};
use super::super::{Msg, Vm};
use super::{due_label, glyph, selected_item, selected_work_editor, status_dot};

pub(super) fn view(model: &Vm<'_>, projects: &PortalProjectsPage, on_msg: &Callback<Msg>) -> Html {
    let state = model.catch_up;
    let buckets = buckets(projects);
    let tab = effective_tab(state, &buckets).to_owned();
    let people_count = match &state.people {
        Remote::Loaded(page) => Some(page.items.len()),
        _ => None,
    };
    let tabs = [
        ("today", "Today", Some(buckets.today.iter().filter(|item| item.status != "done").count())),
        ("unscheduled", "Unscheduled", Some(buckets.unscheduled.len())),
        ("people", "People", people_count),
    ];
    let entries: &[&PortalProjectWorkItem] = if tab == "unscheduled" { &buckets.unscheduled } else { &buckets.today };
    let summary = if tab == "people" {
        match &state.people {
            Remote::Loaded(page) if page.items.is_empty() => "Caught up".to_owned(),
            Remote::Loaded(page) => format!("{} need you · {} high priority", page.total, page.high_priority_count),
            _ => String::new(),
        }
    } else {
        work_summary(entries, &tab)
    };

    html! {
        <section class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="shrink-0 border-b border-[var(--portal-panel-border)] px-3 py-2">
                <div class="flex min-w-0 items-center gap-3">
                    <div class="flex w-[190px] shrink-0 items-center gap-2.5">
                        <span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-[var(--portal-navy)] text-[var(--portal-gold)] shadow-sm">
                            { glyph("list-checks", "h-5 w-5") }
                        </span>
                        <span class="min-w-0">
                            <span class="block text-[14px] font-medium uppercase leading-tight tracking-[0.14em] text-[var(--portal-gold)]">{"Catch-Up"}</span>
                            <span class="block text-[11px] font-light uppercase tracking-[0.08em] text-[var(--portal-blue-gray)]">{ day_label(&projects.calendar_today) }</span>
                        </span>
                    </div>
                    <div class="min-w-0 flex-1 overflow-x-auto">
                    <nav aria-label="Catch-Up worklists" class="portal-glass-rail h-11 w-max">
                        { for tabs.iter().map(|(key, label, count)| {
                            let active = tab == *key;
                            let key = (*key).to_owned();
                            let onclick = on_msg.reform(move |_: MouseEvent| Msg::CatchUpTabSelected(key.clone()));
                            html! {
                                <button type="button" {onclick} aria-current={active.then_some("page")}
                                    class={classes!("portal-glass-tab", active.then_some("bg-[var(--portal-navy)] text-white shadow-sm"))}>
                                    { *label }
                                    if let Some(count) = count {
                                        <span class="ml-1.5 opacity-65">{ count }</span>
                                    }
                                </button>
                            }
                        }) }
                    </nav>
                    </div>
                    <span class="shrink-0 text-[11px] font-light text-[var(--portal-blue-gray)]">{ summary }</span>
                </div>
                if let Some(error) = model.error.as_ref() {
                    <p class="mt-1 text-xs text-red-700">{ (*error).clone() }</p>
                }
            </div>
            if tab == "people" {
                { people(state, on_msg) }
            } else {
                { schedule(projects) }
                { filters(state, entries, on_msg) }
                <div class="min-h-0 flex-1 overflow-hidden px-3 py-2">
                    { work_table(projects, entries, &tab, state, on_msg) }
                </div>
                <div class="shrink-0 px-3 pb-3">
                    { selected_work_editor(model, projects, selected_item(projects), on_msg) }
                </div>
            }
        </section>
    }
}

fn work_summary(entries: &[&PortalProjectWorkItem], tab: &str) -> String {
    if entries.is_empty() {
        return if tab == "today" { "Nothing due today".into() } else { "No unscheduled work".into() };
    }
    let done = entries.iter().filter(|item| item.status == "done").count();
    format!("{} remaining · {done} complete", entries.len() - done)
}

fn day_label(today: &str) -> String {
    chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d")
        .map(|date| date.format("%a · %b %-d").to_string())
        .unwrap_or_else(|_| "Today".into())
}

/// Today's calendar, as a strip of cards: the same events the Calendar view shows, for today only.
fn schedule(projects: &PortalProjectsPage) -> Html {
    let today = projects.calendar_today.as_str();
    let mut events: Vec<_> = projects
        .calendar
        .iter()
        .filter(|event| crate::calendar::date_key(&event.start_at).as_deref() == Some(today))
        .collect();
    events.sort_by(|a, b| b.all_day.cmp(&a.all_day).then_with(|| a.start_at.cmp(&b.start_at)).then_with(|| a.title.cmp(&b.title)));
    html! {
        <section class="shrink-0 border-b border-[var(--portal-panel-border)]/70 px-3 py-2" aria-label="Today's schedule">
            <div class="flex items-center gap-2">
                { glyph("calendar-days", "h-4 w-4 shrink-0 text-[var(--portal-gold-muted)]") }
                <span class="text-[10px] font-semibold uppercase tracking-[0.12em] text-black/40">{"Schedule"}</span>
                <span class="rounded-full bg-white/40 px-2 py-0.5 text-[10px] font-medium text-[var(--portal-navy-soft)]">{ events.len() }</span>
            </div>
            if events.is_empty() {
                <p class="mt-2 text-[11px] font-light text-black/40">{"No calendar events today."}</p>
            } else {
                <div class="mt-2 flex gap-2 overflow-x-auto pb-1">
                    { for events.iter().map(|event| {
                        let when = if event.all_day { "All day".to_owned() } else { crate::calendar::time_label(&event.start_at).unwrap_or_default() };
                        let who: Vec<&str> = [event.person_name.as_deref(), event.property_name.as_deref()].into_iter().flatten().collect();
                        html! {
                            <article class="min-w-[210px] max-w-[300px] flex-1 rounded-[var(--portal-tab-radius)] border border-white/45 bg-white/30 px-3 py-2">
                                <div class="flex items-center justify-between gap-2">
                                    <span class="text-[11px] font-medium text-[var(--portal-navy)]">{ when }</span>
                                    <span class="truncate text-[9px] font-medium uppercase tracking-[0.08em] text-black/35">{ source_label(&event.source, &event.kind) }</span>
                                </div>
                                <p class="mt-1 truncate text-[13px] font-medium text-[var(--portal-navy)]" title={event.title.clone()}>{ event.title.clone() }</p>
                                if !who.is_empty() {
                                    <p class="mt-0.5 truncate text-[10px] font-light text-black/45">{ who.join(" · ") }</p>
                                }
                            </article>
                        }
                    }) }
                </div>
            }
        </section>
    }
}

fn source_label(source: &str, kind: &str) -> &'static str {
    if kind == "showing" || source.contains("showing") {
        "Showing"
    } else if source.contains("apple") || source.contains("eventkit") {
        "Apple Calendar"
    } else if kind == "work" || source.contains("wbs") {
        "Project work"
    } else {
        "Calendar"
    }
}

fn filters(state: &super::super::CatchUpState, entries: &[&PortalProjectWorkItem], on_msg: &Callback<Msg>) -> Html {
    let counts = |key: &str| entries.iter().filter(|item| status_matches(item, key)).count();
    let mut areas: Vec<&str> = entries.iter().map(|item| item.category.as_str()).filter(|area| !area.is_empty()).collect();
    areas.sort_unstable();
    areas.dedup();
    let onchange = on_msg.reform(|event: Event| {
        Msg::CatchUpAreaSelected(crate::app::exec::select_value(&event))
    });
    html! {
        <div class="flex shrink-0 flex-wrap items-center gap-2 border-b border-[var(--portal-panel-border)]/70 px-3 py-2">
            <span class="mr-1 text-[10px] font-medium uppercase tracking-[0.12em] text-black/35">{"Status"}</span>
            { for [("all", "All"), ("open", "Open"), ("doing", "In progress"), ("done", "Done")].into_iter().map(|(key, label)| {
                let selected = state.status == key;
                let onclick = on_msg.reform(move |_: MouseEvent| Msg::CatchUpStatusSelected(key.to_owned()));
                html! {
                    <button type="button" {onclick}
                        class={classes!("rounded-full", "px-2.5", "py-1", "text-[10px]", "font-medium", "transition",
                            if selected { "bg-[var(--portal-navy)] text-white" } else { "bg-white/35 text-[var(--portal-navy-soft)] hover:bg-white/55" })}>
                        { label }<span class="ml-1 opacity-65">{ counts(key) }</span>
                    </button>
                }
            }) }
            <span class="ml-auto text-[10px] font-medium uppercase tracking-[0.12em] text-black/35">{"Area"}</span>
            <select {onchange} aria-label="Filter Catch-Up by area"
                class="h-7 rounded-full border border-[var(--portal-panel-border)] bg-white/50 px-2.5 text-[10px] font-medium text-[var(--portal-navy-soft)] outline-none">
                <option value="all" selected={state.area == "all"}>{"All areas"}</option>
                { for areas.iter().map(|area| html! {
                    <option value={area.to_string()} selected={state.area == *area}>{ title_case(area) }</option>
                }) }
            </select>
        </div>
    }
}

fn work_table(
    projects: &PortalProjectsPage,
    entries: &[&PortalProjectWorkItem],
    tab: &str,
    state: &super::super::CatchUpState,
    on_msg: &Callback<Msg>,
) -> Html {
    const COLUMNS: &str = "grid-cols-[22px_minmax(0,1.6fr)_minmax(0,1fr)_minmax(0,0.7fr)_88px_92px_28px]";
    let visible: Vec<&&PortalProjectWorkItem> = entries
        .iter()
        .filter(|item| status_matches(item, &state.status))
        .filter(|item| state.area == "all" || item.category == state.area)
        .collect();
    html! {
        <div class="flex h-full min-h-0 flex-col overflow-y-auto rounded-[var(--portal-tab-radius)] border border-white/40 bg-white/20 px-1.5 py-1">
            <div class={classes!("grid", COLUMNS, "gap-2", "border-b", "border-[var(--portal-panel-border)]/70", "px-2", "py-2", "text-[10px]", "font-semibold", "uppercase", "tracking-[0.12em]", "text-black/40")}>
                <span></span><span>{"Task"}</span><span>{"Project"}</span><span>{"Area"}</span><span>{"Owner"}</span><span class="text-right">{"Action"}</span><span></span>
            </div>
            if visible.is_empty() {
                <p class="px-4 py-10 text-center text-sm font-light text-black/40">
                    { if entries.is_empty() {
                        if tab == "today" { "No project work is due today." } else { "No unscheduled project work." }
                    } else { "No work matches those filters." } }
                </p>
            } else {
                <ul class="space-y-0.5">
                    { for visible.into_iter().map(|item| work_row(projects, item, COLUMNS, on_msg)) }
                </ul>
            }
        </div>
    }
}

fn work_row(projects: &PortalProjectsPage, item: &PortalProjectWorkItem, columns: &'static str, on_msg: &Callback<Msg>) -> Html {
    let project_id = item.project_id.clone().unwrap_or_default();
    let ids = (project_id.clone(), item.id.clone());
    let select = {
        let (project_id, node_id) = ids.clone();
        on_msg.reform(move |_: MouseEvent| Msg::ProjectCatchUpItemSelected { project_id: project_id.clone(), node_id: node_id.clone() })
    };
    let complete = {
        let (project_id, node_id) = ids.clone();
        on_msg.reform(move |_: MouseEvent| Msg::ProjectCatchUpItemCompleteRequested { project_id: project_id.clone(), node_id: node_id.clone() })
    };
    let open = {
        let (project_id, node_id) = ids;
        on_msg.reform(move |_: MouseEvent| Msg::NavWorkSelected { project_id: project_id.clone(), node_id: node_id.clone() })
    };
    let selected = projects.selected_node_id.as_deref() == Some(item.id.as_str());
    let done = item.status == "done";
    let late = overdue(item, &projects.calendar_today);
    let status_line = if late {
        format!("Overdue · due {}", due_label(item.due_at.as_deref()))
    } else {
        status_label(&item.status).to_owned()
    };
    html! {
        <li class={classes!("grid", columns, "items-center", "gap-2", "rounded-[var(--portal-tab-radius)]", "px-2", "transition",
            if selected { "bg-white/65 ring-1 ring-inset ring-[var(--portal-panel-border)]" } else { "hover:bg-white/40" },
            done.then_some("opacity-60"))}>
            <button type="button" onclick={select.clone()} aria-label="Select" class={classes!("h-3.5", "w-3.5", "justify-self-center", "rounded-full", "border", "border-black/10", status_dot(&item.status))}></button>
            <button type="button" onclick={select.clone()} class="min-w-0 py-2 text-left">
                <span class={classes!("block", "truncate", "text-[13.5px]", "text-[var(--portal-navy)]", done.then_some("line-through"))}>{ item.title.clone() }</span>
                <span class={classes!("block", "truncate", "text-[10px]", "font-light", "uppercase", "tracking-[0.08em]",
                    if late { "text-[var(--portal-archive)]" } else { "text-black/40" })}>{ status_line }</span>
            </button>
            <button type="button" onclick={select.clone()} class="min-w-0 truncate py-2 text-left text-[13px] font-light text-[var(--portal-navy)]">{ project_name(projects, item).to_owned() }</button>
            <button type="button" onclick={select.clone()} class="min-w-0 truncate py-2 text-left text-[12px] font-medium text-[var(--portal-navy-soft)]">{ title_case(&item.category) }</button>
            <button type="button" onclick={select} class="min-w-0 truncate py-2 text-left text-[12px] font-light text-[var(--portal-blue-gray)]">{ item.owner.clone().unwrap_or_else(|| "—".into()) }</button>
            <span class="flex justify-end">
                if done {
                    <span class="text-[10px] font-medium uppercase tracking-[0.1em] text-[var(--portal-success)]">{"Done"}</span>
                } else {
                    <button type="button" onclick={complete} disabled={projects.saving}
                        class="rounded-full border border-[var(--portal-success)]/35 bg-white/30 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-success)] transition hover:bg-white/55 disabled:opacity-40">
                        { if projects.saving && selected { "Saving…" } else { "Complete" } }
                    </button>
                }
            </span>
            <button type="button" onclick={open} title="Open this work item in its project" aria-label={format!("Open {} in its project", item.title)}
                class="flex h-8 items-center justify-center rounded text-black/25 transition hover:bg-white/30 hover:text-[var(--portal-gold-muted)]">
                { glyph("chevron-right", "h-4 w-4") }
            </button>
        </li>
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "doing" => "In progress",
        "done" => "Complete",
        "dismissed" => "Dismissed",
        _ => "Not started",
    }
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "—".into(),
    }
}

/// People: the relationship queue on the left, the chosen person on the right, in the panel's glass.
fn people(state: &super::super::CatchUpState, on_msg: &Callback<Msg>) -> Html {
    let body = crate::app::template::remote(&state.people, "who needs you", |page| people_workspace(page, state, on_msg));
    html! {
        <div class="flex min-h-0 flex-1 flex-col px-3 py-2">
            if let Some(notice) = &state.notice {
                <p class="pb-2 text-[11px] font-light text-[var(--portal-navy-soft)]" role="status">{ notice.clone() }</p>
            }
            { body }
        </div>
    }
}

fn people_workspace(page: &PortalCatchUpPage, state: &super::super::CatchUpState, on_msg: &Callback<Msg>) -> Html {
    if page.items.is_empty() {
        return html! {
            <p class="px-4 py-10 text-center text-sm font-light text-black/40">
                {"Caught up. New relationship signals appear here on their own."}
            </p>
        };
    }
    let chosen = state
        .person
        .as_deref()
        .and_then(|id| page.items.iter().find(|item| item.person_id == id))
        .or_else(|| page.items.first());
    html! {
        <div class="grid min-h-0 flex-1 overflow-hidden rounded-[var(--portal-tab-radius)] border border-white/40 bg-white/20 lg:grid-cols-[minmax(280px,0.9fr)_minmax(0,1.3fr)]">
            <div class="min-h-0 overflow-y-auto border-b border-[var(--portal-panel-border)]/70 lg:border-b-0 lg:border-r">
                { for page.items.iter().map(|item| person_row(item, chosen.map(|row| row.person_id.as_str()), on_msg)) }
            </div>
            <div class="min-h-0 overflow-y-auto p-4">
                { chosen.map(|item| person_detail(item, state.busy, on_msg)).unwrap_or_default() }
            </div>
        </div>
    }
}

fn person_row(item: &PortalCatchUpItem, chosen: Option<&str>, on_msg: &Callback<Msg>) -> Html {
    let id = item.person_id.clone();
    let onclick = on_msg.reform(move |_: MouseEvent| Msg::PeopleSelected(id.clone()));
    let active = chosen == Some(item.person_id.as_str());
    html! {
        <button type="button" {onclick}
            class={classes!("block", "w-full", "border-b", "border-[var(--portal-panel-border)]/60", "px-3", "py-2.5", "text-left", "transition",
                if active { "bg-white/65 ring-1 ring-inset ring-[var(--portal-panel-border)]" } else { "hover:bg-white/40" })}>
            <div class="flex items-center justify-between gap-3">
                <span class="truncate text-[14px] font-medium text-[var(--portal-navy)]">{ item.display_name.clone() }</span>
                <span class={classes!("shrink-0", "rounded-full", "px-2", "py-0.5", "text-[9px]", "font-medium", "uppercase", "tracking-[0.12em]", priority_tone(item.priority))}>
                    { priority_label(item.priority) }
                </span>
            </div>
            <p class="mt-0.5 line-clamp-2 text-[12px] font-light leading-5 text-black/55">{ item.reason.clone() }</p>
            <p class="mt-1 text-[10px] font-light uppercase tracking-[0.08em] text-black/35">{ item.signal_at_label.clone() }</p>
        </button>
    }
}

fn person_detail(item: &PortalCatchUpItem, busy: bool, on_msg: &Callback<Msg>) -> Html {
    let handle = {
        let (person_id, reason_code) = (item.person_id.clone(), item.reason_code.clone());
        on_msg.reform(move |_: MouseEvent| Msg::PeopleHandleRequested { person_id: person_id.clone(), reason_code: reason_code.clone() })
    };
    html! {
        <div class="space-y-4">
            <div>
                <p class="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">{ format!("{} · {}", item.role, item.status) }</p>
                <h2 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">{ item.display_name.clone() }</h2>
                <p class="mt-2 text-[13px] font-light leading-6 text-black/60">{ item.reason.clone() }</p>
            </div>
            <div class="grid gap-2 sm:grid-cols-2">
                { fact("Last contact", item.last_contact_label.as_deref().unwrap_or("No recorded contact")) }
                { fact("Channel", item.last_contact_channel.as_deref().unwrap_or("—")) }
                { fact("Direction", item.last_contact_direction.as_deref().unwrap_or("—")) }
                { fact("Next follow-up", item.due_at_label.as_deref().unwrap_or("No dated task")) }
            </div>
            if let Some(summary) = &item.last_contact_summary {
                { fact("Last context", summary) }
            }
            if let Some(property) = &item.active_property_name {
                <div class="rounded-[var(--portal-tab-radius)] bg-white/35 px-3 py-2">
                    <p class="text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">{"Active work"}</p>
                    if let Some(deal_id) = &item.active_deal_id {
                        <a href={format!("/portal/deals/{deal_id}")} class="mt-0.5 inline-flex min-h-9 items-center text-[14px] font-medium text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]">{ property.clone() }</a>
                    } else {
                        <p class="mt-0.5 text-[14px] font-medium text-[var(--portal-navy)]">{ property.clone() }</p>
                    }
                </div>
            }
            <div class="flex flex-wrap gap-2">
                <a href={format!("/portal/clients/{}", item.person_id)} class={action_class(false)}>{ glyph("user-round", "h-3.5 w-3.5") }{"Open client"}</a>
                if let Some(phone) = item.primary_phone.as_deref() {
                    <a href={format!("tel:{phone}")} class={action_class(false)}>{ glyph("phone", "h-3.5 w-3.5") }{"Call"}</a>
                    <a href={format!("sms:{phone}")} class={action_class(false)}>{ glyph("message-circle", "h-3.5 w-3.5") }{"iMessage"}</a>
                }
                if let Some(email) = item.primary_email.as_deref() {
                    <a href={format!("mailto:{email}")} class={action_class(false)}>{ glyph("mail", "h-3.5 w-3.5") }{"Email"}</a>
                }
            </div>
            <div class="flex flex-wrap items-center gap-2 border-t border-[var(--portal-panel-border)]/70 pt-3">
                <span class="mr-1 text-[10px] font-medium uppercase tracking-[0.12em] text-black/35">{"Disposition"}</span>
                <button type="button" onclick={handle} disabled={busy} class={action_class(true)}>{"Handled"}</button>
                { for [1, 3, 7].into_iter().map(|days| {
                    let (person_id, reason_code) = (item.person_id.clone(), item.reason_code.clone());
                    let onclick = on_msg.reform(move |_: MouseEvent| Msg::PeopleSnoozeRequested { person_id: person_id.clone(), reason_code: reason_code.clone(), days });
                    html! { <button type="button" {onclick} disabled={busy} class={action_class(false)}>{ format!("Snooze {days}d") }</button> }
                }) }
            </div>
        </div>
    }
}

fn fact(label: &str, value: &str) -> Html {
    html! {
        <div class="rounded-[var(--portal-tab-radius)] bg-white/35 px-3 py-2">
            <p class="text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">{ label.to_owned() }</p>
            <p class="mt-0.5 text-[13px] font-light text-black/65">{ value.to_owned() }</p>
        </div>
    }
}

fn action_class(primary: bool) -> Classes {
    classes!(
        "inline-flex", "min-h-9", "items-center", "gap-1.5", "rounded-full", "px-3", "text-[10px]", "font-medium", "uppercase",
        "tracking-[0.1em]", "transition", "disabled:opacity-40",
        if primary { "bg-[var(--portal-navy)] text-white hover:opacity-90" } else { "border border-[var(--portal-panel-border)] bg-white/35 text-[var(--portal-navy)] hover:bg-white/60" }
    )
}

fn priority_label(priority: i32) -> &'static str {
    if priority >= 85 {
        "Now"
    } else if priority >= 70 {
        "Soon"
    } else {
        "Quiet"
    }
}

fn priority_tone(priority: i32) -> &'static str {
    if priority >= 85 {
        "bg-red-100 text-red-800"
    } else if priority >= 70 {
        "bg-amber-100 text-amber-800"
    } else {
        "bg-white/50 text-[var(--portal-navy-soft)]"
    }
}
