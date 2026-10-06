//! ARCH.ROUTE_MAP — every declared route exists (TST-ARCH-ROUTE-MAP-001).
//!
//! Contract: a URL the UI calls must be a route the server registers. The UI
//! declares its server surface in `web/ui/src/app/api.rs` as
//! `/api/portal/rust-ui/<segment>` paths; the server registers each segment in
//! `web/src/api/portal_bridge.rs`. A UI call to an unregistered segment is a
//! dead screen, and a bridge route no UI names is either load-bearing
//! infrastructure or drift — both deserve a name in this map.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_route_map__001__every_declared_route_exists

use test_harness::source;

/// Every `/api/portal/rust-ui/<segment>` path prefix declared in a source text, in first-seen order.
fn declared_segments(text: &str) -> Vec<String> {
    let marker = "/api/portal/rust-ui/";
    let mut segments = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(marker) {
        let after = &rest[at + marker.len()..];
        let end = after
            .find(|c: char| !c.is_ascii_lowercase() && c != '-')
            .unwrap_or(after.len());
        let segment = &after[..end];
        if !segment.is_empty() && !segments.iter().any(|s| s == segment) {
            segments.push(segment.to_string());
        }
        rest = after;
    }
    segments
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ROUTE-MAP-001); the file and the assay use it.
fn arch_route_map_001__every_declared_route_exists() {
    let root = source::workspace_root();
    let ui = source::read(&root.join("web/ui/src/app/api.rs"));
    let bridge = source::read(&root.join("web/src/api/portal_bridge.rs"));

    // The UI declares a non-empty surface, and it includes the screens this tree is known to serve.
    let ui_segments = declared_segments(&ui);
    assert!(
        ui_segments.len() >= 2,
        "the UI surface is the subject of this map; only {:?} was found",
        ui_segments
    );
    for known in ["clients", "page"] {
        assert!(
            ui_segments.iter().any(|segment| segment == known),
            "the UI declares /api/portal/rust-ui/{known}, so the map must cover it"
        );
    }

    // Every UI-declared segment is a registered server route: no dead screens.
    for segment in &ui_segments {
        assert!(
            bridge.contains(&format!(".route(\"/api/portal/rust-ui/{segment}\"")),
            "the UI calls /api/portal/rust-ui/{segment} but the bridge registers no such route"
        );
    }

    // The bridge side stays a rust-ui map: every route it declares lives under the same prefix,
    // and the map only grows — a removal fails here so it gets a conscious decision.
    let bridge_segments = declared_segments(&bridge);
    assert!(
        bridge_segments.len() >= 17,
        "the bridge map shrank to {} routes ({:?}); removals belong in a story, not in silence",
        bridge_segments.len(),
        bridge_segments
    );
    for segment in &bridge_segments {
        assert!(
            bridge.contains(&format!("/api/portal/rust-ui/{segment}\"")),
            "bridge route {segment} must stay under the /api/portal/rust-ui/ prefix"
        );
    }

    // Negative controls: the extractor takes the segment and stops at queries, screens, and scopes.
    assert_eq!(
        declared_segments("\"/api/portal/rust-ui/clients?screen=clients\""),
        vec!["clients".to_string()]
    );
    assert_eq!(declared_segments("no routes here"), Vec::<String>::new());
}
