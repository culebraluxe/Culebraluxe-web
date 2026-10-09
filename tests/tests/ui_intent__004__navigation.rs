//! UI.INTENT-004 — navigation.
//!
//! CONTRACT. When the Deals portfolio's create request succeeds, the screen navigates to
//! the new deal's workspace (`/portal/deals/<id>`) and resets the create form; when it
//! fails, the screen stays where it is and reports the service's words. Navigation is a
//! `Cmd::Navigate` the shell performs — the screen never loads a page itself. The production
//! boundary is `ui::app::screens::deals::Deals` (`Msg::DealCreated`), driven here through
//! the harness's MVI `ScreenHarness`.
//!
//! Level: L1 Component — pure reducer, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_intent__004__navigation

use serde_json::json;
use test_harness::mvi::screen::{self, ScreenHarness};
use ui::app::cmd::{ApiError, Cmd};
use ui::app::screen::ScreenCtx;
use ui::app::screens::deals::{Deals, Msg};
use ui::model::PortalEntitlements;

fn writer_ctx() -> ScreenCtx {
    ScreenCtx {
        grants: Some(PortalEntitlements {
            account_type: "internal".into(),
            security_level: "USER".into(),
            is_root: false,
            entitlement_codes: vec!["deal.write".into()],
        }),
        ..ScreenCtx::default()
    }
}

/// Open the portfolio and run a complete create up to the in-flight save request.
fn saving(ctx: &ScreenCtx) -> (ScreenHarness<Deals>, ui::app::cmd::Request<Msg>) {
    let (mut harness, open) = ScreenHarness::<Deals>::open(ctx.clone());
    let read = open.into_requests().remove(0);
    let loaded = harness.update(read.respond(Ok(json!({ "deals": {} }))));
    assert_eq!(screen::classify(&loaded), vec![screen::CommandKind::None]);
    harness.update(Msg::DealCreatePropertyChanged("p-1".into()));
    harness.update(Msg::DealCreateClientSelected {
        id: "c-1".into(),
        label: "Alice".into(),
    });
    let save = harness.update(Msg::DealCreateRequested);
    let mut requests = save.into_requests();
    assert_eq!(requests.len(), 1);
    let request = requests.remove(0);
    (harness, request)
}

#[test]
fn ui_intent_004__navigation() {
    let ctx = writer_ctx();
    let (mut harness, request) = saving(&ctx);

    // The created deal opens its workspace: exactly one navigation, to the new id.
    match harness.update(request.respond(Ok(json!({ "id": "deal-9" })))) {
        Cmd::Navigate(path) => assert_eq!(path, "/portal/deals/deal-9"),
        other => panic!("a created deal navigates to its workspace, got {other:?}"),
    }

    // NEGATIVE: a failed create stays on the portfolio and says why — no navigation.
    let (mut harness, request) = saving(&ctx);
    let answered = harness.update(request.respond(Err(ApiError::network("deals unavailable"))));
    assert_eq!(
        screen::classify(&answered),
        vec![screen::CommandKind::None],
        "a failed create navigates nowhere"
    );
    assert_eq!(harness.model().error.as_deref(), Some("deals unavailable"));

    // NEGATIVE: a navigation never carries a blank id — the answer without one is a failure.
    let (mut harness, request) = saving(&ctx);
    let answered = harness.update(request.respond(Ok(json!({}))));
    assert_eq!(
        screen::classify(&answered),
        vec![screen::CommandKind::None],
        "an answer with no id navigates nowhere"
    );
    assert!(
        harness.model().error.is_some(),
        "the id-less answer is reported, not navigated to"
    );
}
