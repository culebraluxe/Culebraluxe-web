//! UI.INTENT-009 — pagination.
//!
//! CONTRACT. On the Listing Media screen, paging moves exactly one page per press within
//! the page count the loaded total implies, clears the selection (the rows on screen are
//! new), and re-reads for the new page — while a press that cannot move (before the first
//! page, past the last) does nothing at all. The production boundary is
//! `ui::app::screens::listing_media::ListingMedia` (`Msg::PageChanged`), driven here
//! through the harness's MVI `ScreenHarness`.
//!
//! Level: L1 Component — pure reducer, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_intent__009__pagination

use serde_json::json;
use test_harness::mvi::screen::{self, ScreenHarness};
use ui::app::screen::ScreenCtx;
use ui::app::screens::listing_media::{ListingMedia, Msg};

fn page() -> serde_json::Value {
    json!({ "listingMedia": {
        "properties": [{ "id": "p1" }, { "id": "p2" }],
        "total": 45,
        "pageSize": 20,
    } })
}

fn opened() -> ScreenHarness<ListingMedia> {
    let ctx = ScreenCtx::default();
    let (mut harness, open) = ScreenHarness::<ListingMedia>::open(ctx.clone());
    let read = open.into_requests().remove(0);
    assert_eq!(
        read.path,
        "/api/portal/rust-ui/listing-media?page=0&search="
    );
    let loaded = harness.update(read.respond(Ok(page())));
    assert_eq!(screen::classify(&loaded), vec![screen::CommandKind::None]);
    harness
}

#[test]
fn ui_intent_009__pagination() {
    let mut harness = opened();
    assert_eq!(harness.model().page, 0);

    // NEGATIVE: before the first page, back does nothing — no state change, no read.
    let stuck = harness.update(Msg::PageChanged(-1));
    assert_eq!(
        screen::classify(&stuck),
        vec![screen::CommandKind::None],
        "page 0 has no previous page"
    );
    assert_eq!(harness.model().page, 0);

    // Forward one page: the model moves, the selection clears, the screen re-reads.
    harness.update(Msg::RowSelected("p1".into()));
    assert_eq!(harness.model().selected.as_deref(), Some("p1"));
    let forward = harness.update(Msg::PageChanged(1));
    let request = forward.into_requests();
    assert_eq!(request.len(), 1, "paging re-reads for the new page");
    assert_eq!(
        request[0].path, "/api/portal/rust-ui/listing-media?page=1&search=",
        "the read names the new page"
    );
    assert_eq!(harness.model().page, 1);
    assert_eq!(
        harness.model().selected,
        None,
        "the selection clears: the rows on screen are new"
    );

    // A leap past the last page clamps to it: 45 rows at 20 a page end on page 2.
    let clamped = harness.update(Msg::PageChanged(99));
    assert_eq!(harness.model().page, 2);
    assert_eq!(
        clamped.into_requests().remove(0).path,
        "/api/portal/rust-ui/listing-media?page=2&search="
    );

    // NEGATIVE: past the last page, forward does nothing.
    let stuck = harness.update(Msg::PageChanged(1));
    assert_eq!(
        screen::classify(&stuck),
        vec![screen::CommandKind::None],
        "the last page has no next page"
    );
    assert_eq!(harness.model().page, 2);
}
