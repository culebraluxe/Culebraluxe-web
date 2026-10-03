//! The Projects screen's update tests: selection, the navigator, lenses, keys, calendar and timeline edits.

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

fn navigator_page() -> serde_json::Value {
    json!({ "projects": {
        "calendarToday": "2026-09-28",
        "identityNames": { "property:prop-1": "Casa Luar", "person:per-1": "Ana Rivera", "property:prop-2": "Villa Mar" },
        "projects": [
            { "id": "p1", "name": "Casa Luar Listing", "propertyId": "prop-1", "personId": "per-1", "status": "doing" },
            { "id": "p2", "name": "Villa Mar Listing", "propertyId": "prop-2", "status": "open" }
        ],
        "items": [
            { "id": "a1", "projectId": "p1", "title": "Clients / Parties", "status": "done", "order": 1 },
            { "id": "a2", "projectId": "p1", "title": "Listing Agreement", "status": "open", "order": 2, "dueAt": "2026-09-20" },
            { "id": "b1", "projectId": "p2", "title": "Photos", "status": "open", "order": 1 }
        ]
    } })
}

fn navigator() -> Model {
    let ctx = ScreenCtx::default();
    let (mut model, cmd) = Projects::init(&ctx);
    let request = cmd.into_requests().remove(0);
    Projects::update(&mut model, request.respond(Ok(navigator_page())), &ctx);
    model
}

fn at(model: &Model) -> (String, Option<String>, String) {
    let p = model.read.loaded().unwrap();
    (
        p.active_domain.clone(),
        p.selected_project_id.clone(),
        p.selected_node_id.clone().unwrap_or_default(),
    )
}

#[test]
fn the_bell_hides_the_overdue_counts_and_remembers_it_on_this_device() {
    let ctx = ScreenCtx::default();
    let mut model = navigator();
    assert!(
        !model.controls.quiet,
        "counts show until someone turns them off"
    );
    let cmd = Projects::update(&mut model, Msg::QuietToggled, &ctx);
    assert!(model.controls.quiet);
    assert!(
        format!("{cmd:?}").contains("culebraluxe.projects.quiet"),
        "the choice is written to the device"
    );
    Projects::update(&mut model, Msg::QuietLoaded(Some("0".into())), &ctx);
    assert!(!model.controls.quiet);
    Projects::update(&mut model, Msg::QuietLoaded(Some("1".into())), &ctx);
    assert!(model.controls.quiet);
}

#[test]
fn a_lens_flip_keeps_the_project_when_the_lens_can_show_it() {
    let ctx = ScreenCtx::default();
    let mut model = navigator();
    Projects::update(
        &mut model,
        Msg::ProjectViewSelected("timeline".into()),
        &ctx,
    );
    Projects::update(
        &mut model,
        Msg::ProjectDomainSelected("people".into()),
        &ctx,
    );
    assert_eq!(
        at(&model).1.as_deref(),
        Some("p1"),
        "Casa Luar Listing, now seen under Ana Rivera"
    );
    assert_eq!(
        model.read.loaded().unwrap().active_view,
        "timeline",
        "and on the same tab"
    );

    Projects::update(
        &mut model,
        Msg::ProjectDomainSelected("properties".into()),
        &ctx,
    );
    Projects::update(&mut model, Msg::ProjectSelected("p2".into()), &ctx);
    Projects::update(
        &mut model,
        Msg::ProjectDomainSelected("people".into()),
        &ctx,
    );
    assert_eq!(
        at(&model).1.as_deref(),
        Some("p1"),
        "Villa Mar has no person, so People falls back to its first"
    );
}

#[test]
fn clicking_a_pole_opens_it_on_its_project_and_a_second_click_keeps_the_tab() {
    let ctx = ScreenCtx::default();
    let mut model = navigator();
    Projects::update(
        &mut model,
        Msg::PoleSelected {
            pole_id: "entity-property-prop-2".into(),
            project_id: "p2".into(),
        },
        &ctx,
    );
    assert_eq!(
        at(&model),
        ("properties".into(), Some("p2".into()), "b1".into())
    );
    assert!(model.controls.nav_open.contains("entity-property-prop-2"));

    Projects::update(
        &mut model,
        Msg::ProjectViewSelected("documents".into()),
        &ctx,
    );
    Projects::update(
        &mut model,
        Msg::PoleSelected {
            pole_id: "entity-property-prop-2".into(),
            project_id: "p2".into(),
        },
        &ctx,
    );
    assert_eq!(model.read.loaded().unwrap().active_view, "documents");
}

#[test]
fn the_arrow_keys_walk_the_tree_and_enter_closed_poles() {
    let ctx = ScreenCtx::default();
    let mut model = navigator();
    assert_eq!(
        at(&model).2,
        "a2",
        "a project opens on its first unfinished work"
    );
    Projects::update(&mut model, Msg::NavKey("ArrowUp".into()), &ctx);
    assert_eq!(at(&model).2, "a1");
    Projects::update(&mut model, Msg::NavKey("ArrowDown".into()), &ctx);
    assert_eq!(at(&model).2, "a2");
    Projects::update(&mut model, Msg::NavKey("ArrowDown".into()), &ctx);
    assert_eq!(
        (at(&model).1.as_deref(), at(&model).2.as_str()),
        (Some("p2"), "b1"),
        "into the next, closed pole"
    );
    Projects::update(&mut model, Msg::NavKey("ArrowUp".into()), &ctx);
    assert_eq!(
        at(&model).1.as_deref(),
        Some("p1"),
        "and back up into the first"
    );

    // Left closes the branch the selection sits in; right opens it again.
    Projects::update(&mut model, Msg::NavKey("ArrowLeft".into()), &ctx);
    assert!(model
        .controls
        .nav_closed
        .iter()
        .any(|id| id.ends_with("::p1")));
    Projects::update(&mut model, Msg::NavKey("ArrowRight".into()), &ctx);
    assert!(!model
        .controls
        .nav_closed
        .iter()
        .any(|id| id.ends_with("::p1")));
}

#[test]
fn seen_from_lists_every_lens_and_a_jump_flips_to_that_record() {
    let ctx = ScreenCtx::default();
    let mut model = navigator();
    let page = model.read.loaded().unwrap().clone();
    let lenses: Vec<(String, String)> = nav::lenses(&page, &page.projects[0])
        .into_iter()
        .map(|lens| (lens.domain.to_owned(), lens.label))
        .collect();
    assert!(lenses.contains(&("properties".into(), "Casa Luar".into())));
    assert!(lenses.contains(&("people".into(), "Ana Rivera".into())));

    Projects::update(
        &mut model,
        Msg::LensJump {
            domain: "people".into(),
            pole_id: "entity-person-per-1".into(),
        },
        &ctx,
    );
    assert_eq!(at(&model).0, "people");
    assert_eq!(at(&model).1.as_deref(), Some("p1"));
    assert!(model.controls.nav_open.contains("entity-person-per-1"));
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
    Projects::update(
        &mut model,
        Msg::ProjectTimelineSortSelected("start".into()),
        &ctx,
    );
    Projects::update(&mut model, Msg::ProjectTimelineToday, &ctx);
    assert_eq!(
        model.read.loaded().unwrap().timeline_focus_date.as_deref(),
        Some("2026-09-27")
    );
    assert_eq!(model.read.loaded().unwrap().timeline_sort_key, "start");

    Projects::update(
        &mut model,
        Msg::ProjectWorkPlannedStartChanged("2026-10-05".into()),
        &ctx,
    );
    Projects::update(
        &mut model,
        Msg::ProjectWorkPlannedFinishChanged("2026-10-03".into()),
        &ctx,
    );
    assert!(
        Projects::update(&mut model, Msg::ProjectWorkSaveRequested, &ctx)
            .into_requests()
            .is_empty()
    );
    assert!(model.error.as_deref().unwrap().contains("Planned finish"));
    Projects::update(
        &mut model,
        Msg::ProjectWorkPlannedFinishChanged("2026-10-07".into()),
        &ctx,
    );
    let request = Projects::update(&mut model, Msg::ProjectWorkSaveRequested, &ctx)
        .into_requests()
        .remove(0);
    assert_eq!(request.body.as_ref().unwrap()["plannedStart"], "2026-10-05");
    assert_eq!(
        request.body.as_ref().unwrap()["plannedFinish"],
        "2026-10-07"
    );
    Projects::update(&mut model, request.respond(Ok(page())), &ctx);
    {
        let Remote::Loaded(projects) = &mut model.read else {
            panic!("projects not loaded")
        };
        let item = &mut projects.items[0];
        item.planned_start = Some("2026-10-05".into());
        item.planned_finish = Some("2026-10-07".into());
    }
    let move_request = Projects::update(
        &mut model,
        Msg::ProjectTimelinePlannedMoved {
            item_id: "w1".into(),
            planned_start: "2026-10-12".into(),
        },
        &ctx,
    )
    .into_requests()
    .remove(0);
    assert_eq!(
        move_request.body.as_ref().unwrap()["plannedFinish"],
        "2026-10-14"
    );
    Projects::update(
        &mut model,
        move_request.respond(Err(ApiError::network("Rejected."))),
        &ctx,
    );
    assert_eq!(
        model.read.loaded().unwrap().items[0]
            .planned_start
            .as_deref(),
        Some("2026-10-05")
    );
    assert_eq!(
        model.read.loaded().unwrap().items[0]
            .planned_finish
            .as_deref(),
        Some("2026-10-07")
    );

    let Remote::Loaded(projects) = &mut model.read else {
        panic!("projects not loaded")
    };
    projects.items.push(PortalProjectWorkItem {
        id: "w3".into(),
        project_id: Some("p1".into()),
        title: "Publish".into(),
        ..Default::default()
    });
    Projects::update(
        &mut model,
        Msg::ProjectTimelineLinkTargetSelected("w3".into()),
        &ctx,
    );
    let add = Projects::update(&mut model, Msg::ProjectTimelineLinkAddRequested, &ctx)
        .into_requests()
        .remove(0);
    assert_eq!(add.body.as_ref().unwrap()["action"], "wbsDependencyAdd");
    assert_eq!(add.body.as_ref().unwrap()["sourceId"], "w3");
    assert_eq!(add.body.as_ref().unwrap()["targetId"], "w1");
    Projects::update(
        &mut model,
        add.respond(Err(ApiError::network("Link rejected."))),
        &ctx,
    );
    assert_eq!(model.error.as_deref(), Some("Link rejected."));
    let Remote::Loaded(projects) = &mut model.read else {
        panic!("projects not loaded")
    };
    projects
        .dependencies
        .push(crate::model::PortalWbsDependency {
            project_id: "p1".into(),
            source_id: "w3".into(),
            target_id: "w1".into(),
            kind: "finish_to_start".into(),
        });
    let remove = Projects::update(
        &mut model,
        Msg::ProjectTimelineLinkRemoveRequested("w3".into()),
        &ctx,
    )
    .into_requests()
    .remove(0);
    assert_eq!(
        remove.body.as_ref().unwrap()["action"],
        "wbsDependencyRemove"
    );
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
