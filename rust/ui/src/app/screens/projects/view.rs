//! CORE / Project Management — Rust/Yew shell and MVI workspace.
//!
//! The deleted JavaScript widgets return as native Rust/Yew views. Application state remains in
//! Model -> update(); this file only renders that state and emits named intents.

use std::collections::BTreeSet;

use yew::prelude::*;

use crate::model::{PortalProject, PortalProjectWorkItem, PortalProjectsPage};

use super::{Msg, Vm};

mod calendar;
mod catch_up;
mod documents;
mod navigator;
mod timeline;
mod work_editor;

use documents::documents_view;
use navigator::navigator;
use work_editor::selected_work_editor;

pub(super) fn workspace(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    on_msg: &Callback<Msg>,
) -> Html {
    if projects.projects.is_empty() {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] px-8 py-14 text-center">
                <h1 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"No Projects yet"}</h1>
                <p class="mt-2 text-sm font-light text-black/50">
                    {"Project creation still uses the legacy playbook transaction and is intentionally not hidden behind this Rust screen."}
                </p>
            </section>
        };
    }

    html! {
        <div class="grid min-h-0 gap-3 lg:h-[calc(100dvh-8.5rem)] lg:grid-cols-[390px_minmax(0,1fr)]">
            { navigator(model, projects, on_msg) }
            { center_panel(model, projects, on_msg) }
        </div>
    }
}

fn glyph(name: &str, class: &str) -> Html {
    crate::icons::icon_html(name, class, "1.6").unwrap_or_default()
}

fn catchup_center(model: &Vm<'_>, projects: &PortalProjectsPage, on_msg: &Callback<Msg>) -> Html {
    catch_up::view(model, projects, on_msg)
}

fn center_panel(model: &Vm<'_>, projects: &PortalProjectsPage, on_msg: &Callback<Msg>) -> Html {
    if projects.catch_up {
        return catchup_center(model, projects, on_msg);
    }
    let Some(project) = selected_project(projects) else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-8">
                <p class="text-sm font-light text-black/45">{"Select a project."}</p>
            </section>
        };
    };
    let selected = selected_item(projects);

    html! {
        <section class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
            { project_header(model, projects, project, on_msg) }
            <div class="min-h-0 flex-1 overflow-hidden px-3 py-2">
                { active_view(model, projects, project, on_msg) }
            </div>
            <div class="shrink-0 px-3 pb-3">
                {
                    if projects.active_view == "calendar"
                        && projects.calendar_selected_event_id.is_some()
                    {
                        selected_calendar_event_panel(model, projects)
                    } else {
                        selected_work_editor(model, projects, selected, on_msg)
                    }
                }
            </div>
        </section>
    }
}

fn project_header(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    let status = project.status.clone();
    let onchange = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = crate::app::exec::select_value(&event);
            on_msg.emit(Msg::ProjectStatusRequested(value));
        })
    };
    html! {
        <>
            <div class="shrink-0 border-b border-[var(--portal-panel-border)] px-3 py-2">
                <div class="flex min-w-0 items-center gap-3">
                    <p class="w-[190px] shrink-0 whitespace-normal text-left text-[14px] font-medium uppercase leading-tight tracking-[0.14em] text-[var(--portal-gold)]">
                        { project.name.clone() }
                    </p>
                    <div class="min-w-0 flex-1 overflow-x-auto">
                        { project_tabs(projects, on_msg) }
                    </div>
                    <div class="flex shrink-0 items-center gap-2">
                        <span class="text-[11px] font-light text-[var(--portal-blue-gray)]">
                            { format!("{}%", project_progress(projects, &project.id)) }
                        </span>
                        <select
                            value={status.clone()}
                            onchange={onchange}
                            disabled={projects.saving}
                            aria-label="Project status"
                            class="rounded-full bg-white/50 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] outline-none disabled:opacity-40"
                        >
                            <option value="open" selected={status == "open"}>{"Open"}</option>
                            <option value="doing" selected={status == "doing"}>{"In progress"}</option>
                            <option value="done" selected={status == "done"}>{"Complete"}</option>
                            <option value="archived" selected={status == "archived"}>{"Archived"}</option>
                        </select>
                        <button
                            type="button"
                            disabled=true
                            title="The listing-playbook instantiation command still lives only in retired TypeScript; Rust create primitives exist, but the orchestration has not been ported."
                            class="rounded-full bg-[var(--portal-navy)] px-3.5 py-2 text-[12px] font-medium text-white opacity-35 shadow-sm"
                        >
                            {"New Project"}
                        </button>
                    </div>
                </div>
                { seen_from(projects, project, on_msg) }
                if let Some(error) = model.error.as_ref() {
                    <p class="mt-1 text-xs text-red-700">{ (*error).clone() }</p>
                }
            </div>
        </>
    }
}

/// Every place this project can be seen from, as links: its property, its people, its contract, or a lens's collection.
/// A link flips the rail to that lens with that record open — the lens flip, from the project itself.
fn seen_from(projects: &PortalProjectsPage, project: &PortalProject, on_msg: &Callback<Msg>) -> Html {
    let lenses = super::nav::lenses(projects, project);
    if lenses.is_empty() {
        return Html::default();
    }
    html! {
        <div class="mt-1.5 flex min-w-0 flex-wrap items-center gap-1.5">
            <span class="mr-0.5 text-[10px] font-medium uppercase tracking-[0.12em] text-black/35">{"Seen from"}</span>
            { for lenses.into_iter().map(|lens| {
                let active = projects.active_domain == lens.domain;
                let (domain, pole_id) = (lens.domain.to_owned(), lens.pole_id.clone());
                let onclick = on_msg.reform(move |_: MouseEvent| Msg::LensJump { domain: domain.clone(), pole_id: pole_id.clone() });
                html! {
                    <button type="button" {onclick} title={format!("See this project from {}", super::nav::domain_label(lens.domain))}
                        aria-current={active.then_some("true")}
                        class={classes!("inline-flex", "h-7", "max-w-[240px]", "items-center", "gap-1.5", "rounded-full", "px-2.5", "text-[11px]", "font-medium", "transition",
                            if active { "bg-[var(--portal-navy)] text-white" } else { "bg-white/40 text-[var(--portal-navy-soft)] ring-1 ring-inset ring-[var(--portal-panel-border)] hover:bg-white/65" })}>
                        { glyph(super::nav::domain_icon(lens.domain), if active { "h-3.5 w-3.5 shrink-0 text-[var(--portal-gold)]" } else { "h-3.5 w-3.5 shrink-0 text-[var(--portal-gold-muted)]" }) }
                        <span class="truncate">{ lens.label }</span>
                    </button>
                }
            }) }
        </div>
    }
}

fn project_tabs(projects: &PortalProjectsPage, on_msg: &Callback<Msg>) -> Html {
    const TABS: &[(&str, &str)] = &[
        ("work-plan", "Work Plan"),
        ("timeline", "Timeline"),
        ("calendar", "Calendar"),
        ("financials", "Financials"),
        ("documents", "Documents"),
        ("activity", "Activity"),
    ];
    html! {
        <nav aria-label="Project workspace views" class="portal-glass-rail h-11 w-max">
            { for TABS.iter().map(|(key, label)| {
                let active = projects.active_view == *key;
                let key_string = (*key).to_string();
                let on_msg = on_msg.clone();
                html! {
                    <button
                        type="button"
                        onclick={Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectViewSelected(key_string.clone())))}
                        aria-current={if active { Some("page") } else { None }}
                        class={classes!("portal-glass-tab", active.then_some("bg-[var(--portal-navy)] text-white shadow-sm"))}
                    >
                        { *label }
                    </button>
                }
            }) }
        </nav>
    }
}

fn active_view(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    match projects.active_view.as_str() {
        "timeline" => timeline::view(model, projects, project, on_msg),
        "calendar" => calendar::view(model, projects, project, on_msg),
        "financials" => placeholder_view(
            "Financials",
            "No project-scoped accounting read model is attached to the Rust workspace yet.",
        ),
        "documents" => documents_view(projects, project, on_msg),
        "activity" => activity_view(projects, project),
        _ => work_plan_view(projects, project, on_msg),
    }
}

fn work_plan_view(
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    let roots = root_items(projects, &project.id);
    html! {
        <div class="flex h-full min-h-0 flex-1 flex-col overflow-y-auto rounded-[var(--portal-tab-radius)] border border-white/40 bg-white/20 px-1.5 py-1">
            <div class="grid grid-cols-[22px_minmax(0,1fr)_72px_88px] gap-2 border-b border-[var(--portal-panel-border)]/70 px-2 py-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-black/40">
                <span></span><span>{"Work item"}</span><span class="text-right">{"Due"}</span><span class="text-right">{"Owner"}</span>
            </div>
            if roots.is_empty() {
                <p class="px-4 py-10 text-center text-sm font-light text-black/40">{"No WBS items are attached to this project."}</p>
            } else {
                <ul class="space-y-0.5">
                    { for roots.into_iter().map(|item| work_plan_node(projects, item, on_msg)) }
                </ul>
            }
        </div>
    }
}

fn work_plan_node(
    projects: &PortalProjectsPage,
    item: &PortalProjectWorkItem,
    on_msg: &Callback<Msg>,
) -> Html {
    let selected = projects.selected_node_id.as_deref() == Some(item.id.as_str());
    let id = item.id.clone();
    let on_select = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectNodeSelected(Some(id.clone()))))
    };
    let children = child_items(projects, &item.id);
    html! {
        <li>
            <button
                type="button"
                onclick={on_select}
                class={classes!(
                    "grid","w-full","grid-cols-[22px_minmax(0,1fr)_72px_88px]","items-center","gap-2",
                    "rounded-[var(--portal-tab-radius)]","px-2","py-2","text-left","transition",
                    if selected { "bg-white/65 ring-1 ring-inset ring-[var(--portal-panel-border)]" } else { "hover:bg-white/40" }
                )}
            >
                <span class={classes!("h-3.5","w-3.5","justify-self-center","rounded-full","border","border-black/10",status_dot(&item.status))}></span>
                <span class="min-w-0 flex-1">
                    <span class="block truncate text-[13.5px] text-[var(--portal-navy)]">{ item.title.clone() }</span>
                    <span class="block truncate text-[10px] font-light text-black/40">{ item.category.clone() }</span>
                </span>
                <span class="text-right text-[10px] font-light text-[var(--portal-blue-gray)]">
                    { due_label(item.due_at.as_deref()) }
                </span>
                <span class="truncate text-right text-[9px] font-light text-black/40">
                    { item.owner.clone().unwrap_or_else(|| "—".into()) }
                </span>
            </button>
            if !children.is_empty() {
                <ul class="ml-5 border-l border-[var(--portal-mist-3)]/70 pl-1.5">
                    { for children.into_iter().map(|child| work_plan_node(projects, child, on_msg)) }
                </ul>
            }
        </li>
    }
}

fn placeholder_view(title: &str, message: &str) -> Html {
    html! {
        <div class="flex h-full min-h-64 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/25 px-8 text-center">
            <div>
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{ title }</h2>
                <p class="mt-2 max-w-xl text-sm font-light text-black/45">{ message }</p>
            </div>
        </div>
    }
}

fn selected_calendar_event_panel(model: &Vm<'_>, projects: &PortalProjectsPage) -> Html {
    let event = projects
        .calendar_selected_event_id
        .as_deref()
        .and_then(|id| projects.calendar.iter().find(|event| event.id == id));
    let Some(event) = event else {
        return html! {};
    };

    let pending = model
        .calendar_pending
        .filter(|pending| pending.occurrence_id == event.id);
    let ownership = if event.source == "apple_calendar" && event.provider_event_id.is_some() {
        "Apple · move/resize"
    } else {
        "Canonical · read only"
    };
    let schedule = if event.all_day {
        "All day".to_owned()
    } else {
        match event.end_at.as_deref() {
            Some(end) => format!(
                "{} – {}",
                crate::calendar::time_label(&event.start_at).unwrap_or_else(|| event.start_at.clone()),
                crate::calendar::time_label(end).unwrap_or_else(|| end.to_owned())
            ),
            None => crate::calendar::time_label(&event.start_at)
                .unwrap_or_else(|| event.start_at.clone()),
        }
    };

    html! {
        <section class="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
            <div class="flex min-h-10 items-center gap-3 px-3 py-2">
                <span class="shrink-0 text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                    {"Selected event"}
                </span>
                <span class="min-w-0 flex-1 truncate text-[15px] font-medium text-[var(--portal-navy)]">
                    { event.title.clone() }
                </span>
                <span class="shrink-0 rounded-full bg-white/60 px-2 py-1 text-[9px] font-medium uppercase tracking-[0.08em] text-[var(--portal-blue-gray)]">
                    { ownership }
                </span>
            </div>
            <div class="grid gap-x-5 gap-y-2 border-t border-[var(--portal-panel-border)] px-3 py-3 text-[11px] md:grid-cols-3 xl:grid-cols-6">
                { event_detail("Date", crate::calendar::date_key(&event.start_at).unwrap_or_default()) }
                { event_detail("Time", schedule) }
                { event_detail("Source", event.source.replace('_', " ")) }
                { event_detail(
                    "Repeat",
                    if event.recurring {
                        if event.detached { "Modified occurrence".into() } else { "Recurring".into() }
                    } else {
                        "One time".into()
                    }
                ) }
                { event_detail("Person", event.person_name.clone().unwrap_or_else(|| "—".into())) }
                { event_detail("Property", event.property_name.clone().unwrap_or_else(|| "—".into())) }
                if let Some(location) = event.location.as_ref().filter(|value| !value.trim().is_empty()) {
                    <div class="md:col-span-2 xl:col-span-3">
                        { event_detail("Location", location.clone()) }
                    </div>
                }
                if let Some(pending) = pending {
                    <div class="md:col-span-1 xl:col-span-3">
                        { event_detail(
                            "Apple state",
                            match pending.phase.as_str() {
                                "queueing" => "Queueing…".into(),
                                "queued" => "Queued for Mac".into(),
                                "delivered" => "Delivered to EventKit · awaiting sync".into(),
                                "reconciled" => "Reconciled".into(),
                                other => other.replace('_', " "),
                            }
                        ) }
                    </div>
                }
            </div>
        </section>
    }
}

fn event_detail(label: &str, value: String) -> Html {
    html! {
        <div class="min-w-0">
            <p class="text-[9px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">{ label }</p>
            <p class="mt-0.5 truncate text-[12px] font-light text-[var(--portal-navy)]">{ value }</p>
        </div>
    }
}

fn selected_project(projects: &PortalProjectsPage) -> Option<&PortalProject> {
    let id = projects.selected_project_id.as_deref()?;
    projects.projects.iter().find(|project| project.id == id)
}

fn selected_item(projects: &PortalProjectsPage) -> Option<&PortalProjectWorkItem> {
    let id = projects.selected_node_id.as_deref()?;
    projects.items.iter().find(|item| item.id == id)
}

fn project_items<'a>(
    projects: &'a PortalProjectsPage,
    project_id: &str,
) -> Vec<&'a PortalProjectWorkItem> {
    projects
        .items
        .iter()
        .filter(|item| item.project_id.as_deref() == Some(project_id))
        .collect()
}

fn root_items<'a>(
    projects: &'a PortalProjectsPage,
    project_id: &str,
) -> Vec<&'a PortalProjectWorkItem> {
    let mut items = projects
        .items
        .iter()
        .filter(|item| item.project_id.as_deref() == Some(project_id) && item.parent_id.is_none())
        .collect::<Vec<_>>();
    items.sort_by_key(|item| item.order.unwrap_or(i32::MAX));
    items
}

fn child_items<'a>(
    projects: &'a PortalProjectsPage,
    parent_id: &str,
) -> Vec<&'a PortalProjectWorkItem> {
    let mut items = projects
        .items
        .iter()
        .filter(|item| item.parent_id.as_deref() == Some(parent_id))
        .collect::<Vec<_>>();
    items.sort_by_key(|item| item.order.unwrap_or(i32::MAX));
    items
}

fn project_progress(projects: &PortalProjectsPage, project_id: &str) -> i32 {
    let items = project_items(projects, project_id);
    let planned = items
        .iter()
        .filter(|item| item.status != "dismissed")
        .count();
    if planned == 0 {
        return 0;
    }
    let done = items.iter().filter(|item| item.status == "done").count();
    ((done * 100) / planned) as i32
}

fn project_person_ids(projects: &PortalProjectsPage, project: &PortalProject) -> BTreeSet<String> {
    if let Some(id) = project.person_id.as_ref() {
        return BTreeSet::from([id.clone()]);
    }
    project_items(projects, &project.id)
        .into_iter()
        .filter_map(|item| item.entity.as_ref())
        .filter(|entity| entity.entity_type == "person")
        .map(|entity| entity.id.clone())
        .collect()
}

fn effective_project_property_ids(
    projects: &PortalProjectsPage,
    project: &PortalProject,
) -> BTreeSet<String> {
    if let Some(id) = project.property_id.as_ref() {
        return BTreeSet::from([id.clone()]);
    }
    project_items(projects, &project.id)
        .into_iter()
        .filter_map(|item| item.entity.as_ref())
        .filter(|entity| entity.entity_type == "property")
        .map(|entity| entity.id.clone())
        .collect()
}

fn activity_view(projects: &PortalProjectsPage, project: &PortalProject) -> Html {
    let people = project_person_ids(projects, project);
    let properties = effective_project_property_ids(projects, project);
    let entries = projects
        .activity
        .iter()
        .filter(|entry| {
            entry
                .person_id
                .as_ref()
                .is_some_and(|id| people.contains(id))
                || entry
                    .property_id
                    .as_ref()
                    .is_some_and(|id| properties.contains(id))
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        let message = if people.is_empty() && properties.is_empty() {
            "This project is not anchored to a contact or property, so activity cannot be linked."
        } else {
            "No activity is linked to this project yet."
        };
        return placeholder_view("Activity", message);
    }

    html! {
        <div class="h-full min-h-0 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-white/40 bg-white/20 p-2">
            <ul class="divide-y divide-[var(--portal-panel-border)]/70">
                { for entries.into_iter().map(|entry| {
                    let channel = entry.direction.as_ref()
                        .map(|direction| format!("{} · {}", entry.channel, direction))
                        .unwrap_or_else(|| entry.channel.clone());
                    let title = entry.title.clone()
                        .or_else(|| entry.summary.clone())
                        .unwrap_or_else(|| "Activity recorded".into());
                    html! {
                        <li key={entry.id.clone()} class="px-2 py-2.5">
                            <div class="flex items-center justify-between gap-3">
                                <span class="text-[11px] font-medium uppercase tracking-[0.08em] text-[var(--portal-gold-muted)]">
                                    { channel }
                                </span>
                                <time datetime={entry.occurred_at.clone()} class="text-[10px] font-light text-black/40">
                                    { entry.occurred_at_label.clone() }
                                </time>
                            </div>
                            <p class="mt-1 text-[13px] text-[var(--portal-navy)]">{ title }</p>
                            if entry.summary.is_some() && entry.title.is_some() {
                                <p class="mt-0.5 truncate text-[11px] font-light text-black/45">
                                    { entry.summary.clone().unwrap_or_default() }
                                </p>
                            }
                        </li>
                    }
                }) }
            </ul>
        </div>
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "doing" => "In progress",
        "done" => "Complete",
        "dismissed" => "Dismissed",
        _ => "Open",
    }
}

fn status_dot(status: &str) -> &'static str {
    match status {
        "done" => "bg-[var(--portal-success)]",
        "doing" => "bg-[var(--portal-gold)]",
        "dismissed" => "bg-black/25",
        _ => "bg-white/35",
    }
}

fn due_label(value: Option<&str>) -> String {
    value
        .and_then(|value| value.get(0..10))
        .filter(|value| !value.is_empty())
        .unwrap_or("—")
        .to_string()
}

fn date_value(value: Option<&str>) -> String {
    value
        .and_then(|value| value.get(0..10))
        .unwrap_or("")
        .to_string()
}
