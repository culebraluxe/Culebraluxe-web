//! UI.ROUTE — navigation active state (TST-UI-ROUTE-003).
//!
//! Contract: the registry's `is_current` and `parent_of` functions correctly identify the active menu item and parent screen for any path.
//! The same boundary production uses: the real `registry::is_current`, `registry::parent_of`, and `registry::ENTRIES`.
//!
//! Level: L1 Component, harness `MviHarness` (uses registry functions directly).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_route__003__navigation_active_state

use ui::app::registry::{is_current, parent_of, by_key, ENTRIES};
use ui::navigation::{Actor, Level};
use ui::model::Surface;

const HARNESS: &str = "MviHarness/L1 Component";

fn root_actor() -> Actor {
    Actor {
        level: Some(Level::Root),
        account_type: "internal".into(),
        authority_codes: vec!["portal.read".into(), "deal.read".into(), "settings.read".into(), "tech.access".into()],
        entitlement_codes: vec!["cockpit.read".into(), "person.read".into(), "project.read".into(), "deal.read".into(), "vault.read".into(), "form.read".into(), "property.read".into(), "accounting.read".into(), "tech.access".into()],
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ROUTE-003)
fn ui_route_003__navigation_active_state() {
    // Positive case: a list screen is current when at its path or any child path
    let clients = by_key("clients").expect("clients must exist");
    assert!(is_current(clients, "/portal/clients"), "{HARNESS}: clients list is current at /portal/clients");
    assert!(is_current(clients, "/portal/clients/abc-123"), "{HARNESS}: clients list is current at child record path");
    assert!(!is_current(clients, "/portal/deals"), "{HARNESS}: clients list is not current at /portal/deals");

    // Positive case: a record screen is current at ANY record path of its type
    let client_record = by_key("client-record").expect("client-record must exist");
    assert!(is_current(client_record, "/portal/clients/abc-123"), "{HARNESS}: client-record is current at its path");
    assert!(is_current(client_record, "/portal/clients/other-id"), "{HARNESS}: client-record is current at any record path");
    assert!(!is_current(client_record, "/portal/clients"), "{HARNESS}: client-record is not current at list path");

    // Positive case: parent_of returns the correct parent for drill-in screens
    assert_eq!(
        parent_of("/portal/clients/abc-123").map(|e| e.key),
        Some("clients"),
        "{HARNESS}: parent of client record must be clients list"
    );
    assert_eq!(
        parent_of("/portal/deals/deal-1").map(|e| e.key),
        Some("deals"),
        "{HARNESS}: parent of deal record must be deals list"
    );
    assert_eq!(
        parent_of("/portal/workflows/inst-1").map(|e| e.key),
        Some("workflows"),
        "{HARNESS}: parent of workflow record must be workflows list"
    );
    assert_eq!(
        parent_of("/portal/forms/form-1").map(|e| e.key),
        Some("forms"),
        "{HARNESS}: parent of form record must be forms list"
    );
    assert_eq!(
        parent_of("/portal/storyboard/story-1").map(|e| e.key),
        Some("storyboard"),
        "{HARNESS}: parent of story record must be storyboard list"
    );
    assert_eq!(
        parent_of("/portal/tech/flight-recorder/trace-1").map(|e| e.key),
        Some("tech"),
        "{HARNESS}: parent of trace record must be tech"
    );
    assert_eq!(
        parent_of("/portal/property-admin/prop-1").map(|e| e.key),
        Some("property-admin"),
        "{HARNESS}: parent of property record must be property-admin"
    );

    // Positive case: list screens have no parent
    assert!(parent_of("/portal/clients").is_none(), "{HARNESS}: clients list has no parent");
    assert!(parent_of("/portal/deals").is_none(), "{HARNESS}: deals list has no parent");
    assert!(parent_of("/").is_none(), "{HARNESS}: site home has no parent");

    // Positive case: rail items follow the registry order and visibility rules
    let labels: Vec<_> = ui::app::registry::rail_items(Surface::Core, &root_actor())
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    assert_eq!(
        labels,
        vec![
            "Cockpit", "Clients", "Projects", "Contracts", "Cabinet",
            "Workflows", "Forms", "Seller Strategy"
        ],
        "{HARNESS}: Core rail items must be in registry order"
    );

    // Negative case: a non-root user with limited entitlements sees only permitted items
    let limited_user = Actor {
        level: Some(Level::User),
        account_type: "internal".into(),
        authority_codes: vec!["portal.read".into()],
        entitlement_codes: vec!["person.read".into()],
    };
    let limited_rail: Vec<_> = ui::app::registry::rail_items(Surface::Core, &limited_user)
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    assert_eq!(limited_rail, vec!["Clients"], "{HARNESS}: limited user sees only permitted rail items");

    // Negative case: an external account sees no operating world
    let guest = Actor {
        account_type: "external".into(),
        ..limited_user
    };
    assert!(
        ui::app::registry::visible_surfaces(&guest).is_empty(),
        "{HARNESS}: external account sees no operating surfaces"
    );

    // Negative case: a path that doesn't match any screen still has an area
    use ui::app::registry::area_of;
    assert_eq!(area_of("/portal/unknown"), ui::app::registry::Area::Portal);
    assert_eq!(area_of("/unknown"), ui::app::registry::Area::Site);
}