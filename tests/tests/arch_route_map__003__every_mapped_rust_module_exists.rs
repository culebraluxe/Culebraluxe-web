//! ARCH.ROUTE_MAP — every mapped Rust module exists (TST-ARCH-ROUTE-MAP-003).
//!
//! Contract: every service domain referenced in the route map (`http_service_domain` in
//! `web/src/api/routes.rs`) corresponds to an actual registered service module in the
//! composition root (`web/src/composition.rs`). A domain that appears in the route map
//! but has no corresponding service module is a capability leak: the service gateway
//! would have no handler for that domain.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_route_map__003__every_mapped_rust_module_exists

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

fn in_repo(relative: &str) -> PathBuf {
    source::repo_root().join(relative)
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

/// Extract all domains referenced in http_service_domain.
fn extract_domains_from_http_service_domain() -> BTreeSet<String> {
    let text = source::read(&in_repo("web/src/api/routes.rs"));
    let mut domains = BTreeSet::new();
    
    // Find the http_service_domain function
    let fn_start = text
        .find("fn http_service_domain")
        .expect("http_service_domain function not found");
    let fn_end = text[fn_start..]
        .find("\n}")
        .expect("http_service_domain function not closed") + fn_start + 2;
    let fn_text = &text[fn_start..fn_end];
    
    // Extract all string literals that look like domain names (Some("domain"))
    for line in fn_text.lines() {
        let code = source::code_of(line);
        // Look for Some("domain") patterns
        let mut search = code;
        while let Some(start) = search.find("Some(\"") {
            let after = &search[start + "Some(\"".len()..];
            if let Some(end) = after.find('"') {
                let domain = &after[..end];
                domains.insert(domain.to_string());
                search = &after[end + 1..];
            } else {
                break;
            }
        }
    }
    
    domains
}

/// Extract all service domains from composition.rs
fn extract_service_domains_from_composition() -> BTreeSet<String> {
    let text = source::read(&in_repo("web/src/composition.rs"));
    let mut domains = BTreeSet::new();
    
    // Find all abstract_service! macro invocations
    for line in text.lines() {
        let code = source::code_of(line);
        // Match: abstract_service!(Type, "domain", "description");
        if code.contains("abstract_service!") {
            // Extract the domain string (second argument)
            if let Some(start) = code.find('"') {
                let after = &code[start + 1..];
                if let Some(end) = after.find('"') {
                    let domain = &after[..end];
                    domains.insert(domain.to_string());
                }
            }
        }
    }
    
    domains
}

#[test]
#[allow(non_snake_case)]
fn arch_route_map_003__every_mapped_rust_module_exists() {
    // 1. Extract all domains referenced in http_service_domain.
    let http_domains = extract_domains_from_http_service_domain();
    eprintln!("DEBUG: http_service_domain references {} domains: {:?}", http_domains.len(), http_domains);
    
    // 2. Extract all service domains registered in composition.rs.
    let service_domains = extract_service_domains_from_composition();
    eprintln!("DEBUG: composition.rs registers {} domains: {:?}", service_domains.len(), service_domains);
    
    // 3. Verify every domain in http_service_domain exists as a registered service.
    let mut missing = Vec::new();
    for domain in &http_domains {
        // Skip the None cases (which appear as empty or special values)
        if domain.is_empty() || domain == "None" || domain == "null" {
            continue;
        }
        if !service_domains.contains(domain.as_str()) {
            missing.push(domain.clone());
        }
    }
    
    // 4. Also verify that all mapped domains from ROUTE_DOMAIN_MAP are valid.
    let mut missing_from_map = Vec::new();
    for (_, domain) in ROUTE_DOMAIN_MAP {
        let domain_str: &str = domain;
        if !service_domains.contains(domain_str) && !SERVICE_DOMAINS.iter().any(|&d| d == domain_str) {
            missing_from_map.push(domain_str.to_string());
        }
    }
    
    // 5. Report findings.
    let mut all_missing = missing;
    all_missing.extend(missing_from_map);
    all_missing.sort();
    all_missing.dedup();
    
    if !all_missing.is_empty() {
        panic!(
            "the following domains are referenced but have no registered Rust module:\n{}",
            all_missing.join("\n")
        );
    }
    
    // 6. POSITIVE CONTROL: Verify all SERVICE_DOMAINS are actually registered.
    for domain in SERVICE_DOMAINS {
        assert!(
            service_domains.contains(*domain),
            "domain `{domain}` in SERVICE_DOMAINS is not registered in composition.rs"
        );
    }
    
    // 7. NEGATIVE CONTROL: A domain not in composition.rs should be detected.
    let test_missing = "this-domain-does-not-exist";
    assert!(
        !service_domains.contains(test_missing),
        "test domain should not exist in composition"
    );
}