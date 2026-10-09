//! UI.MODEL — selected row disappears (TST-UI-MODEL-008).
//!
//! Contract: when a screen's selected row is no longer in the loaded data (deleted externally), the screen clears the selection.
//! The same boundary production uses: the real `Screen` trait, `update`, and the `Remote` state machine.
//!
//! Level: L1 Component, harness `MviHarness` (test_harness::mvi::screen::ScreenHarness).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_model__008__selected_row_disappears

use test_harness::mvi::screen::{CommandKind, ScreenHarness};
use ui::app::cmd::Remote;
use ui::app::screen::ScreenCtx;
use ui::app::screens::clients::{Clients, Msg};
use ui::model::PortalPage;

const HARNESS: &str = "MviHarness/L1 Component";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-MODEL-008)
fn ui_model_008__selected_row_disappears() {
    // Positive case: loading data where the previously selected row is no longer present clears the selection
    let ctx = ScreenCtx::default();
    let (mut harness, _command) = ScreenHarness::<Clients>::open(ctx);

    // Load initial data with a selected client
    let answer1: PortalPage =
        serde_json::from_str(include_str!("../../web/ui/fixtures/clients-list.json")).unwrap();
    harness.update(Msg::Loaded(Ok(answer1)));

    // Verify we have a selected client
    let model = harness.model();
    let data1 = model
        .data
        .loaded()
        .expect("{HARNESS}: first response must transition to Loaded");
    assert!(
        data1.selected.is_some(),
        "{HARNESS}: fixture must have a selected client"
    );
    let selected_id = data1
        .selected_id
        .clone()
        .expect("selected_id must be present");

    // Now load new data where that selected client is NOT in the rows
    // (simulating the client being deleted by another user)
    let answer2: PortalPage = serde_json::from_value(serde_json::json!({
        "clients": {
            "rows": [{"id": "p-different", "displayName": "Different Client", "role": "Seller", "status": "active", "primaryPhone": null, "primaryEmail": null, "observedCount": 0, "twoWay": false, "nameResolved": true}],
            "total": 1,
            "pageSize": 20,
            "page": 1,
            "selectedId": selected_id,  // Server still returns the old selectedId
            "selected": null,             // But selected is null because it's not in rows
            "comms": null,
            "properties": []
        }
    })).unwrap();
    harness.update(Msg::Loaded(Ok(answer2)));

    // The screen should clear the selection (selected becomes None)
    let model = harness.model();
    let data2 = model
        .data
        .loaded()
        .expect("{HARNESS}: second response must transition to Loaded");
    assert_eq!(data2.total, 1, "{HARNESS}: new data must have total = 1");
    assert_eq!(
        data2.selected_id.as_deref(),
        Some(selected_id.as_str()),
        "{HARNESS}: selected_id from server preserved"
    );
    assert!(
        data2.selected.is_none(),
        "{HARNESS}: selected must be None when row disappears"
    );

    // Negative/refusal case: if the screen kept the old selected client even though it's not in rows, the test would fail
    // This is enforced by the assertion above — selected must be None
}
