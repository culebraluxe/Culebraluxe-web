use std::collections::BTreeSet;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use yew::prelude::*;

use crate::model::{PortalProject, PortalProjectWorkItem, PortalProjectsPage};
use crate::timeline::{self, TimelineRange, TimelineSpec};

use super::super::{Msg, Vm};

struct TimelineRow<'a> {
    item: &'a PortalProjectWorkItem,
    depth: usize,
    has_children: bool,
    deadline_start: Option<NaiveDate>,
    deadline_end: Option<NaiveDate>,
}

struct HeaderSegment {
    left: i64,
    width: i64,
    label: String,
}

pub(super) fn view(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    let rows = visible_rows(projects, project);
    let timeline_spec = timeline::spec(&projects.timeline_mode);

    let mut range_values = project_items(projects, &project.id)
        .into_iter()
        .filter_map(|item| item.due_at.as_deref())
        .collect::<Vec<_>>();
    if let Some(value) = project.starts_at.as_deref() {
        range_values.push(value);
    }
    if let Some(value) = project.ends_at.as_deref() {
        range_values.push(value);
    }
    let range = timeline::range(
        range_values,
        &projects.calendar_today,
        &projects.timeline_mode,
    );
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
                <div style={format!("min-width: {}px;", 402 + timeline_width)}>
                    { header(projects, range, timeline_spec, timeline_width, &segments) }
                    { project_row(projects, project, project_bounds, range, timeline_spec, timeline_width, &grid_style) }
                    if rows.is_empty() {
                        <div class="grid h-16 items-center border-t border-[var(--portal-panel-border)]/60 bg-white/35 text-sm font-light text-black/40"
                            style={format!("grid-template-columns: 320px 82px {timeline_width}px;")}>
                            <div class="sticky left-0 z-10 bg-[var(--portal-soft-bg)] px-4">{"No WBS items are attached to this project."}</div>
                            <div class="sticky left-[320px] z-10 h-full bg-[var(--portal-soft-bg)]"></div>
                            <div></div>
                        </div>
                    } else {
                        { for rows.into_iter().map(|row| task_row(
                            model,
                            projects,
                            row,
                            range,
                            timeline_spec,
                            timeline_width,
                            &days,
                            &grid_style,
                            on_msg,
                        )) }
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
                        {"Due dates are milestones. Parent bars summarize descendant deadlines; no duration is invented."}
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
            </div>
            <div class="mt-1 flex items-center justify-between text-[9px] font-medium uppercase tracking-[0.08em] text-[var(--portal-blue-gray)]">
                <span>{"◆ WBS deadline · bar = project/child deadline envelope"}</span>
                if pending.is_some() {
                    <span class="text-[var(--portal-gold-muted)]">{"Saving deadline through WBS…"}</span>
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
) -> Html {
    html! {
        <div
            class="sticky top-0 z-40 grid h-[52px] border-b border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)]/95 backdrop-blur"
            style={format!("grid-template-columns: 320px 82px {timeline_width}px;")}
        >
            <div class="sticky left-0 z-50 flex items-center border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-3 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                {"Work item"}
            </div>
            <div class="sticky left-[320px] z-50 flex items-center justify-end border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-3 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                {"Due"}
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
            style={format!("grid-template-columns: 320px 82px {timeline_width}px;")}
        >
            <div class="sticky left-0 z-20 flex items-center gap-2 border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-3">
                <span class="flex h-5 w-5 items-center justify-center text-[var(--portal-gold-muted)]">{"◆"}</span>
                <span class="min-w-0 flex-1 truncate font-serif text-[15px] font-medium text-[var(--portal-navy)]">
                    { project.name.clone() }
                </span>
                <span class="text-[9px] font-medium text-[var(--portal-blue-gray)]">{ format!("{progress}%") }</span>
            </div>
            <div class="sticky left-[320px] z-20 flex items-center justify-end border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-3 text-[9px] font-light text-[var(--portal-blue-gray)]">
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
    row: TimelineRow<'_>,
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
            style={format!("grid-template-columns: 320px 82px {timeline_width}px;")}
        >
            <button
                type="button"
                onclick={select}
                class={classes!(
                    "sticky","left-0","z-20","flex","min-w-0","items-center","border-r","border-[var(--portal-panel-border)]","px-2","text-left",
                    if selected { "bg-[var(--portal-soft-bg)] ring-1 ring-inset ring-[var(--portal-gold)]/35" } else { "bg-[var(--portal-soft-bg)] hover:bg-white" }
                )}
            >
                <span style={format!("width:{indent}px")} class="shrink-0"></span>
                if let Some(toggle) = toggle {
                    <span
                        role="button"
                        tabindex="0"
                        onclick={toggle}
                        class="mr-1 flex h-6 w-5 shrink-0 items-center justify-center text-[12px] text-[var(--portal-blue-gray)]"
                        aria-label={if collapsed { "Expand work item" } else { "Collapse work item" }}
                    >
                        { if collapsed { "›" } else { "⌄" } }
                    </span>
                } else {
                    <span class="mr-1 w-5 shrink-0"></span>
                }
                <span class={classes!("mr-2","h-2.5","w-2.5","shrink-0","rounded-full",super::status_dot(&item.status))}></span>
                <span class="min-w-0 flex-1">
                    <span class="block truncate text-[12px] font-medium text-[var(--portal-navy)]">{ item.title.clone() }</span>
                    <span class="block truncate text-[9px] font-light uppercase tracking-[0.06em] text-black/35">{ item.category.clone() }</span>
                </span>
            </button>
            <div class="sticky left-[320px] z-20 flex items-center justify-end border-r border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] px-3 text-[9px] font-light text-[var(--portal-blue-gray)]">
                { super::due_label(item.due_at.as_deref()) }
            </div>
            <div class="relative" style={grid_style.to_owned()}>
                if row.has_children {
                    if let (Some(start), Some(end)) = (row.deadline_start, row.deadline_end) {
                        { summary_bar(
                            start,
                            end,
                            range,
                            spec,
                            "Descendant deadline span",
                            timeline::progress(&item.status),
                            false,
                        ) }
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
                } else if !row.has_children {
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
                    Callback::from(move |event: web_sys::DragEvent| {
                        event.prevent_default();
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
                        title={format!("Move deadline to {date_text}")}
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
    let width = timeline::width_between(start, end, range, spec).max(14) - 4;
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
    for root in super::root_items(projects, &project.id) {
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
    let children = super::child_items(projects, &item.id);
    let bounds = deadline_bounds(projects, item);
    out.push(TimelineRow {
        item,
        depth,
        has_children: !children.is_empty(),
        deadline_start: bounds.map(|value| value.0),
        deadline_end: bounds.map(|value| value.1),
    });
    if projects.timeline_collapsed_items.contains(&item.id) {
        return;
    }
    for child in children {
        append_row(projects, child, depth + 1, visited, out);
    }
}

fn deadline_bounds(
    projects: &PortalProjectsPage,
    item: &PortalProjectWorkItem,
) -> Option<(NaiveDate, NaiveDate)> {
    let mut values = Vec::new();
    collect_deadlines(projects, item, &mut BTreeSet::new(), &mut values);
    let mut iter = values.into_iter();
    let first = iter.next()?;
    let mut min = first;
    let mut max = first;
    for value in iter {
        min = min.min(value);
        max = max.max(value);
    }
    Some((min, max))
}

fn collect_deadlines(
    projects: &PortalProjectsPage,
    item: &PortalProjectWorkItem,
    visited: &mut BTreeSet<String>,
    out: &mut Vec<NaiveDate>,
) {
    if !visited.insert(item.id.clone()) {
        return;
    }
    if let Some(due) = item.due_at.as_deref().and_then(timeline::date) {
        out.push(due);
    }
    for child in super::child_items(projects, &item.id) {
        collect_deadlines(projects, child, visited, out);
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
