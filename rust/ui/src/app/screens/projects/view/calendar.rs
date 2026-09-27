use yew::prelude::*;

use crate::model::{
    PortalProject, PortalProjectCalendarEvent, PortalProjectWorkItem, PortalProjectsPage,
};

use super::super::Msg;

#[derive(Clone)]
struct CalendarChip {
    id: String,
    title: String,
    date: String,
    time: Option<String>,
    start_at: String,
    end_at: Option<String>,
    all_day: bool,
    source: String,
    kind: String,
    provider_event_id: Option<String>,
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

pub(super) fn view(
    projects: &PortalProjectsPage,
    project: &PortalProject,
    on_msg: &Callback<Msg>,
) -> Html {
    let chips = project_calendar_chips(projects, project);
    html! {
        <section class="flex h-full min-h-0 flex-col overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35">
            { toolbar(projects, &chips, on_msg) }
            <div class="min-h-0 flex-1">
                {
                    match projects.calendar_mode.as_str() {
                        "week" => time_grid(projects, &chips, crate::calendar::week_dates(&projects.calendar_cursor), on_msg),
                        "day" => time_grid(projects, &chips, vec![projects.calendar_cursor.clone()], on_msg),
                        "list" => list_view(projects, &chips),
                        _ => month_view(projects, &chips),
                    }
                }
            </div>
        </section>
    }
}

fn toolbar(projects: &PortalProjectsPage, chips: &[CalendarChip], on_msg: &Callback<Msg>) -> Html {
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
            if projects.saving {
                <p class="mt-1 text-right text-[9px] font-medium uppercase tracking-[0.09em] text-[var(--portal-gold-muted)]">
                    {"Queueing Apple calendar change…"}
                </p>
            }
        </div>
    }
}

fn calendar_event_linked_to_project(
    projects: &PortalProjectsPage,
    project: &PortalProject,
    event: &PortalProjectCalendarEvent,
) -> bool {
    // The Apple schedule is the user's schedule and remains visible while a
    // project is selected. Canonical showings are narrowed to project context.
    if event.source == "apple_calendar" {
        return true;
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
                title: item.title.clone(),
                date: crate::calendar::date_key(due)?,
                time: None,
                start_at: due.to_owned(),
                end_at: None,
                all_day: true,
                source: "wbs".into(),
                kind: item.category.clone(),
                provider_event_id: None,
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
                    provider_event_id: event.provider_event_id.clone(),
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

fn month_view(projects: &PortalProjectsPage, chips: &[CalendarChip]) -> Html {
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
                                { for day_events.iter().take(4).map(|event| month_chip(event)) }
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

fn month_chip(event: &&CalendarChip) -> Html {
    let tooltip = event
        .time
        .as_ref()
        .map(|time| format!("{time} · {}", event.title))
        .unwrap_or_else(|| event.title.clone());
    html! {
        <div
            key={event.id.clone()}
            title={tooltip}
            class={classes!("truncate","rounded","border","px-1.5","py-0.5","text-[9px]","leading-tight",event.tone())}
        >
            if event.recurring {
                <span class="mr-1" aria-label="Recurring">{"↻"}</span>
            }
            if let Some(time) = event.time.as_ref() {
                <span class="mr-1 font-semibold">{ time }</span>
            }
            { event.title.clone() }
        </div>
    }
}

fn time_grid(
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
                { for days.iter().map(|date| all_day_cell(date, chips, on_msg)) }
            </div>

            <div style={columns} class="grid">
                { time_labels() }
                { for days.iter().map(|date| timed_day_column(date, chips, on_msg)) }
            </div>
        </div>
    }
}

fn time_labels() -> Html {
    html! {
        <div
            class="grid border-r border-[var(--portal-panel-border)] bg-white/45"
            style={format!("grid-template-rows: repeat({}, 28px);", crate::calendar::SLOT_COUNT)}
        >
            { for (0..crate::calendar::SLOT_COUNT).map(|slot| html! {
                <div
                    key={format!("time-{slot}")}
                    class="border-b border-[var(--portal-panel-border)]/45 pr-2 pt-1 text-right text-[8px] font-light text-black/35"
                >
                    { if slot % 2 == 0 { crate::calendar::slot_label(slot) } else { String::new() } }
                </div>
            }) }
        </div>
    }
}

fn all_day_cell(date: &str, chips: &[CalendarChip], on_msg: &Callback<Msg>) -> Html {
    let date_owned = date.to_owned();
    let ondrop = all_day_drop(on_msg, date_owned);
    let ondragover = Callback::from(|event: web_sys::DragEvent| event.prevent_default());
    let events = chips
        .iter()
        .filter(|event| event.date == date && event.all_day)
        .collect::<Vec<_>>();
    html! {
        <div
            key={format!("all-day-{date}")}
            {ondrop}
            {ondragover}
            class="min-h-10 border-r border-[var(--portal-panel-border)] px-1 py-1"
        >
            { for events.iter().map(|event| all_day_chip(event)) }
        </div>
    }
}

fn all_day_chip(event: &&CalendarChip) -> Html {
    let editable = event.editable();
    let ondragstart = drag_start(event, "move");
    html! {
        <div
            key={event.id.clone()}
            draggable={editable.to_string()}
            ondragstart={ondragstart}
            title={event.title.clone()}
            class={classes!(
                "mb-0.5","truncate","rounded","border","px-1.5","py-0.5","text-[9px]","leading-tight",
                event.tone(),
                editable.then_some("cursor-grab")
            )}
        >
            if event.recurring { <span class="mr-1">{"↻"}</span> }
            { event.title.clone() }
        </div>
    }
}

fn timed_day_column(date: &str, chips: &[CalendarChip], on_msg: &Callback<Msg>) -> Html {
    let events = chips
        .iter()
        .filter(|event| event.date == date && !event.all_day)
        .filter_map(|event| crate::calendar::event_slot(&event.start_at).map(|slot| (event, slot)))
        .collect::<Vec<_>>();
    html! {
        <div
            key={format!("timed-{date}")}
            class="grid border-r border-[var(--portal-panel-border)] bg-white/15"
            style={format!("grid-template-rows: repeat({}, 28px);", crate::calendar::SLOT_COUNT)}
        >
            { for (0..crate::calendar::SLOT_COUNT).map(|slot| {
                let ondrop = timed_drop(on_msg, date.to_owned(), slot);
                let ondragover = Callback::from(|event: web_sys::DragEvent| event.prevent_default());
                html! {
                    <div
                        key={format!("slot-{date}-{slot}")}
                        {ondrop}
                        {ondragover}
                        style={format!("grid-row: {}; grid-column: 1;", slot + 1)}
                        class={classes!(
                            "border-b","border-[var(--portal-panel-border)]/45",
                            (slot % 2 == 0).then_some("bg-white/15")
                        )}
                    ></div>
                }
            }) }
            { for events.into_iter().map(|(event, slot)| timed_event(event, slot)) }
        </div>
    }
}

fn timed_event(event: &CalendarChip, slot: usize) -> Html {
    let span = crate::calendar::event_span_slots(&event.start_at, event.end_at.as_deref())
        .min(crate::calendar::SLOT_COUNT.saturating_sub(slot).max(1));
    let editable = event.editable();
    let move_start = drag_start(&event, "move");
    let resize_start = drag_start(&event, "resize");
    let tooltip = event
        .time
        .as_ref()
        .map(|time| format!("{time} · {}", event.title))
        .unwrap_or_else(|| event.title.clone());
    html! {
        <article
            key={event.id.clone()}
            draggable={editable.to_string()}
            ondragstart={move_start}
            title={tooltip}
            style={format!(
                "grid-row: {} / span {}; grid-column: 1; z-index: 10; margin: 1px 3px; min-height: 26px;",
                slot + 1,
                span
            )}
            class={classes!(
                "relative","overflow-hidden","rounded","border","px-1.5","py-1","text-[9px]","shadow-sm",
                event.tone(),
                editable.then_some("cursor-grab")
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
                    title="Drag to resize"
                    class="absolute inset-x-1 bottom-0 h-1.5 cursor-ns-resize border-t border-current/20"
                ></div>
            }
        </article>
    }
}

fn drag_start(event: &CalendarChip, kind: &'static str) -> Callback<web_sys::DragEvent> {
    let payload = serde_json::json!({
        "kind": kind,
        "occurrenceId": event.id,
        "providerEventId": event.provider_event_id,
        "startAt": event.start_at,
        "endAt": event.end_at,
        "allDay": event.all_day,
    })
    .to_string();
    Callback::from(move |event: web_sys::DragEvent| {
        if let Some(data) = event.data_transfer() {
            let _ = data.set_data("text/plain", &payload);
            data.set_effect_allowed("move");
        }
    })
}

fn drop_payload(event: &web_sys::DragEvent) -> Option<serde_json::Value> {
    let data = event.data_transfer()?;
    let raw = data.get_data("text/plain").ok()?;
    serde_json::from_str(&raw).ok()
}

fn text_field<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn timed_drop(on_msg: &Callback<Msg>, date: String, slot: usize) -> Callback<web_sys::DragEvent> {
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
        let Some(target) = crate::calendar::slot_timestamp(&date, slot) else {
            return;
        };
        let span = if text_field(&payload, "kind") == Some("resize") {
            crate::calendar::resize_span(old_start, &target)
        } else {
            crate::calendar::move_span(old_start, old_end, &target)
        };
        let Some((start_at, end_at)) = span else {
            return;
        };
        on_msg.emit(Msg::ProjectCalendarEditRequested {
            occurrence_id: occurrence_id.to_owned(),
            provider_event_id: provider_event_id.to_owned(),
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
        if payload.get("allDay").and_then(serde_json::Value::as_bool) != Some(true) {
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
        let Some(target) = crate::calendar::midnight_timestamp(&date) else {
            return;
        };
        let Some((start_at, end_at)) = crate::calendar::move_span(old_start, old_end, &target)
        else {
            return;
        };
        on_msg.emit(Msg::ProjectCalendarEditRequested {
            occurrence_id: occurrence_id.to_owned(),
            provider_event_id: provider_event_id.to_owned(),
            start_at,
            end_at,
            all_day: true,
        });
    })
}

fn list_view(projects: &PortalProjectsPage, chips: &[CalendarChip]) -> Html {
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
                                { for events.into_iter().map(|event| html! {
                                    <div key={event.id.clone()} class="grid grid-cols-[72px_minmax(0,1fr)_100px] items-center gap-3 px-3 py-2">
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
                                    </div>
                                }) }
                            </div>
                        }
                    </section>
                }
            }) }
        </div>
    }
}
