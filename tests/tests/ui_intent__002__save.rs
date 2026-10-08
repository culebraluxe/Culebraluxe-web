//! UI.INTENT-002 — save.
//!
//! CONTRACT. On the Deals portfolio, saving a new deal (`Msg::DealCreateRequested`) sends
//! exactly one `DealCreate` request carrying the chosen property and client person — and it
//! is refused with the screen's own words when either is missing, when the operator may not
//! write deals, or while a save is already in flight. A failed answer surfaces the service's
//! words without navigating anywhere. The production boundary is
//! `ui::app::screens::deals::Deals`, driven here through the harness's MVI `ScreenHarness`.
//!
//! Level: L1 Component — pure reducer, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_intent__002__save

use serde_json::json;
use test_harness::mvi::screen::{self, ScreenHarness};
use ui::app::cmd::ApiError;
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

fn opened(ctx: &ScreenCtx) -> ScreenHarness<Deals> {
    let (mut harness, open) = ScreenHarness::<Deals>::open(ctx.clone());
    let read = open.into_requests().remove(0);
    assert_eq!(read.path, "/api/portal/rust-ui/deals?screen=deals");
    let loaded = harness.update(read.respond(Ok(json!({ "deals": {} }))));
    assert_eq!(screen::classify(&loaded), vec![screen::CommandKind::None]);
    harness
}

#[test]
fn ui_intent_002__save() {
    let ctx = writer_ctx();
    let mut harness = opened(&ctx);

    // NEGATIVE: saving with no property is refused and issues no request.
    let refused = harness.update(Msg::DealCreateRequested);
    assert!(refused.into_requests().is_empty());
    assert_eq!(
        harness.model().error.as_deref(),
        Some("Choose a property first.")
    );

    // NEGATIVE: a property alone is not enough — the client person is still missing.
    harness.update(Msg::DealCreatePropertyChanged("p-1".into()));
    let refused = harness.update(Msg::DealCreateRequested);
    assert!(refused.into_requests().is_empty());
    assert_eq!(
        harness.model().error.as_deref(),
        Some("Select an existing client person first.")
    );

    // The save: one request carrying exactly the chosen property and client.
    harness.update(Msg::DealCreateClientSelected {
        id: "c-1".into(),
        label: "Alice".into(),
    });
    let save = harness.update(Msg::DealCreateRequested);
    let request = save.into_requests();
    assert_eq!(request.len(), 1, "a complete save sends exactly one request");
    assert_eq!(request[0].path, "/api/portal/rust-ui/deals");
    assert_eq!(request[0].body.clone().unwrap()["propertyId"], "p-1");
    assert_eq!(request[0].body.clone().unwrap()["clientPersonId"], "c-1");
    assert_eq!(harness.model().error, None);

    // NEGATIVE: while the save is in flight a second save is swallowed, not duplicated.
    let duplicate = harness.update(Msg::DealCreateRequested);
    assert!(
        duplicate.into_requests().is_empty(),
        "an in-flight save takes no second request"
    );

    // A failed answer reports the service's words and navigates nowhere.
    let answered = harness.update(
        request
            .into_iter()
            .next()
            .unwrap()
            .respond(Err(ApiError::network("deals unavailable"))),
    );
    assert_eq!(
        screen::classify(&answered),
        vec![screen::CommandKind::None],
        "a failed save navigates nowhere"
    );
    assert_eq!(
        harness.model().error.as_deref(),
        Some("deals unavailable")
    );

    // NEGATIVE: an operator who may not write deals cannot save, even with a complete form.
    let unprivileged = ScreenCtx::default();
    let mut outsider = opened(&unprivileged);
    outsider.update(Msg::DealCreatePropertyChanged("p-1".into()));
    outsider.update(Msg::DealCreateClientSelected {
        id: "c-1".into(),
        label: "Alice".into(),
    });
    let refused = outsider.update(Msg::DealCreateRequested);
    assert!(
        refused.into_requests().is_empty(),
        "without deal.write the save issues no request"
    );
}
