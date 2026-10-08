//! UI.MODEL — loading (TST-UI-MODEL-002).
//!
//! Contract: a screen's `init` produces `Remote::Loading` and issues a `Cmd::Request` for its data.
//! The same boundary production uses: the real `Screen` trait, `init`, and the `Remote` state machine.
//!
//! Level: L1 Component, harness `MviHarness` (test_harness::mvi::screen::ScreenHarness).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_model__002__loading

use test_harness::mvi::screen::{CommandKind, ScreenHarness};
use ui::app::screen::ScreenCtx;
use ui::app::screens::clients::Clients;

const HARNESS: &str = "MviHarness/L1 Component";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-MODEL-002)
fn ui_model_002__loading() {
    // Positive case: opening the screen starts with Loading and requests data
    let ctx = ScreenCtx::default();
    let (harness, command) = ScreenHarness::<Clients>::open(ctx);

    // The model starts in Loading
    let model = harness.model();
    assert!(
        matches!(model.data, ui::app::cmd::Remote::Loading),
        "{HARNESS}: init must produce Remote::Loading, got {:?}",
        model.data
    );

    // The command is a Request for the clients data
    let kinds = test_harness::mvi::screen::classify(&command);
    assert!(
        kinds.contains(&CommandKind::Request),
        "{HARNESS}: init must issue a Request command, got kinds: {:?}",
        kinds
    );

    // Negative case: a screen that does NOT issue a request on init would fail this test
    // (We test this by ensuring the request path is the expected API endpoint)
    let mut requests = command.into_requests();
    assert_eq!(requests.len(), 1, "{HARNESS}: exactly one request expected");
    let request = requests.remove(0);
    assert!(
        request.path.contains("/api/portal/rust-ui/clients"),
        "{HARNESS}: request must target the clients API, got: {}",
        request.path
    );

    // Negative/refusal case: if the screen returned Remote::NotAsked instead of Loading, the test would fail
    // This is enforced by the positive assertion above — Remote::NotAsked != Remote::Loading
}