//! UI.MODEL-001 — initial state.
//!
//! CONTRACT. A screen opens in exactly its initial state: nothing is loaded yet
//! (`Remote::Loading`), the last failure is empty, and the screen's first command is the
//! one read its route implies — the portfolio reads the portfolio, a record route reads
//! its record. Opening issues no navigation, no write, and no storage command. The
//! production boundary is `Screen::init` (here `ui::app::screens::deals::Deals`, whose
//! route carries a record id), driven here through the harness's MVI `ScreenHarness`.
//!
//! Level: L1 Component — pure constructor, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_model__001__initial_state

use test_harness::mvi::screen::{self, ScreenHarness};
use ui::app::cmd::Remote;
use ui::app::screen::ScreenCtx;
use ui::app::screens::deals::{Deals, Model};

#[test]
fn ui_model_001__initial_state() {
    // A fresh model holds nothing: not asked, no failure, no selection made.
    let fresh = Model::default();
    assert_eq!(fresh.read, Remote::NotAsked);
    assert_eq!(fresh.error, None);
    assert_eq!(fresh.refreshing, false);

    // The portfolio opens loading and reads the portfolio — and only that.
    let (harness, open) = ScreenHarness::<Deals>::open(ScreenCtx::default());
    assert_eq!(harness.model().read, Remote::Loading);
    assert_eq!(harness.model().error, None);
    assert_eq!(
        screen::classify(&open),
        vec![screen::CommandKind::Request],
        "opening reads, and reads only"
    );
    let read = open.into_requests();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].path, "/api/portal/rust-ui/deals?screen=deals");
    assert_eq!(harness.updates(), 0, "opening applies no message");

    // NEGATIVE: the record route opens on its record, not on the portfolio.
    let record_ctx = ScreenCtx {
        id: Some("deal-7".into()),
        ..ScreenCtx::default()
    };
    let (harness, open) = ScreenHarness::<Deals>::open(record_ctx);
    assert_eq!(harness.model().read, Remote::Loading);
    let read = open.into_requests();
    assert_eq!(read.len(), 1);
    assert_eq!(
        read[0].path, "/api/portal/rust-ui/deals?screen=deal-record&scope=deal-7",
        "a record id opens its own record, never the portfolio"
    );

    // NEGATIVE: opening never navigates, writes, or touches storage — whatever the route.
    for ctx in [
        ScreenCtx::default(),
        ScreenCtx {
            id: Some("deal-7".into()),
            ..ScreenCtx::default()
        },
    ] {
        let (_, open) = ScreenHarness::<Deals>::open(ctx);
        assert_eq!(
            screen::effect_kind(&open),
            Some(screen::CommandKind::Request),
            "the only effect an open may have is its first read"
        );
    }
}
