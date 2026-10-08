//! UI.MODEL — empty (TST-UI-MODEL-004).
//!
//! Contract: a screen's `update` with a successful but empty `Loaded(Ok(...))` transitions to `Remote::Loaded(data)` where data is empty.
//! The same boundary production uses: the real `Screen` trait, `update`, and the `Remote` state machine.
//!
//! Level: L1 Component, harness `MviHarness` (test_harness::mvi::screen::ScreenHarness).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_model__004__empty

use test_harness::mvi::screen::{CommandKind, ScreenHarness};
use ui::app::cmd::Remote;
use ui::app::screen::ScreenCtx;
use ui::app::screens::clients::{Clients, Msg};
use ui::model::PortalPage;

const HARNESS: &str = "MviHarness/L1 Component";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-MODEL-004)
fn ui_model_004__empty() {
    // Positive case: a successful but empty Loaded answer transitions to Loaded(empty data)
    let ctx = ScreenCtx::default();
    let (mut harness, _command) = ScreenHarness::<Clients>::open(ctx);

    // Verify initial state is Loading
    let model = harness.model();
    assert!(
        matches!(model.data, Remote::Loading),
        "{HARNESS}: init must produce Remote::Loading"
    );

    // Simulate an empty but successful response
    let empty_answer: PortalPage = serde_json::from_value(serde_json::json!({
        "clients": {
            "rows": [],
            "total": 0,
            "page_size": 20,
            "page": 1,
            "selected_id": null,
            "selected": null,
            "comms": null,
            "properties": []
        }
    })).unwrap();

    let loaded_msg = Msg::Loaded(Ok(empty_answer));
    let follow_up = harness.update(loaded_msg);

    // The model should now be Loaded with empty data
    let model = harness.model();
    let data = model.data.loaded().expect("{HARNESS}: update must transition to Remote::Loaded");
    assert_eq!(data.total, 0, "{HARNESS}: empty data must have total = 0");
    assert!(data.rows.is_empty(), "{HARNESS}: empty data must have no rows");
    assert!(data.selected.is_none(), "{HARNESS}: empty data must have no selected client");

    // The follow-up command should be None
    let kinds = test_harness::mvi::screen::classify(&follow_up);
    assert!(
        kinds == vec![CommandKind::None] || kinds.is_empty(),
        "{HARNESS}: successful empty load must not issue further commands, got: {:?}",
        kinds
    );

    // Negative/refusal case: if the screen went to Failed or stayed in Loading, the test would fail
    // This is enforced by the positive assertion above — Remote::Loaded with empty data is required
}