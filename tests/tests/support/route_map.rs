//! Shared facts for the ARCH.ROUTE_MAP contract tests: the router's routes, the service domains that are REGISTERED,
//! and the routes that deliberately belong to no domain — each with the reason.
//!
//! The mapping itself is never re-parsed from source: the tests ask the real `web::api::service_domain_for_path`. What
//! lives here is what a function cannot say about itself — which paths the router defines, which domains exist, and
//! why a route is allowed to have no capability owner.
#![allow(dead_code)]

use std::collections::BTreeSet;

use test_harness::source;

/// Routes that deliberately have NO service domain, with the reason. An entry ending in `/` covers a prefix; any other
/// entry is one exact path. The tests require every entry to match a real route (a stale entry is a lie about the
/// router) and every domain-less route to be covered (an unexplained one is a capability leak).
pub const NO_DOMAIN: &[(&str, &str)] = &[
    ("/healthz", "health probe"),
    ("/readyz", "readiness probe"),
    (
        "/api/build-info",
        "release gate: which commit is live (public, read-only)",
    ),
    (
        "/api/rust-ready",
        "release gate: readiness plus the resolved database target",
    ),
    ("/v1/whoami", "identity echo for the calling session"),
    (
        "/v1/cockpit",
        "screen served by the cockpit's own entitlement check",
    ),
    (
        "/v1/workflows",
        "workflow portal, authorized by entitlement in its handler",
    ),
    (
        "/v1/workflows/",
        "workflow instance pages, authorized by entitlement in their handler",
    ),
    (
        "/v1/flight-recorder/",
        "flight recorder, authorized by entitlement in its handler",
    ),
    (
        "/v1/tasks/",
        "task completion, authorized against the task's own candidates",
    ),
    (
        "/v1/engine/",
        "engine surface: internal API key only (resolve_engine_context), no user identity",
    ),
    (
        "/v1/diagnostics/",
        "operator diagnostics: internal API key or app-error capture, no domain",
    ),
    (
        "/v1/commands/dispatch",
        "the service gateway's own dispatch door",
    ),
    (
        "/v1/services",
        "service catalog and health: the gateway itself, not a domain",
    ),
    (
        "/v1/services/",
        "service catalog and health: the gateway itself, not a domain",
    ),
    (
        "/v1/signer/",
        "the EXTERNAL signer's edge: authenticated by the signer's session token, not by a user",
    ),
];

/// The reason `path` is allowed to have no domain, if it is.
pub fn explained(path: &str) -> Option<&'static str> {
    NO_DOMAIN.iter().find_map(|(entry, reason)| {
        let covers = if entry.ends_with('/') {
            path.starts_with(entry)
        } else {
            path == *entry
        };
        covers.then_some(*reason)
    })
}

/// Every path the router defines (`.route("…")` in `web/src/api/routes.rs`).
pub fn router_routes() -> BTreeSet<String> {
    let text = source::read(&source::repo_root().join("web/src/api/routes.rs"));
    let mut full = String::new();
    for line in text.lines() {
        let code = source::code_of(line);
        if !code.trim().is_empty() {
            full.push_str(code);
            full.push(' ');
        }
    }
    let mut routes = BTreeSet::new();
    let mut from = 0;
    while let Some(at) = full[from..].find(".route(") {
        let start = from + at;
        let after = &full[start + ".route(".len()..];
        if let Some(open) = after.find('"') {
            if let Some(len) = after[open + 1..].find('"') {
                let path = &after[open + 1..open + 1 + len];
                if path.starts_with('/') {
                    routes.insert(path.to_string());
                }
            }
        }
        from = start + 1;
    }
    routes
}

/// Every service domain that is REGISTERED: an `abstract_service!(…, "domain", …)` invocation (wrapped or not) or a
/// `ServiceDescriptor`'s `domain: "…"`, in the composition root and the service gateway.
pub fn registered_domains() -> BTreeSet<String> {
    let mut domains = BTreeSet::new();
    for file in ["web/src/composition.rs", "web/src/service_gateway.rs"] {
        let text = source::read(&source::repo_root().join(file));
        // abstract_service!( Type<Dao>, "domain", "description" );  — rustfmt may wrap it over several lines.
        let mut rest = text.as_str();
        while let Some(at) = rest.find("abstract_service!(") {
            let body = &rest[at + "abstract_service!(".len()..];
            let end = body.find(");").unwrap_or(body.len());
            if let Some(open) = body[..end].find('"') {
                if let Some(len) = body[open + 1..end].find('"') {
                    domains.insert(body[open + 1..open + 1 + len].to_string());
                }
            }
            rest = &body[end..];
        }
        // domain: "x".into()  /  domain: "x",
        let mut rest = text.as_str();
        while let Some(at) = rest.find("domain: \"") {
            let after = &rest[at + "domain: \"".len()..];
            if let Some(len) = after.find('"') {
                domains.insert(after[..len].to_string());
            }
            rest = after;
        }
    }
    domains
}
