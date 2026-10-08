//! UI.INTENT-001 — select.
//!
//! CONTRACT. On the Listing Media screen (`/portal/property-media`), choosing a listing row
//! selects exactly that listing and re-reads the screen for it — and only a row that is on
//! the loaded page can be selected. The production boundary is
//! `ui::app::screens::listing_media::ListingMedia` (`Msg::RowSelected`), driven here through
//! the harness's MVI `ScreenHarness`, the same `init`/`update` the browser runs.
//!
//! Level: L1 Component — pure reducer, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_intent__001__select

use serde_json::json;
use test_harness::mvi::screen::{self, ScreenHarness};
use ui::app::cmd::ApiError;
use ui::app::screen::ScreenCtx;
use ui::app::screens::listing_media::{ListingMedia, Msg};

fn page() -> serde_json::Value {
    json!({ "listingMedia": {
        "properties": [{ "id": "p1" }, { "id": "p2" }],
        "total": 2,
        "pageSize": 20,
    } })
}

#[test]
fn ui_intent_001__select() {
    let ctx = ScreenCtx::default();
    let (mut harness, open) = ScreenHarness::<ListingMedia>::open(ctx.clone());
    assert_eq!(
        screen::classify(&open),
        vec![screen::CommandKind::Request],
        "opening the screen reads its listings"
    );

    // A failed first read leaves nothing selected: there is no row to choose.
    let failed = harness.update(open.into_requests().remove(0).respond(Err(
        ApiError::network("listings unavailable"),
    )));
    assert_eq!(screen::classify(&failed), vec![screen::CommandKind::None]);
    assert_eq!(harness.model().selected, None);
    assert!(
        screen::classify(&harness.update(Msg::RowSelected("p1".into())))
            == vec![screen::CommandKind::None],
        "with no loaded page no row can be selected"
    );

    // Load the page, then select a row that is on it.
    let (mut harness, open) = ScreenHarness::<ListingMedia>::open(ctx.clone());
    let read = open.into_requests().remove(0);
    let loaded = harness.update(read.respond(Ok(page())));
    assert_eq!(screen::classify(&loaded), vec![screen::CommandKind::None]);
    assert_eq!(harness.model().selected, None);

    let reread = harness.update(Msg::RowSelected("p1".into()));
    assert_eq!(
        screen::classify(&reread),
        vec![screen::CommandKind::Request],
        "selecting a listed row re-reads the screen for it"
    );
    assert_eq!(harness.model().selected.as_deref(), Some("p1"));
    assert_eq!(harness.updates(), 2);

    // NEGATIVE: a row that is not on the loaded page is refused — no state change, no read.
    let refused = harness.update(Msg::RowSelected("ghost".into()));
    assert_eq!(
        screen::classify(&refused),
        vec![screen::CommandKind::None],
        "an unlisted id selects nothing and reads nothing"
    );
    assert_eq!(
        harness.model().selected.as_deref(),
        Some("p1"),
        "the refused selection keeps the previous one"
    );
}
