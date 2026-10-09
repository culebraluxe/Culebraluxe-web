//! UI.INTENT-007 — filter.
//!
//! CONTRACT. On the Deals portfolio, changing the stage filter sets exactly the filter
//! on the screen's controls: it issues no read (the rows on the page are filtered where
//! they stand), it disturbs nothing else on the model, and a failed refresh afterwards
//! keeps the filter while saying why. The production boundary is
//! `ui::app::screens::deals::Deals` (`Msg::FilterChanged`), driven here through the
//! harness's MVI `ScreenHarness`.
//!
//! Level: L1 Component — pure reducer, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_intent__007__filter

use serde_json::json;
use test_harness::mvi::screen::{self, ScreenHarness};
use ui::app::cmd::ApiError;
use ui::app::screen::ScreenCtx;
use ui::app::screens::deals::{Deals, Msg};

fn opened() -> ScreenHarness<Deals> {
    let ctx = ScreenCtx::default();
    let (mut harness, open) = ScreenHarness::<Deals>::open(ctx.clone());
    let read = open.into_requests().remove(0);
    assert_eq!(read.path, "/api/portal/rust-ui/deals?screen=deals");
    let loaded = harness.update(read.respond(Ok(json!({ "deals": {} }))));
    assert_eq!(screen::classify(&loaded), vec![screen::CommandKind::None]);
    harness
}

#[test]
fn ui_intent_007__filter() {
    let mut harness = opened();
    assert_eq!(harness.model().controls.filter, None);

    // The filter lands on the controls and issues no read.
    let changed = harness.update(Msg::FilterChanged("offer".into()));
    assert_eq!(
        screen::classify(&changed),
        vec![screen::CommandKind::None],
        "filtering filters the rows on screen; it reads nothing"
    );
    assert_eq!(harness.model().controls.filter.as_deref(), Some("offer"));
    assert_eq!(harness.model().error, None, "filtering reports no error");

    // Replacing the filter replaces it exactly — the old value does not linger.
    harness.update(Msg::FilterChanged("closing".into()));
    assert_eq!(harness.model().controls.filter.as_deref(), Some("closing"));

    // NEGATIVE: a failed refresh keeps the filter and says why — the operator's
    // narrowing is not silently dropped by somebody else's failure.
    harness.update(Msg::FilterChanged("offer".into()));
    let failed = harness.update(Msg::Loaded(Err(ApiError::network("deals unavailable"))));
    assert_eq!(
        screen::classify(&failed),
        vec![screen::CommandKind::None],
        "a failed refresh reads nothing further"
    );
    assert_eq!(
        harness.model().controls.filter.as_deref(),
        Some("offer"),
        "a failed refresh keeps the filter"
    );
    assert_eq!(harness.model().error.as_deref(), Some("deals unavailable"));
    assert!(
        harness.model().read.loaded().is_some(),
        "the page the filter narrows stays on screen"
    );

    // NEGATIVE: an unrelated intent never clears the filter.
    let mut harness = opened();
    harness.update(Msg::FilterChanged("offer".into()));
    harness.update(Msg::DealCreateToggled);
    assert_eq!(
        harness.model().controls.filter.as_deref(),
        Some("offer"),
        "toggling the create panel leaves the filter alone"
    );
}
