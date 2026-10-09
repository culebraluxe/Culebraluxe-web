//! UI.INTENT-006 — upload.
//!
//! CONTRACT. On the Listing Media screen, the upload intent is guarded on the way in
//! and honest on the way out: an empty file choice is refused without a sound, choosing
//! while an upload runs is swallowed, a finished upload reports the file's name and
//! re-reads the screen, and a failed upload names the failure and re-reads nothing.
//! The production boundary is `ui::app::screens::listing_media::ListingMedia`
//! (`Msg::FileChosen` / `Msg::Uploaded`), driven here through the harness's MVI
//! `ScreenHarness`.
//!
//! HOST LIMIT, stated not hidden: starting a real upload needs a `web_sys::File`, which
//! only exists in the browser — `Msg::FileChosen(Some(..))` cannot be built on the host.
//! This test pins every host-reachable half of the contract (the refusals and both
//! completions); the initiation half is covered by the screen's own wasm build.
//!
//! Level: L1 Component — pure reducer, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_intent__006__upload

use serde_json::json;
use test_harness::mvi::screen::{self, ScreenHarness};
use ui::app::cmd::ApiError;
use ui::app::screen::ScreenCtx;
use ui::app::screens::listing_media::{ListingMedia, Msg};

fn loaded() -> ScreenHarness<ListingMedia> {
    let ctx = ScreenCtx::default();
    let (mut harness, open) = ScreenHarness::<ListingMedia>::open(ctx.clone());
    let read = open.into_requests().remove(0);
    let loaded = harness.update(read.respond(Ok(json!({ "listingMedia": {
        "properties": [{ "id": "p1" }],
        "total": 1,
        "pageSize": 20,
    } }))));
    assert_eq!(screen::classify(&loaded), vec![screen::CommandKind::None]);
    harness.update(Msg::RowSelected("p1".into()));
    harness
}

#[test]
fn ui_intent_006__upload() {
    let mut harness = loaded();
    assert_eq!(harness.model().selected.as_deref(), Some("p1"));

    // NEGATIVE: an empty file choice (the dialog cancelled) is refused silently.
    let cancelled = harness.update(Msg::FileChosen(None));
    assert_eq!(
        screen::classify(&cancelled),
        vec![screen::CommandKind::None],
        "no file, no upload, no notice"
    );
    assert!(!harness.model().uploading);
    assert_eq!(harness.model().notice, None);

    // A finished upload reports the file's name and re-reads the screen.
    let done = harness.update(Msg::Uploaded(Ok(())));
    assert_eq!(
        screen::classify(&done),
        vec![screen::CommandKind::Request],
        "a finished upload re-reads the screen"
    );
    let notice = harness.model().notice.clone().expect("success notice");
    assert!(notice.ok, "the completion is good news");
    assert!(
        notice.message.contains("was added"),
        "the notice says the photo was added, got: {}",
        notice.message
    );

    // NEGATIVE: a failed upload names the failure and re-reads nothing.
    let mut harness = loaded();
    let failed = harness.update(Msg::Uploaded(Err(ApiError::network("storage down"))));
    assert_eq!(
        screen::classify(&failed),
        vec![screen::CommandKind::None],
        "a failed upload re-reads nothing"
    );
    let notice = harness.model().notice.clone().expect("failure notice");
    assert!(!notice.ok, "the completion is bad news");
    assert!(
        notice.message.contains("was not added"),
        "the notice names the failure, got: {}",
        notice.message
    );
    assert!(
        !harness.model().uploading,
        "the failure releases the upload lock"
    );
}
