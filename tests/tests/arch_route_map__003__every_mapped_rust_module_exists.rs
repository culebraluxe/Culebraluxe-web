//! ARCH.ROUTE_MAP — every mapped Rust module exists (TST-ARCH-ROUTE-MAP-003).
//!
//! Contract: every service domain the route map hands a request to is REGISTERED in the composition root or the
//! service gateway. A mapping to a domain nobody registered is a request routed into a mailbox that does not exist.
//! (The registered set is read from every `abstract_service!` invocation — wrapped over several lines or not — and every
//! `ServiceDescriptor`; this test used to read the domain from the macro's first line only and so never saw
//! `accounting` or `calendar`.)
//!
//! Level: L0 Pure — filesystem reads and the real mapping function; no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_route_map__003__every_mapped_rust_module_exists

#[path = "support/route_map.rs"]
mod route_map;

use route_map::{explained, registered_domains, router_routes, NO_DOMAIN};

#[test]
#[allow(non_snake_case)]
fn arch_route_map_003__every_mapped_rust_module_exists() {
    let registered = registered_domains();
    assert!(
        registered.len() >= 25,
        "the registered domains were not read: {registered:?}"
    );

    let routes = router_routes();
    let mapped: std::collections::BTreeSet<&str> = routes
        .iter()
        .filter_map(|path| web::api::service_domain_for_path(path))
        .collect();
    assert!(
        mapped.len() >= 15,
        "the mapping returned too few domains to be the real one: {mapped:?}"
    );

    let missing: Vec<&str> = mapped
        .iter()
        .copied()
        .filter(|d| !registered.contains(*d))
        .collect();
    assert!(
        missing.is_empty(),
        "the following domains are mapped to but have no registered Rust module:\n{}",
        missing.join("\n")
    );

    // POSITIVE CONTROL: the domains the engine's own surface relies on are really in the set.
    for domain in [
        "accounting",
        "calendar",
        "signature",
        "vault",
        "security",
        "person",
        "property",
    ] {
        assert!(
            registered.contains(domain),
            "`{domain}` must be a registered service domain"
        );
    }

    // NEGATIVE CONTROL: a domain that was never registered is detected.
    assert!(!registered.contains("this-domain-does-not-exist"));
}
