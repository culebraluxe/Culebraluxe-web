//! The optimistic writes that answer later: a timeline drag (due or planned date) and a calendar edit queued to Apple
//! Calendar, each with its rollback and, for the calendar, the poll for the command's outcome.

use super::*;

pub(super) fn queue_timeline_move(
    model: &mut Model,
    item_id: String,
    date: String,
    planned: bool,
) -> Cmd<Msg> {
    if model.pending_timeline.is_some() {
        return Cmd::none();
    }
    let Remote::Loaded(projects) = &mut model.read else {
        return Cmd::none();
    };
    if projects.saving {
        return Cmd::none();
    }
    let Some(index) = projects.items.iter().position(|item| {
        item.id == item_id && item.project_id.as_deref() == projects.selected_project_id.as_deref()
    }) else {
        model.error = Some("The timeline item no longer belongs to this project.".into());
        return Cmd::none();
    };

    let old_due_at = projects.items[index].due_at.clone();
    let old_planned_start = projects.items[index].planned_start.clone();
    let old_planned_finish = projects.items[index].planned_finish.clone();
    let mut item = projects.items[index].clone();
    if planned {
        let Some((start, finish)) = item
            .planned_start
            .as_deref()
            .and_then(crate::timeline::date)
            .zip(
                item.planned_finish
                    .as_deref()
                    .and_then(crate::timeline::date),
            )
        else {
            return Cmd::none();
        };
        let Some(new_start) = crate::timeline::date(&date) else {
            return Cmd::none();
        };
        item.planned_start = Some(new_start.to_string());
        item.planned_finish = Some((new_start + (finish - start)).to_string());
    } else {
        item.due_at = Some(date);
    }
    projects.items[index].due_at = item.due_at.clone();
    projects.items[index].planned_start = item.planned_start.clone();
    projects.items[index].planned_finish = item.planned_finish.clone();
    projects.selected_node_id = Some(item_id.clone());
    projects.work_collapsed = false;
    projects.work_dirty = false;
    projects.timeline_dragging_item_id = None;
    projects.timeline_drag_kind.clear();
    projects.timeline_drag_target_date = None;
    projects.saving = true;
    model.pending_timeline = Some(PendingTimelineEdit {
        item_id,
        old_due_at,
        old_planned_start,
        old_planned_finish,
    });
    model.error = None;

    Cmd::request(
        ProjectsCommand {
            body: serde_json::json!({
                "action": "wbsSave",
                "itemId": item.id,
                "title": item.title,
                "notes": item.notes,
                "status": item.status,
                "dueAt": item.due_at,
                "plannedStart": item.planned_start,
                "plannedFinish": item.planned_finish,
                "owner": item.owner,
            }),
        },
        Msg::TimelineSaved,
    )
}

pub(super) fn timeline_saved(model: &mut Model, result: Result<PortalPage, ApiError>) {
    match result {
        Ok(page) => {
            model.pending_timeline = None;
            if let Err(error) = answer(model, Ok(page)) {
                model.error = Some(error.message);
            } else {
                model.error = None;
            }
        }
        Err(error) => {
            let pending = model.pending_timeline.take();
            if let Remote::Loaded(projects) = &mut model.read {
                projects.saving = false;
                projects.timeline_dragging_item_id = None;
                projects.timeline_drag_kind.clear();
                projects.timeline_drag_target_date = None;
                if let Some(pending) = pending {
                    if let Some(item) = projects
                        .items
                        .iter_mut()
                        .find(|item| item.id == pending.item_id)
                    {
                        item.due_at = pending.old_due_at;
                        item.planned_start = pending.old_planned_start;
                        item.planned_finish = pending.old_planned_finish;
                    }
                }
            }
            model.error = Some(error.message);
        }
    }
}

pub(super) fn calendar_viewport(projects: &mut PortalProjectsPage) -> Cmd<Msg> {
    let Some((start_at, end_at)) =
        crate::calendar::viewport_bounds(&projects.calendar_cursor, &projects.calendar_mode)
    else {
        return Cmd::none();
    };
    if !projects.calendar_loading
        && projects.calendar_loaded_start.as_deref() == Some(start_at.as_str())
        && projects.calendar_loaded_end.as_deref() == Some(end_at.as_str())
    {
        return Cmd::none();
    }

    projects.calendar_loading = true;
    let response_start = start_at.clone();
    let response_end = end_at.clone();
    Cmd::request(ProjectsCalendarRead { start_at, end_at }, move |result| {
        Msg::CalendarViewportLoaded {
            start_at: response_start,
            end_at: response_end,
            result,
        }
    })
}

pub(super) fn apply_calendar_viewport(
    model: &mut Model,
    start_at: String,
    end_at: String,
    result: Result<ProjectsCalendarViewportResponse, ApiError>,
) {
    let Remote::Loaded(projects) = &mut model.read else {
        return;
    };
    let Some((wanted_start, wanted_end)) =
        crate::calendar::viewport_bounds(&projects.calendar_cursor, &projects.calendar_mode)
    else {
        projects.calendar_loading = false;
        return;
    };

    // A quick second navigation can finish before the first HTTP response.
    // Never let the stale answer replace the currently requested interval.
    if start_at != wanted_start || end_at != wanted_end {
        return;
    }

    projects.calendar_loading = false;
    match result {
        Ok(answer) => {
            projects.calendar = answer.calendar;
            projects.calendar_loaded_start = Some(start_at);
            projects.calendar_loaded_end = Some(end_at);
            projects.calendar_selected_event_id = projects
                .calendar_selected_event_id
                .clone()
                .filter(|id| projects.calendar.iter().any(|event| &event.id == id));
            model.error = None;
        }
        Err(error) => model.error = Some(error.message),
    }
}

pub(super) fn rollback_calendar_edit(model: &mut Model, message: String) {
    let pending = model.pending_calendar.take();
    let Remote::Loaded(projects) = &mut model.read else {
        model.error = Some(message);
        return;
    };
    projects.saving = false;
    if let Some(pending) = pending {
        if let Some(event) = projects
            .calendar
            .iter_mut()
            .find(|event| event.id == pending.occurrence_id)
        {
            event.start_at = pending.old_start;
            event.end_at = pending.old_end;
            event.all_day = pending.old_all_day;
        }
    }
    model.error = Some(message);
}

pub(super) fn queue_calendar_edit(
    model: &mut Model,
    occurrence_id: String,
    provider_event_id: String,
    provider_series_id: Option<String>,
    start_at: String,
    end_at: String,
    all_day: bool,
) -> Cmd<Msg> {
    if model.pending_calendar.is_some() {
        return Cmd::none();
    }
    let Remote::Loaded(projects) = &mut model.read else {
        return Cmd::none();
    };
    let Some(event) = projects.calendar.iter_mut().find(|event| {
        event.id == occurrence_id
            && event.source == "apple_calendar"
            && event.provider_event_id.as_deref() == Some(provider_event_id.as_str())
    }) else {
        model.error = Some("Only writable Apple Calendar events can be moved or resized.".into());
        return Cmd::none();
    };

    let pending = PendingCalendarEdit {
        occurrence_id: occurrence_id.clone(),
        old_start: event.start_at.clone(),
        old_end: event.end_at.clone(),
        old_all_day: event.all_day,
        provider_event_id: provider_event_id.clone(),
        provider_series_id: provider_series_id.clone(),
        command_id: None,
        phase: "queueing".into(),
        poll_count: 0,
    };
    event.start_at = start_at.clone();
    event.end_at = Some(end_at.clone());
    event.all_day = all_day;
    projects.saving = true;
    projects.calendar_dragging_event_id = None;
    projects.calendar_drag_target = None;
    model.pending_calendar = Some(pending);
    model.error = None;

    Cmd::request(
        ProjectsCalendarUpdate {
            event_id: provider_event_id,
            calendar_item_id: provider_series_id,
            start_at,
            end_at,
            all_day,
            recurrence_scope: projects.calendar_recurrence_scope.clone(),
        },
        Msg::CalendarQueued,
    )
}

pub(super) fn calendar_queued(
    model: &mut Model,
    result: Result<CalendarCommandReceipt, ApiError>,
) -> Cmd<Msg> {
    let Remote::Loaded(projects) = &mut model.read else {
        return Cmd::none();
    };
    projects.saving = false;

    match result {
        Ok(receipt) => {
            let Some(pending) = model.pending_calendar.as_mut() else {
                return Cmd::none();
            };
            pending.command_id = Some(receipt.command_id);
            pending.phase = if receipt.state.is_empty() {
                "queued".into()
            } else {
                receipt.state
            };
            pending.poll_count = 0;
            model.error = None;
            Cmd::after(750, Msg::CalendarPoll)
        }
        Err(error) => {
            rollback_calendar_edit(model, error.message);
            Cmd::none()
        }
    }
}

pub(super) fn calendar_poll(model: &mut Model) -> Cmd<Msg> {
    let Some(pending) = model.pending_calendar.as_mut() else {
        return Cmd::none();
    };
    let Some(command_id) = pending.command_id.clone() else {
        return Cmd::none();
    };
    if pending.poll_count >= 70 {
        return Cmd::none();
    }
    pending.poll_count += 1;
    Cmd::request(
        ProjectsCalendarCommandState { command_id },
        Msg::CalendarStateLoaded,
    )
}

pub(super) fn calendar_state_loaded(
    model: &mut Model,
    result: Result<CalendarCommandState, ApiError>,
) -> Cmd<Msg> {
    let Ok(state) = result else {
        if model
            .pending_calendar
            .as_ref()
            .is_some_and(|pending| pending.poll_count < 70)
        {
            return Cmd::after(30_000, Msg::CalendarPoll);
        }
        return Cmd::none();
    };

    let Some(pending) = model.pending_calendar.as_ref() else {
        return Cmd::none();
    };
    if pending.command_id.as_deref() != Some(state.command_id.as_str()) {
        return Cmd::none();
    }
    let poll_count = pending.poll_count;
    if let Some(pending) = model.pending_calendar.as_mut() {
        pending.phase = state.state.clone();
    }

    match state.state.as_str() {
        "reconciled" => {
            model.pending_calendar = None;
            let Remote::Loaded(projects) = &mut model.read else {
                return Cmd::none();
            };
            projects.calendar_loaded_start = None;
            projects.calendar_loaded_end = None;
            calendar_viewport(projects)
        }
        "dead" => {
            rollback_calendar_edit(
                model,
                state
                    .last_error
                    .unwrap_or_else(|| "Apple Calendar could not apply the change.".into()),
            );
            Cmd::none()
        }
        "delivered" if poll_count < 70 => Cmd::after(2_000, Msg::CalendarPoll),
        "failed" if poll_count < 70 => Cmd::after(30_000, Msg::CalendarPoll),
        _ if poll_count < 70 => Cmd::after(30_000, Msg::CalendarPoll),
        _ => Cmd::none(),
    }
}
