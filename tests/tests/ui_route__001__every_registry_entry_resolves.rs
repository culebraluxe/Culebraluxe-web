//! UI.ROUTE — every registry entry resolves (TST-UI-ROUTE-001).
//!
//! Contract: every screen registered in the registry (`app/registry.rs`) can be resolved by its path and produces a valid entry.
//! The same boundary production uses: the real `registry::resolve` function and `ENTRIES` table.
//!
//! Level: L1 Component, harness `MviHarness` (test_harness::mvi::screen::ScreenHarness not needed; uses registry directly).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_route__001__every_registry_entry_resolves

use ui::app::registry::{resolve, ENTRIES};

const HARNESS: &str = "MviHarness/L1 Component";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ROUTE-001)
fn ui_route_001__every_registry_entry_resolves() {
    // Positive case: every registry entry's path resolves to itself
    for entry in ENTRIES {
        // Skip external entries - they don't have a screen to resolve
        if matches!(entry.kind, ui::app::registry::Kind::External) {
            continue;
        }

        let (resolved_entry, params) = resolve(entry.path).unwrap_or_else(|| {
            panic!("{HARNESS}: entry '{}' with path '{}' must resolve", entry.key, entry.path);
        });

        assert_eq!(
            resolved_entry.key, entry.key,
            "{HARNESS}: resolved entry key must match for {}: expected {}, got {}",
            entry.path, entry.key, resolved_entry.key
        );

        // For entries with path params, params should be extracted
        if entry.path.contains(':') {
            assert!(
                !params.is_empty(),
                "{HARNESS}: entry '{}' with param path '{}' must extract params",
                entry.key, entry.path
            );
        } else {
            assert!(
                params.is_empty(),
                "{HARNESS}: entry '{}' with static path '{}' must have no params",
                entry.key, entry.path
            );
        }
    }

    // Negative case: a non-existent path must not resolve
    assert!(
        resolve("/portal/non-existent-screen").is_none(),
        "{HARNESS}: non-existent path must not resolve"
    );
    assert!(
        resolve("/portal/clients/invalid/extra/segments").is_none(),
        "{HARNESS}: path with too many segments must not resolve"
    );

    // Negative case: path with wrong static segment must not match a param route
    assert!(
        resolve("/portal/clients/wrong/static").is_none(),
        "{HARNESS}: wrong static segment must not match param route"
    );
}