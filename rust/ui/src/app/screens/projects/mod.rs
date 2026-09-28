//! CORE — Project Management (`/portal/projects`): the navigator (domains → projects → work), the project's views (work
//! plan, timeline, calendar, financials, documents, activity), catch-up, and the work-item editor.
//!
//! Yew owns every piece of state: selection, view, catch-up, the draft of the open work item, and the two writes (a
//! project's status; a work item's save). The navigator, catch-up list, timeline, calendar and asset browser were
//! JavaScript widgets and were deleted with the TypeScript (owner decision, 2026-09-26); they return as Rust ports.
//!
//! A WRITE ANSWERS WITH THE REFRESHED PAGE, and `crate::projects::carry_over` keeps what the user was looking at.

mod nav;
mod view;

use yew::prelude::*;

use crate::app::api::{
    CalendarCommandReceipt, CalendarCommandState, ProjectsCalendarCommandState,
    ProjectsCalendarRead, ProjectsCalendarUpdate, ProjectsCalendarViewportResponse,
    ProjectsCommand, ProjectsRead,
};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{PortalPage, PortalProjectWorkItem, PortalProjectsPage};
use crate::projects::{carry_over, first_node_for_project, first_project_for_domain};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Controls {
    /// The navigator's filter, as typed.
    pub query: String,
    /// Tree branches the operator opened, and ones they closed (the selection's branch is open unless closed).
    pub nav_open: std::collections::BTreeSet<String>,
    pub nav_closed: std::collections::BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PendingCalendarEdit {
    pub occurrence_id: String,
    pub old_start: String,
    pub old_end: Option<String>,
    pub old_all_day: bool,
    pub provider_event_id: String,
    pub provider_series_id: Option<String>,
    pub command_id: Option<String>,
    pub phase: String,
    pub poll_count: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PendingTimelineEdit {
    pub item_id: String,
    pub old_due_at: Option<String>,
    pub old_planned_start: Option<String>,
    pub old_planned_finish: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalProjectsPage>,
    pub controls: Controls,
    /// Why the last write did not happen, in the service's words.
    pub error: Option<String>,
    pending_calendar: Option<PendingCalendarEdit>,
    pending_timeline: Option<PendingTimelineEdit>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    Saved(Result<PortalPage, ApiError>),
    QueryChanged(String),
    /// A navigator branch toggled; `open` is whether it was open when clicked.
    NavToggled { id: String, open: bool },
    /// A work item picked in the navigator: its project, then the item.
    NavWorkSelected { project_id: String, node_id: String },
    ProjectDomainSelected(String),
    ProjectSelected(String),
    ProjectNodeSelected(Option<String>),
    ProjectViewSelected(String),
    ProjectTimelineModeSelected(String),
    ProjectTimelineSortSelected(String),
    ProjectTimelineFocusChanged(String),
    ProjectTimelineFocusShifted(i32),
    ProjectTimelineToday,
    ProjectTimelineGroupToggled(String),
    ProjectTimelineDragStarted(String),
    ProjectTimelinePlannedDragStarted(String),
    ProjectTimelineDragTargetChanged(Option<String>),
    ProjectTimelineDragEnded,
    ProjectTimelineDueMoved {
        item_id: String,
        due_at: String,
    },
    ProjectTimelinePlannedMoved { item_id: String, planned_start: String },
    ProjectTimelineLinkTargetSelected(String),
    ProjectTimelineLinkAddRequested,
    ProjectTimelineLinkRemoveRequested(String),
    TimelineSaved(Result<PortalPage, ApiError>),
    ProjectCalendarPrevious,
    ProjectCalendarNext,
    ProjectCalendarToday,
    ProjectCalendarModeSelected(String),
    ProjectCalendarRecurrenceScopeSelected(String),
    ProjectCalendarFilterSelected(String),
    ProjectCalendarEventSelected(Option<String>),
    ProjectCalendarDragStarted(String),
    ProjectCalendarDragTargetChanged(Option<String>),
    ProjectCalendarDragEnded,
    ProjectCalendarEditRequested {
        occurrence_id: String,
        provider_event_id: String,
        provider_series_id: Option<String>,
        start_at: String,
        end_at: String,
        all_day: bool,
    },
    CalendarViewportLoaded {
        start_at: String,
        end_at: String,
        result: Result<ProjectsCalendarViewportResponse, ApiError>,
    },
    CalendarQueued(Result<CalendarCommandReceipt, ApiError>),
    CalendarPoll,
    CalendarStateLoaded(Result<CalendarCommandState, ApiError>),
    ProjectCatchUpToggled(bool),
    ProjectCatchUpItemSelected {
        project_id: String,
        node_id: String,
    },
    ProjectCatchUpItemCompleteRequested {
        project_id: String,
        node_id: String,
    },
    ProjectStatusRequested(String),
    ProjectWorkTitleChanged(String),
    ProjectWorkNotesChanged(String),
    ProjectWorkOwnerChanged(String),
    ProjectWorkDueChanged(String),
    ProjectWorkPlannedStartChanged(String),
    ProjectWorkPlannedFinishChanged(String),
    ProjectWorkStatusChanged(String),
    ProjectWorkCollapsedToggled,
    ProjectWorkSaveRequested,
}

pub struct Projects;

fn answer(model: &mut Model, answer: Result<PortalPage, ApiError>) -> Result<(), ApiError> {
    let mut projects = answer.and_then(|page| {
        page.projects
            .ok_or_else(|| ApiError::decode("The answer had no projects in it."))
    })?;
    carry_over(model.read.loaded(), &mut projects);
    model.read = Remote::Loaded(projects);
    Ok(())
}

/// Save one work item as it stands (with `status` overriding its own when given).
fn save(
    projects: &mut PortalProjectsPage,
    item: &PortalProjectWorkItem,
    status: Option<&str>,
) -> Cmd<Msg> {
    projects.saving = true;
    Cmd::request(
        ProjectsCommand {
            body: serde_json::json!({
                "action": "wbsSave",
                "itemId": item.id,
                "title": item.title,
                "notes": item.notes,
                "status": status.unwrap_or(&item.status),
                "dueAt": item.due_at,
                "plannedStart": item.planned_start,
                "plannedFinish": item.planned_finish,
                "owner": item.owner,
            }),
        },
        Msg::Saved,
    )
}

/// Edit the open work item's draft.
fn edit(model: &mut Model, change: impl FnOnce(&mut PortalProjectWorkItem)) {
    let Remote::Loaded(projects) = &mut model.read else {
        return;
    };
    let Some(node_id) = projects.selected_node_id.clone() else {
        return;
    };
    if let Some(item) = projects.items.iter_mut().find(|item| item.id == node_id) {
        change(item);
        projects.work_dirty = true;
        model.error = None;
    }
}

fn queue_timeline_move(model: &mut Model, item_id: String, date: String, planned: bool) -> Cmd<Msg> {
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
        item.id == item_id
            && item.project_id.as_deref() == projects.selected_project_id.as_deref()
    }) else {
        model.error = Some("The timeline item no longer belongs to this project.".into());
        return Cmd::none();
    };

    let old_due_at = projects.items[index].due_at.clone();
    let old_planned_start = projects.items[index].planned_start.clone();
    let old_planned_finish = projects.items[index].planned_finish.clone();
    let mut item = projects.items[index].clone();
    if planned {
        let Some((start, finish)) = item.planned_start.as_deref().and_then(crate::timeline::date)
            .zip(item.planned_finish.as_deref().and_then(crate::timeline::date)) else { return Cmd::none(); };
        let Some(new_start) = crate::timeline::date(&date) else { return Cmd::none(); };
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

fn timeline_saved(model: &mut Model, result: Result<PortalPage, ApiError>) {
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
                    if let Some(item) = projects.items.iter_mut().find(|item| item.id == pending.item_id) {
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

fn calendar_viewport(projects: &mut PortalProjectsPage) -> Cmd<Msg> {
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
    Cmd::request(
        ProjectsCalendarRead { start_at, end_at },
        move |result| Msg::CalendarViewportLoaded {
            start_at: response_start,
            end_at: response_end,
            result,
        },
    )
}

fn apply_calendar_viewport(
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

fn rollback_calendar_edit(model: &mut Model, message: String) {
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

fn queue_calendar_edit(
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

fn calendar_queued(
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

fn calendar_poll(model: &mut Model) -> Cmd<Msg> {
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

fn calendar_state_loaded(
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

impl Screen for Projects {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
                ..Model::default()
            },
            Cmd::request(ProjectsRead, Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        // Intents that need a loaded page are ignored before it arrives.
        match msg {
            Msg::Loaded(result) => {
                if let Err(error) = answer(model, result) {
                    model.read = Remote::Failed(error);
                }
                return Cmd::none();
            }
            Msg::TimelineSaved(result) => {
                timeline_saved(model, result);
                return Cmd::none();
            }
            Msg::Saved(result) => {
                if let Err(error) = answer(model, result) {
                    if let Remote::Loaded(projects) = &mut model.read {
                        projects.saving = false;
                    }
                    model.error = Some(error.message);
                } else {
                    model.error = None;
                }
                return Cmd::none();
            }
            Msg::NavToggled { id, open } => {
                if open {
                    model.controls.nav_open.remove(&id);
                    model.controls.nav_closed.insert(id);
                } else {
                    model.controls.nav_closed.remove(&id);
                    model.controls.nav_open.insert(id);
                }
                return Cmd::none();
            }
            Msg::NavWorkSelected { project_id, node_id } => {
                Self::update(model, Msg::ProjectSelected(project_id), _ctx);
                return Self::update(model, Msg::ProjectNodeSelected(Some(node_id)), _ctx);
            }
            Msg::QueryChanged(query) => {
                model.controls.query = query;
                return Cmd::none();
            }
            Msg::CalendarViewportLoaded {
                start_at,
                end_at,
                result,
            } => {
                apply_calendar_viewport(model, start_at, end_at, result);
                return Cmd::none();
            }
            Msg::CalendarQueued(result) => {
                return calendar_queued(model, result);
            }
            Msg::CalendarPoll => {
                return calendar_poll(model);
            }
            Msg::CalendarStateLoaded(result) => {
                return calendar_state_loaded(model, result);
            }
            Msg::ProjectTimelineDueMoved { item_id, due_at } => {
                return queue_timeline_move(model, item_id, due_at, false);
            }
            Msg::ProjectTimelinePlannedMoved { item_id, planned_start } => {
                return queue_timeline_move(model, item_id, planned_start, true);
            }
            Msg::ProjectCalendarEditRequested {
                occurrence_id,
                provider_event_id,
                provider_series_id,
                start_at,
                end_at,
                all_day,
            } => {
                return queue_calendar_edit(
                    model,
                    occurrence_id,
                    provider_event_id,
                    provider_series_id,
                    start_at,
                    end_at,
                    all_day,
                );
            }
            Msg::ProjectWorkTitleChanged(value) => edit(model, |item| item.title = value),
            Msg::ProjectWorkNotesChanged(value) => edit(model, |item| item.notes = value),
            Msg::ProjectWorkOwnerChanged(value) => edit(model, |item| {
                item.owner = (!value.trim().is_empty()).then_some(value)
            }),
            Msg::ProjectWorkDueChanged(value) => edit(model, |item| {
                item.due_at = (!value.trim().is_empty()).then_some(value)
            }),
            Msg::ProjectWorkPlannedStartChanged(value) => edit(model, |item| {
                item.planned_start = (!value.trim().is_empty()).then_some(value)
            }),
            Msg::ProjectWorkPlannedFinishChanged(value) => edit(model, |item| {
                item.planned_finish = (!value.trim().is_empty()).then_some(value)
            }),
            Msg::ProjectWorkStatusChanged(value) => {
                if matches!(value.as_str(), "open" | "doing" | "done" | "dismissed") {
                    edit(model, |item| item.status = value)
                }
            }
            msg => {
                let Remote::Loaded(projects) = &mut model.read else {
                    return Cmd::none();
                };
                return selection(projects, &mut model.error, msg);
            }
        }
        Cmd::none()
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let on_msg = link.callback(|msg: Msg| msg);
        template::remote(&model.read, "the projects", |projects| {
            view::workspace(
                &Vm {
                    controls: &model.controls,
                    error: model.error.as_ref(),
                    calendar_pending: model.pending_calendar.as_ref(),
                    timeline_pending: model.pending_timeline.as_ref(),
                },
                projects,
                &on_msg,
            )
        })
    }
}

/// Selection, views, catch-up and the two writes, on a loaded page.
fn selection(projects: &mut PortalProjectsPage, error: &mut Option<String>, msg: Msg) -> Cmd<Msg> {
    match msg {
        Msg::ProjectDomainSelected(domain) => {
            if matches!(
                domain.as_str(),
                "properties" | "people" | "deals" | "firm" | "marketing" | "accounting"
            ) {
                projects.selected_project_id = first_project_for_domain(projects, &domain);
                projects.selected_node_id =
                    first_node_for_project(projects, projects.selected_project_id.as_deref());
                projects.active_domain = domain;
                projects.catch_up = false;
                projects.active_view = "work-plan".into();
                projects.work_collapsed = false;
                projects.work_dirty = false;
            }
        }
        Msg::ProjectSelected(project_id) => {
            if projects
                .projects
                .iter()
                .any(|project| project.id == project_id)
            {
                projects.selected_project_id = Some(project_id);
                projects.selected_node_id =
                    first_node_for_project(projects, projects.selected_project_id.as_deref());
                projects.catch_up = false;
                projects.active_view = "work-plan".into();
                projects.work_collapsed = false;
                projects.work_dirty = false;
            }
        }
        Msg::ProjectNodeSelected(node_id) => {
            let valid = node_id.as_deref().is_none_or(|id| {
                projects.items.iter().any(|item| {
                    item.id == id
                        && item.project_id.as_deref() == projects.selected_project_id.as_deref()
                })
            });
            if valid {
                if node_id.is_some() {
                    projects.work_collapsed = false;
                }
                projects.selected_node_id = node_id;
                projects.timeline_link_target_id = None;
                projects.calendar_selected_event_id = None;
                projects.work_dirty = false;
            }
        }
        Msg::ProjectTimelineModeSelected(mode) => {
            if matches!(mode.as_str(), "day" | "week" | "month") {
                projects.timeline_mode = mode;
            }
        }
        Msg::ProjectTimelineSortSelected(key) => {
            if matches!(key.as_str(), "title" | "start" | "days") {
                if projects.timeline_sort_key == key {
                    projects.timeline_sort_desc = !projects.timeline_sort_desc;
                } else {
                    projects.timeline_sort_key = key;
                    projects.timeline_sort_desc = false;
                }
            }
        }
        Msg::ProjectTimelineFocusChanged(date) => {
            if crate::timeline::date(&date).is_some() {
                projects.timeline_focus_date = Some(date);
            }
        }
        Msg::ProjectTimelineToday => {
            projects.timeline_focus_date = Some(projects.calendar_today.clone());
        }
        Msg::ProjectTimelineFocusShifted(direction) => {
            let anchor = projects.timeline_focus_date.as_deref()
                .and_then(crate::timeline::date)
                .or_else(|| crate::timeline::date(&projects.calendar_today));
            if let Some(anchor) = anchor {
                let step = match projects.timeline_mode.as_str() { "day" => 7, "month" => 60, _ => 28 };
                projects.timeline_focus_date = Some((anchor + chrono::Duration::days(i64::from(direction) * step)).to_string());
            }
        }
        Msg::ProjectTimelineGroupToggled(item_id) => {
            if projects.timeline_collapsed_items.contains(&item_id) {
                projects.timeline_collapsed_items.remove(&item_id);
            } else {
                projects.timeline_collapsed_items.insert(item_id);
            }
        }
        Msg::ProjectTimelineDragStarted(item_id) => {
            if projects.items.iter().any(|item| {
                item.id == item_id
                    && item.project_id.as_deref() == projects.selected_project_id.as_deref()
                    && item.due_at.is_some()
            }) {
                projects.timeline_dragging_item_id = Some(item_id);
                projects.timeline_drag_kind = "due".into();
                projects.timeline_drag_target_date = None;
            }
        }
        Msg::ProjectTimelinePlannedDragStarted(item_id) => {
            if projects.items.iter().any(|item| item.id == item_id
                && item.project_id.as_deref() == projects.selected_project_id.as_deref()
                && item.planned_start.is_some() && item.planned_finish.is_some()) {
                projects.timeline_dragging_item_id = Some(item_id);
                projects.timeline_drag_kind = "planned".into();
                projects.timeline_drag_target_date = None;
            }
        }
        Msg::ProjectTimelineDragTargetChanged(date) => {
            projects.timeline_drag_target_date = date;
        }
        Msg::ProjectTimelineDragEnded => {
            projects.timeline_dragging_item_id = None;
            projects.timeline_drag_kind.clear();
            projects.timeline_drag_target_date = None;
        }
        Msg::ProjectTimelineLinkTargetSelected(id) => {
            projects.timeline_link_target_id = Some(id).filter(|id| !id.is_empty());
        }
        Msg::ProjectTimelineLinkAddRequested => {
            if projects.saving { return Cmd::none(); }
            let (Some(project_id), Some(target_id), Some(source_id)) = (
                projects.selected_project_id.as_deref(), projects.selected_node_id.as_deref(),
                projects.timeline_link_target_id.as_deref(),
            ) else { return Cmd::none(); };
            if source_id == target_id || projects.dependencies.iter().any(|edge|
                edge.project_id == project_id && edge.source_id == source_id && edge.target_id == target_id)
                || !projects.items.iter().any(|item| item.id == source_id
                && item.project_id.as_deref() == Some(project_id)) { return Cmd::none(); }
            projects.saving = true;
            *error = None;
            return Cmd::request(ProjectsCommand { body: serde_json::json!({
                "action": "wbsDependencyAdd", "projectId": project_id,
                "sourceId": source_id, "targetId": target_id,
            }) }, Msg::Saved);
        }
        Msg::ProjectTimelineLinkRemoveRequested(source_id) => {
            if projects.saving { return Cmd::none(); }
            let (Some(project_id), Some(target_id)) = (
                projects.selected_project_id.as_deref(), projects.selected_node_id.as_deref(),
            ) else { return Cmd::none(); };
            if !projects.dependencies.iter().any(|edge| edge.project_id == project_id
                && edge.source_id == source_id && edge.target_id == target_id) { return Cmd::none(); }
            projects.saving = true;
            *error = None;
            return Cmd::request(ProjectsCommand { body: serde_json::json!({
                "action": "wbsDependencyRemove", "projectId": project_id,
                "sourceId": source_id, "targetId": target_id,
            }) }, Msg::Saved);
        }
        Msg::ProjectViewSelected(view) => {
            if matches!(
                view.as_str(),
                "work-plan" | "timeline" | "calendar" | "financials" | "documents" | "activity"
            ) {
                let calendar_opened = view == "calendar" && projects.active_view != "calendar";
                projects.active_view = view;
                projects.catch_up = false;
                if calendar_opened {
                    return calendar_viewport(projects);
                }
            }
        }
        Msg::ProjectCalendarPrevious => {
            projects.calendar_cursor = crate::calendar::shift_cursor(
                &projects.calendar_cursor,
                &projects.calendar_mode,
                -1,
            );
            projects.calendar_loaded_start = None;
            projects.calendar_loaded_end = None;
            return calendar_viewport(projects);
        }
        Msg::ProjectCalendarNext => {
            projects.calendar_cursor = crate::calendar::shift_cursor(
                &projects.calendar_cursor,
                &projects.calendar_mode,
                1,
            );
            projects.calendar_loaded_start = None;
            projects.calendar_loaded_end = None;
            return calendar_viewport(projects);
        }
        Msg::ProjectCalendarToday => {
            projects.calendar_cursor = crate::projects::calendar_anchor(projects);
            projects.calendar_loaded_start = None;
            projects.calendar_loaded_end = None;
            return calendar_viewport(projects);
        }
        Msg::ProjectCalendarModeSelected(mode) => {
            if matches!(mode.as_str(), "month" | "week" | "day" | "list")
                && projects.calendar_mode != mode
            {
                projects.calendar_mode = mode;
                projects.calendar_loaded_start = None;
                projects.calendar_loaded_end = None;
                return calendar_viewport(projects);
            }
        }
        Msg::ProjectCalendarRecurrenceScopeSelected(scope) => {
            if matches!(scope.as_str(), "this" | "future") {
                projects.calendar_recurrence_scope = scope;
            }
        }
        Msg::ProjectCalendarFilterSelected(filter) => {
            if matches!(filter.as_str(), "all" | "project") {
                projects.calendar_filter = filter;
                projects.calendar_selected_event_id = None;
            }
        }
        Msg::ProjectCalendarEventSelected(event_id) => {
            let valid = event_id
                .as_deref()
                .is_none_or(|id| projects.calendar.iter().any(|event| event.id == id));
            if valid {
                projects.calendar_selected_event_id = event_id;
                projects.work_collapsed = false;
            }
        }
        Msg::ProjectCalendarDragStarted(event_id) => {
            projects.calendar_dragging_event_id = Some(event_id);
            projects.calendar_drag_target = None;
        }
        Msg::ProjectCalendarDragTargetChanged(target) => {
            projects.calendar_drag_target = target;
        }
        Msg::ProjectCalendarDragEnded => {
            projects.calendar_dragging_event_id = None;
            projects.calendar_drag_target = None;
        }
        Msg::ProjectCatchUpToggled(on) => {
            projects.catch_up = on;
            projects.work_dirty = false;
        }
        Msg::ProjectCatchUpItemSelected {
            project_id,
            node_id,
        } => {
            if projects.items.iter().any(|item| {
                item.id == node_id && item.project_id.as_deref() == Some(project_id.as_str())
            }) {
                projects.selected_project_id = Some(project_id);
                projects.selected_node_id = Some(node_id);
                projects.catch_up = true;
                projects.work_collapsed = false;
                projects.work_dirty = false;
            }
        }
        Msg::ProjectCatchUpItemCompleteRequested {
            project_id,
            node_id,
        } => {
            if projects.saving {
                return Cmd::none();
            }
            let Some(item) = projects
                .items
                .iter()
                .find(|item| {
                    item.id == node_id && item.project_id.as_deref() == Some(project_id.as_str())
                })
                .cloned()
            else {
                return Cmd::none();
            };
            if matches!(item.status.as_str(), "done" | "dismissed") {
                return Cmd::none();
            }
            projects.selected_project_id = Some(project_id);
            projects.selected_node_id = Some(node_id);
            projects.catch_up = true;
            projects.work_collapsed = false;
            projects.work_dirty = false;
            *error = None;
            return save(projects, &item, Some("done"));
        }
        Msg::ProjectStatusRequested(status) => {
            if projects.saving || !matches!(status.as_str(), "open" | "doing" | "done" | "archived")
            {
                return Cmd::none();
            }
            let Some(project_id) = projects.selected_project_id.clone() else {
                return Cmd::none();
            };
            projects.saving = true;
            *error = None;
            return Cmd::request(
                ProjectsCommand {
                    body: serde_json::json!({ "action": "projectStatus", "projectId": project_id, "status": status }),
                },
                Msg::Saved,
            );
        }
        Msg::ProjectWorkCollapsedToggled => projects.work_collapsed = !projects.work_collapsed,
        Msg::ProjectWorkSaveRequested => {
            if projects.saving || !projects.work_dirty {
                return Cmd::none();
            }
            let Some(item) = projects
                .selected_node_id
                .as_deref()
                .and_then(|id| projects.items.iter().find(|item| item.id == id))
                .cloned()
            else {
                return Cmd::none();
            };
            if let (Some(start), Some(finish)) = (item.planned_start.as_deref(), item.planned_finish.as_deref()) {
                if crate::timeline::date(start).zip(crate::timeline::date(finish))
                    .is_none_or(|(start, finish)| start > finish) {
                    *error = Some("Planned finish must be on or after planned start.".into());
                    return Cmd::none();
                }
            }
            *error = None;
            return save(projects, &item, None);
        }
        _ => {}
    }
    Cmd::none()
}

/// What the view reads besides the page.
pub struct Vm<'a> {
    pub controls: &'a Controls,
    pub error: Option<&'a String>,
    pub calendar_pending: Option<&'a PendingCalendarEdit>,
    pub timeline_pending: Option<&'a PendingTimelineEdit>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page() -> serde_json::Value {
        json!({ "projects": {
            "calendarToday": "2026-09-27",
            "projects": [
                { "id": "p1", "name": "Villa listing", "propertyId": "prop-1", "status": "doing" },
                { "id": "p2", "name": "Firm ops", "status": "open" }
            ],
            "items": [
                { "id": "w1", "projectId": "p1", "title": "Photos", "status": "doing", "dueAt": "2026-09-30T00:00:00+00:00" },
                { "id": "w2", "projectId": "p2", "title": "Books", "status": "open" }
            ],
            "calendar": [
                {
                    "id": "occ-1",
                    "title": "Seller meeting",
                    "startAt": "2026-09-28T09:00:00+00:00",
                    "endAt": "2026-09-28T10:00:00+00:00",
                    "source": "apple_calendar",
                    "providerEventId": "ek-1",
                    "recurring": true
                }
            ]
        } })
    }

    fn opened() -> Model {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = Projects::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/api/portal/rust-ui/projects");
        Projects::update(&mut model, request.respond(Ok(page())), &ctx);
        model
    }

    #[test]
    fn the_first_answer_opens_the_first_domain_with_work_and_selection_steers_it() {
        let ctx = ScreenCtx::default();
        let mut model = opened();
        let projects = model.read.loaded().unwrap();
        assert_eq!(
            (
                projects.active_domain.as_str(),
                projects.selected_project_id.as_deref(),
                projects.selected_node_id.as_deref()
            ),
            ("properties", Some("p1"), Some("w1"))
        );
        Projects::update(&mut model, Msg::ProjectSelected("p2".into()), &ctx);
        Projects::update(
            &mut model,
            Msg::ProjectNodeSelected(Some("w2".into())),
            &ctx,
        );
        let projects = model.read.loaded().unwrap();
        assert_eq!(
            (
                projects.selected_project_id.as_deref(),
                projects.selected_node_id.as_deref()
            ),
            (Some("p2"), Some("w2"))
        );
        Projects::update(&mut model, Msg::QueryChanged("villa".into()), &ctx);
        assert_eq!(model.controls.query, "villa");

        Projects::update(
            &mut model,
            Msg::ProjectViewSelected("calendar".into()),
            &ctx,
        );
        Projects::update(&mut model, Msg::ProjectCalendarNext, &ctx);
        assert_eq!(model.read.loaded().unwrap().calendar_cursor, "2026-10-27");
        Projects::update(&mut model, Msg::ProjectCalendarToday, &ctx);
        assert_eq!(model.read.loaded().unwrap().calendar_cursor, "2026-09-27");
    }

    #[test]
    fn calendar_mode_navigation_and_optimistic_edit_stay_in_mvi() {
        let ctx = ScreenCtx::default();
        let mut model = opened();

        Projects::update(
            &mut model,
            Msg::ProjectCalendarModeSelected("week".into()),
            &ctx,
        );
        Projects::update(&mut model, Msg::ProjectCalendarNext, &ctx);
        assert_eq!(model.read.loaded().unwrap().calendar_cursor, "2026-10-04");

        Projects::update(
            &mut model,
            Msg::ProjectCalendarRecurrenceScopeSelected("future".into()),
            &ctx,
        );
        let request = Projects::update(
            &mut model,
            Msg::ProjectCalendarEditRequested {
                occurrence_id: "occ-1".into(),
                provider_event_id: "ek-1".into(),
                provider_series_id: None,
                start_at: "2026-09-29T11:00:00+00:00".into(),
                end_at: "2026-09-29T12:00:00+00:00".into(),
                all_day: false,
            },
            &ctx,
        )
        .into_requests()
        .remove(0);
        let body = request.body.clone().unwrap();
        assert_eq!(request.path, "/api/portal/rust-ui/projects/calendar");
        assert_eq!(body["eventId"], "ek-1");
        assert_eq!(body["recurrenceScope"], "future");
        let projects = model.read.loaded().unwrap();
        assert_eq!(
            projects.calendar[0].start_at, "2026-09-29T11:00:00+00:00",
            "drag/resize is optimistic while Apple delivery is queued"
        );
        assert!(projects.saving);

        Projects::update(
            &mut model,
            request.respond(Err(ApiError::network("Apple queue unavailable."))),
            &ctx,
        );
        let projects = model.read.loaded().unwrap();
        assert_eq!(projects.calendar[0].start_at, "2026-09-28T09:00:00+00:00");
        assert!(!projects.saving);
        assert_eq!(model.error.as_deref(), Some("Apple queue unavailable."));
    }

    #[test]
    fn timeline_scale_and_deadline_drag_stay_in_mvi_and_use_wbs_save() {
        let ctx = ScreenCtx::default();
        let mut model = opened();

        Projects::update(
            &mut model,
            Msg::ProjectTimelineModeSelected("month".into()),
            &ctx,
        );
        Projects::update(
            &mut model,
            Msg::ProjectTimelineGroupToggled("w1".into()),
            &ctx,
        );
        let projects = model.read.loaded().unwrap();
        assert_eq!(projects.timeline_mode, "month");
        assert!(projects.timeline_collapsed_items.contains("w1"));

        let request = Projects::update(
            &mut model,
            Msg::ProjectTimelineDueMoved {
                item_id: "w1".into(),
                due_at: "2026-10-05T00:00:00+00:00".into(),
            },
            &ctx,
        )
        .into_requests()
        .remove(0);
        let body = request.body.clone().unwrap();
        assert_eq!(request.path, "/api/portal/rust-ui/projects");
        assert_eq!(body["action"], "wbsSave");
        assert_eq!(body["itemId"], "w1");
        assert_eq!(body["dueAt"], "2026-10-05T00:00:00+00:00");
        assert_eq!(
            model
                .read
                .loaded()
                .unwrap()
                .items
                .iter()
                .find(|item| item.id == "w1")
                .and_then(|item| item.due_at.as_deref()),
            Some("2026-10-05T00:00:00+00:00"),
            "timeline drag is optimistic"
        );

        Projects::update(
            &mut model,
            request.respond(Err(ApiError::network("WBS save unavailable."))),
            &ctx,
        );
        let projects = model.read.loaded().unwrap();
        assert_eq!(
            projects
                .items
                .iter()
                .find(|item| item.id == "w1")
                .and_then(|item| item.due_at.as_deref()),
            Some("2026-09-30T00:00:00+00:00"),
            "a rejected WBS save restores the old due date"
        );
        assert!(!projects.saving);
        assert_eq!(model.error.as_deref(), Some("WBS save unavailable."));
    }

    #[test]
    fn planned_editor_drag_and_dependency_commands_use_canonical_wbs() {
        let ctx = ScreenCtx::default();
        let mut model = opened();
        Projects::update(&mut model, Msg::ProjectTimelineSortSelected("start".into()), &ctx);
        Projects::update(&mut model, Msg::ProjectTimelineToday, &ctx);
        assert_eq!(model.read.loaded().unwrap().timeline_focus_date.as_deref(), Some("2026-09-27"));
        assert_eq!(model.read.loaded().unwrap().timeline_sort_key, "start");

        Projects::update(&mut model, Msg::ProjectWorkPlannedStartChanged("2026-10-05".into()), &ctx);
        Projects::update(&mut model, Msg::ProjectWorkPlannedFinishChanged("2026-10-03".into()), &ctx);
        assert!(Projects::update(&mut model, Msg::ProjectWorkSaveRequested, &ctx).into_requests().is_empty());
        assert!(model.error.as_deref().unwrap().contains("Planned finish"));
        Projects::update(&mut model, Msg::ProjectWorkPlannedFinishChanged("2026-10-07".into()), &ctx);
        let request = Projects::update(&mut model, Msg::ProjectWorkSaveRequested, &ctx)
            .into_requests().remove(0);
        assert_eq!(request.body.as_ref().unwrap()["plannedStart"], "2026-10-05");
        assert_eq!(request.body.as_ref().unwrap()["plannedFinish"], "2026-10-07");
        Projects::update(&mut model, request.respond(Ok(page())), &ctx);
        {
            let Remote::Loaded(projects) = &mut model.read else { panic!("projects not loaded") };
            let item = &mut projects.items[0];
            item.planned_start = Some("2026-10-05".into());
            item.planned_finish = Some("2026-10-07".into());
        }
        let move_request = Projects::update(&mut model, Msg::ProjectTimelinePlannedMoved {
            item_id: "w1".into(), planned_start: "2026-10-12".into(),
        }, &ctx).into_requests().remove(0);
        assert_eq!(move_request.body.as_ref().unwrap()["plannedFinish"], "2026-10-14");
        Projects::update(&mut model, move_request.respond(Err(ApiError::network("Rejected."))), &ctx);
        assert_eq!(model.read.loaded().unwrap().items[0].planned_start.as_deref(), Some("2026-10-05"));
        assert_eq!(model.read.loaded().unwrap().items[0].planned_finish.as_deref(), Some("2026-10-07"));

        let Remote::Loaded(projects) = &mut model.read else { panic!("projects not loaded") };
        projects.items.push(PortalProjectWorkItem {
            id: "w3".into(), project_id: Some("p1".into()), title: "Publish".into(), ..Default::default()
        });
        Projects::update(&mut model, Msg::ProjectTimelineLinkTargetSelected("w3".into()), &ctx);
        let add = Projects::update(&mut model, Msg::ProjectTimelineLinkAddRequested, &ctx)
            .into_requests().remove(0);
        assert_eq!(add.body.as_ref().unwrap()["action"], "wbsDependencyAdd");
        assert_eq!(add.body.as_ref().unwrap()["sourceId"], "w3");
        assert_eq!(add.body.as_ref().unwrap()["targetId"], "w1");
        Projects::update(&mut model, add.respond(Err(ApiError::network("Link rejected."))), &ctx);
        assert_eq!(model.error.as_deref(), Some("Link rejected."));
        let Remote::Loaded(projects) = &mut model.read else { panic!("projects not loaded") };
        projects.dependencies.push(crate::model::PortalWbsDependency {
            project_id: "p1".into(), source_id: "w3".into(), target_id: "w1".into(),
            kind: "finish_to_start".into(),
        });
        let remove = Projects::update(&mut model, Msg::ProjectTimelineLinkRemoveRequested("w3".into()), &ctx)
            .into_requests().remove(0);
        assert_eq!(remove.body.as_ref().unwrap()["action"], "wbsDependencyRemove");
        assert_eq!(remove.body.as_ref().unwrap()["projectId"], "p1");
    }

    #[test]
    fn an_edit_saves_once_and_the_answer_keeps_the_selection() {
        let ctx = ScreenCtx::default();
        let mut model = opened();
        assert!(
            Projects::update(&mut model, Msg::ProjectWorkSaveRequested, &ctx)
                .into_requests()
                .is_empty(),
            "nothing to save"
        );
        Projects::update(
            &mut model,
            Msg::ProjectWorkTitleChanged("Photos + video".into()),
            &ctx,
        );
        let request = Projects::update(&mut model, Msg::ProjectWorkSaveRequested, &ctx)
            .into_requests()
            .remove(0);
        let body = request.body.clone().unwrap();
        assert_eq!(
            (body["action"].as_str(), body["title"].as_str()),
            (Some("wbsSave"), Some("Photos + video"))
        );
        assert!(
            Projects::update(&mut model, Msg::ProjectWorkSaveRequested, &ctx)
                .into_requests()
                .is_empty(),
            "one save at a time"
        );

        Projects::update(
            &mut model,
            Msg::ProjectViewSelected("timeline".into()),
            &ctx,
        );
        Projects::update(&mut model, request.respond(Ok(page())), &ctx);
        let projects = model.read.loaded().unwrap();
        assert_eq!(
            projects.active_view, "timeline",
            "the answer keeps what the user was looking at"
        );
        assert!(!projects.saving && !projects.work_dirty);

        Projects::update(&mut model, Msg::ProjectStatusRequested("done".into()), &ctx);
        Projects::update(
            &mut model,
            Msg::Saved(Err(ApiError::network("Project is archived."))),
            &ctx,
        );
        assert_eq!(model.error.as_deref(), Some("Project is archived."));
        assert!(!model.read.loaded().unwrap().saving);
    }
}
