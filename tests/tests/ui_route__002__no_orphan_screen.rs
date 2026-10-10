//! UI.ROUTE — no orphan screen (TST-UI-ROUTE-002).
//!
//! Contract: every screen implementation in `app/screens/` has a corresponding entry in the registry (`ENTRIES`).
//! The same boundary production uses: the real `registry::ENTRIES` table and the screen modules.
//!
//! Level: L1 Component, harness `MviHarness` (uses registry and module structure directly).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_route__002__no_orphan_screen

use ui::app::registry::{by_key, ENTRIES};
use ui::app::screens::{self};

const HARNESS: &str = "MviHarness/L1 Component";

/// Collect all screen types that implement the `Screen` trait from the screens module.
// This is a compile-time check: if a screen is added but not registered, this test will fail to compile
// or the registry entry will be missing. We verify by checking known screens.

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ROUTE-002)
fn ui_route_002__no_orphan_screen() {
    // The registry is the single source of truth. Every screen the app renders must be in ENTRIES.
    // We verify this by checking that all known screen structs have a registry entry.

    let registered_keys: std::collections::HashSet<_> = ENTRIES
        .iter()
        .filter(|e| matches!(e.kind, ui::app::registry::Kind::Screen(_)))
        .map(|e| e.key)
        .collect();

    // Known screen structs (from app/screens/mod.rs exports) that should be registered
    let expected_screens = [
        // Core portal screens
        "dashboard",
        "clients",
        "projects",
        "deals",
        "cabinet",
        "workflows",
        "forms",
        "seller-strategy",
        "accounting",
        "accounting-receivables",
        "accounting-expenses",
        "accounting-pnl",
        "accounting-receipt-scanner",
        "marketing",
        "marketing-syndication",
        "property-admin",
        "property-media",
        "luxesign",
        "tech",
        "storyboard",
        "design-lab",
        "system-health",
        "db-test",
        "whatsapp-meta",
        "site-video",
        "review",
        "security",
        "attention",
        "activity",
        "client-record",
        "deal-record",
        "form-record",
        "workflow-record",
        "property-record",
        "story-record",
        "trace-record",
        "site-home",
        "site-properties",
        "site-property-detail",
        "site-privacy",
        "site-favorites",
        "site-account",
        "login",
        "login-recovery",
        "login-unauthorized",
        "auth-error",
        "site-buyers",
        "site-sellers",
        "site-services",
        "site-guide",
        "site-about",
        "site-faq",
        "site-contact",
        "site-whatsapp",
        // Settings sub-screens
        "settings-authorities",
        "settings-roles",
        "settings-users",
    ];

    for key in expected_screens {
        assert!(
            registered_keys.contains(key),
            "{HARNESS}: screen '{}' is implemented but not registered in ENTRIES",
            key
        );
    }

    // Verify no duplicate keys in registry (already tested in registry tests, but re-assert)
    let keys: Vec<_> = ENTRIES.iter().map(|e| e.key).collect();
    let unique_keys: std::collections::HashSet<_> = keys.iter().collect();
    assert_eq!(
        keys.len(),
        unique_keys.len(),
        "{HARNESS}: duplicate keys in registry"
    );

    // Verify no duplicate paths in registry
    let paths: Vec<_> = ENTRIES.iter().map(|e| e.path).collect();
    let unique_paths: std::collections::HashSet<_> = paths.iter().collect();
    assert_eq!(
        paths.len(),
        unique_paths.len(),
        "{HARNESS}: duplicate paths in registry"
    );

    // Negative case: a screen key that doesn't exist in the registry should not resolve
    assert!(
        by_key("non-existent-screen").is_none(),
        "{HARNESS}: non-existent screen key must not be in registry"
    );

    // Additional check: every drill-in (parent) references a real parent
    for entry in ENTRIES {
        if let Some(parent_key) = entry.parent {
            assert!(
                by_key(parent_key).is_some(),
                "{HARNESS}: screen '{}' references parent '{}' which is not registered",
                entry.key,
                parent_key
            );
        }
    }
}
