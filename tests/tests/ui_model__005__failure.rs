//! UI.MODEL — failure (TST-UI-MODEL-005).
//!
//! Contract: a screen's `update` with a failed `Loaded(Err(...))` transitions `Remote::Loading` to `Remote::Failed(error)`.
//! The same boundary production uses: the real `Screen` trait, `update`, and the `Remote` state machine.
//!
//! Level: L1 Component, harness `MviHarness` (test_harness::mvi::screen::ScreenHarness).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_model__005__failure

use test_harness::mvi::screen::{CommandKind, ScreenHarness};
use ui::app::cmd::{ApiError, Remote};
use ui::app::screen::ScreenCtx;
use ui::app::screens::clients::{Clients, Msg};

const HARNESS: &str = "MviHarness/L1 Component";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-MODEL-005)
fn ui_model_005__failure() {
    // Positive case: a failed Loaded answer transitions Loading -> Failed(error)
    let ctx = ScreenCtx::default();
    let (mut harness, _command) = ScreenHarness::<Clients>::open(ctx);

    // Verify initial state is Loading
    let model = harness.model();
    assert!(
        matches!(model.data, Remote::Loading),
        "{HARNESS}: init must produce Remote::Loading"
    );

    // Simulate a failed response
    let error = ApiError::network("connection refused");
    let loaded_msg = Msg::Loaded(Err(error.clone()));
    let follow_up = harness.update(loaded_msg);

    // The model should now be Failed with the error
    let model = harness.model();
    let failed_error = match &model.data {
        Remote::Failed(e) => e.clone(),
        other => panic!("{HARNESS}: update must transition to Remote::Failed, got {:?}", other),
    };
    assert_eq!(failed_error.code, "NETWORK");
    assert_eq!(failed_error.message, "connection refused");
    assert_eq!(failed_error.status, 0);

    // The follow-up command should be None
    let kinds = test_harness::mvi::screen::classify(&follow_up);
    assert!(
        kinds == vec![CommandKind::None] || kinds.is_empty(),
        "{HARNESS}: failed load must not issue further commands, got: {:?}",
        kinds
    );

    // Negative/refusal case: if the screen stayed in Loading or went to Loaded, the test would fail
    // This is enforced by the positive assertion above — Remote::Failed is required

    // Additional negative case: a decode error is also a valid failure
    let decode_error = ApiError::decode("invalid JSON");
    let (mut harness2, _command2) = ScreenHarness::<Clients>::open(ScreenCtx::default());
    let follow_up2 = harness2.update(Msg::Loaded(Err(decode_error.clone())));
    let model2 = harness2.model();
    match &model2.data {
        Remote::Failed(e) => {
            assert_eq!(e.code, "DECODE");
            assert_eq!(e.message, "invalid JSON");
        }
        other => panic!("{HARNESS}: decode error must transition to Remote::Failed, got {:?}", other),
    }
}