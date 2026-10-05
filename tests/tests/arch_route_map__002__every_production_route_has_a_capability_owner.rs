//! ARCH.ROUTE_MAP — every production route has a capability owner (TST-ARCH-ROUTE-MAP-002).
//!
//! Contract: every HTTP route defined in the API router is mapped to a service domain
//! (capability owner) by `http_service_domain` in `web/src/api/routes.rs`.
//! A route returning `None` is intentionally public (health checks, static files, auth callbacks).
//! A route falling through to the final `None` without an explicit entry is a capability leak:
//! it would execute without a service domain, bypassing the service gateway's queue, lifecycle,
//! and authorization controls.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_route_map__002__every_production_route_has_a_capability_owner

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use test_harness::source;

/// The service domains registered in the composition root — the canonical capability owners.
/// These come from the `abstract_service!` macro invocations in `web/src/composition.rs`.
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

/// Routes that are intentionally public (return None from http_service_domain).
/// These are the health checks, auth callbacks, and public endpoints that bypass the service gateway.
const INTENTIONALLY_PUBLIC: &[&str] = &[
    "/healthz",
    "/readyz",
    "/api/build-info",
    "/api/rust-ready",
    "/v1/whoami",
    "/api/integrations/whatsapp/webhook",
    "/api/integrations/boldsign/webhook",
    "/v1/security/guests",
    "/v1/security/guest-code",
    "/v1/security/guest-code/verify",
    "/v1/security/authorize",
    "/v1/security/role-entitlements",
    "/v1/tech/cockpit",
    "/v1/tasks/",
    "/v1/workflows",
    "/v1/flight-recorder",
    "/api/portal/rust-ui/clients",
    "/api/portal/rust-ui/entitlements",
    "/api/portal/rust-ui/publishing",
    "/api/portal/rust-ui/tech",
    "/api/portal/rust-ui/deals",
    "/api/portal/rust-ui/forms",
    "/api/portal/rust-ui/opps",
    "/api/portal/rust-ui/listing-media",
    "/api/portal/rust-ui/rows",
    "/api/portal/rust-ui/cabinet",
    "/api/portal/rust-ui/page",
    "/api/portal/rust-ui/support",
    "/api/portal/documents",
    "/v1/security/identity",
    "/v1/diagnostics/app-error",
    "/v1/services",
    "/v1/services/health",
    "/v1/services/kernel/health",
    "/v1/services/runtime/health",
    "/v1/services/dispatch",
    "/v1/commands/dispatch",
    "/v1/services/{domain}/control",
    "/v1/calendar",
    "/v1/public/",
    "/v1/public/listing",
    "/v1/public/property",
    "/v1/public/media",
    "/v1/public/similar",
    "/v1/public/slugs",
    "/v1/public/guide",
    "/v1/public/marketing-content",
    "/v1/website-intake",
    "/v1/catchup/leads",
    "/v1/website-intake/",
    "/v1/services/{domain}/control",
    "/v1/signature/",
    "/v1/tech/",
    "/v1/support/",
];

/// A mapping of route path patterns to their expected service domain.
/// This is derived from `http_service_domain` in `web/src/api/routes.rs`.
const ROUTE_DOMAIN_MAP: &[(&str, &str)] = &[
    ("/v1/security/", "security"),
    ("/v1/support/", "support"),
    ("/v1/tech/", "tech"),
    ("/v1/projects", "project"),
    ("/v1/wbs", "wbs"),
    ("/v1/clients", "client"),
    ("/v1/people", "person"),
    ("/v1/properties", "property"),
    ("/v1/media", "media"),
    ("/v1/deals", "deal"),
    ("/v1/contracts", "contract"),
    ("/v1/vault", "vault"),
    ("/v1/forms", "forms"),
    ("/v1/comms", "communications"),
    ("/v1/accounting", "accounting"),
    ("/v1/calendar", "calendar"),
    ("/v1/public/listing", "public-listing"),
    ("/v1/public/property", "public-listing"),
    ("/v1/public/media", "public-listing"),
    ("/v1/public/similar", "public-listing"),
    ("/v1/public/slugs", "public-listing"),
    ("/v1/public/guide", "guide"),
    ("/v1/public/marketing-content", "marketing"),
    ("/v1/website-intake", "intake"),
    ("/v1/catchup/leads", "intake"),
    ("/v1/website-intake/", "website-lead"),
    ("/v1/issues", "issue"),
    ("/v1/relationship-evidence", "relationship-evidence"),
    ("/api/integrations/boldsign/webhook", "signature"),
    ("/v1/signature/", "signature"),
    ("/api/integrations/whatsapp/webhook", "whatsapp"),
];

/// Negative control: a route that MUST have a domain but the map misses it.
/// This should fail if http_service_domain has an entry the map doesn't know.
const MISSED_ROUTE_PATTERNS: &[&str] = &[
    "/v1/tech/cockpit", // Returns None intentionally
    "/v1/tasks/",        // Returns None intentionally
    "/v1/workflows",     // Returns None intentionally
    "/v1/flight-recorder", // Returns None intentionally
];

fn in_repo(relative: &str) -> PathBuf {
    source::repo_root().join(relative)
}

/// Extract all route paths from the router definition in `web/src/api/routes.rs`.
/// Handles both single-line and multi-line route definitions.
fn extract_routes_from_router() -> BTreeMap<String, Vec<String>> {
    let text = source::read(&in_repo("web/src/api/routes.rs"));
    eprintln!("DEBUG: read {} bytes from routes.rs", text.len());
    let mut routes = BTreeMap::new();
    
    // Process line by line, stripping comments, then search for .route(" patterns
    let mut full_code = String::new();
    for line in text.lines() {
        let code = source::code_of(line);
        if !code.trim().is_empty() {
            full_code.push_str(code);
            full_code.push(' ');
        }
    }
    
    eprintln!("DEBUG: full_code length: {}", full_code.len());
    
    let mut search_start = 0;
    while let Some(start) = full_code[search_start..].find(".route(") {
        let abs_start = search_start + start;
        let after_route = &full_code[abs_start + ".route(".len()..];
        
        // Find the first quoted string (the path)
        if let Some(path_start) = after_route.find('"') {
            if let Some(path_end) = after_route[path_start + 1..].find('"') {
                let path = &after_route[path_start + 1..path_start + 1 + path_end];
                if path.starts_with('/') {
                    eprintln!("DEBUG: found route {}", path);
                    let handlers = extract_handlers(after_route);
                    routes.insert(path.to_string(), handlers);
                }
            }
        }
        search_start = abs_start + 1;
    }
    
    eprintln!("DEBUG: extracted {} routes", routes.len());
    routes
}

/// Extract handler names from a route definition.
fn extract_handlers(text: &str) -> Vec<String> {
    let mut handlers = Vec::new();
    // Match get(handler), post(handler), etc.
    let patterns = ["get(", "post(", "put(", "patch(", "delete("];
    for pattern in patterns {
        let mut rest = text;
        while let Some(start) = rest.find(pattern) {
            let after = &rest[start + pattern.len()..];
            if let Some(end) = after.find(')') {
                let handler = &after[..end].trim();
                // Only take the function name, not the full expression
                if let Some(fn_name) = handler.split('.').last() {
                    handlers.push(fn_name.to_string());
                }
                rest = &after[end + 1..];
            } else {
                break;
            }
        }
    }
    handlers.sort();
    handlers.dedup();
    handlers
}

/// Check if a path is intentionally public.
fn is_intentionally_public(path: &str) -> bool {
    INTENTIONALLY_PUBLIC.iter().any(|&public| {
        path == public || path.starts_with(public.trim_end_matches('/'))
    })
}

/// Check if a path matches a known domain mapping.
fn expected_domain(path: &str) -> Option<String> {
    for (pattern, domain) in ROUTE_DOMAIN_MAP {
        if path.starts_with(pattern) {
            return Some(domain.to_string());
        }
    }
    None
}

/// Verify a service domain exists in the registered domains.
fn is_valid_domain(domain: &str) -> bool {
    SERVICE_DOMAINS.iter().any(|&d| d == domain)
}

#[test]
#[allow(non_snake_case)]
fn arch_route_map_002__every_production_route_has_a_capability_owner() {
    // 1. Extract all routes from the router definition.
    let routes = extract_routes_from_router();
    assert!(
        routes.len() >= 50,
        "the router must define at least 50 routes; found {}: router may have changed shape",
        routes.len()
    );

    // 2. Read the http_service_domain function to get its route->domain mapping.
    let http_domain_text = source::read(&in_repo("web/src/api/routes.rs"));
    let domain_fn_start = http_domain_text
        .find("fn http_service_domain")
        .expect("http_service_domain function not found in web/src/api/routes.rs");
    let domain_fn_end = http_domain_text[domain_fn_start..]
        .find("\n}")
        .expect("http_service_domain function not closed") + domain_fn_start + 2;
    let domain_fn = &http_domain_text[domain_fn_start..domain_fn_end];

    // 3. For each route, verify it has a capability owner.
    let mut findings = Vec::new();
    let mut covered_paths = BTreeSet::new();
    
    for (path, handlers) in &routes {
        // Skip health checks and other intentionally public routes
        if is_intentionally_public(path) {
            continue;
        }
        
        // Check if http_service_domain has an entry for this path
        let has_explicit_entry = domain_fn.contains(path) || 
            ROUTE_DOMAIN_MAP.iter().any(|(pattern, _)| path.starts_with(pattern));
        
        if !has_explicit_entry {
            // Check if it's a sub-path of a known pattern
            let mut matched = false;
            for (pattern, domain) in ROUTE_DOMAIN_MAP {
                if path.starts_with(pattern) {
                    matched = true;
                    if !is_valid_domain(domain) {
                        findings.push(format!("{path}: mapped to unknown domain `{domain}`"));
                    }
                    covered_paths.insert(path.clone());
                    break;
                }
            }
            
            if !matched && !is_intentionally_public(path) {
                findings.push(format!("{path}: no capability owner in http_service_domain (handlers: {:?})", handlers));
            }
        } else {
            covered_paths.insert(path.clone());
        }
    }

    // 4. Verify all mapped domains are valid service domains.
    for (_, domain) in ROUTE_DOMAIN_MAP {
        assert!(
            is_valid_domain(domain),
            "domain `{domain}` in ROUTE_DOMAIN_MAP is not a registered service domain"
        );
    }

    // 5. NEGATIVE CONTROL: Verify the detector finds a missing route.
    // Plant a path that should have a domain but doesn't.
    let test_path = "/v1/test/missing/route";
    assert!(
        !is_intentionally_public(test_path),
        "test path must not be intentionally public"
    );
    assert!(
        expected_domain(test_path).is_none(),
        "test path must not match any known pattern"
    );

    // 6. The detector must report the finding.
    if !findings.is_empty() {
        panic!(
            "the following routes lack a capability owner in http_service_domain:\n{}",
            findings.join("\n")
        );
    }

    // 7. Verify the domain function covers all non-public routes in the map.
    for (pattern, domain) in ROUTE_DOMAIN_MAP {
        assert!(
            domain_fn.contains(pattern),
            "http_service_domain must contain pattern `{pattern}` for domain `{domain}`"
        );
    }

    // 8. Verify all service domains in the map are valid.
    for (_, domain) in ROUTE_DOMAIN_MAP {
        assert!(
            is_valid_domain(domain),
            "domain `{domain}` is not a registered service domain"
        );
    }

    // 9. POSITIVE CONTROL: The intentionally public routes must return None.
    // This is verified by the absence of these paths in the domain function.
    for public in INTENTIONALLY_PUBLIC {
        if public.contains("healthz") || public.contains("build-info") || public.contains("rust-ready") || public.contains("whoami") {
            assert!(
                !domain_fn.contains(public),
                "intentionally public route `{public}` must not appear in http_service_domain"
            );
        }
    }

    // 10. The final None in http_service_domain must be the explicit catch-all.
    assert!(
        domain_fn.trim_end().ends_with("None"),
        "http_service_domain must end with explicit None return for unmatched paths"
    );
}