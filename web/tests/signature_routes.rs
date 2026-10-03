//! The fence for the Signature HTTP attachment.
//!
//! `docs/rust-parity-ledger.md` reported the `signature` capability as "built, 0 routes": the service, the DAO and
//! the BoldSign adapter all existed while nothing could call them, which is why the production webhook still ran
//! through TypeScript. These assertions make that gap hard to reopen — the routes must be declared, the handlers
//! must exist, the provider must be wired, and the parity map must agree with the router.
//!
//! The checks are structural, not textual-on-formatting: paths and JSON, never indentation. An earlier version of
//! this file matched exact `.route(...)` lines and broke the moment `cargo fmt` reflowed them, which is how a test
//! gets deleted instead of fixed.
//!
//! Building the real router needs a `Database`, so this is a declaration fence rather than an end-to-end test. It
//! catches the failure that actually happens: a route quietly disappearing, or a handler losing its provider.
use std::fs;

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("the repository root must resolve")
}

fn routes_source() -> String {
    fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/api/routes.rs"))
        .expect("routes.rs must be readable")
}

/// Every `.rs` file of the `api` module — the router and the handlers it mounts — as one string.
///
/// This fence used to read `src/api/routes.rs` alone, which made it a test about a FILE rather than about the
/// HTTP surface. When the signature handlers moved out of `routes.rs` into `routes/webhooks_support.rs` (a
/// move-only split under the 800-line rule), the routes stayed mounted and the handlers stayed defined — and
/// the fence failed anyway, because the text it searched had travelled. What is worth asserting is that the
/// handlers exist in the module the router lives in, which is what this reads.
fn api_sources() -> String {
    fn walk(dir: &std::path::Path, out: &mut String) {
        let entries = fs::read_dir(dir).expect("the api module must be readable");
        for entry in entries {
            let path = entry.expect("a readable directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                out.push_str(
                    &fs::read_to_string(&path).expect("an api source file must be readable"),
                );
                out.push('\n');
            }
        }
    }
    let mut out = String::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api"),
        &mut out,
    );
    out
}

fn composition_source() -> String {
    fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/composition.rs"))
        .expect("composition.rs must be readable")
}

fn parity_map() -> serde_json::Value {
    let path = repo_root().join("scripts/rust-parity-map.json");
    let raw = fs::read_to_string(&path)
        .expect("the parity map must exist — it is the ledger's source of truth");
    serde_json::from_str(&raw).expect("the parity map must be valid JSON")
}

fn claimed_routes(map: &serde_json::Value) -> Vec<String> {
    map["capabilities"]
        .as_array()
        .expect("capabilities must be an array")
        .iter()
        .flat_map(|capability| {
            capability["routes"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|route| route.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn signature_routes_are_declared_and_mounted() {
    let source = routes_source();
    for path in [
        "/v1/signature/requests",
        "/v1/signature/requests/{id}",
        "/v1/signature/requests/{id}/refresh",
    ] {
        assert!(
            source.contains(&format!("\"{path}\"")),
            "routes.rs no longer declares {path}"
        );
    }
    for handler in ["signature_send", "signature_request", "signature_refresh"] {
        assert!(
            source.contains(&format!("post({handler})"))
                || source.contains(&format!("get({handler})")),
            "{handler} is not mounted on any signature route"
        );
    }
}

#[test]
fn every_signature_handler_exists_and_uses_the_provider() {
    let source = api_sources();
    for handler in ["signature_send", "signature_request", "signature_refresh"] {
        assert!(
            source.contains(&format!("async fn {handler}(")),
            "handler {handler} is mounted by the router but not defined"
        );
    }
    assert!(source.contains("signature_service(&state)"));
    assert!(
        composition_source().contains("BoldSignSignatureProvider::new"),
        "the long-lived service catalog no longer constructs the BoldSign adapter"
    );
}

#[test]
fn the_parity_map_claims_the_signature_routes() {
    let claimed = claimed_routes(&parity_map());
    for route in [
        "/v1/signature/requests",
        "/v1/signature/requests/{id}",
        "/v1/signature/requests/{id}/refresh",
    ] {
        assert!(
            claimed.iter().any(|entry| entry == route),
            "the parity map does not claim {route}; the ledger would report drift"
        );
    }
}

#[test]
fn the_webhook_is_not_claimed_until_it_is_attached() {
    let attached = routes_source().contains("\"/api/integrations/boldsign/webhook\"");
    let claimed = claimed_routes(&parity_map())
        .iter()
        .any(|route| route == "/api/integrations/boldsign/webhook");
    assert_eq!(
        attached, claimed,
        "the webhook route and the parity map disagree (attached={attached}, claimed={claimed}). The map is a claim \
         about reality, so it and the router have to move together."
    );
}
