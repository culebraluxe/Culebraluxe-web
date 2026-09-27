use yew::prelude::*;

use crate::calendar::GridSpec;
use crate::model::{
    PortalProject, PortalProjectCalendarEvent, PortalProjectWorkItem, PortalProjectsPage,
};

use super::super::{Msg, Vm};

#[derive(Clone)]
struct CalendarChip {
    id: String,
    work_item_id: Option<String>,
    title: String,
    date: String,
    time: Option<String>,
    start_at: String,
    end_at: Option<String>,
    all_day: bool,
    source: String,
    kind: String,
    location: Option<String>,
    provider_event_id: Option<String>,
    provider_series_id: Option<String>,
    recurring: bool,
    detached: bool,
}

impl CalendarChip {
    fn editable(&self) -> bool {
        self.source == "apple_calendar" && self.provider_event_id.is_some()
    }

    fn tone(&self) -> &'static str {
        if self.source == "wbs" {
            "border-[var(--portal-navy)]/15 bg-[var(--portal-navy)]/[0.07] text-[var(--portal-navy)]"
        } else if self.kind == "showing" {
            "border-[var(--portal-gold)]/30 bg-[var(--portal-gold)]/15 text-[var(--portal-navy)]"
        } else {
            "border-[var(--portal-blue-gray)]/20 bg-white/80 text-[var(--portal-navy)]"
        }
    }
}

fn grid(projects: &PortalProjectsPage) -> GridSpec {
    GridSpec::new(
        projects.calendar_day_start_hour,
        projects.calendar_day_end_hour,
        projects.calendar_slot_minutes,
    )
}

pub(super) fn view(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    let chips = project_calendar_chips(projects, project);
    html! {
        <section class="flex h-full min-h-0 flex-col overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35">
            { toolbar(model, projects, &chips, on_msg) }
            <div class="min-h-0 flex-1">
                {
                    match projects.calendar_mode.as_str() {
                        "week" => time_grid(model, projects, &chips, crate::calendar::week_dates(&projects.calendar_cursor), on_msg),
                        "day" => time_grid(model, projects, &chips, vec![projects.calendar_cursor.clone()], on_msg),
                        "list" => list_view(model, projects, &chips, on_msg),
                        _ => month_view(model, projects, &chips, on_msg),
                    }
                }
            </div>
        </section>
    }
}

fn toolbar(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    chips: &[CalendarChip],
    on_msg: &Callback<Msg>,
) -> Html {
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
    let title = match projects.calendar_mode.as_str() {
        "week" | "list" => crate::calendar::week_title(&projects.calendar_cursor),
        "day" => crate::calendar::date_title(&projects.calendar_cursor),
        _ => crate::calendar::month_title(&projects.calendar_cursor),
    };
    let recurrence_visible = chips
        .iter()
        .any(|event| event.recurring && event.editable());
    let recurrence_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value();
            on_msg.emit(Msg::ProjectCalendarRecurrenceScopeSelected(value));
        })
    };
    let filter_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value();
            on_msg.emit(Msg::ProjectCalendarFilterSelected(value));
        })
    };
    let grid = grid(projects);

    html! {
        <div class="shrink-0 border-b border-[var(--portal-panel-border)] bg-white/35 px-3 py-2">
            <div class="flex flex-wrap items-center gap-2">
                <div class="flex items-center gap-1">
                    <button type="button" onclick={previous} aria-label="Previous calendar period"
                        class="h-8 rounded-md border border-[var(--portal-panel-border)] bg-white/70 px-2.5 text-[15px] text-[var(--portal-navy)] hover:bg-white">
                        {"‹"}
                    </button>
                    <button type="button" onclick={today}
                        class="h-8 rounded-md border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[11px] font-medium uppercase tracking-[0.08em] text-[var(--portal-navy)] hover:bg-white">
                        {"Today"}
                    </button>
                    <button type="button" onclick={next} aria-label="Next calendar period"
                        class="h-8 rounded-md border border-[var(--portal-panel-border)] bg-white/70 px-2.5 text-[15px] text-[var(--portal-navy)] hover:bg-white">
                        {"›"}
                    </button>
                </div>

                <h2 class="min-w-[180px] flex-1 text-center font-serif text-[20px] font-light text-[var(--portal-navy)]">
                    { title }
                </h2>

                <label class="flex h-8 items-center gap-1.5 rounded-md border border-[var(--portal-panel-border)] bg-white/65 px-2 text-[9px] font-semibold uppercase tracking-[0.08em] text-[var(--portal-blue-gray)]">
                    {"Schedule"}
                    <select
                        value={projects.calendar_filter.clone()}
                        onchange={filter_change}
                        class="bg-transparent text-[10px] font-medium normal-case tracking-normal text-[var(--portal-navy)] outline-none"
                    >
                        <option value="all" selected={projects.calendar_filter == "all"}>{"Project + Apple"}</option>
                        <option value="project" selected={projects.calendar_filter == "project"}>{"Project only"}</option>
                    </select>
                </label>

                if recurrence_visible {
                    <label class="flex h-8 items-center gap-1.5 rounded-md border border-[var(--portal-panel-border)] bg-white/65 px-2 text-[9px] font-semibold uppercase tracking-[0.08em] text-[var(--portal-blue-gray)]">
                        {"Repeat edit"}
                        <select
                            value={projects.calendar_recurrence_scope.clone()}
                            onchange={recurrence_change}
                            class="bg-transparent text-[10px] font-medium normal-case tracking-normal text-[var(--portal-navy)] outline-none"
                        >
                            <option value="this" selected={projects.calendar_recurrence_scope == "this"}>{"This event"}</option>
                            <option value="future" selected={projects.calendar_recurrence_scope == "future"}>{"This + future"}</option>
                        </select>
                    </label>
                }

                <nav aria-label="Calendar views" class="flex h-8 overflow-hidden rounded-md border border-[var(--portal-panel-border)] bg-white/65">
                    { for [("month", "Month"), ("week", "Week"), ("day", "Day"), ("list", "List")].into_iter().map(|(key, label)| {
                        let active = projects.calendar_mode == key;
                        let key = key.to_string();
                        let on_msg = on_msg.clone();
                        html! {
                            <button
                                type="button"
                                aria-current={active.then_some("page")}
                                onclick={Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectCalendarModeSelected(key.clone())))}
                                class={classes!(
                                    "px-2.5","text-[10px]","font-medium","transition",
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
                <span>
                    { format!(
                        "{:02}:00–{:02}:00 · {} min",
                        grid.start_hour, grid.end_hour, grid.slot_minutes
                    ) }
                </span>
                <span class="text-[var(--portal-gold-muted)]">
                    {
                        if projects.calendar_loading {
                            "Loading visible dates…".to_owned()
                        } else if let Some(pending) = model.calendar_pending {
                            match pending.phase.as_str() {
                                "queueing" => "Queueing Apple change…".into(),
                                "queued" => "Apple change queued".into(),
                                "delivered" => "EventKit confirmed · awaiting sync".into(),
                                other => other.replace('_', " "),
                            }
                        } else {
                            String::new()
                        }
                    }
                </span>
            </div>
        </div>
    }
}

fn calendar_event_linked_to_project(
    projects: &PortalProjectsPage,
    project: &PortalProject,
    event: &PortalProjectCalendarEvent,
) -> bool {
    if event.source == "apple_calendar" {
        return projects.calendar_filter == "all";
    }
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
                work_item_id: Some(item.id.clone()),
                title: item.title.clone(),
                date: crate::calendar::date_key(due)?,
                time: None,
                start_at: due.to_owned(),
                end_at: None,
                all_day: true,
                source: "wbs".into(),
                kind: item.category.clone(),
                location: None,
                provider_event_id: None,
                provider_series_id: None,
                recurring: false,
                detached: false,
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
                    work_item_id: None,
                    title: event.title.clone(),
                    date: crate::calendar::date_key(&event.start_at)?,
                    time: (!event.all_day)
                        .then(|| crate::calendar::time_label(&event.start_at))
                        .flatten(),
                    start_at: event.start_at.clone(),
                    end_at: event.end_at.clone(),
                    all_day: event.all_day,
                    source: event.source.clone(),
                    kind: event.kind.clone(),
                    location: event.location.clone(),
                    provider_event_id: event.provider_event_id.clone(),
                    provider_series_id: event.provider_series_id.clone(),
                    recurring: event.recurring,
                    detached: event.detached,
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

fn select_event(event: &CalendarChip, on_msg: &Callback<Msg>) -> Callback<MouseEvent> {
    let on_msg = on_msg.clone();
    if let Some(item_id) = event.work_item_id.clone() {
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::ProjectNodeSelected(Some(item_id.clone())))
        })
    } else {
        let id = event.id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::ProjectCalendarEventSelected(Some(id.clone())))
        })
    }
}

fn selected(projects: &PortalProjectsPage, event: &CalendarChip) -> bool {
    event
        .work_item_id
        .as_deref()
        .is_some_and(|id| projects.selected_node_id.as_deref() == Some(id))
        || projects.calendar_selected_event_id.as_deref() == Some(event.id.as_str())
}

fn pending(model: &Vm<'_>, event: &CalendarChip) -> bool {
    model
        .calendar_pending
        .is_some_and(|pending| pending.occurrence_id == event.id)
}

fn month_view(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    chips: &[CalendarChip],
    on_msg: &Callback<Msg>,
) -> Html {
    let days = crate::calendar::month_cells(&projects.calendar_cursor);
    html! {
        <div class="flex h-full min-h-0 flex-col">
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
                                { for day_events.iter().take(4).map(|event| month_chip(model, projects, event, on_msg)) }
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
        </div>
    }
}

fn month_chip(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    event: &&CalendarChip,
    on_msg: &Callback<Msg>,
) -> Html {
    let tooltip = event
        .time
        .as_ref()
        .map(|time| format!("{time} · {}", event.title))
        .unwrap_or_else(|| event.title.clone());
    html! {
        <button
            type="button"
            key={event.id.clone()}
            title={tooltip}
            onclick={select_event(event, on_msg)}
            class={classes!(
                "block","w-full","truncate","rounded","border","px-1.5","py-0.5","text-left","text-[9px]","leading-tight",
                event.tone(),
                selected(projects, event).then_some("ring-1 ring-[var(--portal-gold)]"),
                pending(model, event).then_some("opacity-70")
            )}
        >
            if event.recurring {
                <span class="mr-1" aria-label="Recurring">{"↻"}</span>
            }
            if let Some(time) = event.time.as_ref() {
                <span class="mr-1 font-semibold">{ time }</span>
            }
            { event.title.clone() }
        </button>
    }
}

fn time_grid(
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

fn time_labels(grid: GridSpec) -> Html {
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

fn drag_target(
    key: String,
    on_msg: &Callback<Msg>,
) -> (Callback<web_sys::DragEvent>, Callback<web_sys::DragEvent>) {
    let enter_msg = on_msg.clone();
    let enter_key = key;
    let ondragenter = Callback::from(move |event: web_sys::DragEvent| {
        event.prevent_default();
        enter_msg.emit(Msg::ProjectCalendarDragTargetChanged(Some(enter_key.clone())));
    });
    let ondragover = Callback::from(|event: web_sys::DragEvent| event.prevent_default());
    (ondragenter, ondragover)
}

fn all_day_cell(
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

fn all_day_chip(
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
struct TimedPosition<'a> {
    event: &'a CalendarChip,
    slot: usize,
    span: usize,
    lane: usize,
    lanes: usize,
}

fn timed_positions<'a>(
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

fn timed_day_column(
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

fn timed_event(
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
                    ondragstart={Callback::from(move |event: web_sys::DragEvent| {
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

fn drag_start(
    event: &CalendarChip,
    kind: &'static str,
    on_msg: &Callback<Msg>,
) -> Callback<web_sys::DragEvent> {
    let payload = serde_json::json!({
        "kind": kind,
        "occurrenceId": event.id,
        "providerEventId": event.provider_event_id,
        "providerSeriesId": event.provider_series_id,
        "startAt": event.start_at,
        "endAt": event.end_at,
        "allDay": event.all_day,
    })
    .to_string();
    let id = event.id.clone();
    let on_msg = on_msg.clone();
    Callback::from(move |event: web_sys::DragEvent| {
        if let Some(data) = event.data_transfer() {
            let _ = data.set_data("text/plain", &payload);
            data.set_effect_allowed("move");
            on_msg.emit(Msg::ProjectCalendarDragStarted(id.clone()));
        }
    })
}

fn drag_end(on_msg: &Callback<Msg>) -> Callback<web_sys::DragEvent> {
    let on_msg = on_msg.clone();
    Callback::from(move |_: web_sys::DragEvent| on_msg.emit(Msg::ProjectCalendarDragEnded))
}

fn drop_payload(event: &web_sys::DragEvent) -> Option<serde_json::Value> {
    let data = event.data_transfer()?;
    let raw = data.get_data("text/plain").ok()?;
    serde_json::from_str(&raw).ok()
}

fn text_field<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn timed_drop(
    on_msg: &Callback<Msg>,
    date: String,
    slot: usize,
    grid: GridSpec,
) -> Callback<web_sys::DragEvent> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: web_sys::DragEvent| {
        event.prevent_default();
        let Some(payload) = drop_payload(&event) else {
            return;
        };
        let Some(occurrence_id) = text_field(&payload, "occurrenceId") else {
            return;
        };
        let Some(provider_event_id) = text_field(&payload, "providerEventId") else {
            return;
        };
        let Some(old_start) = text_field(&payload, "startAt") else {
            return;
        };
        let old_end = text_field(&payload, "endAt");
        let was_all_day = payload
            .get("allDay")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let Some(target) = crate::calendar::slot_timestamp(&date, slot, grid) else {
            return;
        };
        let span = if text_field(&payload, "kind") == Some("resize") {
            if was_all_day {
                None
            } else {
                crate::calendar::resize_span(old_start, &target, grid)
            }
        } else {
            crate::calendar::move_to_timed(old_start, old_end, was_all_day, &target)
        };
        let Some((start_at, end_at)) = span else {
            return;
        };
        on_msg.emit(Msg::ProjectCalendarEditRequested {
            occurrence_id: occurrence_id.to_owned(),
            provider_event_id: provider_event_id.to_owned(),
            provider_series_id: text_field(&payload, "providerSeriesId").map(str::to_owned),
            start_at,
            end_at,
            all_day: false,
        });
    })
}

fn all_day_drop(on_msg: &Callback<Msg>, date: String) -> Callback<web_sys::DragEvent> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: web_sys::DragEvent| {
        event.prevent_default();
        let Some(payload) = drop_payload(&event) else {
            return;
        };
        if text_field(&payload, "kind") == Some("resize") {
            return;
        }
        let Some(occurrence_id) = text_field(&payload, "occurrenceId") else {
            return;
        };
        let Some(provider_event_id) = text_field(&payload, "providerEventId") else {
            return;
        };
        let Some(old_start) = text_field(&payload, "startAt") else {
            return;
        };
        let old_end = text_field(&payload, "endAt");
        let was_all_day = payload
            .get("allDay")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let Some((start_at, end_at)) =
            crate::calendar::move_to_all_day(old_start, old_end, was_all_day, &date)
        else {
            return;
        };
        on_msg.emit(Msg::ProjectCalendarEditRequested {
            occurrence_id: occurrence_id.to_owned(),
            provider_event_id: provider_event_id.to_owned(),
            provider_series_id: text_field(&payload, "providerSeriesId").map(str::to_owned),
            start_at,
            end_at,
            all_day: true,
        });
    })
}

fn list_view(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    chips: &[CalendarChip],
    on_msg: &Callback<Msg>,
) -> Html {
    let dates = crate::calendar::week_dates(&projects.calendar_cursor);
    html! {
        <div class="h-full overflow-y-auto px-3 py-2">
            { for dates.into_iter().map(|date| {
                let events = chips.iter().filter(|event| event.date == date).collect::<Vec<_>>();
                html! {
                    <section key={format!("agenda-{date}")} class="mb-3 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45">
                        <header class={classes!(
                            "border-b","border-[var(--portal-panel-border)]","px-3","py-2",
                            (date == projects.calendar_today).then_some("bg-[var(--portal-gold)]/[0.08]")
                        )}>
                            <h3 class="text-[11px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-navy)]">
                                { crate::calendar::day_heading(&date) }
                            </h3>
                        </header>
                        if events.is_empty() {
                            <p class="px-3 py-3 text-[11px] font-light text-black/35">{"No events"}</p>
                        } else {
                            <div class="divide-y divide-[var(--portal-panel-border)]/60">
                                { for events.into_iter().map(|event| {
                                    let onclick = select_event(event, on_msg);
                                    html! {
                                        <button
                                            type="button"
                                            key={event.id.clone()}
                                            onclick={onclick}
                                            class={classes!(
                                                "grid","w-full","grid-cols-[72px_minmax(0,1fr)_100px]","items-center","gap-3","px-3","py-2","text-left",
                                                "hover:bg-white/45",
                                                selected(projects, event).then_some("bg-[var(--portal-gold)]/[0.08]"),
                                                pending(model, event).then_some("opacity-70")
                                            )}
                                        >
                                            <span class="text-[10px] font-medium text-[var(--portal-blue-gray)]">
                                                { event.time.clone().unwrap_or_else(|| "All day".into()) }
                                            </span>
                                            <span class="min-w-0 truncate text-[13px] text-[var(--portal-navy)]">
                                                if event.recurring { <span class="mr-1 text-[var(--portal-gold-muted)]">{"↻"}</span> }
                                                { event.title.clone() }
                                            </span>
                                            <span class="truncate text-right text-[9px] font-medium uppercase tracking-[0.07em] text-black/35">
                                                { if event.source == "wbs" { "Work" } else if event.kind == "showing" { "Showing" } else { "Calendar" } }
                                            </span>
                                        </button>
                                    }
                                }) }
                            </div>
                        }
                    </section>
                }
            }) }
        </div>
    }
}
