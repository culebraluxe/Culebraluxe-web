//! CORE — Project Management (`/portal/projects`): the navigator (domains → projects → work), the project's views (work
//! plan, timeline, calendar, financials, documents, activity), catch-up, and the work-item editor.
//!
//! Yew owns every piece of state: selection, view, catch-up, the draft of the open work item, and the two writes (a
//! project's status; a work item's save). The mature widgets — Arborist navigator, catch-up list, SVAR Gantt,
//! FullCalendar, the asset browser — are islands: they draw from props and report intents as events.
//!
//! A WRITE ANSWERS WITH THE REFRESHED PAGE, and `crate::projects::carry_over` keeps what the user was looking at.

mod view;

use yew::prelude::*;

use crate::app::api::{ProjectsCommand, ProjectsRead};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{PortalPage, PortalProjectWorkItem, PortalProjectsPage};
use crate::projects::{carry_over, first_node_for_project, first_project_for_domain};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Controls {
    /// The navigator's filter, as typed.
    pub query: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalProjectsPage>,
    pub controls: Controls,
    /// Why the last write did not happen, in the service's words.
    pub error: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    Saved(Result<PortalPage, ApiError>),
    /// An intent from the navigator or catch-up island (`{ kind: "domain" | "project" | "work" | ... }`).
    Navigator(serde_json::Value),
    QueryChanged(String),
    ProjectDomainSelected(String),
    ProjectSelected(String),
    ProjectNodeSelected(Option<String>),
    ProjectViewSelected(String),
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

fn field<'a>(event: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    event.get(name).and_then(|value| value.as_str())
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

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        // Intents that need a loaded page are ignored before it arrives.
        match msg {
            Msg::Loaded(result) => {
                if let Err(error) = answer(model, result) {
                    model.read = Remote::Failed(error);
                }
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
            Msg::Navigator(event) => {
                let msg = match field(&event, "kind") {
                    Some("query") => {
                        field(&event, "query").map(|query| Msg::QueryChanged(query.into()))
                    }
                    Some("domain") => field(&event, "domain")
                        .map(|domain| Msg::ProjectDomainSelected(domain.into())),
                    Some("catchup") => Some(Msg::ProjectCatchUpToggled(true)),
                    Some("project") => {
                        field(&event, "projectId").map(|id| Msg::ProjectSelected(id.into()))
                    }
                    Some(kind @ ("catchupSelect" | "catchupComplete" | "work")) => {
                        match (field(&event, "projectId"), field(&event, "nodeId")) {
                            (Some(project_id), Some(node_id)) => {
                                let (project_id, node_id) =
                                    (project_id.to_string(), node_id.to_string());
                                if kind == "work" {
                                    Self::update(model, Msg::ProjectSelected(project_id), ctx);
                                    Some(Msg::ProjectNodeSelected(Some(node_id)))
                                } else if kind == "catchupSelect" {
                                    Some(Msg::ProjectCatchUpItemSelected {
                                        project_id,
                                        node_id,
                                    })
                                } else {
                                    Some(Msg::ProjectCatchUpItemCompleteRequested {
                                        project_id,
                                        node_id,
                                    })
                                }
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                return match msg {
                    Some(msg) => Self::update(model, msg, ctx),
                    None => Cmd::none(),
                };
            }
            Msg::QueryChanged(query) => {
                model.controls.query = query;
                return Cmd::none();
            }
            Msg::ProjectWorkTitleChanged(value) => edit(model, |item| item.title = value),
            Msg::ProjectWorkNotesChanged(value) => edit(model, |item| item.notes = value),
            Msg::ProjectWorkOwnerChanged(value) => edit(model, |item| {
                item.owner = (!value.trim().is_empty()).then_some(value)
            }),
            Msg::ProjectWorkDueChanged(value) => edit(model, |item| {
                item.due_at = (!value.trim().is_empty()).then_some(value)
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
                projects.work_dirty = false;
            }
        }
        Msg::ProjectViewSelected(view) => {
            if matches!(
                view.as_str(),
                "work-plan" | "timeline" | "calendar" | "financials" | "documents" | "activity"
            ) {
                projects.active_view = view;
                projects.catch_up = false;
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page() -> serde_json::Value {
        json!({ "projects": {
            "projects": [
                { "id": "p1", "name": "Villa listing", "propertyId": "prop-1", "status": "doing" },
                { "id": "p2", "name": "Firm ops", "status": "open" }
            ],
            "items": [
                { "id": "w1", "projectId": "p1", "title": "Photos", "status": "doing" },
                { "id": "w2", "projectId": "p2", "title": "Books", "status": "open" }
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
    fn the_first_answer_opens_the_first_domain_with_work_and_islands_steer_it() {
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
        Projects::update(
            &mut model,
            Msg::Navigator(json!({ "kind": "work", "projectId": "p2", "nodeId": "w2" })),
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
        Projects::update(
            &mut model,
            Msg::Navigator(json!({ "kind": "query", "query": "villa" })),
            &ctx,
        );
        assert_eq!(model.controls.query, "villa");
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
