//! What the operator is looking at: domain (lens), pole, project, work item, view, catch-up — every intent that moves the
//! selection on a loaded page, and the arrow keys over the navigator.

use super::*;

/// The arrow keys over the drawn tree. Up and down move between projects and work: an open pole is a heading, a closed
/// one is entered at its nearest project, and the selected project's own row is skipped going up (selecting it lands
/// back on its first work item). Right
/// opens the row, left closes it — or, on a closed or leaf row, closes the branch it sits in.
pub(super) fn nav_key(model: &mut Model, key: &str, ctx: &ScreenCtx) -> Cmd<Msg> {
    let Remote::Loaded(projects) = &model.read else {
        return Cmd::none();
    };
    let tree = nav::build(projects);
    let selected = nav::selected_id(&tree, projects.selected_project_id.as_deref(), projects.selected_node_id.as_deref());
    let opened = selected.as_deref().map(|id| nav::ancestors(&tree, id)).unwrap_or_default();
    let query = model.controls.query.trim().to_lowercase();
    let rows = nav::visible_rows(&tree, &query, &model.controls.nav_open, &model.controls.nav_closed, &opened);
    // The selection's row, or — when a closed branch hides it — the nearest ancestor that is drawn.
    let current = selected
        .iter()
        .chain(opened.iter().rev())
        .find_map(|id| rows.iter().position(|row| &row.id == id));
    let current_project = projects.selected_project_id.clone();
    let step = |down: bool| -> Option<Msg> {
        let indices: Box<dyn Iterator<Item = usize>> = match (current, down) {
            (Some(at), true) => Box::new(at + 1..rows.len()),
            (Some(at), false) => Box::new((0..at).rev()),
            (None, _) => Box::new(0..rows.len()),
        };
        indices.map(|index| &rows[index]).find_map(|row| match row.kind {
            // An open pole is a heading (its projects follow); a closed one is entered at its nearest project.
            nav::NodeKind::Pole if row.open => None,
            nav::NodeKind::Pole => {
                let entry = if down { row.projects.first() } else { row.projects.last() };
                entry.filter(|id| Some(*id) != current_project.as_ref()).cloned().map(Msg::ProjectSelected)
            }
            nav::NodeKind::Project if !down && row.project_id == current_project => None,
            nav::NodeKind::Project => row.project_id.clone().map(Msg::ProjectSelected),
            nav::NodeKind::Work => Some(Msg::NavWorkSelected {
                project_id: row.project_id.clone().unwrap_or_default(),
                node_id: row.work_id.clone().unwrap_or_default(),
            }),
        })
    };
    let row = current.map(|at| rows[at].clone());
    let msg = match key {
        "ArrowDown" => step(true),
        "ArrowUp" => step(false),
        "ArrowRight" => row.filter(|row| row.has_children && !row.open).map(|row| Msg::NavToggled { id: row.id, open: false }),
        "ArrowLeft" => row.and_then(|row| {
            if row.has_children && row.open {
                Some(Msg::NavToggled { id: row.id, open: true })
            } else {
                row.parent.map(|parent| Msg::NavToggled { id: parent, open: true })
            }
        }),
        _ => None,
    };
    match msg {
        Some(msg) => Projects::update(model, msg, ctx),
        None => Cmd::none(),
    }
}

/// Selection, views, catch-up and the two writes, on a loaded page.
pub(super) fn selection(projects: &mut PortalProjectsPage, error: &mut Option<String>, msg: Msg) -> Cmd<Msg> {
    match msg {
        Msg::ProjectDomainSelected(domain) => {
            if matches!(
                domain.as_str(),
                "properties" | "people" | "deals" | "firm" | "marketing" | "accounting"
            ) {
                // THE LENS CHANGES, THE PROJECT DOES NOT. Flipping from Properties to People shows the same project from
                // the other side (under its seller, say); only a project the new lens cannot show falls back to that
                // lens's first one.
                let keeps = projects.selected_project_id.as_deref().is_some_and(|id| {
                    projects.projects.iter().any(|project| {
                        project.id == id
                            && crate::projects::project_in_domain(project, &projects.items, &domain)
                    })
                });
                if !keeps {
                    projects.selected_project_id = first_project_for_domain(projects, &domain);
                    projects.selected_node_id =
                        first_node_for_project(projects, projects.selected_project_id.as_deref());
                    projects.active_view = "work-plan".into();
                    projects.work_collapsed = false;
                    projects.work_dirty = false;
                }
                projects.active_domain = domain;
                projects.catch_up = false;
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
        Msg::ProjectDocumentsFilterChanged(filter) => {
            if matches!(filter.as_str(), "all" | "document" | "photo" | "video") {
                projects.documents_filter = filter;
            }
        }
        Msg::ProjectSignedCopyStart(document_id) => {
            if projects.signing_document_id.as_deref() == Some(document_id.as_str()) {
                projects.signing_document_id = None;
            } else {
                projects.signing_document_id = Some(document_id);
                projects.signing_date = projects.calendar_today.clone();
            }
        }
        Msg::ProjectSignedCopyDate(date) => projects.signing_date = date,
        Msg::ProjectSignedCopyChosen(file) => {
            let Some(document_id) = projects.signing_document_id.clone() else {
                return Cmd::none();
            };
            // One request carries the PDF; the gateway takes about 4.5 MB.
            if file.size() > 4.0 * 1024.0 * 1024.0 {
                *error = Some("That PDF is over 4 MB — save it smaller (fewer pages or a lower scan resolution) and choose it again.".into());
                return Cmd::none();
            }
            if projects.signing_date.trim().is_empty() {
                *error = Some("Give the date it was signed, then choose the PDF.".into());
                return Cmd::none();
            }
            projects.signing_busy = true;
            *error = None;
            let fields = vec![
                ("signedAt".to_string(), projects.signing_date.clone()),
                ("projectId".to_string(), projects.selected_project_id.clone().unwrap_or_default()),
            ];
            return Cmd::post_form(
                crate::app::api::ProjectSignedCopy { document_id },
                fields,
                file,
                Msg::ProjectSignedCopySaved,
            );
        }
        Msg::ProjectSignedCopyToCome => {
            let Some(document_id) = projects.signing_document_id.clone() else {
                return Cmd::none();
            };
            if projects.signing_date.trim().is_empty() {
                *error = Some("Give the date it was signed.".into());
                return Cmd::none();
            }
            projects.signing_busy = true;
            *error = None;
            return Cmd::request(
                crate::app::api::ProjectDocumentSignedCopyToCome {
                    document_id,
                    project_id: projects.selected_project_id.clone().unwrap_or_default(),
                    signed_at: projects.signing_date.clone(),
                },
                Msg::ProjectSignedCopySaved,
            );
        }
        Msg::ProjectSignedCopySaved(result) => {
            projects.signing_busy = false;
            match result {
                Ok(_) => {
                    projects.signing_document_id = None;
                    *error = None;
                    return Cmd::request(ProjectsRead, Msg::Loaded);
                }
                Err(failure) => *error = Some(format!("The signed copy was not recorded: {}", failure.message)),
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
