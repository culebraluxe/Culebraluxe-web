//! UI.INTENT-008 — sort.
//!
//! CONTRACT. On the Projects timeline, choosing a sort key sorts by it ascending, choosing
//! it again flips the direction, choosing another key re-sorts ascending by the new key,
//! and an unknown key is ignored without touching the current sort. Sorting is pure view
//! state: it issues no command. The production boundary is
//! `ui::app::screens::projects::Projects` (`Msg::ProjectTimelineSortSelected`), driven
//! here through the harness's MVI `ScreenHarness`.
//!
//! Level: L1 Component — pure reducer, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_intent__008__sort

use serde_json::json;
use test_harness::mvi::screen::{self, ScreenHarness};
use ui::app::screen::ScreenCtx;
use ui::app::screens::projects::{Msg, Projects};

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
    } })
}

fn opened() -> ScreenHarness<Projects> {
    let ctx = ScreenCtx::default();
    let (mut harness, open) = ScreenHarness::<Projects>::open(ctx.clone());
    let read = open.into_requests().remove(0);
    assert_eq!(read.path, "/api/portal/rust-ui/projects");
    let loaded = harness.update(read.respond(Ok(page())));
    assert_eq!(screen::classify(&loaded), vec![screen::CommandKind::None]);
    assert!(
        harness.model().read.loaded().is_some(),
        "the projects page is on screen"
    );
    harness
}

#[test]
fn ui_intent_008__sort() {
    let mut harness = opened();

    // Choosing a sort key sorts ascending by it and issues no command.
    let sorted = harness.update(Msg::ProjectTimelineSortSelected("start".into()));
    assert_eq!(
        screen::classify(&sorted),
        vec![screen::CommandKind::None],
        "sorting is view state; it commands nothing"
    );
    {
        let projects = harness.model().read.loaded().unwrap();
        assert_eq!(projects.timeline_sort_key, "start");
        assert!(!projects.timeline_sort_desc, "a new key sorts ascending");
    }

    // Choosing it again flips the direction; choosing another key re-sorts ascending.
    harness.update(Msg::ProjectTimelineSortSelected("start".into()));
    assert!(
        harness.model().read.loaded().unwrap().timeline_sort_desc,
        "repeating the key flips to descending"
    );
    harness.update(Msg::ProjectTimelineSortSelected("title".into()));
    {
        let projects = harness.model().read.loaded().unwrap();
        assert_eq!(projects.timeline_sort_key, "title");
        assert!(
            !projects.timeline_sort_desc,
            "a new key returns to ascending"
        );
    }

    // NEGATIVE: an unknown key is ignored — the current sort stands untouched.
    let refused = harness.update(Msg::ProjectTimelineSortSelected("bogus".into()));
    assert_eq!(
        screen::classify(&refused),
        vec![screen::CommandKind::None],
        "an unknown key commands nothing"
    );
    {
        let projects = harness.model().read.loaded().unwrap();
        assert_eq!(
            (projects.timeline_sort_key.as_str(), projects.timeline_sort_desc),
            ("title", false),
            "the unknown key left the sort exactly as it was"
        );
    }
}
