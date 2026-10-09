//! UI.INTENT-005 — drag.
//!
//! CONTRACT. On the TECH cockpit sorter, picking up a card (`Msg::SorterDragStarted`)
//! only remembers it, and dropping it (`Msg::SorterDropped`) moves its story to the
//! target column with exactly one `moveStoryBucket` command — unless nothing was picked
//! up, or the card is dropped where it already is, in which case nothing happens. The
//! production boundary is `ui::app::screens::tech::TechCockpit`, driven here through the
//! harness's MVI `ScreenHarness`.
//!
//! Level: L1 Component — pure reducer, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_intent__005__drag

use serde_json::json;
use test_harness::mvi::screen::{self, ScreenHarness};
use ui::app::screen::ScreenCtx;
use ui::app::screens::tech::{Msg, TechCockpit};

fn loaded() -> ScreenHarness<TechCockpit> {
    let ctx = ScreenCtx::default();
    let (mut harness, open) = ScreenHarness::<TechCockpit>::open(ctx.clone());
    let read = open.into_requests().remove(0);
    assert_eq!(read.path, "/api/portal/rust-ui/tech");
    let answered = harness.update(read.respond(Ok(json!({ "tech": {
        "sorterCards": [{ "id": "FORGE-9#handoff", "column": "engine" }],
    } }))));
    assert!(
        answered.into_requests().is_empty(),
        "the cockpit read answers quietly"
    );
    assert!(
        harness.model().read.loaded().is_some(),
        "the cockpit page is on screen"
    );
    harness
}

#[test]
fn ui_intent_005__drag() {
    let mut harness = loaded();

    // Picking up a card is state only: no command leaves the screen.
    let picked = harness.update(Msg::SorterDragStarted("FORGE-9#handoff".into()));
    assert_eq!(
        screen::classify(&picked),
        vec![screen::CommandKind::None],
        "picking up a card moves nothing yet"
    );

    // NEGATIVE: dropping the card where it already sits moves nothing.
    let stayed = harness.update(Msg::SorterDropped("engine".into()));
    assert!(stayed.into_requests().is_empty(), "same column: no command");

    // Dropping it on another column moves its story there with one command.
    harness.update(Msg::SorterDragStarted("FORGE-9#handoff".into()));
    let moved = harness.update(Msg::SorterDropped("backlog".into()));
    let request = moved.into_requests();
    assert_eq!(request.len(), 1, "a real drop sends exactly one command");
    let body = request[0].body.clone().unwrap();
    assert_eq!(
        (
            body["action"].as_str(),
            body["storyId"].as_str(),
            body["target"].as_str()
        ),
        (Some("moveStoryBucket"), Some("FORGE-9"), Some("backlog")),
        "the drop moves the story (before the #), not the card id"
    );

    // NEGATIVE: a drop with nothing picked up is ignored — a stray drop moves no story.
    let mut harness = loaded();
    let stray = harness.update(Msg::SorterDropped("backlog".into()));
    assert!(stray.into_requests().is_empty(), "no pickup, no command");
}
