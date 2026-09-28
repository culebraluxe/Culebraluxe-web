use std::collections::BTreeSet;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use yew::prelude::*;

use crate::model::{PortalProject, PortalProjectWorkItem, PortalProjectsPage};
use crate::timeline::{self, ProjectedSchedule, ProjectedTask, TimelineRange, TimelineSpec};

use super::super::{Msg, Vm};

struct TimelineRow<'a> {
    item: &'a PortalProjectWorkItem,
    depth: usize,
    has_children: bool,
}

struct HeaderSegment {
    left: i64,
    width: i64,
    label: String,
}

const GRID_WIDTH: i64 = 534;

fn columns(timeline_width: i64) -> String {
    format!("grid-template-columns: 270px 100px 64px 100px {timeline_width}px;")
}

pub(super) fn view(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    let rows = visible_rows(projects, project);
    let schedule = timeline::project_schedule(projects, &project.id);
    let timeline_spec = timeline::spec(&projects.timeline_mode);
    let range = if let Some(focus) = projects.timeline_focus_date.as_deref().and_then(timeline::date) {
        TimelineRange {
            start: focus - Duration::days(timeline_spec.margin_before),
            end: focus + Duration::days(timeline_spec.margin_after),
        }
    } else {
        let mut range_values = project_items(projects, &project.id)
            .into_iter()
            .flat_map(|item| [item.due_at.as_deref(), item.planned_start.as_deref(), item.planned_finish.as_deref()])
            .flatten()
            .collect::<Vec<_>>();
        range_values.extend(project.starts_at.as_deref());
        range_values.extend(project.ends_at.as_deref());
        timeline::range(range_values, &projects.calendar_today, &projects.timeline_mode)
    };
    let timeline_width = range.width(timeline_spec).max(640);
    let days = timeline::days(range);
    let segments = header_segments(range, timeline_spec, &projects.timeline_mode);
    let project_bounds = project_span(projects, project);
    let grid_days = if projects.timeline_mode == "day" {
        1
    } else {
        7
    };
    let grid_px = timeline_spec.pixels_per_day * grid_days;
    let grid_style = format!(
        "background-image: linear-gradient(to right, var(--portal-panel-border) 1px, transparent 1px); background-size: {grid_px}px 100%;"
    );

    html! {
        <section class="flex h-full min-h-0 flex-col overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30">
            { toolbar(model, projects, on_msg) }
            <div class="min-h-0 flex-1 overflow-auto">
                <div class="relative" style={format!("min-width: {}px;", GRID_WIDTH + timeline_width)}>
                    { header(projects, range, timeline_spec, timeline_width, &segments, on_msg) }
                    { project_row(projects, project, project_bounds, range, timeline_spec, timeline_width, &grid_style) }
                    if rows.is_empty() {
                        <div class="grid h-16 items-center border-t border-[var(--portal-panel-border)]/60 bg-white/35 text-sm font-light text-black/40"
                            style={columns(timeline_width)}>
                            <div class="sticky left-0 z-10 bg-[var(--portal-soft-bg)] px-4">{"No WBS items are attached to this project."}</div>
                            <div class="col-span-3 bg-[var(--portal-soft-bg)]"></div>
                            <div></div>
                        </div>
                    } else {
                        { for rows.iter().map(|row| task_row(
                            model,
                            projects,
                            row,
                            schedule.tasks.iter().find(|task| task.id == row.item.id),
                            range,
                            timeline_spec,
                            timeline_width,
                            &days,
                            &grid_style,
                            on_msg,
                        )) }
                        { link_overlay(&schedule, &rows, range, timeline_spec, timeline_width) }
                    }
                </div>
            </div>
        </section>
    }
}

fn toolbar(model: &Vm<'_>, projects: &PortalProjectsPage, on_msg: &Callback<Msg>) -> Html {
    let pending = model
        .timeline_pending
        .map(|pending| pending.item_id.as_str());

    html! {
        <div class="shrink-0 border-b border-[var(--portal-panel-border)] bg-white/35 px-3 py-2">
            <div class="flex flex-wrap items-center gap-2">
                <div class="min-w-[220px] flex-1">
                    <p class="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-gold-muted)]">
                        {"Native WBS timeline"}
                    </p>
                    <p class="mt-0.5 text-[11px] font-light text-[var(--portal-blue-gray)]">
                        {"Planned spans are bars; due dates are separate milestones. Undated work stays unscheduled."}
                    </p>
                </div>
                <nav aria-label="Timeline scale" class="flex h-8 overflow-hidden rounded-md border border-[var(--portal-panel-border)] bg-white/65">
                    { for [("day", "Day"), ("week", "Week"), ("month", "Month")].into_iter().map(|(key, label)| {
                        let active = projects.timeline_mode == key;
                        let mode = key.to_owned();
                        let on_msg = on_msg.clone();
                        html! {
                            <button
                                type="button"
                                aria-current={active.then_some("page")}
                                onclick={Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectTimelineModeSelected(mode.clone())))}
                                class={classes!(
                                    "px-3","text-[10px]","font-medium","transition",
                                    active.then_some("bg-[var(--portal-navy)] text-white")
                                )}
                            >
                                { label }
                            </button>
                        }
                    }) }
                </nav>
                <nav aria-label="Timeline navigation" class="flex items-center gap-1">
                    <button type="button" aria-label="Previous period" onclick={on_msg.reform(|_: MouseEvent| Msg::ProjectTimelineFocusShifted(-1))} class="rounded border border-[var(--portal-panel-border)] px-2 py-1 text-sm">{"‹"}</button>
                    <button type="button" onclick={on_msg.reform(|_: MouseEvent| Msg::ProjectTimelineToday)} class="rounded border border-[var(--portal-panel-border)] px-2 py-1 text-xs">{"Today"}</button>
                    <button type="button" aria-label="Next period" onclick={on_msg.reform(|_: MouseEvent| Msg::ProjectTimelineFocusShifted(1))} class="rounded border border-[var(--portal-panel-border)] px-2 py-1 text-sm">{"›"}</button>
                    <input type="date" aria-label="Jump to date" value={projects.timeline_focus_date.clone().unwrap_or_default()}
                        oninput={on_msg.reform(|event: InputEvent| Msg::ProjectTimelineFocusChanged(crate::app::template::input_value(&event)))}
                        class="h-8 rounded border border-[var(--portal-panel-border)] bg-white/65 px-1 text-xs" />
                </nav>
            </div>
            <div class="mt-1 flex items-center justify-between text-[9px] font-medium uppercase tracking-[0.08em] text-[var(--portal-blue-gray)]">
                <span>{"◆ Due date · Solid bar = planned dates (color shows status) · Outline = span of the steps inside it · % = steps completed"}</span>
                if pending.is_some() {
                    <span class="text-[var(--portal-gold-muted)]">{"Saving through WBS…"}</span>
                }
            </div>
        </div>
    }
}

fn header(
    projects: &PortalProjectsPage,
    range: TimelineRange,
    spec: TimelineSpec,
    timeline_width: i64,
    segments: &[HeaderSegment],
    on_msg: &Callback<Msg>,
) -> Html {
    html! {
        <div
            class="sticky top-0 z-40 grid h-[52px] border-b border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)]/95 backdrop-blur"
            style={columns(timeline_width)}
        >
            <div class="sticky left-0 z-50 flex items-center border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-3 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                { sort_heading(projects, "title", "Work item", on_msg) }
            </div>
            <div class="sticky left-[270px] z-50 flex items-center justify-end border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-2 text-xs font-semibold text-[var(--portal-blue-gray)]">
                { sort_heading(projects, "start", "Start", on_msg) }
            </div>
            <div class="sticky left-[370px] z-50 flex items-center justify-end border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-2 text-xs font-semibold text-[var(--portal-blue-gray)]">
                { sort_heading(projects, "days", "Days", on_msg) }
            </div>
            <div class="sticky left-[434px] z-50 flex items-center justify-end border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-2 text-xs font-semibold text-[var(--portal-blue-gray)]">
                // Not sortable, but set like the sortable headings beside it.
                <span class="px-1 py-1 uppercase tracking-[0.06em]">{"Due"}</span>
            </div>
            <div class="relative overflow-hidden">
                { for segments.iter().map(|segment| html! {
                    <div
                        class="absolute inset-y-0 flex items-center border-r border-[var(--portal-panel-border)] px-2 text-[9px] font-semibold uppercase tracking-[0.08em] text-[var(--portal-navy)]"
                        style={format!("left:{}px;width:{}px;", segment.left, segment.width)}
                    >
                        { segment.label.clone() }
                    </div>
                }) }
                { today_line(projects, range, spec, 52) }
            </div>
        </div>
    }
}

fn sort_heading(projects: &PortalProjectsPage, key: &'static str, label: &'static str, on_msg: &Callback<Msg>) -> Html {
    let active = projects.timeline_sort_key == key;
    let arrow = if active { if projects.timeline_sort_desc { " ↓" } else { " ↑" } } else { "" };
    html! { <button type="button" aria-label={format!("Sort by {label}")}
        onclick={on_msg.reform(move |_: MouseEvent| Msg::ProjectTimelineSortSelected(key.into()))}
        class="rounded px-1 py-1 text-left text-xs font-semibold uppercase tracking-[0.06em] hover:text-[var(--portal-gold-muted)]">
        { format!("{label}{arrow}") }
    </button> }
}

fn project_row(
    projects: &PortalProjectsPage,
    project: &PortalProject,
    bounds: Option<(NaiveDate, NaiveDate, bool)>,
    range: TimelineRange,
    spec: TimelineSpec,
    timeline_width: i64,
    grid_style: &str,
) -> Html {
    let progress = super::project_progress(projects, &project.id);
    html! {
        <div
            class="grid h-[46px] border-b border-[var(--portal-panel-border)] bg-white/55"
            style={columns(timeline_width)}
        >
            <div class="sticky left-0 z-20 flex items-center gap-2 border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-3">
                <span class="flex h-5 w-5 items-center justify-center text-[var(--portal-gold-muted)]">{"◆"}</span>
                <span class="min-w-0 flex-1 truncate font-serif text-[15px] font-medium text-[var(--portal-navy)]">
                    { project.name.clone() }
                </span>
                <span class="text-[9px] font-medium text-[var(--portal-blue-gray)]">{ format!("{progress}%") }</span>
            </div>
            <div class="sticky left-[270px] z-20 bg-[var(--portal-soft-bg)]"></div>
            <div class="sticky left-[370px] z-20 bg-[var(--portal-soft-bg)]"></div>
            <div class="sticky left-[434px] z-20 flex items-center justify-end border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-2 text-xs font-light text-[var(--portal-blue-gray)]">
                { project.ends_at.as_deref().and_then(|value| value.get(0..10)).unwrap_or("—") }
            </div>
            <div class="relative" style={grid_style.to_owned()}>
                if let Some((start, end, canonical)) = bounds {
                    { summary_bar(
                        start,
                        end,
                        range,
                        spec,
                        if canonical { "Project schedule" } else { "Project deadline envelope" },
                        i64::from(progress),
                        true,
                    ) }
                }
                { today_line(projects, range, spec, 46) }
            </div>
        </div>
    }
}

fn task_row(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    row: &TimelineRow<'_>,
    projected: Option<&ProjectedTask>,
    range: TimelineRange,
    spec: TimelineSpec,
    timeline_width: i64,
    days: &[NaiveDate],
    grid_style: &str,
    on_msg: &Callback<Msg>,
) -> Html {
    let item = row.item;
    let selected = projects.selected_node_id.as_deref() == Some(item.id.as_str());
    let dragging = projects.timeline_dragging_item_id.as_deref() == Some(item.id.as_str());
    let pending = model
        .timeline_pending
        .is_some_and(|pending| pending.item_id == item.id);
    let indent = row.depth * 18;
    let item_id = item.id.clone();
    let select = {
        let on_msg = on_msg.clone();
        let item_id = item_id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::ProjectNodeSelected(Some(item_id.clone())))
        })
    };
    let collapsed = projects.timeline_collapsed_items.contains(&item.id);
    let toggle = if row.has_children {
        let on_msg = on_msg.clone();
        let item_id = item.id.clone();
        Some(Callback::from(move |event: MouseEvent| {
            event.stop_propagation();
            on_msg.emit(Msg::ProjectTimelineGroupToggled(item_id.clone()));
        }))
    } else {
        None
    };

    html! {
        <div
            key={item.id.clone()}
            class={classes!(
                "grid","h-[42px]","border-b","border-[var(--portal-panel-border)]/60",
                selected.then_some("bg-[var(--portal-gold)]/[0.07]"),
                pending.then_some("opacity-70")
            )}
            style={columns(timeline_width)}
        >
            <div
                class={classes!(
                    "sticky","left-0","z-20","flex","min-w-0","items-center","border-r","border-[var(--portal-panel-border)]","px-2","text-left",
                    if selected { "bg-[var(--portal-soft-bg)] ring-1 ring-inset ring-[var(--portal-gold)]/35" } else { "bg-[var(--portal-soft-bg)] hover:bg-white" }
                )}
            >
                <span style={format!("width:{indent}px")} class="shrink-0"></span>
                if let Some(toggle) = toggle {
                    <button type="button" onclick={toggle}
                        class="mr-1 flex h-8 w-6 shrink-0 items-center justify-center text-base text-[var(--portal-blue-gray)]"
                        aria-label={if collapsed { "Expand work item" } else { "Collapse work item" }}
                        aria-expanded={(!collapsed).to_string()}
                    >
                        { if collapsed { "›" } else { "⌄" } }
                    </button>
                } else {
                    <span class="mr-1 w-6 shrink-0"></span>
                }
                <button type="button" onclick={select} title={item.notes.clone()}
                    class="flex min-w-0 flex-1 items-center text-left" aria-label={format!("Select {}", item.title)}>
                    <span class={classes!("mr-2","h-2.5","w-2.5","shrink-0","rounded-full",super::status_dot(&item.status))}></span>
                    <span class="min-w-0 flex-1">
                        <span class="block truncate text-[13px] font-medium text-[var(--portal-navy)]">{ item.title.clone() }</span>
                        <span class="block truncate text-[10px] font-light uppercase tracking-[0.06em] text-black/45">{ item.category.clone() }</span>
                    </span>
                </button>
            </div>
            <div class="sticky left-[270px] z-20 flex items-center justify-end border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-2 text-xs text-[var(--portal-blue-gray)]">
                { projected.and_then(|task| task.planned.map(|(start, _)| start.to_string())).or_else(|| item.planned_start.clone()).unwrap_or_else(|| "—".into()) }
            </div>
            <div class="sticky left-[370px] z-20 flex items-center justify-end border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-2 text-xs text-[var(--portal-blue-gray)]">
                { projected.and_then(|task| task.planned.map(|(start, finish)| timeline::planned_duration_days(start, finish).to_string())).unwrap_or_else(|| "—".into()) }
            </div>
            <div class="sticky left-[434px] z-20 flex items-center justify-end border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-2 text-xs text-[var(--portal-blue-gray)]">
                { super::due_label(item.due_at.as_deref()) }
            </div>
            <div class="relative" style={grid_style.to_owned()}>
                if let Some(task) = projected {
                    if let Some((start, end)) = task.descendant_span {
                        { summary_bar(
                            start,
                            end,
                            range,
                            spec,
                            "Descendant planned span",
                            0,
                            false,
                        ) }
                    }
                    if let Some((start, finish)) = task.planned {
                        { planned_bar(item, start, finish, range, spec, on_msg) }
                    }
                }
                if let Some(due) = item.due_at.as_deref().and_then(timeline::date) {
                    { milestone(
                        projects,
                        item,
                        due,
                        range,
                        spec,
                        dragging,
                        on_msg,
                    ) }
                } else if projected.is_some_and(|task| task.partially_scheduled) {
                    <span class="absolute left-2 top-1/2 -translate-y-1/2 rounded-full border border-dashed border-[var(--portal-gold-muted)] bg-white/70 px-2 py-1 text-[8px] font-medium uppercase tracking-[0.08em] text-[var(--portal-navy)]">
                        {"Incomplete plan"}
                    </span>
                } else if !row.has_children && projected.is_none_or(|task| task.planned.is_none()) {
                    <span class="absolute left-2 top-1/2 -translate-y-1/2 rounded-full border border-dashed border-[var(--portal-panel-border)] bg-white/50 px-2 py-1 text-[8px] font-medium uppercase tracking-[0.08em] text-black/35">
                        {"Unscheduled"}
                    </span>
                }
                if dragging {
                    { drag_targets(projects, item, range, spec, days, on_msg) }
                }
                { today_line(projects, range, spec, 42) }
            </div>
        </div>
    }
}

fn planned_bar(item: &PortalProjectWorkItem, start: NaiveDate, finish: NaiveDate,
    range: TimelineRange, spec: TimelineSpec, on_msg: &Callback<Msg>) -> Html {
    let left = timeline::x(start, range, spec) + 2;
    let width = timeline::planned_bar_width(start, finish, spec).max(10) - 4;
    let id = item.id.clone();
    let drag_id = id.clone();
    let drag_start = on_msg.reform(move |event: web_sys::DragEvent| {
        if let Some(data) = event.data_transfer() {
            let _ = data.set_data("text/plain", &drag_id);
            data.set_effect_allowed("move");
        }
        Msg::ProjectTimelinePlannedDragStarted(drag_id.clone())
    });
    let label = format!("{} planned {start} through {finish}; status {}; select or drag to reschedule", item.title, item.status);
    html! {
        <button type="button" draggable="true" ondragstart={drag_start}
            ondragend={on_msg.reform(|_: web_sys::DragEvent| Msg::ProjectTimelineDragEnded)}
            onclick={on_msg.reform(move |_: MouseEvent| Msg::ProjectNodeSelected(Some(id.clone())))}
            aria-label={label.clone()} title={label}
            class={classes!("absolute","top-[19px]","z-10","h-4","cursor-grab","rounded","shadow-sm","ring-1","ring-white/70",
                match item.status.as_str() {
                    "done" => "bg-[var(--portal-success)]/85",
                    "doing" => "bg-[var(--portal-gold-muted)]/85",
                    "dismissed" => "bg-black/30",
                    _ => "bg-[var(--portal-navy)]/85",
                })}
            style={format!("left:{left}px;width:{width}px;")}>
        </button>
    }
}

fn link_overlay(schedule: &ProjectedSchedule, rows: &[TimelineRow<'_>], range: TimelineRange,
    spec: TimelineSpec, width: i64) -> Html {
    let height = rows.len() as i64 * 42;
    html! {
        <svg class="pointer-events-none absolute z-20 overflow-visible" aria-label="Project dependencies"
            style={format!("left:{GRID_WIDTH}px;top:98px;width:{width}px;height:{height}px;")}
            viewBox={format!("0 0 {width} {height}")}>
            { for schedule.links.iter().filter_map(|link| {
                let source_row = rows.iter().position(|row| row.item.id == link.source_id)? as i64;
                let target_row = rows.iter().position(|row| row.item.id == link.target_id)? as i64;
                let source = schedule.tasks.iter().find(|task| task.id == link.source_id)?;
                let target = schedule.tasks.iter().find(|task| task.id == link.target_id)?;
                let ((x1,y1),(x2,y2)) = timeline::link_anchors(source, target,
                    source_row * 42 + 27, target_row * 42 + 27, range, spec)?;
                if x1 < 0 || x1 > width || x2 < 0 || x2 > width { return None; }
                let bend = if x2 > x1 + 16 { x1 + (x2-x1)/2 } else { x1 + 12 };
                Some(html! {
                    <g key={format!("{}:{}", link.source_id, link.target_id)}>
                        <path d={format!("M{x1} {y1} H{bend} V{y2} H{x2}")}
                            fill="none" stroke="var(--portal-gold-muted)" stroke-width="2" opacity="0.85" />
                        <circle cx={x2.to_string()} cy={y2.to_string()} r="3" fill="var(--portal-gold-muted)" />
                    </g>
                })
            }) }
        </svg>
    }
}

fn milestone(
    projects: &PortalProjectsPage,
    item: &PortalProjectWorkItem,
    due: NaiveDate,
    range: TimelineRange,
    spec: TimelineSpec,
    dragging: bool,
    on_msg: &Callback<Msg>,
) -> Html {
    let x = timeline::x(due, range, spec) + spec.pixels_per_day / 2 - 9;
    let item_id = item.id.clone();
    let ondragstart = {
        let on_msg = on_msg.clone();
        let id = item_id.clone();
        Callback::from(move |event: web_sys::DragEvent| {
            if let Some(data) = event.data_transfer() {
                let _ = data.set_data("text/plain", &id);
                data.set_effect_allowed("move");
            }
            on_msg.emit(Msg::ProjectTimelineDragStarted(id.clone()));
        })
    };
    let ondragend = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: web_sys::DragEvent| on_msg.emit(Msg::ProjectTimelineDragEnded))
    };
    let onclick = {
        let on_msg = on_msg.clone();
        let id = item_id;
        Callback::from(move |event: MouseEvent| {
            event.stop_propagation();
            on_msg.emit(Msg::ProjectNodeSelected(Some(id.clone())));
        })
    };
    let selected = projects.selected_node_id.as_deref() == Some(item.id.as_str());

    html! {
        <button
            type="button"
            draggable="true"
            {ondragstart}
            {ondragend}
            {onclick}
            title={format!("{} · due {}", item.title, due)}
            aria-label={format!("{} due {}; drag to reschedule", item.title, due)}
            class={classes!(
                "absolute","top-1/2","z-20","flex","h-[28px]","w-[28px]","-translate-y-1/2","items-center","justify-center","rounded-full","cursor-grab",
                dragging.then_some("opacity-40"),
                selected.then_some("ring-2 ring-[var(--portal-gold)] ring-offset-1")
            )}
            style={format!("left:{x}px;")}
        >
            <span
                class={classes!(
                    "block","h-[13px]","w-[13px]","rotate-45","border-2","border-white","shadow-sm",
                    match item.status.as_str() {
                        "done" => "bg-[var(--portal-success)]",
                        "doing" => "bg-[var(--portal-gold)]",
                        "dismissed" => "bg-black/30",
                        _ => "bg-[var(--portal-navy)]",
                    }
                )}
            ></span>
        </button>
    }
}

fn drag_targets(
    projects: &PortalProjectsPage,
    item: &PortalProjectWorkItem,
    _range: TimelineRange,
    spec: TimelineSpec,
    days: &[NaiveDate],
    on_msg: &Callback<Msg>,
) -> Html {
    let item_id = item.id.clone();
    html! {
        <div class="absolute inset-0 z-30 flex">
            { for days.iter().map(|date| {
                let date_text = date.to_string();
                let active = projects.timeline_drag_target_date.as_deref() == Some(date_text.as_str());
                let enter = {
                    let on_msg = on_msg.clone();
                    let date_text = date_text.clone();
                    Callback::from(move |event: web_sys::DragEvent| {
                        event.prevent_default();
                        on_msg.emit(Msg::ProjectTimelineDragTargetChanged(Some(date_text.clone())));
                    })
                };
                let over = Callback::from(|event: web_sys::DragEvent| event.prevent_default());
                let drop = {
                    let on_msg = on_msg.clone();
                    let date_text = date_text.clone();
                    let item_id = item_id.clone();
                    let planned = projects.timeline_drag_kind == "planned";
                    Callback::from(move |event: web_sys::DragEvent| {
                        event.prevent_default();
                        if planned {
                            on_msg.emit(Msg::ProjectTimelinePlannedMoved {
                                item_id: item_id.clone(), planned_start: date_text.clone(),
                            });
                            return;
                        }
                        let Some(due_at) = timeline::midnight_utc(&date_text) else {
                            return;
                        };
                        on_msg.emit(Msg::ProjectTimelineDueMoved {
                            item_id: item_id.clone(),
                            due_at,
                        });
                    })
                };
                html! {
                    <div
                        key={date_text.clone()}
                        ondragenter={enter}
                        ondragover={over}
                        ondrop={drop}
                        title={format!("Move {} to {date_text}", if projects.timeline_drag_kind == "planned" { "planned start" } else { "deadline" })}
                        class={classes!(
                            "h-full","shrink-0","border-r","border-transparent",
                            active.then_some("bg-[var(--portal-gold)]/20 border-[var(--portal-gold)]/40")
                        )}
                        style={format!("width:{}px;", spec.pixels_per_day)}
                    ></div>
                }
            }) }
        </div>
    }
}

fn summary_bar(
    start: NaiveDate,
    end: NaiveDate,
    range: TimelineRange,
    spec: TimelineSpec,
    title: &'static str,
    progress: i64,
    project: bool,
) -> Html {
    let left = timeline::x(start, range, spec) + 2;
    let width = timeline::planned_bar_width(start, end, spec).max(14) - 4;
    let progress = progress.clamp(0, 100);
    html! {
        <div
            class={classes!(
                "absolute","top-1/2","z-10","h-[14px]","-translate-y-1/2","overflow-hidden","rounded-[3px]","border","shadow-sm",
                if project {
                    "border-[var(--portal-navy)]/35 bg-[var(--portal-navy)]/20"
                } else {
                    "border-[var(--portal-gold)]/35 bg-[var(--portal-gold)]/15"
                }
            )}
            style={format!("left:{left}px;width:{width}px;")}
            title={format!("{title}: {start} – {end}")}
        >
            <div
                class={if project { "h-full bg-[var(--portal-navy)]/35" } else { "h-full bg-[var(--portal-gold)]/35" }}
                style={format!("width:{progress}%;")}
            ></div>
        </div>
    }
}

fn today_line(
    projects: &PortalProjectsPage,
    range: TimelineRange,
    spec: TimelineSpec,
    height: i64,
) -> Html {
    let Some(today) = timeline::date(&projects.calendar_today) else {
        return html! {};
    };
    if today < range.start || today >= range.end {
        return html! {};
    }
    let left = timeline::x(today, range, spec) + spec.pixels_per_day / 2;
    html! {
        <div
            class="pointer-events-none absolute top-0 z-10 w-px bg-[var(--portal-gold)]/60"
            style={format!("left:{left}px;height:{height}px;")}
            title="Today"
        ></div>
    }
}

fn visible_rows<'a>(
    projects: &'a PortalProjectsPage,
    project: &'a PortalProject,
) -> Vec<TimelineRow<'a>> {
    let mut out = Vec::new();
    let mut visited = BTreeSet::new();
    let mut roots = super::root_items(projects, &project.id);
    timeline::sort_siblings(&mut roots, &projects.timeline_sort_key, projects.timeline_sort_desc);
    for root in roots {
        append_row(projects, root, 0, &mut visited, &mut out);
    }
    out
}

fn append_row<'a>(
    projects: &'a PortalProjectsPage,
    item: &'a PortalProjectWorkItem,
    depth: usize,
    visited: &mut BTreeSet<String>,
    out: &mut Vec<TimelineRow<'a>>,
) {
    if !visited.insert(item.id.clone()) {
        return;
    }
    let mut children = super::child_items(projects, &item.id);
    timeline::sort_siblings(&mut children, &projects.timeline_sort_key, projects.timeline_sort_desc);
    out.push(TimelineRow {
        item,
        depth,
        has_children: !children.is_empty(),
    });
    if projects.timeline_collapsed_items.contains(&item.id) {
        return;
    }
    for child in children {
        append_row(projects, child, depth + 1, visited, out);
    }
}

fn project_span(
    projects: &PortalProjectsPage,
    project: &PortalProject,
) -> Option<(NaiveDate, NaiveDate, bool)> {
    match (
        project.starts_at.as_deref().and_then(timeline::date),
        project.ends_at.as_deref().and_then(timeline::date),
    ) {
        (Some(start), Some(end)) => Some((start.min(end), start.max(end), true)),
        _ => {
            let mut due = project_items(projects, &project.id)
                .into_iter()
                .filter_map(|item| item.due_at.as_deref().and_then(timeline::date));
            let first = due.next()?;
            let mut min = first;
            let mut max = first;
            for value in due {
                min = min.min(value);
                max = max.max(value);
            }
            Some((min, max, false))
        }
    }
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

fn header_segments(range: TimelineRange, spec: TimelineSpec, mode: &str) -> Vec<HeaderSegment> {
    match mode {
        "month" => month_segments(range, spec),
        "day" => timeline::days(range)
            .into_iter()
            .map(|date| HeaderSegment {
                left: timeline::x(date, range, spec),
                width: spec.pixels_per_day,
                label: date.format("%a %-d").to_string(),
            })
            .collect(),
        _ => week_segments(range, spec),
    }
}

fn week_segments(range: TimelineRange, spec: TimelineSpec) -> Vec<HeaderSegment> {
    let mut out = Vec::new();
    let mut start = range.start;
    while start < range.end {
        let end = (start + Duration::days(7)).min(range.end);
        out.push(HeaderSegment {
            left: timeline::x(start, range, spec),
            width: (end - start).num_days() * spec.pixels_per_day,
            label: format!("{} {}", start.format("%b"), start.day()),
        });
        start = end;
    }
    out
}

fn month_segments(range: TimelineRange, spec: TimelineSpec) -> Vec<HeaderSegment> {
    let mut out = Vec::new();
    let mut start = range.start;
    while start < range.end {
        let next_month = if start.month() == 12 {
            NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
        }
        .expect("next month is valid");
        let end = next_month.min(range.end);
        out.push(HeaderSegment {
            left: timeline::x(start, range, spec),
            width: (end - start).num_days() * spec.pixels_per_day,
            label: start.format("%b %Y").to_string(),
        });
        start = end;
    }
    out
}

#[allow(dead_code)]
fn _is_weekend(date: NaiveDate) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}
