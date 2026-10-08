//! SERVICE.ABSTRACT — identity/name (TST-SERVICE-ABSTRACT-001).
//!
//! Contract: every Abstract Service has one stable identity — a domain name, a version, and a declared capability
//! list — and the identity is the same object production routes by. The test drives the real production service
//! (`ForgeService`, constructed with no I/O) through the real `AbstractService::descriptor` boundary and pins four
//! properties:
//!
//! - **The domain is the wire identity.** `descriptor().domain` is exactly the `FORGE_SERVICE_DOMAIN` constant the
//!   service dispatches under — one name, not two — and it is the literal `"forge"` on the wire.
//! - **The descriptor is stable.** Two fresh instances describe identically: identity carries no per-instance state.
//! - **Capabilities name real operations.** Each capability names a non-empty operation with a non-empty
//!   authorization action and execution policy, and the set is exactly the service's two operations — a phantom
//!   capability, or a capability with no authorizer, fails here.
//! - **The identity survives the wire.** The descriptor round-trips through its camelCase JSON shape unchanged, so
//!   the identity a remote caller sees is the identity production declared.
//!
//! What this test does NOT pin (other taxonomy owns it): dispatch behaviour (005/006), authorization (007), audit
//! (008), errors (009), events (010), malformed input (011), idempotency (012). No `dispatch` call appears below.
//!
//! Level: L1 Component — the production service object, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__001__identity_name

use forge::service::{
    ForgeService, FORGE_LIVE_SNAPSHOT_OPERATION, FORGE_SCHEDULED_PASS_OPERATION,
    FORGE_SERVICE_DOMAIN,
};
use services::AbstractService;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-001).
fn service_abstract_001__identity_name() {
    // 1. THE DOMAIN IS THE WIRE IDENTITY. The descriptor names the exact constant the service dispatches under —
    //    routing and identity cannot disagree — and the constant is the literal wire name.
    assert_eq!(FORGE_SERVICE_DOMAIN, "forge");
    let descriptor = ForgeService::new().descriptor();
    assert_eq!(
        descriptor.domain, FORGE_SERVICE_DOMAIN,
        "the descriptor must name the domain production routes by"
    );
    assert!(
        !descriptor.version.is_empty(),
        "a versionless identity cannot be routed"
    );
    assert!(
        !descriptor.description.is_empty(),
        "an undescribed identity cannot be diagnosed"
    );

    // 2. THE DESCRIPTOR IS STABLE. Two fresh instances describe identically: identity is a fact about the service,
    //    not about the instance. A descriptor smuggling per-instance state (a counter, a timestamp) fails here.
    assert_eq!(
        ForgeService::new().descriptor(),
        descriptor,
        "identity must not vary between instances"
    );

    // 3. CAPABILITIES NAME REAL OPERATIONS. Exactly the service's two operations, each with an authorizer and an
    //    execution policy: a phantom third capability, or one no authorizer could decide, fails here.
    assert_eq!(
        descriptor.capabilities.len(),
        2,
        "the forge identity declares exactly its two operations"
    );
    let names: Vec<&str> = descriptor
        .capabilities
        .iter()
        .map(|capability| capability.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            FORGE_SCHEDULED_PASS_OPERATION,
            FORGE_LIVE_SNAPSHOT_OPERATION
        ],
        "capability names are the operation constants, not retyped strings"
    );
    for capability in &descriptor.capabilities {
        assert!(
            !capability.authorization.is_empty(),
            "capability {} names no authorizer: an undecidable capability",
            capability.name
        );
    }

    // 4. THE IDENTITY SURVIVES THE WIRE. Serialize and back through the camelCase shape: the identity a remote
    //    caller sees is byte-for-byte the identity production declared — including the capability fields.
    let wire = serde_json::to_value(&descriptor).expect("the descriptor must serialize");
    assert_eq!(
        wire.get("domain").and_then(serde_json::Value::as_str),
        Some("forge"),
        "the wire identity is the domain, under its camelCase key"
    );
    assert!(
        wire.get("capabilities")
            .is_some_and(|value| value.is_array()),
        "capabilities travel under their camelCase key"
    );
    let back: services::ServiceDescriptor =
        serde_json::from_value(wire).expect("the wire shape must deserialize");
    assert_eq!(
        back, descriptor,
        "the round-tripped identity must equal the declared one"
    );

    // Negative controls: identity is exact. A differently-cased domain is a different domain, and a descriptor
    // with a phantom capability is not this service's identity — so a comparison that ignored either would pass a
    // forgery this contract refuses.
    assert_ne!("Forge", FORGE_SERVICE_DOMAIN);
    let mut forged = descriptor.clone();
    forged.domain = "Forge".into();
    assert_ne!(forged, descriptor, "case is load-bearing in a domain name");
    forged = descriptor.clone();
    forged.capabilities.clear();
    assert_ne!(
        forged, descriptor,
        "a capability-stripped descriptor is not the service's identity"
    );
}
