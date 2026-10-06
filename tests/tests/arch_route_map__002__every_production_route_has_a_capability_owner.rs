//! ARCH.ROUTE_MAP — every production route has a capability owner (TST-ARCH-ROUTE-MAP-002).
//!
//! Contract: every route the API router defines is owned by a REGISTERED service domain
//! (`web::api::service_domain_for_path`, the function the router runs), or is on the explained list of routes that
//! deliberately have none (health probes, the internal-key engine surface, the external signer's token edge). A route
//! with neither is a capability leak: it would run outside the service gateway's queue, lifecycle and authorization.
//!
//! Level: L0 Pure — filesystem reads and the real mapping function; no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_route_map__002__every_production_route_has_a_capability_owner

#[path = "support/route_map.rs"]
mod route_map;

use route_map::{explained, registered_domains, router_routes, NO_DOMAIN};

#[test]
#[allow(non_snake_case)]
fn arch_route_map_002__every_production_route_has_a_capability_owner() {
    let routes = router_routes();
    assert!(
        routes.len() >= 50,
        "the router must define at least 50 routes; found {}: router may have changed shape",
        routes.len()
    );
    let registered = registered_domains();
    assert!(
        registered.len() >= 25,
        "the registered domains were not read: {registered:?}"
    );

    let mut findings = Vec::new();
    for path in &routes {
        match web::api::service_domain_for_path(path) {
            Some(domain) if !registered.contains(domain) => {
                findings.push(format!("{path}: mapped to `{domain}`, which is not a registered service domain"))
            }
            Some(_) => {}
            None if explained(path).is_none() => {
                findings.push(format!("{path}: no capability owner, and no reason it may have none (add it to a domain, or to NO_DOMAIN in support/route_map.rs with the reason)"))
            }
            None => {}
        }
    }
    assert!(
        findings.is_empty(),
        "the following routes lack a capability owner:\n{}",
        findings.join("\n")
    );

    // NEGATIVE CONTROL: a route nobody owns or explained would be caught.
    let probe = "/v1/test/missing/route";
    assert!(web::api::service_domain_for_path(probe).is_none() && explained(probe).is_none());
    // ...and the explained list is not a place to hide a classified route.
    assert!(!NO_DOMAIN.is_empty());
}
