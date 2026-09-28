//! CORE — Project Management (`/portal/projects`): the navigator (domains → projects → work), the project's views (work
//! plan, timeline, calendar, financials, documents, activity), catch-up, and the work-item editor.
//!
//! Yew owns every piece of state: selection, view, catch-up, the draft of the open work item, and the two writes (a
//! project's status; a work item's save). The navigator, catch-up list, timeline, calendar and asset browser were
//! JavaScript widgets and were deleted with the TypeScript (owner decision, 2026-09-26); they return as Rust ports.
//!
//! A WRITE ANSWERS WITH THE REFRESHED PAGE, and `crate::projects::carry_over` keeps what the user was looking at.

mod catch_up;
mod edits;
mod nav;
mod selection;
mod view;

use edits::*;
use selection::*;

pub use catch_up::CatchUpState;

use yew::prelude::*;

use crate::app::api::{
    CatchUpAction, CalendarCommandReceipt, CalendarCommandState, ProjectsCalendarCommandState,
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
    /// The red overdue counts are hidden. A per-device choice, kept in the browser's storage (`QUIET_KEY`).
    pub quiet: bool,
}

/// Where the overdue-count switch is remembered on this device.
const QUIET_KEY: &str = "culebraluxe.projects.quiet";

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
    /// Catch-Up's own controls and the relationship queue it shows under People.
    pub catch_up: CatchUpState,
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
    /// A pole (a property, a person, a contract, or a lens's collection) clicked: open it and show its first project,
    /// or keep the selected project when it is already one of the pole's.
    PoleSelected { pole_id: String, project_id: String },
    /// An arrow key in the navigator: "ArrowUp" / "ArrowDown" move, "ArrowLeft" / "ArrowRight" close and open.
    NavKey(String),
    /// The navigator's bell: show or hide the red overdue counts.
    QuietToggled,
    QuietLoaded(Option<String>),
    /// A "seen from" link in the project header: flip to that lens, with that record open, on the same project.
    LensJump { domain: String, pole_id: String },
    ProjectDomainSelected(String),
    ProjectSelected(String),
    ProjectNodeSelected(Option<String>),
    ProjectViewSelected(String),
    ProjectDocumentsFilterChanged(String),
    /// "Record signed copy" pressed on a document (again, to close it).
    ProjectSignedCopyStart(String),
    ProjectSignedCopyDate(String),
    /// The signed PDF chosen: it uploads at once, with the date given.
    ProjectSignedCopyChosen(web_sys::File),
    ProjectSignedCopySaved(Result<serde_json::Value, ApiError>),
    /// Signed, but no PDF yet: "copy to come".
    ProjectSignedCopyToCome,
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
    /// Catch-Up's worklist: "today", "unscheduled" or "people".
    CatchUpTabSelected(String),
    /// "all", "open", "doing" or "done".
    CatchUpStatusSelected(String),
    /// A work-item category, or "all".
    CatchUpAreaSelected(String),
    PeopleLoaded(Result<PortalPage, ApiError>),
    PeopleSelected(String),
    PeopleHandleRequested {
        person_id: String,
        reason_code: String,
    },
    PeopleSnoozeRequested {
        person_id: String,
        reason_code: String,
        days: i32,
    },
    PeopleAnswered(Result<PortalPage, ApiError>),
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

impl Screen for Projects {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
                ..Model::default()
            },
            Cmd::batch([
                Cmd::request(ProjectsRead, Msg::Loaded),
                Cmd::storage_read(QUIET_KEY, Msg::QuietLoaded),
            ]),
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
            Msg::QuietLoaded(value) => {
                model.controls.quiet = value.as_deref() == Some("1");
                return Cmd::none();
            }
            Msg::QuietToggled => {
                model.controls.quiet = !model.controls.quiet;
                return Cmd::storage_write(QUIET_KEY, Some(if model.controls.quiet { "1" } else { "0" }.to_owned()));
            }
            Msg::NavKey(key) => {
                return nav_key(model, &key, _ctx);
            }
            Msg::LensJump { domain, pole_id } => {
                model.controls.nav_closed.remove(&pole_id);
                model.controls.nav_open.insert(pole_id);
                model.controls.query.clear();
                return Self::update(model, Msg::ProjectDomainSelected(domain), _ctx);
            }
            Msg::PoleSelected { pole_id, project_id } => {
                model.controls.nav_closed.remove(&pole_id);
                model.controls.nav_open.insert(pole_id);
                let already = matches!(&model.read, Remote::Loaded(projects)
                    if projects.selected_project_id.as_deref() == Some(project_id.as_str()) && !projects.catch_up);
                if already {
                    return Cmd::none();
                }
                return Self::update(model, Msg::ProjectSelected(project_id), _ctx);
            }
            Msg::NavWorkSelected { project_id, node_id } => {
                Self::update(model, Msg::ProjectSelected(project_id), _ctx);
                return Self::update(model, Msg::ProjectNodeSelected(Some(node_id)), _ctx);
            }
            Msg::QueryChanged(query) => {
                model.controls.query = query;
                return Cmd::none();
            }
            Msg::ProjectCatchUpToggled(on) => {
                let cmd = match &mut model.read {
                    Remote::Loaded(projects) => {
                        selection(projects, &mut model.error, Msg::ProjectCatchUpToggled(on))
                    }
                    _ => Cmd::none(),
                };
                return Cmd::batch([cmd, catch_up::load_people(&mut model.catch_up)]);
            }
            Msg::CatchUpTabSelected(tab) => {
                if matches!(tab.as_str(), "today" | "unscheduled" | "people") {
                    model.catch_up.tab = tab;
                }
                return catch_up::load_people(&mut model.catch_up);
            }
            Msg::CatchUpStatusSelected(status) => {
                if matches!(status.as_str(), "all" | "open" | "doing" | "done") {
                    model.catch_up.status = status;
                }
                return Cmd::none();
            }
            Msg::CatchUpAreaSelected(area) => {
                model.catch_up.area = area;
                return Cmd::none();
            }
            Msg::PeopleLoaded(answer) => {
                catch_up::apply_people(&mut model.catch_up, answer, false);
                return Cmd::none();
            }
            Msg::PeopleSelected(person_id) => {
                model.catch_up.person = Some(person_id);
                return Cmd::none();
            }
            Msg::PeopleHandleRequested {
                person_id,
                reason_code,
            } => {
                return catch_up::act(
                    &mut model.catch_up,
                    CatchUpAction::handle(person_id, reason_code),
                );
            }
            Msg::PeopleSnoozeRequested {
                person_id,
                reason_code,
                days,
            } => {
                return catch_up::act(
                    &mut model.catch_up,
                    CatchUpAction::snooze(person_id, reason_code, days),
                );
            }
            Msg::PeopleAnswered(answer) => {
                model.catch_up.busy = false;
                catch_up::apply_people(&mut model.catch_up, answer, true);
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
                    catch_up: &model.catch_up,
                },
                projects,
                &on_msg,
            )
        })
    }
}

/// What the view reads besides the page.
pub struct Vm<'a> {
    pub controls: &'a Controls,
    pub error: Option<&'a String>,
    pub(super) calendar_pending: Option<&'a PendingCalendarEdit>,
    pub(super) timeline_pending: Option<&'a PendingTimelineEdit>,
    pub catch_up: &'a CatchUpState,
}

#[cfg(test)]
mod tests;
