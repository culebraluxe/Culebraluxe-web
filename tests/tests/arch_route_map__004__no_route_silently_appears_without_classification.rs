//! ARCH.ROUTE_MAP — no route silently appears without classification (TST-ARCH-ROUTE-MAP-004).
//!
//! Contract: every HTTP route defined in the API router must be explicitly handled by
//! `http_service_domain` in `web/src/api/routes.rs`. A route that falls through to the
//! final implicit `None` without an explicit entry is a capability leak: it would execute
//! without a service domain, bypassing the service gateway's queue, lifecycle, and
//! authorization controls.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_route_map__004__no_route_silently_appears_without_classification

use std::collections::BTreeSet;
use std::path::PathBuf;

use test_harness::source;

/// Routes that are intentionally public (explicitly return None from http_service_domain).
/// These are explicitly listed in http_service_domain with explicit `return None;`.
const EXPLICITLY_PUBLIC: &[&str] = &[
    "/v1/cockpit",
    "/v1/workflows",
    "/v1/flight-recorder",
    "/v1/tasks/",
    "/api/portal/rust-ui/clients",
    // Health checks and public endpoints
    "/healthz",
    "/readyz",
    "/api/build-info",
    "/api/rust-ready",
    "/v1/whoami",
    // Diagnostics
    "/v1/diagnostics/app-error",
    "/v1/diagnostics/db",
    // Engine endpoints (no service domain)
    "/v1/engine/reclaim",
    "/v1/engine/tasks/complete",
    "/v1/engine/timers/reconcile",
    "/v1/engine/transactions",
    // Process instances
    "/v1/process-instances/",
    // Signer endpoints
    "/v1/signer/",
    // Services catalog and dispatch
    "/v1/services",
    "/v1/services/dispatch",
    "/v1/services/health",
    "/v1/services/kernel/health",
    "/v1/services/runtime/health",
    "/v1/services/{domain}/control",
    // Commands dispatch
    "/v1/commands/dispatch",
    // Catchup
    "/v1/catchup/leads",
];

/// The service domains registered in the composition root.
const SERVICE_DOMAINS: &[&str] = &[
    "accounting",
    "calendar",
    "catch-up",
    "client",
    "client-room",
    "cockpit",
    "communications",
    "contract",
    "deal",
    "document-sign",
    "email",
    "firm",
    "flight-recorder",
    "forms",
    "guide",
    "intake",
    "issue",
    "marketing",
    "media",
    "person",
    "project",
    "property",
    "public-listing",
    "publishing",
    "relationship-evidence",
    "security",
    "showings",
    "signature",
    "signer",
    "support",
    "task",
    "tech",
    "vault",
    "wbs",
    "website-lead",
    "whatsapp",
    "workflow-portal",
];

fn in_repo(relative: &str) -> PathBuf {
    source::repo_root().join(relative)
}

/// Extract all route paths from the router definition in `web/src/api/routes.rs`.
fn extract_routes_from_router() -> BTreeSet<String> {
    let text = source::read(&in_repo("web/src/api/routes.rs"));
    let mut routes = BTreeSet::new();

    // Join all lines and find .route(" patterns anywhere in the text
    let mut full_code = String::new();
    for line in text.lines() {
        let code = source::code_of(line);
        if !code.trim().is_empty() {
            full_code.push_str(code);
            full_code.push(' ');
        }
    }

    let mut search_start = 0;
    while let Some(start) = full_code[search_start..].find(".route(") {
        let abs_start = search_start + start;
        let after_route = &full_code[abs_start + ".route(".len()..];

        if let Some(path_start) = after_route.find('"') {
            if let Some(path_end) = after_route[path_start + 1..].find('"') {
                let path = &after_route[path_start + 1..path_start + 1 + path_end];
                if path.starts_with('/') {
                    routes.insert(path.to_string());
                }
            }
        }
        search_start = abs_start + 1;
    }

    routes
}

/// Check if a path is explicitly listed as public in http_service_domain.
fn is_explicitly_public(path: &str) -> bool {
    EXPLICITLY_PUBLIC
        .iter()
        .any(|&public| path == public || path.starts_with(public.trim_end_matches('/')))
}

/// Extract all explicitly handled paths from http_service_domain function.
/// Only captures paths that return `Some(domain)`, not those that return `None`.
fn extract_explicitly_handled_paths() -> BTreeSet<String> {
    let text = source::read(&in_repo("web/src/api/routes.rs"));
    let mut handled = BTreeSet::new();

    let fn_start = text
        .find("fn http_service_domain")
        .expect("http_service_domain function not found");
    let fn_end = text[fn_start..]
        .find("\n}")
        .expect("http_service_domain function not closed")
        + fn_start
        + 2;
    let fn_text = &text[fn_start..fn_end];

    // Extract paths from if conditions that return Some(domain).
    // We track whether we're in a block that returns Some(...).
    let mut in_some_block = false;
    let mut some_block_depth = 0;

    for line in fn_text.lines() {
        let code = source::code_of(line);

        // Track block entry/exit for Some returns
        if code.contains("return Some(") {
            in_some_block = true;
            some_block_depth = 1;
        }
        if in_some_block {
            for ch in code.chars() {
                if ch == '{' {
                    some_block_depth += 1;
                } else if ch == '}' {
                    some_block_depth -= 1;
                    if some_block_depth == 0 {
                        in_some_block = false;
                    }
                }
            }
        }

        // Only extract paths when we're in a Some-returning block
        if in_some_block {
            // Match path == "..."
            if let Some(start) = code.find("path ==") {
                let after = &code[start + "path ==".len()..];
                if let Some(path_start) = after.find('"') {
                    if let Some(path_end) = after[path_start + 1..].find('"') {
                        let path = &after[path_start + 1..path_start + 1 + path_end];
                        if path.starts_with('/') {
                            handled.insert(path.to_string());
                        }
                    }
                }
            }

            // Match path.starts_with("...")
            if let Some(start) = code.find("path.starts_with(") {
                let after = &code[start + "path.starts_with(".len()..];
                if let Some(path_start) = after.find('"') {
                    if let Some(path_end) = after[path_start + 1..].find('"') {
                        let pattern = &after[path_start + 1..path_start + 1 + path_end];
                        handled.insert(pattern.to_string());
                    }
                }
            }

            // Match path.contains("...")
            if let Some(start) = code.find("path.contains(") {
                let after = &code[start + "path.contains(".len()..];
                if let Some(path_start) = after.find('"') {
                    if let Some(path_end) = after[path_start + 1..].find('"') {
                        let pattern = &after[path_start + 1..path_start + 1 + path_end];
                        handled.insert(pattern.to_string());
                    }
                }
            }
        }
    }

    handled
}

#[test]
#[allow(non_snake_case)]
fn arch_route_map_004__no_route_silently_appears_without_classification() {
    // 1. Extract all routes from the router definition.
    let routes = extract_routes_from_router();
    assert!(
        routes.len() >= 50,
        "the router must define at least 50 routes; found {}: router may have changed shape",
        routes.len()
    );

    // 2. Extract all explicitly handled paths from http_service_domain.
    let handled_paths = extract_explicitly_handled_paths();
    eprintln!(
        "DEBUG: http_service_domain explicitly handles {} path patterns",
        handled_paths.len()
    );

    // 3. For each route, verify it is explicitly handled or explicitly public.
    let mut unclassified = Vec::new();

    for path in &routes {
        // Check if explicitly public
        if is_explicitly_public(path) {
            continue;
        }

        // Check if explicitly handled by http_service_domain
        let mut handled = false;
        for pattern in &handled_paths {
            if path == pattern || path.starts_with(pattern.trim_end_matches('/')) {
                handled = true;
                break;
            }
        }

        if !handled {
            unclassified.push(path.clone());
        }
    }

    // 4. Report any unclassified routes.
    if !unclassified.is_empty() {
        eprintln!("UNCLASSIFIED ROUTES (fall through to implicit None):");
        for path in &unclassified {
            eprintln!("  {}", path);
        }
        panic!(
            "the following routes lack explicit classification in http_service_domain:\n{}",
            unclassified.join("\n")
        );
    }

    // 5. POSITIVE CONTROL: Verify all explicitly public routes are correctly unclassified
    // (they intentionally fall through to implicit None or are explicitly returned as None).
    for public in EXPLICITLY_PUBLIC {
        assert!(
            !handled_paths.contains(*public),
            "explicitly public route `{public}` must NOT be in handled_paths (it should fall through to None)"
        );
    }

    // 6. NEGATIVE CONTROL: A route not in http_service_domain must be detected.
    let test_unclassified = "/v1/test/unclassified/route";
    assert!(
        !is_explicitly_public(test_unclassified),
        "test path must not be explicitly public"
    );
    assert!(
        !handled_paths
            .iter()
            .any(|p| test_unclassified.starts_with(p.trim_end_matches('/'))),
        "test path must not match any handled pattern"
    );
}
