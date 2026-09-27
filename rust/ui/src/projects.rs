//! Project Management's pure rules: which projects belong to a domain, where a selection lands, and how a fresh answer
//! keeps what the user was looking at. Pure, so they are tested on the host and shared by the screen and the old loop.

use crate::model::PortalProjectsPage;

pub fn project_in_domain(
    project: &crate::model::PortalProject,
    items: &[crate::model::PortalProjectWorkItem],
    domain: &str,
) -> bool {
    let project_items = items
        .iter()
        .filter(|item| item.project_id.as_deref() == Some(project.id.as_str()));
    match domain {
        "properties" => {
            project.property_id.is_some()
                || project
                    .areas
                    .iter()
                    .any(|area| area == "properties" || area == "media")
                || project_items.clone().any(|item| {
                    item.entity
                        .as_ref()
                        .is_some_and(|entity| entity.entity_type == "property")
                })
        }
        "people" => {
            project.person_id.is_some()
                || project.areas.iter().any(|area| area == "clients")
                || project_items.clone().any(|item| {
                    item.entity
                        .as_ref()
                        .is_some_and(|entity| entity.entity_type == "person")
                })
        }
        "deals" => {
            project.contract_id.is_some()
                || project.areas.iter().any(|area| area == "contracts")
                || project_items.clone().any(|item| {
                    item.entity.as_ref().is_some_and(|entity| {
                        matches!(entity.entity_type.as_str(), "contract" | "deal")
                    })
                })
        }
        "marketing" => project.areas.iter().any(|area| area == "marketing"),
        "accounting" => project.areas.iter().any(|area| area == "accounting"),
        "firm" => {
            project.areas.iter().any(|area| area == "management")
                || (project.person_id.is_none()
                    && project.property_id.is_none()
                    && project.contract_id.is_none())
        }
        _ => false,
    }
}

pub fn initial_project_domain(projects: &PortalProjectsPage) -> String {
    const DOMAINS: &[&str] = &[
        "properties",
        "people",
        "deals",
        "firm",
        "marketing",
        "accounting",
    ];
    DOMAINS
        .iter()
        .find(|domain| {
            projects
                .projects
                .iter()
                .any(|project| project_in_domain(project, &projects.items, domain))
        })
        .copied()
        .unwrap_or("properties")
        .to_string()
}

pub fn first_project_for_domain(projects: &PortalProjectsPage, domain: &str) -> Option<String> {
    projects
        .projects
        .iter()
        .find(|project| project_in_domain(project, &projects.items, domain))
        .or_else(|| projects.projects.first())
        .map(|project| project.id.clone())
}

pub fn first_node_for_project(
    projects: &PortalProjectsPage,
    project_id: Option<&str>,
) -> Option<String> {
    let project_id = project_id?;
    projects
        .items
        .iter()
        .filter(|item| item.project_id.as_deref() == Some(project_id))
        .find(|item| matches!(item.status.as_str(), "doing" | "open"))
        .or_else(|| {
            projects
                .items
                .iter()
                .find(|item| item.project_id.as_deref() == Some(project_id))
        })
        .map(|item| item.id.clone())
}

/// Take a fresh Projects answer, keeping what the user was looking at: the domain, the project and work item (while they
/// still exist), the view, catch-up and the editor's collapse. A first answer gets the first domain with a project in it.
/// Unsaved edits are gone either way: the answer is the saved state.
pub fn carry_over(previous: Option<&PortalProjectsPage>, projects: &mut PortalProjectsPage) {
    if let Some(previous) = previous {
        projects.active_domain = if previous.active_domain.is_empty() {
            initial_project_domain(projects)
        } else {
            previous.active_domain.clone()
        };
        projects.selected_project_id = previous
            .selected_project_id
            .clone()
            .filter(|id| projects.projects.iter().any(|project| &project.id == id))
            .or_else(|| first_project_for_domain(projects, &projects.active_domain));
        projects.selected_node_id = previous
            .selected_node_id
            .clone()
            .filter(|id| {
                projects.items.iter().any(|item| {
                    &item.id == id
                        && item.project_id.as_deref() == projects.selected_project_id.as_deref()
                })
            })
            .or_else(|| first_node_for_project(projects, projects.selected_project_id.as_deref()));
        projects.active_view = if previous.active_view.is_empty() {
            "work-plan".into()
        } else {
            previous.active_view.clone()
        };
        projects.catch_up = previous.catch_up;
        projects.work_collapsed = previous.work_collapsed;
        projects.calendar_cursor = if previous.calendar_cursor.is_empty() {
            calendar_anchor(projects)
        } else {
            previous.calendar_cursor.clone()
        };
        projects.calendar_mode = if previous.calendar_mode.is_empty() {
            "month".into()
        } else {
            previous.calendar_mode.clone()
        };
        projects.calendar_recurrence_scope = if previous.calendar_recurrence_scope.is_empty() {
            "this".into()
        } else {
            previous.calendar_recurrence_scope.clone()
        };
        // Calendar data is owned by the viewport endpoint, not by a refreshed
        // Projects workspace answer. Preserve it across unrelated project/WBS writes.
        projects.calendar = previous.calendar.clone();
        projects.calendar_filter = if previous.calendar_filter.is_empty() {
            "all".into()
        } else {
            previous.calendar_filter.clone()
        };
        projects.calendar_selected_event_id = previous
            .calendar_selected_event_id
            .clone()
            .filter(|id| projects.calendar.iter().any(|event| &event.id == id));
        projects.calendar_dragging_event_id = None;
        projects.calendar_drag_target = None;
        projects.calendar_loading = previous.calendar_loading;
        projects.calendar_loaded_start = previous.calendar_loaded_start.clone();
        projects.calendar_loaded_end = previous.calendar_loaded_end.clone();
    } else {
        projects.active_domain = initial_project_domain(projects);
        projects.selected_project_id = first_project_for_domain(projects, &projects.active_domain);
        projects.selected_node_id =
            first_node_for_project(projects, projects.selected_project_id.as_deref());
        projects.active_view = "work-plan".into();
        projects.work_collapsed = false;
        projects.calendar_cursor = calendar_anchor(projects);
        projects.calendar_mode = "month".into();
        projects.calendar_recurrence_scope = "this".into();
        projects.calendar_filter = "all".into();
        projects.calendar_selected_event_id = None;
        projects.calendar_dragging_event_id = None;
        projects.calendar_drag_target = None;
        projects.calendar_loading = false;
        projects.calendar_loaded_start = None;
        projects.calendar_loaded_end = None;
    }
    if projects.calendar_day_start_hour >= projects.calendar_day_end_hour {
        projects.calendar_day_start_hour = 8;
        projects.calendar_day_end_hour = 20;
    }
    if !matches!(projects.calendar_slot_minutes, 15 | 30 | 60) {
        projects.calendar_slot_minutes = 30;
    }
    projects.work_dirty = false;
    projects.saving = false;
}

/// Stable initial cursor for the calendar. The server supplies today so the
/// reducer remains deterministic; older fixtures fall back to a dated event/work item.
pub fn calendar_anchor(projects: &PortalProjectsPage) -> String {
    if !projects.calendar_today.is_empty() {
        return projects.calendar_today.clone();
    }
    projects
        .calendar
        .iter()
        .find_map(|event| crate::calendar::date_key(&event.start_at))
        .or_else(|| {
            projects
                .items
                .iter()
                .find_map(|item| item.due_at.as_deref().and_then(crate::calendar::date_key))
        })
        .unwrap_or_else(|| "1970-01-01".into())
}
