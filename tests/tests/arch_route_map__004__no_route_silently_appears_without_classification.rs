//! ARCH.ROUTE_MAP — no route silently appears without classification (TST-ARCH-ROUTE-MAP-004).
//!
//! Contract: a route cannot appear in the router and quietly fall through the service map. Every route is classified
//! by `http_service_domain` (asked of the real function), or is on the explained list of routes that deliberately have
//! no domain; and that list tells the truth in both directions — no entry covers a route that IS classified (it would be
//! exempt AND owned), and no entry is left over for a route that no longer exists.
//!
//! Level: L0 Pure — filesystem reads and the real mapping function; no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_route_map__004__no_route_silently_appears_without_classification

#[path = "support/route_map.rs"]
mod route_map;

use route_map::{explained, registered_domains, router_routes, NO_DOMAIN};

#[test]
#[allow(non_snake_case)]
fn arch_route_map_004__no_route_silently_appears_without_classification() {
    let routes = router_routes();
    assert!(
        routes.len() >= 50,
        "the router must define at least 50 routes; found {}: router may have changed shape",
        routes.len()
    );

    // 1. Nothing is silently unclassified.
    let unclassified: Vec<&String> = routes
        .iter()
        .filter(|path| {
            web::api::service_domain_for_path(path).is_none() && explained(path).is_none()
        })
        .collect();
    assert!(
        unclassified.is_empty(),
        "routes that fall through http_service_domain with no explanation:\n{}",
        unclassified
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );

    // 2. The explained list does not exempt a route that is classified.
    let lying: Vec<String> = routes
        .iter()
        .filter_map(|path| {
            let domain = web::api::service_domain_for_path(path)?;
            let reason = explained(path)?;
            Some(format!(
                "{path} is classified as `{domain}` and also exempt ({reason})"
            ))
        })
        .collect();
    assert!(
        lying.is_empty(),
        "NO_DOMAIN disagrees with the real mapping:\n{}",
        lying.join("\n")
    );

    // 3. No entry is stale: each matches at least one real route.
    let stale: Vec<&str> = NO_DOMAIN
        .iter()
        .map(|(entry, _)| *entry)
        .filter(|entry| {
            !routes.iter().any(|path| {
                if entry.ends_with('/') {
                    path.starts_with(entry)
                } else {
                    path == entry
                }
            })
        })
        .collect();
    assert!(
        stale.is_empty(),
        "NO_DOMAIN entries that match no route in the router: {stale:?}"
    );

    // NEGATIVE CONTROL.
    let probe = "/v1/test/unclassified/route";
    assert!(web::api::service_domain_for_path(probe).is_none() && explained(probe).is_none());
}
