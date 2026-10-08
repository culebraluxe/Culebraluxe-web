//! UI.MODEL — stale response (TST-UI-MODEL-007).
//!
//! Contract: a screen ignores a stale response (one that arrives after a newer request was issued) and does not overwrite newer state.
//! The same boundary production uses: the real `Screen` trait, `update`, `refreshing` flag, and the `Remote` state machine.
//!
//! Level: L1 Component, harness `MviHarness` (test_harness::mvi::screen::ScreenHarness).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_model__007__stale_response

use test_harness::mvi::screen::{CommandKind, ScreenHarness};
use ui::app::cmd::{ApiError, Remote};
use ui::app::screen::ScreenCtx;
use ui::app::screens::clients::{Clients, Msg};
use ui::model::PortalPage;

const HARNESS: &str = "MviHarness/L1 Component";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-MODEL-007)
fn ui_model_007__stale_response() {
    // Positive case: a stale response (arriving after a newer request) is ignored
    let ctx = ScreenCtx::default();
    let (mut harness, _command) = ScreenHarness::<Clients>::open(ctx);

    // Verify initial state is Loading
    let model = harness.model();
    assert!(
        matches!(model.data, Remote::Loading),
        "{HARNESS}: init must produce Remote::Loading"
    );

    // Simulate first successful response (page 1)
    let answer1: PortalPage =
        serde_json::from_str(include_str!("../../web/ui/fixtures/clients-list.json")).unwrap();
    harness.update(Msg::Loaded(Ok(answer1)));

    // Verify we're now Loaded with page 1 data
    let model = harness.model();
    let data1 = model.data.loaded().expect("{HARNESS}: first response must transition to Loaded");
    let total1 = data1.total;

    // Now simulate a search that triggers a new request (page 0 with search)
    // This sets refreshing = true
    let search_msg = Msg::List(ui::app::list::ListMsg::Typed("search term".into()));
    harness.update(search_msg);
    let pause_msg = Msg::List(ui::app::list::ListMsg::Paused(1));
    harness.update(pause_msg);

    // The model should be refreshing
    let model = harness.model();
    assert!(model.refreshing, "{HARNESS}: search must set refreshing = true");

    // Now simulate the SECOND response arriving (the search results)
    let answer2: PortalPage = serde_json::from_value(serde_json::json!({
        "clients": {
            "rows": [{"id": "p-new", "display_name": "New Client", "role": "Buyer", "status": "active", "primary_phone": null, "primary_email": null, "observed_count": 0, "two_way": false, "name_resolved": true}],
            "total": 1,
            "page_size": 20,
            "page": 1,
            "selected_id": "p-new",
            "selected": {"id": "p-new", "display_name": "New Client", "role": "Buyer", "status": "active", "phone": null, "email": null, "assigned_agent": null, "timeline": null, "budget_min": null, "budget_max": null, "notes": null, "properties": [], "comms": null},
            "comms": null,
            "properties": []
        }
    })).unwrap();
    harness.update(Msg::Loaded(Ok(answer2)));

    // Verify we're now Loaded with search results
    let model = harness.model();
    let data2 = model.data.loaded().expect("{HARNESS}: search response must transition to Loaded");
    assert_eq!(data2.total, 1, "{HARNESS}: search results must have total = 1");
    assert!(!model.refreshing, "{HARNESS}: refreshing must be false after response");

    // NOW simulate a STALE response arriving (the original page 1 response arriving late)
    // The screen should ignore this because refreshing was true and we already got a newer response
    let stale_answer: PortalPage =
        serde_json::from_str(include_str!("../../web/ui/fixtures/clients-list.json")).unwrap();
    harness.update(Msg::Loaded(Ok(stale_answer)));

    // The model should STILL have the search results (stale response ignored)
    let model = harness.model();
    let data = model.data.loaded().expect("{HARNESS}: stale response must not overwrite newer data");
    assert_eq!(data.total, 1, "{HARNESS}: stale response must not overwrite search results");
    assert!(!model.refreshing, "{HARNESS}: refreshing must remain false");

    // Negative/refusal case: if the screen accepted the stale response and reverted to old data, the test would fail
    // This is enforced by the assertion above — data.total must remain 1 (search results), not revert to total1
}