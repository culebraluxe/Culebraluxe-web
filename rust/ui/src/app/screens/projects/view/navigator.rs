//! The Projects navigator: the lens rail, the lens's header, search and bell, and the pole → project → work tree.

use super::*;

/// The navigator, as designed: the domain rail, the domain's header and search, and the tree — poles (property, client,
/// contract, or the domain's collection) over projects over the work.
pub(super) fn navigator(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    on_msg: &Callback<Msg>,
) -> Html {
    let tree = super::super::nav::build(projects);
    let selected = super::super::nav::selected_id(
        &tree,
        projects.selected_project_id.as_deref(),
        projects.selected_node_id.as_deref(),
    );
    let opened: Vec<String> = selected
        .as_deref()
        .map(|id| super::super::nav::ancestors(&tree, id))
        .unwrap_or_default();
    let query = model.controls.query.trim().to_lowercase();
    let search = on_msg
        .reform(|event: InputEvent| Msg::QueryChanged(crate::app::template::input_value(&event)));
    let rail_button = |active: bool,
                       icon: &'static str,
                       label: &'static str,
                       overdue: usize,
                       msg: Msg| {
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
                <span class={classes!("relative", "flex", "h-10", "w-10", "items-center", "justify-center", "rounded-xl", "transition",
                    if active { "bg-black/25 text-[var(--portal-gold)] shadow-sm ring-1 ring-inset ring-white/25" } else { "bg-white/[0.07] text-white/85 group-hover:bg-white/[0.16] group-hover:text-white" })}>
                    { glyph(icon, "h-[23px] w-[23px]") }
                    if overdue > 0 {
                        { overdue_badge(overdue, "absolute -right-1.5 -top-1.5") }
                    }
                </span>
                <span class={classes!("text-center", "text-[11px]", "font-medium", "uppercase", "leading-tight", "tracking-[0.04em]",
                    if active { "text-white" } else { "text-white/70 group-hover:text-white/95" })}>{label}</span>
            </button>
        }
    };
    let ctx = NavCtx {
        page: projects,
        quiet: model.controls.quiet,
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
                { rail_button(projects.catch_up, "list-checks", "Catch-Up", 0, Msg::ProjectCatchUpToggled(true)) }
                <div class="my-1 w-[60%] border-b border-white/15" aria-hidden="true"></div>
                {for super::super::nav::DOMAINS.iter().map(|(key, label, icon)| {
                    let overdue = if model.controls.quiet { 0 } else { lens_overdue(projects, key) };
                    rail_button(!projects.catch_up && projects.active_domain == *key, icon, label, overdue, Msg::ProjectDomainSelected((*key).to_owned()))
                })}
            </div>
            <div class="flex min-h-0 min-w-0 flex-1 flex-col">
                <div class="border-b border-white/10 px-3 pb-2 pt-3">
                    <div class="flex items-center justify-between gap-2">
                        <p class="text-[14px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)]">{ super::super::nav::domain_label(&projects.active_domain) }</p>
                        <button type="button" onclick={on_msg.reform(|_: MouseEvent| Msg::QuietToggled)}
                            title={if model.controls.quiet { "Show overdue counts" } else { "Hide overdue counts" }}
                            aria-pressed={if model.controls.quiet { "false" } else { "true" }}
                            class="flex h-8 w-8 items-center justify-center rounded-lg text-white/45 transition hover:bg-white/10 hover:text-white/85">
                            { glyph(if model.controls.quiet { "bell-off" } else { "bell" }, "h-4 w-4") }
                        </button>
                    </div>
                    <label class="mt-2 flex h-11 items-center gap-2 rounded-[var(--portal-tab-radius)] border border-white/15 bg-white/10 px-3">
                        { glyph("search", "h-4 w-4 shrink-0 text-white/50") }
                        <input value={model.controls.query.clone()} oninput={search} placeholder="Find work…"
                            class="min-w-0 flex-1 bg-transparent text-[16px] font-light text-white outline-none placeholder:text-white/55" />
                    </label>
                </div>
                <div tabindex="0" onkeydown={on_msg.reform(|event: KeyboardEvent| {
                        let key = event.key();
                        if matches!(key.as_str(), "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight") {
                            event.prevent_default();
                        }
                        Msg::NavKey(key)
                    })}
                    aria-label="Projects and work — arrow keys move, left and right close and open"
                    class="min-h-0 flex-1 overflow-y-auto rounded-md px-1 pt-1.5 outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-white/20">
                    if tree.is_empty() {
                        <div class="px-3 py-8 text-sm font-light text-white/45">{"No matching projects in this perspective."}</div>
                    }
                    {for tree.iter().map(|node| nav_node(&ctx, node, 0))}
                </div>
            </div>
        </aside>
    }
}

pub(super) struct NavCtx<'a> {
    page: &'a PortalProjectsPage,
    quiet: bool,
    selected: Option<&'a str>,
    opened: &'a [String],
    query: &'a str,
    open: &'a std::collections::BTreeSet<String>,
    closed: &'a std::collections::BTreeSet<String>,
    on_msg: &'a Callback<Msg>,
}

pub(super) fn progress_bar(value: i64) -> Html {
    let clamped = value.clamp(0, 100);
    html! {
        <div class="h-1 w-11 overflow-hidden rounded-full bg-white/15">
            <div class="h-full rounded-full bg-[var(--portal-gold)]" style={format!("width: {clamped}%")}></div>
        </div>
    }
}

/// Unfinished work past its due date, in one project.
pub(super) fn project_overdue(page: &PortalProjectsPage, project_id: &str) -> usize {
    page.items
        .iter()
        .filter(|item| item.project_id.as_deref() == Some(project_id) && item.status != "dismissed")
        .filter(|item| super::super::catch_up::overdue(item, &page.calendar_today))
        .count()
}

/// Overdue work in every project this lens can show — the badge on its rail button.
pub(super) fn lens_overdue(page: &PortalProjectsPage, domain: &str) -> usize {
    page.projects
        .iter()
        .filter(|project| crate::projects::project_in_domain(project, &page.items, domain))
        .map(|project| project_overdue(page, &project.id))
        .sum()
}

pub(super) fn overdue_badge(count: usize, position: &str) -> Html {
    html! {
        <span title={format!("{count} overdue")}
            class={classes!("flex", "h-[18px]", "min-w-[18px]", "items-center", "justify-center", "rounded-full", "bg-red-600", "px-1",
                "text-[10px]", "font-semibold", "leading-none", "text-white", "shadow-sm", position.to_owned())}>
            { count }
        </span>
    }
}

/// The selected project's one-line summary under its row: what is open, what is done, and what is next. The work
/// itself is the center's Work Plan, so the tree does not repeat it.
pub(super) fn project_summary(page: &PortalProjectsPage, project_id: &str) -> Html {
    let items: Vec<&PortalProjectWorkItem> = page
        .items
        .iter()
        .filter(|item| item.project_id.as_deref() == Some(project_id) && item.status != "dismissed")
        .collect();
    let done = items.iter().filter(|item| item.status == "done").count();
    let mut open: Vec<&&PortalProjectWorkItem> =
        items.iter().filter(|item| item.status != "done").collect();
    open.sort_by(|a, b| {
        super::super::catch_up::due_key(a)
            .unwrap_or_else(|| "9999".into())
            .cmp(&super::super::catch_up::due_key(b).unwrap_or_else(|| "9999".into()))
            .then_with(|| {
                a.order
                    .unwrap_or(i32::MAX)
                    .cmp(&b.order.unwrap_or(i32::MAX))
            })
    });
    let next = open.first().map(|item| {
        let due = super::super::nav::due_label(item.due_at.as_deref());
        if due.is_empty() {
            item.title.clone()
        } else {
            format!("{}, {due}", item.title)
        }
    });
    html! {
        <p class="truncate pb-1.5 pl-[46px] pr-2 text-[12px] font-light text-white/55">
            { format!("{} open · {done} done", open.len()) }
            if let Some(next) = next {
                { " · next: " }<span class="text-white/80">{ next }</span>
            }
        </p>
    }
}

/// One row of the tree and, when open, its children: the pole (the property, person, contract or the lens's
/// collection), its projects, and each project's work.
pub(super) fn nav_node(ctx: &NavCtx<'_>, node: &super::super::nav::NavNode, depth: usize) -> Html {
    use super::super::nav::NodeKind;
    if !ctx.query.is_empty() && !node.search.contains(ctx.query) {
        return Html::default();
    }
    match node.kind {
        NodeKind::Pole => {
            let is_open = super::super::nav::is_open(
                node,
                !ctx.query.is_empty(),
                ctx.open,
                ctx.closed,
                ctx.opened,
            );
            let selected_here = node
                .children
                .iter()
                .any(|child| child.project_id == ctx.page.selected_project_id);
            let toggle = {
                let (id, open) = (node.id.clone(), is_open);
                ctx.on_msg.reform(move |event: MouseEvent| {
                    event.stop_propagation();
                    Msg::NavToggled {
                        id: id.clone(),
                        open,
                    }
                })
            };
            // Clicking the pole opens it on its project: the one already selected here, or its first.
            let target = node
                .children
                .iter()
                .find(|child| child.project_id == ctx.page.selected_project_id)
                .or_else(|| node.children.first())
                .and_then(|child| child.project_id.clone());
            let pick = match target {
                Some(project_id) => {
                    let pole_id = node.id.clone();
                    ctx.on_msg.reform(move |_: MouseEvent| Msg::PoleSelected {
                        pole_id: pole_id.clone(),
                        project_id: project_id.clone(),
                    })
                }
                None => toggle.clone(),
            };
            let overdue: usize = node
                .children
                .iter()
                .filter_map(|child| child.project_id.as_deref())
                .map(|id| project_overdue(ctx.page, id))
                .sum();
            html! {
                <>
                    <div onclick={pick} class={classes!("mt-1", "flex", "h-[52px]", "cursor-pointer", "items-center", "gap-2", "rounded-xl", "px-1",
                        if selected_here { "bg-white/10 shadow-[0_2px_12px_rgba(0,0,0,0.16)]" } else { "hover:bg-white/[0.06]" })}>
                        <button type="button" onclick={toggle} aria-label={if is_open { "Collapse" } else { "Expand" }}
                            class="flex h-6 w-4 shrink-0 items-center justify-center rounded text-white/55 transition hover:text-white">
                            { glyph("chevron-down", if is_open { "h-3.5 w-3.5 transition" } else { "h-3.5 w-3.5 -rotate-90 transition" }) }
                        </button>
                        <span class="flex h-8 w-8 shrink-0 items-center justify-center rounded-[8px] bg-white/10 text-[var(--portal-gold)] ring-1 ring-inset ring-white/15">
                            { glyph(super::super::nav::domain_icon(&node.domain), "h-4 w-4") }
                        </span>
                        <span class="min-w-0 flex-1">
                            <span class="block truncate font-serif text-[18px] font-semibold leading-tight text-white/95" title={node.label.clone()}>{ &node.label }</span>
                            if let Some(subtitle) = node.subtitle.as_deref().filter(|s| !s.is_empty()) {
                                <span class="block truncate text-[12px] font-light leading-snug text-white/55">{ subtitle }</span>
                            }
                        </span>
                        if overdue > 0 && !ctx.quiet {
                            { overdue_badge(overdue, "shrink-0") }
                        }
                        if let Some(progress) = node.progress {
                            <span class="flex shrink-0 items-center gap-1.5 pr-1">
                                { progress_bar(progress) }
                                <span class="w-8 text-right text-[12px] font-light text-white/55">{ format!("{progress}%") }</span>
                            </span>
                        }
                    </div>
                    if is_open {
                        {for node.children.iter().map(|child| nav_node(ctx, child, depth + 1))}
                    }
                </>
            }
        }
        NodeKind::Project => {
            let project_id = node.project_id.clone().unwrap_or_default();
            let selected = ctx.page.selected_project_id.as_deref() == Some(project_id.as_str())
                && !ctx.page.catch_up;
            let pick = {
                let id = project_id.clone();
                ctx.on_msg
                    .reform(move |_: MouseEvent| Msg::ProjectSelected(id.clone()))
            };
            let kind = node.meta.split(" · ").next().unwrap_or("");
            let overdue = project_overdue(ctx.page, &project_id);
            let is_open = super::super::nav::is_open(
                node,
                !ctx.query.is_empty(),
                ctx.open,
                ctx.closed,
                ctx.opened,
            );
            html! {
                <>
                    <div onclick={pick} class={classes!("flex", "h-10", "cursor-pointer", "items-center", "gap-2", "rounded-lg", "pl-[10px]", "pr-1",
                        if selected { "bg-white/15 ring-1 ring-inset ring-white/25" } else { "hover:bg-white/[0.06]" })}>
                        { chevron(ctx, node, is_open) }
                        if !kind.is_empty() { { glyph(super::super::nav::project_kind_icon(kind), "h-4 w-4 shrink-0 text-[var(--portal-gold)]") } }
                        <span class="min-w-0 flex-1 truncate text-[15px] font-light leading-tight text-white/95" title={node.label.clone()}>{ &node.label }</span>
                        if overdue > 0 && !ctx.quiet {
                            { overdue_badge(overdue, "shrink-0") }
                        }
                        if let Some(progress) = node.progress {
                            <span class="w-8 shrink-0 pr-1 text-right text-[12px] font-light text-white/55">{ format!("{progress}%") }</span>
                        }
                    </div>
                    if is_open {
                        {for node.children.iter().map(|child| nav_node(ctx, child, depth + 1))}
                    } else if selected {
                        { project_summary(ctx.page, &project_id) }
                    }
                </>
            }
        }
        NodeKind::Work => {
            let is_open = super::super::nav::is_open(
                node,
                !ctx.query.is_empty(),
                ctx.open,
                ctx.closed,
                ctx.opened,
            );
            let selected = ctx.selected == Some(node.id.as_str());
            let pick = {
                let (project_id, node_id) = (
                    node.project_id.clone().unwrap_or_default(),
                    node.work_id.clone().unwrap_or_default(),
                );
                ctx.on_msg
                    .reform(move |_: MouseEvent| Msg::NavWorkSelected {
                        project_id: project_id.clone(),
                        node_id: node_id.clone(),
                    })
            };
            let kind = node.meta.split(" · ").next().unwrap_or("");
            let icon_class = format!(
                "h-4 w-4 shrink-0 {}",
                super::super::nav::status_class(node.status.as_deref())
            );
            let indent = format!("padding-left: {}px", 12 + depth * 10);
            html! {
                <>
                    <div onclick={pick} style={indent} title={node.meta.clone()}
                        class={classes!("flex", "h-9", "cursor-pointer", "items-center", "gap-2", "rounded-md", "pr-1",
                            if selected { "bg-white/15 ring-1 ring-inset ring-white/25" } else { "hover:bg-white/[0.06]" })}>
                        { chevron(ctx, node, is_open) }
                        { glyph(super::super::nav::work_type_icon(kind, &node.label), &icon_class) }
                        <span class="min-w-0 flex-1 truncate text-[14px] font-light leading-tight text-white/90">{ &node.label }</span>
                        { work_due(ctx.page, node.work_id.as_deref()) }
                    </div>
                    if is_open {
                        {for node.children.iter().map(|child| nav_node(ctx, child, depth + 1))}
                    }
                </>
            }
        }
    }
}

/// A task's due date at the row's end: red when it is past and the work is unfinished.
pub(super) fn work_due(page: &PortalProjectsPage, work_id: Option<&str>) -> Html {
    let Some(item) = work_id.and_then(|id| page.items.iter().find(|item| item.id == id)) else {
        return Html::default();
    };
    let due = super::super::nav::due_label(item.due_at.as_deref());
    if due.is_empty() {
        return Html::default();
    }
    let late =
        item.status != "dismissed" && super::super::catch_up::overdue(item, &page.calendar_today);
    html! {
        <span class={classes!("shrink-0", "pr-1", "text-[11px]", "font-light", "tabular-nums",
            if late { "text-red-300" } else if item.status == "done" { "text-white/35" } else { "text-white/55" })}>
            { due }
        </span>
    }
}

/// The expand/collapse control of a project or a work item; a leaf keeps its place with a spacer.
pub(super) fn chevron(ctx: &NavCtx<'_>, node: &super::super::nav::NavNode, is_open: bool) -> Html {
    if node.children.is_empty() {
        return html! { <span class="w-4 shrink-0" aria-hidden="true"></span> };
    }
    let toggle = {
        let (id, open) = (node.id.clone(), is_open);
        ctx.on_msg.reform(move |event: MouseEvent| {
            event.stop_propagation();
            Msg::NavToggled {
                id: id.clone(),
                open,
            }
        })
    };
    html! {
        <button type="button" onclick={toggle} aria-label={if is_open { "Collapse" } else { "Expand" }}
            class="flex h-6 w-4 shrink-0 items-center justify-center rounded text-white/55 transition hover:text-white">
            { glyph("chevron-down", if is_open { "h-3.5 w-3.5 transition" } else { "h-3.5 w-3.5 -rotate-90 transition" }) }
        </button>
    }
}
