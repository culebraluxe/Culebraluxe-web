//! UI.MODEL — permission denied (TST-UI-MODEL-006).
//!
//! Contract: a screen's `update` with a permission denied error (HTTP 403) transitions to `Remote::Failed(error)` with the correct error code.
//! The same boundary production uses: the real `Screen` trait, `update`, and the `Remote` state machine.
//!
//! Level: L1 Component, harness `MviHarness` (test_harness::mvi::screen::ScreenHarness).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_model__006__permission_denied

use test_harness::mvi::screen::{CommandKind, ScreenHarness};
use ui::app::cmd::{ApiError, Remote};
use ui::app::screen::ScreenCtx;
use ui::app::screens::clients::{Clients, Msg};

const HARNESS: &str = "MviHarness/L1 Component";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-MODEL-006)
fn ui_model_006__permission_denied() {
    // Positive case: a permission denied (403) response transitions to Failed with correct error
    let ctx = ScreenCtx::default();
    let (mut harness, _command) = ScreenHarness::<Clients>::open(ctx);

    // Verify initial state is Loading
    let model = harness.model();
    assert!(
        matches!(model.data, Remote::Loading),
        "{HARNESS}: init must produce Remote::Loading"
    );

    // Simulate a permission denied response (HTTP 403)
    let error = ApiError {
        status: 403,
        code: "FORBIDDEN".into(),
        message: "Insufficient permissions to view clients".into(),
    };
    let loaded_msg = Msg::Loaded(Err(error.clone()));
    let follow_up = harness.update(loaded_msg);

    // The model should now be Failed with the permission denied error
    let model = harness.model();
    let failed_error = match &model.data {
        Remote::Failed(e) => e.clone(),
        other => panic!(
            "{HARNESS}: update must transition to Remote::Failed, got {:?}",
            other
        ),
    };
    assert_eq!(
        failed_error.status, 403,
        "{HARNESS}: permission denied must have status 403"
    );
    assert_eq!(
        failed_error.code, "FORBIDDEN",
        "{HARNESS}: permission denied must have code FORBIDDEN"
    );
    assert!(
        failed_error.message.contains("permission"),
        "{HARNESS}: message must mention permission"
    );

    // The follow-up command should be None
    let kinds = test_harness::mvi::screen::classify(&follow_up);
    assert!(
        kinds == vec![CommandKind::None] || kinds.is_empty(),
        "{HARNESS}: permission denied must not issue further commands, got: {:?}",
        kinds
    );

    // Negative/refusal case: if the screen treated 403 as a generic network error, the test would fail
    // This is enforced by the status/code assertions above

    // Additional negative case: a 401 Unauthorized should also be captured correctly
    let unauthorized_error = ApiError {
        status: 401,
        code: "UNAUTHORIZED".into(),
        message: "Session expired".into(),
    };
    let (mut harness2, _command2) = ScreenHarness::<Clients>::open(ScreenCtx::default());
    let _follow_up2 = harness2.update(Msg::Loaded(Err(unauthorized_error.clone())));
    let model2 = harness2.model();
    match &model2.data {
        Remote::Failed(e) => {
            assert_eq!(
                e.status, 401,
                "{HARNESS}: unauthorized must have status 401"
            );
            assert_eq!(
                e.code, "UNAUTHORIZED",
                "{HARNESS}: unauthorized must have code UNAUTHORIZED"
            );
        }
        other => panic!(
            "{HARNESS}: 401 must transition to Remote::Failed, got {:?}",
            other
        ),
    }
}
