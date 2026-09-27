//! CORE / Project Management — Rust/Yew shell and MVI workspace.
//!
//! The deleted JavaScript widgets return as native Rust/Yew views. Application state remains in
//! Model -> update(); this file only renders that state and emits named intents.

use std::collections::BTreeSet;

use yew::prelude::*;

use crate::model::{
    PortalProject, PortalProjectCalendarEvent, PortalProjectWorkItem, PortalProjectsPage,
};

use super::{Msg, Vm};

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

const DOMAINS: [(&str, &str); 6] = [
    ("properties", "Properties"),
    ("people", "People"),
    ("deals", "Deals"),
    ("firm", "Firm"),
    ("marketing", "Marketing"),
    ("accounting", "Accounting"),
];

/// The left pane: the domain, then that domain's projects (with what each is about), then the open project's work as
/// a tree. Search narrows projects and work; Catch-up switches the centre to the catch-up list.
fn navigator(model: &Vm<'_>, projects: &PortalProjectsPage, on_msg: &Callback<Msg>) -> Html {
    let query = model.controls.query.trim().to_lowercase();
    let matches = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);
    let listed: Vec<&PortalProject> = projects
        .projects
        .iter()
        .filter(|project| crate::projects::project_in_domain(project, &projects.items, &projects.active_domain))
        .filter(|project| {
            matches(&project.name)
                || matches(project.owner.as_deref().unwrap_or(""))
                || project_items(projects, &project.id).iter().any(|item| matches(&item.title))
        })
        .collect();
    let about = |project: &PortalProject| {
        [
            project.property_id.as_ref().map(|id| format!("property:{id}")),
            project.person_id.as_ref().map(|id| format!("person:{id}")),
            project.contract_id.as_ref().map(|id| format!("contract:{id}")),
        ]
        .into_iter()
        .flatten()
        .find_map(|key| projects.identity_names.get(&key).cloned())
    };
    let search = on_msg.reform(|event: InputEvent| Msg::QueryChanged(crate::app::template::input_value(&event)));
    let catch_up = projects.catch_up;
    let toggle = on_msg.reform(move |_: MouseEvent| Msg::ProjectCatchUpToggled(!catch_up));
    html! {
        <aside
            class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)] text-white"
            style="background-color: color-mix(in srgb, var(--portal-navy) 90%, transparent);"
        >
            <div class="shrink-0 space-y-2 border-b border-white/10 p-3">
                <div class="flex flex-wrap gap-1">
                    {for DOMAINS.iter().map(|(key, label)| {
                        let active = projects.active_domain == *key;
                        let pick = on_msg.reform(move |_: MouseEvent| Msg::ProjectDomainSelected((*key).to_owned()));
                        html! {
                            <button type="button" onclick={pick}
                                class={classes!("rounded-md", "px-2", "py-1", "text-[10px]", "font-semibold", "uppercase", "tracking-[0.1em]",
                                    if active { "bg-white text-[var(--portal-navy)]" } else { "text-white/65 hover:bg-white/10" })}>
                                {*label}
                            </button>
                        }
                    })}
                </div>
                <div class="flex gap-2">
                    <input type="search" value={model.controls.query.clone()} oninput={search} placeholder="Search projects and work…"
                        class="h-8 min-w-0 flex-1 rounded-md border border-white/15 bg-white/10 px-2 text-[12px] text-white placeholder:text-white/40 outline-none focus:border-white/40" />
                    <button type="button" onclick={toggle}
                        class={classes!("rounded-md", "px-2", "text-[10px]", "font-semibold", "uppercase", "tracking-[0.1em]",
                            if catch_up { "bg-[var(--portal-gold)] text-[var(--portal-navy)]" } else { "border border-white/20 text-white/75" })}>
                        {"Catch-up"}
                    </button>
                </div>
            </div>
            <div class="min-h-0 flex-1 overflow-y-auto p-2">
                if listed.is_empty() {
                    <p class="px-2 py-6 text-center text-[12px] text-white/45">{"No projects here."}</p>
                }
                {for listed.iter().map(|project| {
                    let selected = projects.selected_project_id.as_deref() == Some(project.id.as_str());
                    let id = project.id.clone();
                    let open = on_msg.reform(move |_: MouseEvent| Msg::ProjectSelected(id.clone()));
                    let progress = project_progress(projects, &project.id);
                    html! {
                        <div class="mb-1">
                            <button type="button" onclick={open}
                                class={classes!("w-full", "rounded-md", "px-2.5", "py-2", "text-left", "transition",
                                    if selected { "bg-white/15" } else { "hover:bg-white/10" })}>
                                <div class="flex items-center justify-between gap-2">
                                    <span class="truncate text-[13px] font-medium">{&project.name}</span>
                                    <span class="shrink-0 text-[10px] text-white/50">{format!("{progress}%")}</span>
                                </div>
                                if let Some(about) = about(project) {
                                    <div class="truncate text-[11px] text-white/50">{about}</div>
                                }
                            </button>
                            if selected {
                                <div class="ml-2 border-l border-white/10 pl-1">
                                    {work_tree(model, projects, &project.id, None, &query, on_msg, 0)}
                                </div>
                            }
                        </div>
                    }
                })}
            </div>
        </aside>
    }
}

/// A project's work under `parent`, in order, each row opening that work item in the centre.
fn work_tree(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    project_id: &str,
    parent: Option<&str>,
    query: &str,
    on_msg: &Callback<Msg>,
    depth: usize,
) -> Html {
    if depth > 8 {
        return Html::default();
    }
    let mut children: Vec<&PortalProjectWorkItem> = project_items(projects, project_id)
        .into_iter()
        .filter(|item| item.parent_id.as_deref() == parent)
        .collect();
    children.sort_by(|a, b| {
        a.order
            .unwrap_or(i32::MAX)
            .cmp(&b.order.unwrap_or(i32::MAX))
            .then_with(|| a.due_at.as_deref().unwrap_or("9999").cmp(b.due_at.as_deref().unwrap_or("9999")))
            .then_with(|| a.id.cmp(&b.id))
    });
    html! {
        {for children.into_iter().filter(|item| query.is_empty() || item.title.to_lowercase().contains(query) || parent.is_some()).map(|item| {
            let selected = projects.selected_node_id.as_deref() == Some(item.id.as_str());
            let id = item.id.clone();
            let open = on_msg.reform(move |_: MouseEvent| Msg::ProjectNodeSelected(Some(id.clone())));
            let due = due_label(item.due_at.as_deref());
            html! {
                <>
                    <button type="button" onclick={open}
                        class={classes!("flex", "w-full", "items-center", "gap-2", "rounded", "px-2", "py-1.5", "text-left", "text-[12px]",
                            if selected { "bg-white/15 text-white" } else { "text-white/75 hover:bg-white/10" })}>
                        <span class={classes!("h-1.5", "w-1.5", "shrink-0", "rounded-full", status_dot(&item.status))}></span>
                        <span class="min-w-0 flex-1 truncate">{&item.title}</span>
                        if due != "—" {
                            <span class="shrink-0 text-[10px] text-white/45">{due}</span>
                        }
                    </button>
                    <div class="ml-3">
                        {work_tree(model, projects, project_id, Some(item.id.as_str()), query, on_msg, depth + 1)}
                    </div>
                </>
            }
        })}
    }
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
                { active_view(projects, project, on_msg) }
            </div>
            <div class="shrink-0 px-3 pb-3">
                { selected_work_editor(model, projects, selected, on_msg) }
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
            let value = event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value();
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
                if let Some(error) = model.error.as_ref() {
                    <p class="mt-1 text-xs text-red-700">{ (*error).clone() }</p>
                }
            </div>
        </>
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
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    match projects.active_view.as_str() {
        "timeline" => timeline_view(),
        "calendar" => calendar_view(projects, project, on_msg),
        "financials" => placeholder_view(
            "Financials",
            "No project-scoped accounting read model is attached to the Rust workspace yet.",
        ),
        "documents" => documents_view(),
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

fn timeline_view() -> Html {
    crate::app::template::widget_removed("The timeline")
}

#[derive(Clone)]
struct CalendarChip {
    id: String,
    title: String,
    date: String,
    time: Option<String>,
    source: String,
    kind: String,
}

fn calendar_event_linked_to_project(
    projects: &PortalProjectsPage,
    project: &PortalProject,
    event: &PortalProjectCalendarEvent,
) -> bool {
    if project
        .person_id
        .as_deref()
        .is_some_and(|id| event.person_id.as_deref() == Some(id))
    {
        return true;
    }

    let property_name = project
        .property_id
        .as_ref()
        .and_then(|id| projects.identity_names.get(&format!("property:{id}")));
    property_name.is_some_and(|name| event.property_name.as_deref() == Some(name.as_str()))
}

fn project_calendar_chips(
    projects: &PortalProjectsPage,
    project: &PortalProject,
) -> Vec<CalendarChip> {
    let mut chips = project_items(projects, &project.id)
        .into_iter()
        .filter_map(|item| {
            let due = item.due_at.as_deref()?;
            Some(CalendarChip {
                id: format!("wbs:{}", item.id),
                title: item.title.clone(),
                date: crate::calendar::date_key(due)?,
                time: None,
                source: "wbs".into(),
                kind: item.category.clone(),
            })
        })
        .collect::<Vec<_>>();

    chips.extend(
        projects
            .calendar
            .iter()
            .filter(|event| calendar_event_linked_to_project(projects, project, event))
            .filter_map(|event| {
                Some(CalendarChip {
                    id: event.id.clone(),
                    title: event.title.clone(),
                    date: crate::calendar::date_key(&event.start_at)?,
                    time: (!event.all_day)
                        .then(|| crate::calendar::time_label(&event.start_at))
                        .flatten(),
                    source: event.source.clone(),
                    kind: event.kind.clone(),
                })
            }),
    );

    chips.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.time.cmp(&right.time))
            .then_with(|| left.id.cmp(&right.id))
    });
    chips
}

fn calendar_view(
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    let days = crate::calendar::month_cells(&projects.calendar_cursor);
    let title = crate::calendar::month_title(&projects.calendar_cursor);
    let chips = project_calendar_chips(projects, project);
    let previous = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectCalendarPrevious))
    };
    let today = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectCalendarToday))
    };
    let next = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectCalendarNext))
    };

    html! {
        <section class="flex h-full min-h-0 flex-col overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35">
            <div class="flex shrink-0 items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-3 py-2">
                <div class="flex items-center gap-1">
                    <button type="button" onclick={previous} aria-label="Previous month"
                        class="h-8 rounded-md border border-[var(--portal-panel-border)] bg-white/70 px-2.5 text-[15px] text-[var(--portal-navy)] hover:bg-white">
                        {"‹"}
                    </button>
                    <button type="button" onclick={today}
                        class="h-8 rounded-md border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[11px] font-medium uppercase tracking-[0.08em] text-[var(--portal-navy)] hover:bg-white">
                        {"Today"}
                    </button>
                    <button type="button" onclick={next} aria-label="Next month"
                        class="h-8 rounded-md border border-[var(--portal-panel-border)] bg-white/70 px-2.5 text-[15px] text-[var(--portal-navy)] hover:bg-white">
                        {"›"}
                    </button>
                </div>
                <h2 class="font-serif text-[20px] font-light text-[var(--portal-navy)]">{ title }</h2>
                <span class="rounded-full bg-[var(--portal-navy)] px-3 py-1.5 text-[10px] font-medium uppercase tracking-[0.1em] text-white">
                    {"Month"}
                </span>
            </div>
            <div class="grid shrink-0 grid-cols-7 border-b border-[var(--portal-panel-border)] bg-white/45">
                { for ["Sun","Mon","Tue","Wed","Thu","Fri","Sat"].into_iter().map(|label| html! {
                    <div class="px-2 py-1.5 text-center text-[9px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">{ label }</div>
                }) }
            </div>
            <div class="grid min-h-0 flex-1 grid-cols-7 grid-rows-6">
                { for days.into_iter().map(|day| {
                    let is_today = day.date == projects.calendar_today;
                    let day_events = chips.iter().filter(|event| event.date == day.date).collect::<Vec<_>>();
                    let extra = day_events.len().saturating_sub(4);
                    html! {
                        <div
                            key={day.date.clone()}
                            class={classes!(
                                "min-h-0","overflow-hidden","border-b","border-r","border-[var(--portal-panel-border)]/70","p-1.5",
                                (!day.in_month).then_some("bg-black/[0.025]"),
                                is_today.then_some("bg-[var(--portal-gold)]/[0.09]")
                            )}
                        >
                            <div class="mb-1 flex items-center justify-between">
                                <span class={classes!(
                                    "flex","h-5","w-5","items-center","justify-center","rounded-full","text-[10px]",
                                    if is_today { "bg-[var(--portal-gold)] font-semibold text-[var(--portal-navy)]" }
                                    else if day.in_month { "text-[var(--portal-navy)]" } else { "text-black/25" }
                                )}>
                                    { day.day }
                                </span>
                            </div>
                            <div class="space-y-0.5">
                                { for day_events.iter().take(4).map(|event| {
                                    let tone = if event.source == "wbs" {
                                        "border-[var(--portal-navy)]/15 bg-[var(--portal-navy)]/[0.07] text-[var(--portal-navy)]"
                                    } else if event.kind == "showing" {
                                        "border-[var(--portal-gold)]/30 bg-[var(--portal-gold)]/15 text-[var(--portal-navy)]"
                                    } else {
                                        "border-[var(--portal-blue-gray)]/20 bg-white/70 text-[var(--portal-navy)]"
                                    };
                                    let tooltip = event.time.as_ref()
                                        .map(|time| format!("{time} · {}", event.title))
                                        .unwrap_or_else(|| event.title.clone());
                                    html! {
                                        <div key={event.id.clone()} title={tooltip}
                                            class={classes!("truncate","rounded","border","px-1.5","py-0.5","text-[9px]","leading-tight",tone)}>
                                            if let Some(time) = event.time.as_ref() {
                                                <span class="mr-1 font-semibold">{ time }</span>
                                            }
                                            { event.title.clone() }
                                        </div>
                                    }
                                }) }
                                if extra > 0 {
                                    <div class="px-1 text-[9px] font-medium text-[var(--portal-blue-gray)]">
                                        { format!("+{extra} more") }
                                    </div>
                                }
                            </div>
                        </div>
                    }
                }) }
            </div>
        </section>
    }
}

fn documents_view() -> Html {
    crate::app::template::widget_removed("The document browser")
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

fn selected_work_editor(
    _model: &Vm<'_>,
    projects: &PortalProjectsPage,
    item: Option<&PortalProjectWorkItem>,
    on_msg: &Callback<Msg>,
) -> Html {
    let Some(item) = item else {
        return html! {
            <section class="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
                <div class="flex min-h-10 w-full items-center gap-3 px-3 py-2 text-left">
                    <span class="shrink-0 text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                        {"Selected work"}
                    </span>
                    <span class="min-w-0 flex-1 truncate text-[13px] font-light text-black/45">
                        {"Select a work item to edit it."}
                    </span>
                </div>
            </section>
        };
    };

    let toggle = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectWorkCollapsedToggled))
    };
    let title_change = input_msg(on_msg, MsgKind::Title);
    let owner_change = input_msg(on_msg, MsgKind::Owner);
    let due_change = input_msg(on_msg, MsgKind::Due);
    let notes_change = textarea_msg(on_msg);
    let status_change = select_status_msg(on_msg);
    let save = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectWorkSaveRequested))
    };
    let summary = format!(
        "{} · {}",
        status_label(&item.status),
        due_label(item.due_at.as_deref())
    );

    html! {
        <section class="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
            <button
                type="button"
                onclick={toggle}
                aria-expanded={(!projects.work_collapsed).to_string()}
                class="flex min-h-10 w-full items-center gap-3 px-3 py-2 text-left"
            >
                <span class="shrink-0 text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                    {"Selected work"}
                </span>
                <span class="min-w-0 flex-1 truncate text-[15px] font-medium text-[var(--portal-navy)]">
                    { item.title.clone() }
                </span>
                <span class="hidden shrink-0 text-[12px] font-light text-[var(--portal-blue-gray)] sm:inline">
                    { summary }
                </span>
                <span class="flex shrink-0 items-center gap-1 text-[11px] font-medium text-[var(--portal-blue-gray)]">
                    { if projects.work_collapsed { "Expand" } else { "Collapse" } }
                    <span class="text-base leading-none" aria-hidden="true">
                        { if projects.work_collapsed { "⌄" } else { "⌃" } }
                    </span>
                </span>
            </button>
            if !projects.work_collapsed {
                <div class="grid grid-cols-2 gap-2 border-t border-[var(--portal-panel-border)] px-3 pb-3 pt-2 md:grid-cols-3 xl:grid-cols-[minmax(180px,1.2fr)_130px_135px_150px_minmax(220px,1.35fr)_auto] xl:items-end">
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Title"}
                        <input value={item.title.clone()} oninput={title_change} class={work_input_class()} />
                    </label>
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Status"}
                        <select value={item.status.clone()} onchange={status_change} class={work_input_class()}>
                            <option value="open" selected={item.status == "open"}>{"Not started"}</option>
                            <option value="doing" selected={item.status == "doing"}>{"In progress"}</option>
                            <option value="done" selected={item.status == "done"}>{"Complete"}</option>
                            <option value="dismissed" selected={item.status == "dismissed"}>{"Dismissed"}</option>
                        </select>
                    </label>
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Due"}
                        <input type="date" value={date_value(item.due_at.as_deref())} oninput={due_change} class={work_input_class()} />
                    </label>
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Owner"}
                        <input value={item.owner.clone().unwrap_or_default()} oninput={owner_change} class={work_input_class()} />
                    </label>
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Notes"}
                        <textarea
                            value={item.notes.clone()}
                            oninput={notes_change}
                            rows="2"
                            class="mt-1 min-h-[3.5rem] w-full resize-y rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-2.5 py-2 text-[12px] font-light leading-snug text-black/70 outline-none"
                        />
                    </label>
                    <button
                        type="button"
                        onclick={save}
                        disabled={!projects.work_dirty || projects.saving}
                        class="h-9 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[12px] font-medium text-white shadow-sm transition hover:opacity-90 disabled:opacity-35"
                    >
                        { if projects.saving { "Saving…" } else { "Save" } }
                    </button>
                </div>
            }
        </section>
    }
}

fn catchup_center(model: &Vm<'_>, projects: &PortalProjectsPage, on_msg: &Callback<Msg>) -> Html {
    let selected = selected_item(projects);
    html! {
        <section class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="min-h-0 flex-1 overflow-hidden">
                { crate::app::template::widget_removed("The catch-up view") }
            </div>
            <div class="shrink-0 px-3 pb-3">
                { selected_work_editor(model, projects, selected, on_msg) }
            </div>
        </section>
    }
}

#[derive(Clone, Copy)]
enum MsgKind {
    Title,
    Owner,
    Due,
}

fn input_msg(on_msg: &Callback<Msg>, kind: MsgKind) -> Callback<InputEvent> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: InputEvent| {
        let value = event
            .target_unchecked_into::<web_sys::HtmlInputElement>()
            .value();
        on_msg.emit(match kind {
            MsgKind::Title => Msg::ProjectWorkTitleChanged(value),
            MsgKind::Owner => Msg::ProjectWorkOwnerChanged(value),
            MsgKind::Due => Msg::ProjectWorkDueChanged(value),
        });
    })
}

fn textarea_msg(on_msg: &Callback<Msg>) -> Callback<InputEvent> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: InputEvent| {
        let value = event
            .target_unchecked_into::<web_sys::HtmlTextAreaElement>()
            .value();
        on_msg.emit(Msg::ProjectWorkNotesChanged(value));
    })
}

fn select_status_msg(on_msg: &Callback<Msg>) -> Callback<Event> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: Event| {
        let value = event
            .target_unchecked_into::<web_sys::HtmlSelectElement>()
            .value();
        on_msg.emit(Msg::ProjectWorkStatusChanged(value));
    })
}

fn work_input_class() -> Classes {
    classes!(
        "mt-1",
        "block",
        "h-8",
        "w-full",
        "rounded-[var(--portal-tab-radius)]",
        "border",
        "border-[var(--portal-panel-border)]",
        "bg-white/70",
        "px-2.5",
        "text-[12px]",
        "font-light",
        "text-black/70",
        "outline-none"
    )
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
