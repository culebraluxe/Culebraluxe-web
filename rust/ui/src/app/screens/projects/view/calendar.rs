use yew::prelude::*;

use crate::calendar::GridSpec;
use crate::model::{
    PortalProject, PortalProjectCalendarEvent, PortalProjectWorkItem, PortalProjectsPage,
};

use super::super::{Msg, Vm};
mod drag;
mod month;
mod time_grid;
#[allow(unused_imports)]
pub(super) use drag::*;
#[allow(unused_imports)]
pub(super) use month::*;
#[allow(unused_imports)]
pub(super) use time_grid::*;

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
        // Filled chips on the glass: they must stand out from the cells behind them.
        if self.source == "wbs" {
            "border-transparent bg-[var(--portal-blue-gray)] text-white"
        } else if self.kind == "showing" {
            "border-transparent bg-[var(--portal-gold)] text-[var(--portal-navy)]"
        } else {
            "border-transparent bg-[var(--portal-navy)] text-white"
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
            let value = crate::app::exec::select_value(&event);
            on_msg.emit(Msg::ProjectCalendarRecurrenceScopeSelected(value));
        })
    };
    let filter_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = crate::app::exec::select_value(&event);
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
                            crate::app::template::loading_words("visible dates")
                        } else if let Some(pending) = model.calendar_pending {
                            match pending.phase.as_str() {
                                "queueing" => "Queueing Apple change…".into(),
                                "queued" => "Apple change queued".into(),
                                "failed" => "Apple retry scheduled".into(),
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
