//! UI.INTENT-003 — delete.
//!
//! CONTRACT. On the OPPS Data Workbench, deleting a photograph is a two-press intent:
//! the first press on a photo only arms the confirmation, and only the second press on
//! the SAME photo — with a property selected — sends the `PropertyMediaRemove` request.
//! A confirmed delete re-reads the screen; a failed answer says why and keeps the page.
//! The production boundary is `ui::app::screens::workbench::Workbench`
//! (`Msg::DeletePhoto` / `Msg::PhotoDeleted`), driven here through the harness's MVI
//! `ScreenHarness`.
//!
//! Level: L1 Component — pure reducer, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_intent__003__delete

use serde_json::json;
use test_harness::mvi::screen::{self, ScreenHarness};
use ui::app::cmd::ApiError;
use ui::app::screen::ScreenCtx;
use ui::app::screens::workbench::{Msg, Workbench};

fn opened(selected: Option<&str>) -> ScreenHarness<Workbench> {
    let ctx = ScreenCtx::default();
    let (mut harness, open) = ScreenHarness::<Workbench>::open(ctx.clone());
    assert_eq!(
        screen::classify(&open),
        vec![screen::CommandKind::Request],
        "opening the workbench reads it"
    );
    let read = open.into_requests().remove(0);
    let answer = match selected {
        Some(id) => json!({ "ops": { "selectedId": id } }),
        None => json!({ "ops": {} }),
    };
    let loaded = harness.update(read.respond(Ok(answer)));
    assert_eq!(screen::classify(&loaded), vec![screen::CommandKind::None]);
    harness
}

#[test]
fn ui_intent_003__delete() {
    let mut harness = opened(Some("prop-1"));
    assert_eq!(harness.model().selected.as_deref(), Some("prop-1"));

    // First press arms the confirmation: no request leaves the screen.
    let armed = harness.update(Msg::DeletePhoto("m-1".into()));
    assert_eq!(
        screen::classify(&armed),
        vec![screen::CommandKind::None],
        "the first press only asks"
    );
    assert_eq!(
        harness.model().ops.media_confirm_delete.as_deref(),
        Some("m-1")
    );

    // NEGATIVE: pressing another photo re-arms onto it instead of deleting the first.
    let rearmed = harness.update(Msg::DeletePhoto("m-2".into()));
    assert_eq!(
        screen::classify(&rearmed),
        vec![screen::CommandKind::None],
        "a different photo is a new question, not a delete"
    );
    assert_eq!(
        harness.model().ops.media_confirm_delete.as_deref(),
        Some("m-2")
    );

    // Second press on the armed photo deletes: exactly one remove request for this property.
    let delete = harness.update(Msg::DeletePhoto("m-2".into()));
    let request = delete.into_requests();
    assert_eq!(request.len(), 1, "the confirmed delete sends one request");
    assert_eq!(request[0].path, "/api/property-media/remove");
    assert_eq!(request[0].body.clone().unwrap()["propertyId"], "prop-1");
    assert_eq!(request[0].body.clone().unwrap()["mediaId"], "m-2");

    // The confirmed delete re-reads the screen for the property.
    let reread = harness.update(request.into_iter().next().unwrap().respond(Ok(json!({}))));
    assert_eq!(
        screen::classify(&reread),
        vec![screen::CommandKind::Request],
        "a deleted photo re-reads the screen"
    );

    // NEGATIVE: a failed delete says why and keeps the page instead of re-reading.
    let mut harness = opened(Some("prop-1"));
    harness.update(Msg::DeletePhoto("m-9".into()));
    let delete = harness.update(Msg::DeletePhoto("m-9".into()));
    let request = delete.into_requests().remove(0);
    let failed = harness.update(request.respond(Err(ApiError::network("remove unavailable"))));
    assert_eq!(
        screen::classify(&failed),
        vec![screen::CommandKind::None],
        "a failed delete re-reads nothing"
    );
    assert!(
        harness
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("was not deleted"),
        "the failure names the photo that survived"
    );
    assert!(harness.model().read.loaded().is_some(), "the page stays");

    // NEGATIVE: with no property selected, even a confirmed press deletes nothing.
    let mut harness = opened(None);
    harness.update(Msg::DeletePhoto("m-1".into()));
    let refused = harness.update(Msg::DeletePhoto("m-1".into()));
    assert!(
        refused.into_requests().is_empty(),
        "no selected property, no remove request"
    );
}
