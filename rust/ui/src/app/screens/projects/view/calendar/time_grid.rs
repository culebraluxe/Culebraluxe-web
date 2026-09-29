//! The week and day views: the time grid, all-day cells, timed positions and events.

#[allow(unused_imports)]
use super::*;

pub(super) fn time_grid(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    chips: &[CalendarChip],
    days: Vec<String>,
    on_msg: &Callback<Msg>,
) -> Html {
    if days.is_empty() {
        return html! { <p class="p-8 text-center text-sm text-black/40">{"No calendar date selected."}</p> };
    }
    let columns = format!(
        "grid-template-columns: 64px repeat({}, minmax(120px, 1fr)); min-width: {}px;",
        days.len(),
        64 + days.len() * 120
    );
    let grid = grid(projects);

    html! {
        <div class="h-full min-h-0 overflow-auto bg-white/20">
            <div style={columns.clone()} class="sticky top-0 z-30 grid border-b border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)]/95 backdrop-blur">
                <div class="border-r border-[var(--portal-panel-border)]"></div>
                { for days.iter().map(|date| {
                    let today = *date == projects.calendar_today;
                    html! {
                        <div
                            key={format!("head-{date}")}
                            class={classes!(
                                "border-r","border-[var(--portal-panel-border)]","px-2","py-2","text-center",
                                today.then_some("bg-[var(--portal-gold)]/[0.08]")
                            )}
                        >
                            <span class="text-[10px] font-semibold uppercase tracking-[0.08em] text-[var(--portal-navy)]">
                                { crate::calendar::day_heading(date) }
                            </span>
                        </div>
                    }
                }) }
            </div>

            <div style={columns.clone()} class="grid border-b border-[var(--portal-panel-border)] bg-white/55">
                <div class="border-r border-[var(--portal-panel-border)] px-2 py-2 text-right text-[9px] font-semibold uppercase tracking-[0.08em] text-black/35">
                    {"All day"}
                </div>
                { for days.iter().map(|date| all_day_cell(model, projects, date, chips, on_msg)) }
            </div>

            <div style={columns} class="grid">
                { time_labels(grid) }
                { for days.iter().map(|date| timed_day_column(model, projects, date, chips, on_msg, grid)) }
            </div>
        </div>
    }
}

pub(super) fn time_labels(grid: GridSpec) -> Html {
    let slots = grid.slot_count();
    html! {
        <div
            class="grid border-r border-[var(--portal-panel-border)] bg-white/45"
            style={format!("grid-template-rows: repeat({slots}, 28px);")}
        >
            { for (0..slots).map(|slot| html! {
                <div
                    key={format!("time-{slot}")}
                    class="border-b border-[var(--portal-panel-border)]/45 pr-2 pt-1 text-right text-[8px] font-light text-black/35"
                >
                    { if slot % (60 / grid.slot_minutes) as usize == 0 {
                        crate::calendar::slot_label(slot, grid)
                    } else {
                        String::new()
                    } }
                </div>
            }) }
        </div>
    }
}

pub(super) fn drag_target(
    key: String,
    on_msg: &Callback<Msg>,
) -> (Callback<DragEvent>, Callback<DragEvent>) {
    let enter_msg = on_msg.clone();
    let enter_key = key;
    let ondragenter = Callback::from(move |event: DragEvent| {
        event.prevent_default();
        enter_msg.emit(Msg::ProjectCalendarDragTargetChanged(Some(enter_key.clone())));
    });
    let ondragover = Callback::from(|event: DragEvent| event.prevent_default());
    (ondragenter, ondragover)
}

pub(super) fn all_day_cell(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    date: &str,
    chips: &[CalendarChip],
    on_msg: &Callback<Msg>,
) -> Html {
    let target_key = format!("all-day:{date}");
    let target_active = projects.calendar_drag_target.as_deref() == Some(target_key.as_str());
    let ondrop = all_day_drop(on_msg, date.to_owned());
    let (ondragenter, ondragover) = drag_target(target_key, on_msg);
    let events = chips
        .iter()
        .filter(|event| event.date == date && event.all_day)
        .collect::<Vec<_>>();
    html! {
        <div
            key={format!("all-day-{date}")}
            {ondrop}
            {ondragenter}
            {ondragover}
            class={classes!(
                "min-h-10","border-r","border-[var(--portal-panel-border)]","px-1","py-1","transition",
                target_active.then_some("bg-[var(--portal-gold)]/15 ring-1 ring-inset ring-[var(--portal-gold)]/40")
            )}
        >
            { for events.iter().map(|event| all_day_chip(model, projects, event, on_msg)) }
        </div>
    }
}

pub(super) fn all_day_chip(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    event: &&CalendarChip,
    on_msg: &Callback<Msg>,
) -> Html {
    let editable = event.editable();
    let ondragstart = drag_start(event, "move", on_msg);
    let ondragend = drag_end(on_msg);
    html! {
        <button
            type="button"
            key={event.id.clone()}
            draggable={editable.to_string()}
            {ondragstart}
            {ondragend}
            onclick={select_event(event, on_msg)}
            title={event.title.clone()}
            class={classes!(
                "mb-0.5","block","w-full","truncate","rounded","border","px-1.5","py-0.5","text-left","text-[9px]","leading-tight",
                event.tone(),
                editable.then_some("cursor-grab"),
                selected(projects, event).then_some("ring-1 ring-[var(--portal-gold)]"),
                (projects.calendar_dragging_event_id.as_deref() == Some(event.id.as_str())).then_some("opacity-40"),
                pending(model, event).then_some("opacity-70")
            )}
        >
            if event.recurring { <span class="mr-1">{"↻"}</span> }
            { event.title.clone() }
        </button>
    }
}

#[derive(Clone)]
pub(super) struct TimedPosition<'a> {
    pub(super) event: &'a CalendarChip,
    pub(super) slot: usize,
    pub(super) span: usize,
    pub(super) lane: usize,
    pub(super) lanes: usize,
}

pub(super) fn timed_positions<'a>(
    chips: &'a [CalendarChip],
    date: &str,
    grid: GridSpec,
) -> Vec<TimedPosition<'a>> {
    let mut base = chips
        .iter()
        .filter(|event| event.date == date && !event.all_day)
        .filter_map(|event| {
            let slot = crate::calendar::event_slot(&event.start_at, grid)?;
            let span = crate::calendar::event_span_slots(&event.start_at, event.end_at.as_deref(), grid)
                .min(grid.slot_count().saturating_sub(slot).max(1));
            Some((event, slot, span))
        })
        .collect::<Vec<_>>();
    base.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| right.2.cmp(&left.2)));

    let layout_input = base
        .iter()
        .map(|(_, slot, span)| (*slot, *span))
        .collect::<Vec<_>>();
    let layout = crate::calendar::overlap_lanes(&layout_input);

    base.into_iter()
        .zip(layout)
        .map(|((event, slot, span), (lane, lanes))| TimedPosition {
            event,
            slot,
            span,
            lane,
            lanes,
        })
        .collect()
}

pub(super) fn timed_day_column(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    date: &str,
    chips: &[CalendarChip],
    on_msg: &Callback<Msg>,
    grid: GridSpec,
) -> Html {
    let slots = grid.slot_count();
    let events = timed_positions(chips, date, grid);
    html! {
        <div
            key={format!("timed-{date}")}
            class="grid border-r border-[var(--portal-panel-border)] bg-white/15"
            style={format!("grid-template-rows: repeat({slots}, 28px);")}
        >
            { for (0..slots).map(|slot| {
                let target_key = format!("timed:{date}:{slot}");
                let target_active = projects.calendar_drag_target.as_deref() == Some(target_key.as_str());
                let ondrop = timed_drop(on_msg, date.to_owned(), slot, grid);
                let (ondragenter, ondragover) = drag_target(target_key, on_msg);
                html! {
                    <div
                        key={format!("slot-{date}-{slot}")}
                        {ondrop}
                        {ondragenter}
                        {ondragover}
                        style={format!("grid-row: {}; grid-column: 1;", slot + 1)}
                        class={classes!(
                            "border-b","border-[var(--portal-panel-border)]/45","transition",
                            (slot % (60 / grid.slot_minutes) as usize == 0).then_some("bg-white/15"),
                            target_active.then_some("bg-[var(--portal-gold)]/15 ring-1 ring-inset ring-[var(--portal-gold)]/35")
                        )}
                    ></div>
                }
            }) }
            { for events.into_iter().map(|position| timed_event(model, projects, position, on_msg)) }
        </div>
    }
}

pub(super) fn timed_event(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    position: TimedPosition<'_>,
    on_msg: &Callback<Msg>,
) -> Html {
    let event = position.event;
    let editable = event.editable();
    let move_start = drag_start(event, "move", on_msg);
    let resize_start = drag_start(event, "resize", on_msg);
    let move_end = drag_end(on_msg);
    let resize_end = drag_end(on_msg);
    let onclick = select_event(event, on_msg);
    let tooltip = event
        .time
        .as_ref()
        .map(|time| format!("{time} · {}", event.title))
        .unwrap_or_else(|| event.title.clone());
    let width = 100.0 / position.lanes.max(1) as f64;
    let left = width * position.lane as f64;
    let style = format!(
        "grid-row: {} / span {}; grid-column: 1; z-index: 10; margin-top: 1px; margin-bottom: 1px; margin-left: calc({left:.3}% + 2px); width: calc({width:.3}% - 4px); min-height: 26px;",
        position.slot + 1,
        position.span,
    );

    html! {
        <article
            key={event.id.clone()}
            draggable={editable.to_string()}
            ondragstart={move_start}
            ondragend={move_end}
            onclick={onclick}
            title={tooltip}
            style={style}
            class={classes!(
                "relative","overflow-hidden","rounded","border","px-1.5","py-1","text-[9px]","shadow-sm","transition-opacity",
                event.tone(),
                editable.then_some("cursor-grab"),
                selected(projects, event).then_some("ring-2 ring-[var(--portal-gold)]"),
                (projects.calendar_dragging_event_id.as_deref() == Some(event.id.as_str())).then_some("opacity-40"),
                pending(model, event).then_some("opacity-70")
            )}
        >
            <div class="truncate font-semibold">
                if event.recurring { <span class="mr-1">{"↻"}</span> }
                if event.detached { <span class="mr-1">{"•"}</span> }
                { event.title.clone() }
            </div>
            if let Some(time) = event.time.as_ref() {
                <div class="truncate text-[8px] opacity-70">{ time }</div>
            }
            if editable {
                <div
                    draggable="true"
                    ondragstart={Callback::from(move |event: DragEvent| {
                        event.stop_propagation();
                        resize_start.emit(event);
                    })}
                    ondragend={resize_end}
                    title="Drag bottom edge to resize"
                    class="absolute inset-x-0 bottom-0 flex h-2.5 cursor-ns-resize items-end justify-center bg-gradient-to-t from-black/10 to-transparent"
                >
                    <span class="mb-0.5 block h-0.5 w-6 rounded-full bg-current/40"></span>
                </div>
            }
        </article>
    }
}
