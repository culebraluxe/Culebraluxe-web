//! CORE / Project Management — Rust/Yew shell and MVI workspace.
//!
//! React is retained only for the original vendor widgets (Arborist, SVAR Gantt, FullCalendar).
//! Their containers receive serialized read-only props; all application state remains in Model -> update().

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use yew::prelude::*;

use crate::model::{
    Msg, PortalProject, PortalProjectDocument, PortalProjectWorkItem, PortalProjectsPage,
};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct ProjectsProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Projects;

impl Component for Projects {
    type Message = ();
    type Properties = ProjectsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("projects").expect("projects screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { workspace(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

fn payload(model: &crate::model::Model) -> Option<&PortalProjectsPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.projects.as_ref())
}

fn workspace(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let Some(projects) = payload(model) else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-8">
                <p class="text-sm font-light text-black/45">
                    { if model.loading { "Loading projects…" } else { "Projects are unavailable." } }
                </p>
            </section>
        };
    };

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
            { island_bridge(on_msg) }
            { navigator(model, projects, on_msg) }
            { center_panel(model, projects, on_msg) }
        </div>
    }
}

fn island_bridge(on_msg: &Callback<Msg>) -> Html {
    let dispatch = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: MouseEvent| {
            let target = event.target_unchecked_into::<web_sys::HtmlElement>();
            let Some(raw) = target.get_attribute("data-intent") else {
                return;
            };
            let Ok(intent) = serde_json::from_str::<serde_json::Value>(&raw) else {
                return;
            };
            match intent.get("kind").and_then(|value| value.as_str()) {
                Some("project") => {
                    if let Some(project_id) =
                        intent.get("projectId").and_then(|value| value.as_str())
                    {
                        on_msg.emit(Msg::ProjectSelected(project_id.to_string()));
                    }
                }
                Some("work") => {
                    let project_id = intent.get("projectId").and_then(|value| value.as_str());
                    let node_id = intent.get("nodeId").and_then(|value| value.as_str());
                    if let (Some(project_id), Some(node_id)) = (project_id, node_id) {
                        on_msg.emit(Msg::ProjectSelected(project_id.to_string()));
                        on_msg.emit(Msg::ProjectNodeSelected(Some(node_id.to_string())));
                    }
                }
                _ => {}
            }
        })
    };
    html! {
        <button
            id="project-island-bridge"
            type="button"
            class="hidden"
            data-intent=""
            tabindex="-1"
            aria-hidden="true"
            onclick={dispatch}
        />
    }
}

fn navigator(
    model: &crate::model::Model,
    projects: &PortalProjectsPage,
    on_msg: &Callback<Msg>,
) -> Html {
    const DOMAINS: &[(&str, &str)] = &[
        ("properties", "Properties"),
        ("people", "People"),
        ("deals", "Deals"),
        ("firm", "Firm"),
        ("marketing", "Marketing"),
        ("accounting", "Accounting"),
    ];
    let query = model.controls.query.trim().to_lowercase();
    let on_query = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::QueryChanged(value));
        })
    };

    let visible = projects
        .projects
        .iter()
        .filter(|project| project_in_domain(project, &projects.items, &projects.active_domain))
        .filter(|project| {
            query.is_empty()
                || project.name.to_lowercase().contains(&query)
                || project_context(project, projects)
                    .to_lowercase()
                    .contains(&query)
                || project_items(projects, &project.id)
                    .iter()
                    .any(|item| item.title.to_lowercase().contains(&query))
        })
        .collect::<Vec<_>>();

    html! {
        <aside class="portal-glass-panel flex min-h-0 overflow-hidden rounded-[var(--portal-panel-radius)] bg-[var(--portal-navy)] text-white">
            <div class="w-[92px] shrink-0 border-r border-white/10 py-3">
                <button type="button"
                    onclick={{
                        let on_msg = on_msg.clone();
                        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectCatchUpToggled(true)))
                    }}
                    class={rail_class(projects.catch_up)}>
                    <span class="text-[12px] font-medium">{"Catch-Up"}</span>
                </button>
                <div class="mx-auto my-2 w-12 border-b border-white/15"></div>
                { for DOMAINS.iter().map(|(key, label)| {
                    let key_string = (*key).to_string();
                    let active = !projects.catch_up && projects.active_domain == *key;
                    let on_msg = on_msg.clone();
                    html! {
                        <button type="button"
                            onclick={Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectDomainSelected(key_string.clone())))}
                            class={rail_class(active)}>
                            <span class="text-[12px] font-medium">{ *label }</span>
                        </button>
                    }
                }) }
            </div>

            <div class="flex min-w-0 flex-1 flex-col">
                <div class="shrink-0 border-b border-white/10 p-3">
                    <div class="mb-2 flex items-center justify-between gap-2">
                        <div>
                            <div class="font-serif text-xl font-bold text-white">{"Projects"}</div>
                            <div class="text-[10px] uppercase tracking-[0.13em] text-white/45">
                                { if projects.catch_up { "Cross-project work" } else { projects.active_domain.as_str() } }
                            </div>
                        </div>
                        <button type="button" disabled=true
                            title="New Project stays gated until the Rust playbook-instantiation transaction is attached."
                            class="rounded-full border border-white/15 px-2.5 py-1.5 text-[10px] uppercase tracking-[0.1em] text-white/40">
                            {"New"}
                        </button>
                    </div>
                    <input type="search" value={model.controls.query.clone()} oninput={on_query}
                        placeholder="Search projects / work…"
                        class="w-full rounded-[var(--portal-tab-radius)] border border-white/15 bg-white/10 px-3 py-2 text-sm font-light text-white outline-none placeholder:text-white/35 focus:border-[var(--portal-gold)]/60" />
                </div>

                <div class="min-h-0 flex-1 overflow-y-auto p-2">
                    if projects.catch_up {
                        { catchup_nav(projects, on_msg) }
                    } else if visible.is_empty() {
                        <p class="px-3 py-8 text-sm font-light text-white/45">{"No matching projects in this perspective."}</p>
                    } else {
                        { navigator_island(projects, &visible, &query) }
                    }
                </div>
            </div>
        </aside>
    }
}

fn navigator_island(
    projects: &PortalProjectsPage,
    visible: &[&PortalProject],
    query: &str,
) -> Html {
    let visible_ids = visible
        .iter()
        .map(|project| project.id.as_str())
        .collect::<BTreeSet<_>>();
    let items = projects
        .items
        .iter()
        .filter(|item| {
            item.project_id
                .as_deref()
                .is_some_and(|project_id| visible_ids.contains(project_id))
        })
        .collect::<Vec<_>>();
    let widget = json!({
        "projects": visible,
        "items": items,
        "selectedProjectId": projects.selected_project_id.clone(),
        "selectedNodeId": projects.selected_node_id.clone(),
        "query": query,
    });
    html! {
        <div
            id="project-navigator-island"
            data-project-widget={widget.to_string()}
            class="h-full min-h-[16rem] overflow-hidden"
        >
            <div class="flex h-full items-center justify-center px-3 text-sm font-light text-white/40">
                {"Loading project tree…"}
            </div>
        </div>
    }
}

fn rail_class(active: bool) -> Classes {
    classes!(
        "relative",
        "flex",
        "w-full",
        "min-h-14",
        "items-center",
        "justify-center",
        "px-1",
        "text-center",
        "transition",
        if active {
            "border-l-[3px] border-l-[var(--portal-gold)] bg-black/20 text-[var(--portal-gold)]"
        } else {
            "border-l-[3px] border-l-transparent text-white/65 hover:bg-white/[0.07] hover:text-white"
        }
    )
}

fn project_tree_row(
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    let selected = projects.selected_project_id.as_deref() == Some(project.id.as_str());
    let id = project.id.clone();
    let on_project = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectSelected(id.clone())))
    };
    html! {
        <div class="mb-1">
            <button type="button" onclick={on_project}
                class={classes!(
                    "w-full","rounded-xl","px-3","py-2.5","text-left","transition",
                    if selected { "bg-white/12 ring-1 ring-inset ring-white/15" } else { "hover:bg-white/[0.06]" }
                )}>
                <div class="flex items-start justify-between gap-3">
                    <span class="min-w-0">
                        <span class="block truncate font-serif text-[18px] font-bold text-white/95">{ project.name.clone() }</span>
                        <span class="mt-0.5 block truncate text-[11px] font-light text-white/45">
                            { project_context(project, projects) }
                        </span>
                    </span>
                    <span class="shrink-0 text-[10px] font-light uppercase tracking-[0.1em] text-white/45">
                        { format!("{}%", project_progress(projects, &project.id)) }
                    </span>
                </div>
            </button>
            if selected {
                <div class="ml-4 border-l border-white/10 pl-2">
                    { for root_items(projects, &project.id).into_iter().map(|item| work_tree_row(projects, item, 0, on_msg)) }
                </div>
            }
        </div>
    }
}

fn work_tree_row(
    projects: &PortalProjectsPage,
    item: &PortalProjectWorkItem,
    depth: usize,
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
        <>
            <button type="button" onclick={on_select}
                style={format!("padding-left:{}px", 8 + depth * 14)}
                class={classes!(
                    "flex","w-full","items-center","gap-2","rounded-lg","py-2","pr-2","text-left","transition",
                    if selected { "bg-white/10 text-white" } else { "text-white/68 hover:bg-white/[0.05] hover:text-white" }
                )}>
                <span class={classes!("h-2","w-2","shrink-0","rounded-full",status_dot(&item.status))}></span>
                <span class="min-w-0 flex-1">
                    <span class="block truncate text-[13px] font-medium">{ item.title.clone() }</span>
                    <span class="block truncate text-[10px] font-light text-white/38">
                        { work_meta(item) }
                    </span>
                </span>
            </button>
            { for children.into_iter().map(|child| work_tree_row(projects, child, depth + 1, on_msg)) }
        </>
    }
}

fn catchup_nav(projects: &PortalProjectsPage, on_msg: &Callback<Msg>) -> Html {
    let mut items = projects
        .items
        .iter()
        .filter(|item| matches!(item.status.as_str(), "open" | "doing"))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.due_at
            .as_deref()
            .unwrap_or("9999")
            .cmp(right.due_at.as_deref().unwrap_or("9999"))
            .then_with(|| left.title.cmp(&right.title))
    });
    html! {
        <div>
            { for items.into_iter().map(|item| {
                let id = item.id.clone();
                let project_id = item.project_id.clone();
                let project = project_id.as_deref()
                    .and_then(|project_id| projects.projects.iter().find(|project| project.id == project_id));
                let on_msg = on_msg.clone();
                html! {
                    <button type="button"
                        onclick={Callback::from(move |_: MouseEvent| {
                            if let Some(project_id) = project_id.as_ref() {
                                on_msg.emit(Msg::ProjectSelected(project_id.clone()));
                                on_msg.emit(Msg::ProjectNodeSelected(Some(id.clone())));
                            }
                        })}
                        class="mb-1 w-full rounded-xl px-3 py-2.5 text-left text-white/75 transition hover:bg-white/[0.07] hover:text-white">
                        <span class="block text-[13px] font-medium">{ item.title.clone() }</span>
                        <span class="mt-0.5 block truncate text-[10px] font-light text-white/40">
                            { format!("{} · {}", project.map(|p| p.name.as_str()).unwrap_or("Project"), work_meta(item)) }
                        </span>
                    </button>
                }
            }) }
        </div>
    }
}

fn center_panel(
    model: &crate::model::Model,
    projects: &PortalProjectsPage,
    on_msg: &Callback<Msg>,
) -> Html {
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
            { project_tabs(projects, on_msg) }
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
    model: &crate::model::Model,
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
        <header class="shrink-0 border-b border-[var(--portal-panel-border)] px-4 py-3">
            <div class="flex flex-wrap items-start justify-between gap-3">
                <div class="min-w-0">
                    <div class="text-[10px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">
                        { project.project_type.clone().unwrap_or_else(|| "Project".into()) }
                    </div>
                    <h1 class="mt-1 truncate font-serif text-2xl font-light text-[var(--portal-navy)]">{ project.name.clone() }</h1>
                    <p class="mt-1 truncate text-xs font-light text-black/45">{ project_context(project, projects) }</p>
                    if let Some(error) = model.error.as_ref() {
                        <p class="mt-1 text-xs text-red-700">{ error.clone() }</p>
                    }
                </div>
                <div class="flex items-center gap-3">
                    <span class="text-sm font-light text-[var(--portal-blue-gray)]">
                        { format!("{}%", project_progress(projects, &project.id)) }
                    </span>
                    <select value={status} onchange={onchange} disabled={projects.saving}
                        aria-label="Project status"
                        class="rounded-full bg-white/55 px-3 py-1.5 text-[10px] font-medium uppercase tracking-[0.1em] text-[var(--portal-navy-soft)] outline-none disabled:opacity-40">
                        <option value="open">{"Open"}</option>
                        <option value="doing">{"In progress"}</option>
                        <option value="done">{"Complete"}</option>
                        <option value="archived">{"Archived"}</option>
                    </select>
                    <button type="button" disabled=true
                        title="The existing playbook-instantiation command has not yet been moved into the Rust transaction boundary."
                        class="rounded-full bg-[var(--portal-navy)] px-3.5 py-2 text-[11px] font-medium text-white opacity-35">
                        {"New Project"}
                    </button>
                </div>
            </div>
        </header>
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
        <div class="flex shrink-0 gap-1 overflow-x-auto border-b border-[var(--portal-panel-border)] px-3 py-2">
            { for TABS.iter().map(|(key, label)| {
                let active = projects.active_view == *key;
                let key_string = (*key).to_string();
                let on_msg = on_msg.clone();
                html! {
                    <button type="button"
                        onclick={Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectViewSelected(key_string.clone())))}
                        class={classes!(
                            "rounded-full","px-3","py-1.5","text-[10px]","font-medium","uppercase","tracking-[0.1em]","transition",
                            if active { "bg-[var(--portal-navy)] text-white" } else { "text-[var(--portal-navy-soft)] hover:bg-white/50" }
                        )}>
                        { *label }
                    </button>
                }
            }) }
        </div>
    }
}

fn active_view(
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    match projects.active_view.as_str() {
        "timeline" => timeline_view(projects, project),
        "calendar" => calendar_view(projects, project),
        "financials" => placeholder_view(
            "Financials",
            "No project-scoped accounting read model is attached to the Rust workspace yet.",
        ),
        "documents" => documents_view(projects, project),
        "activity" => placeholder_view(
            "Activity",
            "Project-scoped activity is not yet exposed by the Rust Project/WBS transport. No synthetic history is shown.",
        ),
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
        <div class="h-full min-h-0 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30">
            <div class="grid grid-cols-[minmax(0,1fr)_130px_110px_110px] gap-3 border-b border-[var(--portal-panel-border)] px-3 py-2 text-[9px] font-semibold uppercase tracking-[0.12em] text-black/35">
                <span>{"Work item"}</span><span>{"Owner"}</span><span>{"Due"}</span><span>{"Status"}</span>
            </div>
            if roots.is_empty() {
                <p class="px-4 py-10 text-center text-sm font-light text-black/40">{"No WBS items are attached to this project."}</p>
            } else {
                { for roots.into_iter().map(|item| work_plan_row(projects, item, 0, on_msg)) }
            }
        </div>
    }
}

fn work_plan_row(
    projects: &PortalProjectsPage,
    item: &PortalProjectWorkItem,
    depth: usize,
    on_msg: &Callback<Msg>,
) -> Html {
    let selected = projects.selected_node_id.as_deref() == Some(item.id.as_str());
    let id = item.id.clone();
    let on_select = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectNodeSelected(Some(id.clone()))))
    };
    html! {
        <>
            <button type="button" onclick={on_select}
                class={classes!(
                    "grid","w-full","grid-cols-[minmax(0,1fr)_130px_110px_110px]","items-center","gap-3",
                    "border-b","border-[var(--portal-panel-border)]/65","px-3","py-2.5","text-left","transition",
                    if selected { "bg-[var(--portal-gold)]/8" } else { "hover:bg-white/40" }
                )}>
                <span class="flex min-w-0 items-center gap-2" style={format!("padding-left:{}px", depth * 18)}>
                    <span class={classes!("h-2","w-2","shrink-0","rounded-full",status_dot(&item.status))}></span>
                    <span class="truncate text-[13px] font-medium text-[var(--portal-navy)]">{ item.title.clone() }</span>
                </span>
                <span class="truncate text-[11px] font-light text-black/50">{ item.owner.clone().unwrap_or_else(|| "—".into()) }</span>
                <span class="text-[11px] font-light text-black/45">{ due_label(item.due_at.as_deref()) }</span>
                <span class="text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--portal-navy-soft)]">{ status_label(&item.status) }</span>
            </button>
            { for child_items(projects, &item.id).into_iter().map(|child| work_plan_row(projects, child, depth + 1, on_msg)) }
        </>
    }
}

fn timeline_view(projects: &PortalProjectsPage, project: &PortalProject) -> Html {
    let widget = timeline_widget_json(projects, project);
    if widget.get("tasks").and_then(|value| value.as_array()).is_none_or(|tasks| tasks.len() <= 1) {
        return placeholder_view("Timeline", "No dated WBS work exists yet. The Rust view does not invent a schedule.");
    }
    html! {
        <div id="project-timeline-island"
            data-project-widget={widget.to_string()}
            class="h-full min-h-[26rem] overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)]">
            <div class="flex h-full items-center justify-center text-sm font-light text-black/40">{"Loading timeline…"}</div>
        </div>
    }
}

fn calendar_view(projects: &PortalProjectsPage, project: &PortalProject) -> Html {
    let widget = calendar_widget_json(projects, project);
    if widget
        .get("events")
        .and_then(|value| value.as_array())
        .is_none_or(|events| events.is_empty())
    {
        return placeholder_view("Calendar", "No real WBS due dates exist for this project.");
    }
    html! {
        <div id="project-calendar-island"
            data-project-widget={widget.to_string()}
            class="h-full min-h-[28rem] overflow-hidden">
            <div class="flex h-full items-center justify-center text-sm font-light text-black/40">{"Loading calendar…"}</div>
        </div>
    }
}

fn documents_view(projects: &PortalProjectsPage, project: &PortalProject) -> Html {
    let property_ids = project_property_ids(projects, project);
    let docs = projects
        .documents
        .iter()
        .filter(|document| {
            document
                .property_id
                .as_ref()
                .is_some_and(|id| property_ids.contains(id))
        })
        .collect::<Vec<_>>();
    html! {
        <div class="h-full min-h-0 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30">
            <div class="grid grid-cols-[minmax(0,1fr)_120px_100px] gap-3 border-b border-[var(--portal-panel-border)] px-3 py-2 text-[9px] font-semibold uppercase tracking-[0.12em] text-black/35">
                <span>{"Document"}</span><span>{"State"}</span><span>{"Issued"}</span>
            </div>
            if docs.is_empty() {
                <p class="px-4 py-10 text-center text-sm font-light text-black/40">{"No Cabinet documents are linked through this project's property context."}</p>
            } else {
                { for docs.into_iter().map(document_row) }
            }
        </div>
    }
}

fn document_row(document: &PortalProjectDocument) -> Html {
    html! {
        <a href={format!("/portal/documents/{}", document.id)}
            class="grid grid-cols-[minmax(0,1fr)_120px_100px] items-center gap-3 border-b border-[var(--portal-panel-border)]/65 px-3 py-3 transition hover:bg-white/40">
            <span class="min-w-0">
                <span class="block truncate text-[13px] font-medium text-[var(--portal-navy)]">{ document.title.clone() }</span>
                <span class="mt-0.5 block truncate text-[10px] font-light text-black/40">
                    { document.template_id.clone().unwrap_or_else(|| "Vault".into()) }
                </span>
            </span>
            <span class="text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--portal-navy-soft)]">{ document.state.clone() }</span>
            <span class="text-[11px] font-light text-black/45">
                { document.issued_version.map(|version| format!("v{version}")).unwrap_or_else(|| "—".into()) }
            </span>
        </a>
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

fn selected_work_editor(
    _model: &crate::model::Model,
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
                            <option value="open">{"Not started"}</option>
                            <option value="doing">{"In progress"}</option>
                            <option value="done">{"Complete"}</option>
                            <option value="dismissed">{"Dismissed"}</option>
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

fn catchup_center(
    _model: &crate::model::Model,
    projects: &PortalProjectsPage,
    on_msg: &Callback<Msg>,
) -> Html {
    let mut items = projects
        .items
        .iter()
        .filter(|item| matches!(item.status.as_str(), "open" | "doing"))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.due_at
            .as_deref()
            .unwrap_or("9999")
            .cmp(right.due_at.as_deref().unwrap_or("9999"))
            .then_with(|| left.title.cmp(&right.title))
    });
    html! {
        <section class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
            <header class="shrink-0 border-b border-[var(--portal-panel-border)] px-4 py-3">
                <div class="text-[10px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">{"Catch-Up"}</div>
                <h1 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">{"Open project work"}</h1>
                <p class="mt-1 text-xs font-light text-black/45">{"Real WBS work across Projects, ordered by persisted due date. No sample rows."}</p>
            </header>
            <div class="min-h-0 flex-1 overflow-y-auto p-3">
                { for items.into_iter().map(|item| {
                    let id = item.id.clone();
                    let project_id = item.project_id.clone();
                    let project_name = project_id.as_deref()
                        .and_then(|id| projects.projects.iter().find(|project| project.id == id))
                        .map(|project| project.name.clone())
                        .unwrap_or_else(|| "Project".into());
                    let on_msg = on_msg.clone();
                    html! {
                        <button type="button"
                            onclick={Callback::from(move |_: MouseEvent| {
                                if let Some(project_id) = project_id.as_ref() {
                                    on_msg.emit(Msg::ProjectSelected(project_id.clone()));
                                    on_msg.emit(Msg::ProjectNodeSelected(Some(id.clone())));
                                }
                            })}
                            class="mb-2 grid w-full grid-cols-[minmax(0,1fr)_160px_120px] items-center gap-3 rounded-xl border border-[var(--portal-panel-border)] bg-white/35 px-4 py-3 text-left transition hover:bg-white/55">
                            <span>
                                <span class="block text-[13px] font-medium text-[var(--portal-navy)]">{ item.title.clone() }</span>
                                <span class="mt-0.5 block text-[10px] font-light text-black/40">{ project_name }</span>
                            </span>
                            <span class="text-[11px] font-light text-black/45">{ item.owner.clone().unwrap_or_else(|| "—".into()) }</span>
                            <span class="text-[11px] font-light text-black/45">{ due_label(item.due_at.as_deref()) }</span>
                        </button>
                    }
                }) }
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
    classes!("mt-1","block","h-8","w-full","rounded-[var(--portal-tab-radius)]","border","border-[var(--portal-panel-border)]","bg-white/70","px-2.5","text-[12px]","font-light","text-black/70","outline-none")
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
    let planned = items.iter().filter(|item| item.status != "dismissed").count();
    if planned == 0 {
        return 0;
    }
    let done = items.iter().filter(|item| item.status == "done").count();
    ((done * 100) / planned) as i32
}

fn project_in_domain(
    project: &PortalProject,
    items: &[PortalProjectWorkItem],
    domain: &str,
) -> bool {
    let project_items = items
        .iter()
        .filter(|item| item.project_id.as_deref() == Some(project.id.as_str()))
        .collect::<Vec<_>>();
    match domain {
        "properties" => {
            project.property_id.is_some()
                || project.areas.iter().any(|area| area == "properties" || area == "media")
                || project_items.iter().any(|item| {
                    item.entity
                        .as_ref()
                        .is_some_and(|entity| entity.entity_type == "property")
                })
        }
        "people" => {
            project.person_id.is_some()
                || project.areas.iter().any(|area| area == "clients")
                || project_items.iter().any(|item| {
                    item.entity
                        .as_ref()
                        .is_some_and(|entity| entity.entity_type == "person")
                })
        }
        "deals" => {
            project.contract_id.is_some()
                || project.areas.iter().any(|area| area == "contracts")
                || project_items.iter().any(|item| {
                    item.entity.as_ref().is_some_and(|entity| {
                        matches!(entity.entity_type.as_str(), "contract" | "deal")
                    })
                })
        }
        "marketing" => project.areas.iter().any(|area| area == "marketing"),
        "accounting" => project.areas.iter().any(|area| area == "accounting"),
        "firm" => {
            project.areas.iter().any(|area| area == "management")
                || (project.person_id.is_none()
                    && project.property_id.is_none()
                    && project.contract_id.is_none())
        }
        _ => false,
    }
}

fn project_context(project: &PortalProject, projects: &PortalProjectsPage) -> String {
    let mut labels = Vec::new();
    if let Some(id) = project.person_id.as_ref() {
        labels.push(
            projects
                .identity_names
                .get(&format!("person:{id}"))
                .cloned()
                .unwrap_or_else(|| id.clone()),
        );
    }
    if let Some(id) = project.property_id.as_ref() {
        labels.push(
            projects
                .identity_names
                .get(&format!("property:{id}"))
                .cloned()
                .unwrap_or_else(|| id.clone()),
        );
    }
    if let Some(id) = project.contract_id.as_ref() {
        labels.push(
            projects
                .identity_names
                .get(&format!("contract:{id}"))
                .cloned()
                .unwrap_or_else(|| id.clone()),
        );
    }
    if labels.is_empty() {
        project
            .areas
            .first()
            .cloned()
            .unwrap_or_else(|| "Project".into())
    } else {
        labels.join(" · ")
    }
}

fn project_property_ids(projects: &PortalProjectsPage, project: &PortalProject) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    if let Some(id) = project.property_id.as_ref() {
        ids.insert(id.clone());
    }
    for item in project_items(projects, &project.id) {
        if let Some(entity) = item.entity.as_ref() {
            if entity.entity_type == "property" {
                ids.insert(entity.id.clone());
            }
        }
    }
    ids
}

fn work_meta(item: &PortalProjectWorkItem) -> String {
    [
        Some(status_label(&item.status).to_string()),
        Some(due_label(item.due_at.as_deref())),
        item.owner.clone(),
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.is_empty() && value != "—")
    .collect::<Vec<_>>()
    .join(" · ")
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

fn timeline_widget_json(projects: &PortalProjectsPage, project: &PortalProject) -> serde_json::Value {
    let dated = project_items(projects, &project.id)
        .into_iter()
        .filter(|item| item.due_at.as_deref().and_then(|value| value.get(0..10)).is_some())
        .collect::<Vec<_>>();
    let mut id_map = BTreeMap::new();
    for (index, item) in dated.iter().enumerate() {
        id_map.insert(item.id.clone(), index as i64 + 2);
    }
    let mut tasks = vec![json!({
        "id": 1,
        "text": project.name,
        "type": "summary",
        "parent": 0,
        "open": true,
        "progress": project_progress(projects, &project.id),
    })];
    for item in dated {
        let id = id_map.get(&item.id).copied().unwrap_or(2);
        let parent = item
            .parent_id
            .as_ref()
            .and_then(|parent| id_map.get(parent))
            .copied()
            .unwrap_or(1);
        tasks.push(json!({
            "id": id,
            "text": item.title,
            "type": "task",
            "parent": parent,
            "start": date_value(item.due_at.as_deref()),
            "duration": 1,
            "progress": match item.status.as_str() {
                "done" => 100,
                "doing" => 60,
                _ => 0,
            },
            "details": item.notes,
        }));
    }
    json!({ "tasks": tasks, "links": [] })
}

fn calendar_widget_json(projects: &PortalProjectsPage, project: &PortalProject) -> serde_json::Value {
    let events = project_items(projects, &project.id)
        .into_iter()
        .filter_map(|item| {
            let due = item.due_at.as_deref()?;
            Some(json!({
                "id": item.id,
                "title": item.title,
                "startAt": due,
                "endAt": null,
                "allDay": true,
                "personId": null,
                "personName": null,
                "propertyName": project.property_id.as_ref()
                    .and_then(|id| projects.identity_names.get(&format!("property:{id}"))),
                "kind": "other",
                "source": "wbs",
            }))
        })
        .collect::<Vec<_>>();
    json!({ "events": events })
}
