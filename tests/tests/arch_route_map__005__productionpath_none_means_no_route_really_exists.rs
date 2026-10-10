//! ARCH.ROUTE_MAP — productionPath=none means no route really exists (TST-ARCH-ROUTE-MAP-005).
//!
//! Contract: the production-path classifier (`is_rust_contract_production_path`,
//! the same function the Forge assay uses to decide what a story may touch) is
//! honest in both directions. Paths under the production roots classify, paths
//! outside them do not — and, crucially, no path the classifier rejects is
//! registered as a live route. A `legacy/…` or `scripts/…` route would be a
//! production path of none that really exists.
//!
//! Level: L0 Pure — pure calls and file reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_route_map__005__productionpath_none_means_no_route_really_exists

use forge::engine::assay::is_rust_contract_production_path;
use test_harness::source;

/// Every `"/api/…"` literal registered in the bridge, in first-seen order.
fn registered_routes(bridge: &str) -> Vec<String> {
    let marker = "\"/api/";
    let mut routes = Vec::new();
    let mut rest = bridge;
    while let Some(at) = rest.find(marker) {
        let after = &rest[at + 1..];
        let end = after.find('"').unwrap_or(after.len());
        let route = after[..end].split('{').next().unwrap_or("").to_string();
        if !route.is_empty() && !routes.iter().any(|r| r == &route) {
            routes.push(route);
        }
        rest = &after[end.min(after.len())..];
        if end >= after.len() {
            break;
        }
        rest = &after[end + 1..];
    }
    routes.sort();
    routes
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ROUTE-MAP-005); the file and the assay use it.
fn arch_route_map_005__productionpath_none_means_no_route_really_exists() {
    // The classifier says what it means: production roots in, everything else out.
    for production in [
        "web/src/api/portal_bridge.rs",
        "middle/model/src/security.rs",
        "db/src/media/media_row.rs",
        "cli/src/forge/lint.rs",
        "forge/src/engine/assay.rs",
        "  web/src/site.rs  ",
    ] {
        assert!(
            is_rust_contract_production_path(production),
            "{production:?} is production and must classify so"
        );
    }
    for none in [
        "legacy/agent-runtime/index.ts",
        "scripts/dev.sh",
        "tests/tests/luxesign_native_dev.rs",
        "docs/agent/MEMORY.md",
        "experiments/bench.rs",
        "",
        "   ",
    ] {
        assert!(
            !is_rust_contract_production_path(none),
            "{none:?} is not a production path and must classify as none"
        );
    }

    // And none means none: no registered route points outside the production API surface.
    let bridge = source::read(&source::workspace_root().join("web/src/api/portal_bridge.rs"));
    let routes = registered_routes(&bridge);
    assert!(
        routes.len() >= 17,
        "the bridge registers a real surface; only {} routes were found",
        routes.len()
    );
    for route in &routes {
        assert!(
            route.starts_with("/api/portal/") || route.starts_with("/api/property-media/"),
            "route {route:?} lives outside the classified production API surface"
        );
        for retired in ["legacy", ".ts", ".tsx", ".mjs", "scripts/", "docs/"] {
            assert!(
                !route.contains(retired),
                "route {route:?} references a non-production path {retired:?} that really exists as a route"
            );
        }
    }
}
