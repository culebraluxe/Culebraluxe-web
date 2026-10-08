//! UI.MODEL — loaded (TST-UI-MODEL-003).
//!
//! Contract: a screen's `update` with a successful `Loaded(Ok(...))` transitions `Remote::Loading` to `Remote::Loaded(data)`.
//! The same boundary production uses: the real `Screen` trait, `update`, and the `Remote` state machine.
//!
//! Level: L1 Component, harness `MviHarness` (test_harness::mvi::screen::ScreenHarness).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_model__003__loaded

use test_harness::mvi::screen::{CommandKind, ScreenHarness};
use ui::app::cmd::Remote;
use ui::app::screen::ScreenCtx;
use ui::app::screens::clients::{Clients, Msg};
use ui::model::PortalPage;

const HARNESS: &str = "MviHarness/L1 Component";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-MODEL-003)
fn ui_model_003__loaded() {
    // Positive case: a successful Loaded answer transitions Loading -> Loaded(data)
    let ctx = ScreenCtx::default();
    let (mut harness, _command) = ScreenHarness::<Clients>::open(ctx);

    // Verify initial state is Loading
    let model = harness.model();
    assert!(
        matches!(model.data, Remote::Loading),
        "{HARNESS}: init must produce Remote::Loading"
    );

    // Simulate a successful response using a real fixture
    let answer: PortalPage =
        serde_json::from_str(include_str!("../../web/ui/fixtures/clients-list.json")).unwrap();

    let loaded_msg = Msg::Loaded(Ok(answer));
    let follow_up = harness.update(loaded_msg);

    // The model should now be Loaded with data
    let model = harness.model();
    let data = model.data.loaded().expect("{HARNESS}: update must transition to Remote::Loaded");
    assert!(
        !data.rows.is_empty(),
        "{HARNESS}: loaded data must contain rows"
    );
    assert_eq!(
        data.selected_id.as_deref(),
        data.selected.as_ref().map(|client| client.id.as_str()),
        "{HARNESS}: selected_id must match selected client"
    );

    // The follow-up command should be None (no further requests)
    let kinds = test_harness::mvi::screen::classify(&follow_up);
    assert!(
        kinds == vec![CommandKind::None] || kinds.is_empty(),
        "{HARNESS}: successful load must not issue further commands, got: {:?}",
        kinds
    );

    // Negative/refusal case: if the screen stayed in Loading or went to Failed, the test would fail
    // This is enforced by the positive assertion above — Remote::Loaded is required
}