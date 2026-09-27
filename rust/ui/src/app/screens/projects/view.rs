//! CORE / Project Management — Rust/Yew shell and MVI workspace.
//!
//! The deleted JavaScript widgets return as native Rust/Yew views. Application state remains in
//! Model -> update(); this file only renders that state and emits named intents.

use std::collections::BTreeSet;

use yew::prelude::*;

use crate::model::{PortalProject, PortalProjectWorkItem, PortalProjectsPage};

use super::{Msg, Vm};

mod calendar;
mod timeline;

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

/// The navigator, as designed: the domain rail, the domain's header and search, and the tree — poles (property, client,
/// contract, or the domain's collection) over projects over the work.
fn navigator(model: &Vm<'_>, projects: &PortalProjectsPage, on_msg: &Callback<Msg>) -> Html {
    let tree = super::nav::build(projects);
    let selected = super::nav::selected_id(&tree, projects.selected_project_id.as_deref(), projects.selected_node_id.as_deref());
    let opened: Vec<String> = selected.as_deref().map(|id| super::nav::ancestors(&tree, id)).unwrap_or_default();
    let query = model.controls.query.trim().to_lowercase();
    let search = on_msg.reform(|event: InputEvent| Msg::QueryChanged(crate::app::template::input_value(&event)));
    let rail_button = |active: bool, icon: &'static str, label: &'static str, msg: Msg| {
        let on_msg = on_msg.clone();
        let msg = std::rc::Rc::new(std::cell::RefCell::new(Some(msg)));
        let click = Callback::from(move |_: MouseEvent| {
            if let Some(msg) = msg.borrow_mut().take() {
                on_msg.emit(msg);
            }
        });
        html! {
            <button type="button" onclick={click} title={label}
                class={classes!("group", "relative", "flex", "w-full", "flex-col", "items-center", "gap-1.5", "py-2.5", "transition", (!active).then_some("opacity-95 hover:opacity-100"))}>
                <span class={classes!("absolute", "inset-y-2", "left-0", "w-[3px]", "rounded-r-full", "transition",
                    if active { "bg-[var(--portal-gold)]" } else { "bg-transparent group-hover:bg-white/30" })}></span>
                <span class={classes!("flex", "h-10", "w-10", "items-center", "justify-center", "rounded-xl", "transition",
                    if active { "bg-black/25 text-[var(--portal-gold)] shadow-sm ring-1 ring-inset ring-white/25" } else { "bg-white/[0.07] text-white/85 group-hover:bg-white/[0.16] group-hover:text-white" })}>
                    { glyph(icon, "h-[23px] w-[23px]") }
                </span>
                <span class={classes!("text-center", "text-[14px]", "font-medium", "uppercase", "leading-tight", "tracking-[0.02em]",
                    if active { "text-white" } else { "text-white/70 group-hover:text-white/95" })}>{label}</span>
            </button>
        }
    };
    let ctx = NavCtx {
        selected: selected.as_deref(),
        opened: &opened,
        query: &query,
        open: &model.controls.nav_open,
        closed: &model.controls.nav_closed,
        on_msg,
    };
    html! {
        <aside class="flex min-h-0 overflow-hidden rounded-[var(--portal-panel-radius)] text-white shadow-[var(--portal-panel-shadow)]"
            style="background-color: var(--portal-navy);">
            <div class="flex w-[86px] shrink-0 flex-col items-center overflow-y-auto border-r border-white/10 py-3" aria-label="Project scope and domain">
                { rail_button(projects.catch_up, "list-checks", "Catch-Up", Msg::ProjectCatchUpToggled(true)) }
                <div class="my-1 w-[60%] border-b border-white/15" aria-hidden="true"></div>
                {for super::nav::DOMAINS.iter().map(|(key, label, icon)| {
                    rail_button(!projects.catch_up && projects.active_domain == *key, icon, label, Msg::ProjectDomainSelected((*key).to_owned()))
                })}
            </div>
            <div class="flex min-h-0 min-w-0 flex-1 flex-col">
                <div class="border-b border-white/10 px-3 pb-2 pt-3">
                    <p class="text-[14px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)]">{ super::nav::domain_label(&projects.active_domain) }</p>
                    <label class="mt-2 flex h-11 items-center gap-2 rounded-[var(--portal-tab-radius)] border border-white/15 bg-white/10 px-3">
                        { glyph("search", "h-4 w-4 shrink-0 text-white/50") }
                        <input value={model.controls.query.clone()} oninput={search} placeholder="Find work…"
                            class="min-w-0 flex-1 bg-transparent text-[16px] font-light text-white outline-none placeholder:text-white/55" />
                    </label>
                </div>
                <div class="min-h-0 flex-1 overflow-y-auto px-1 pt-1.5">
                    if tree.is_empty() {
                        <div class="px-3 py-8 text-sm font-light text-white/45">{"No matching projects in this perspective."}</div>
                    }
                    {for tree.iter().map(|node| nav_node(&ctx, node, 0))}
                </div>
            </div>
        </aside>
    }
}

struct NavCtx<'a> {
    selected: Option<&'a str>,
    opened: &'a [String],
    query: &'a str,
    open: &'a std::collections::BTreeSet<String>,
    closed: &'a std::collections::BTreeSet<String>,
    on_msg: &'a Callback<Msg>,
}

fn progress_bar(value: i64) -> Html {
    let clamped = value.clamp(0, 100);
    html! {
        <div class="h-1 w-11 overflow-hidden rounded-full bg-white/15">
            <div class="h-full rounded-full bg-[var(--portal-gold)]" style={format!("width: {clamped}%")}></div>
        </div>
    }
}

/// One row of the tree and, when open, its children. Searching shows the matching branches, open.
fn nav_node(ctx: &NavCtx<'_>, node: &super::nav::NavNode, depth: usize) -> Html {
    use super::nav::NodeKind;
    if !ctx.query.is_empty() && !node.search.contains(ctx.query) {
        return Html::default();
    }
    let leaf = node.children.is_empty();
    let is_open = !leaf
        && (!ctx.query.is_empty()
            || (!ctx.closed.contains(&node.id) && (ctx.open.contains(&node.id) || ctx.opened.contains(&node.id))));
    let selected = ctx.selected == Some(node.id.as_str());
    let toggle = {
        let (id, open) = (node.id.clone(), is_open);
        ctx.on_msg.reform(move |event: MouseEvent| {
            event.stop_propagation();
            Msg::NavToggled { id: id.clone(), open }
        })
    };
    let chevron = if leaf {
        html! { <span class="w-4 shrink-0" aria-hidden="true"></span> }
    } else {
        html! {
            <button type="button" onclick={toggle.clone()} aria-label={if is_open { "Collapse" } else { "Expand" }}
                class="flex h-6 w-4 shrink-0 items-center justify-center rounded text-white/55 transition hover:text-white">
                { glyph("chevron-down", if is_open { "h-3.5 w-3.5 transition" } else { "h-3.5 w-3.5 -rotate-90 transition" }) }
            </button>
        }
    };
    let pick = match node.kind {
        NodeKind::Pole => toggle,
        NodeKind::Project => {
            let id = node.project_id.clone().unwrap_or_default();
            ctx.on_msg.reform(move |_: MouseEvent| Msg::ProjectSelected(id.clone()))
        }
        NodeKind::Work => {
            let (project_id, node_id) = (node.project_id.clone().unwrap_or_default(), node.work_id.clone().unwrap_or_default());
            ctx.on_msg.reform(move |_: MouseEvent| Msg::NavWorkSelected { project_id: project_id.clone(), node_id: node_id.clone() })
        }
    };
    let indent = format!("padding-left: {}px", depth * 7);
    let row = match node.kind {
        NodeKind::Pole => html! {
            <div onclick={pick} style={indent} class={classes!("flex", "h-[70px]", "cursor-pointer", "items-center", "gap-2", "rounded-xl", "px-1", selected.then_some("bg-white/10 shadow-[0_2px_12px_rgba(0,0,0,0.16)]"))}>
                { chevron }
                <span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-[9px] bg-white/10 text-[var(--portal-gold)] ring-1 ring-inset ring-white/15">
                    { glyph(super::nav::domain_icon(&node.domain), "h-[18px] w-[18px]") }
                </span>
                <span class="min-w-0 flex-1">
                    <span class="block truncate font-serif text-[23px] font-bold leading-tight text-white/95">{ &node.label }</span>
                    if let Some(subtitle) = node.subtitle.as_deref().filter(|s| !s.is_empty()) {
                        <span class="mt-0.5 block truncate text-[15px] font-light leading-snug text-white/60">{ subtitle }</span>
                    }
                </span>
                if let Some(progress) = node.progress {
                    <span class="flex shrink-0 items-center gap-1.5 pr-1">
                        { progress_bar(progress) }
                        <span class="text-[14px] font-light text-white/55">{ format!("{progress}%") }</span>
                    </span>
                }
            </div>
        },
        NodeKind::Project => {
            let kind = node.meta.split(" · ").next().unwrap_or("");
            html! {
                <div onclick={pick} style={indent} class={classes!("flex", "h-[50px]", "cursor-pointer", "items-center", "gap-2", "rounded-lg", "px-1", selected.then_some("bg-white/10"))}>
                    { chevron }
                    if !kind.is_empty() { { glyph(super::nav::project_kind_icon(kind), "h-[18px] w-[18px] shrink-0 text-[var(--portal-gold)]") } }
                    <span class="min-w-0 flex-1 truncate text-[19px] font-light leading-tight text-white/95">{ &node.label }</span>
                    if let Some(progress) = node.progress {
                        <span class="shrink-0 pr-1 text-[14px] font-light text-white/55">{ format!("{progress}%") }</span>
                    }
                </div>
            }
        }
        NodeKind::Work => {
            let kind = node.meta.split(" · ").next().unwrap_or("");
            let icon_class = format!("h-[18px] w-[18px] shrink-0 {}", super::nav::status_class(node.status.as_deref()));
            html! {
                <div onclick={pick} style={indent} title={node.meta.clone()}
                    class={classes!("flex", "h-[46px]", "cursor-pointer", "items-center", "gap-2", "rounded-md", "px-1", selected.then_some("bg-white/15 ring-1 ring-inset ring-white/25"))}>
                    { chevron }
                    { glyph(super::nav::work_type_icon(kind, &node.label), &icon_class) }
                    <span class="min-w-0 flex-1 truncate text-[17px] font-light leading-tight text-white/95">{ &node.label }</span>
                </div>
            }
        }
    };
    html! {
        <>
            { row }
            if is_open {
                {for node.children.iter().map(|child| nav_node(ctx, child, depth + 1))}
            }
        </>
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
