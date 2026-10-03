//! The fence for the release gate's two addresses.
//!
//! `GET /api/build-info` and `GET /api/rust-ready` were Vercel functions of the retired TypeScript site, and they went
//! with the port while the production smoke kept asking for them — checks 1 and 5 of six could not pass, and the
//! release path could not tell "the deploy is broken" from "the URL never existed here". This file makes that
//! particular silence hard to reopen, in two directions:
//!
//! 1. the smoke's URLs must be mounted by the router, and the readiness answer must be the SAME handler as `/readyz`,
//!    so the container's healthcheck and the release gate can never disagree about readiness;
//! 2. the build stamp the deploy writes and the one the server reads must be the same variable name, because that
//!    coupling is invisible in both languages and produces a live "no sha" that looks like a deploy failure.
//!
//! Structural, not textual-on-formatting (see `signature_routes.rs` for why): paths and identifiers, never indentation.

use std::fs;

/// Source text for a path spelled from the REPOSITORY ROOT, never from this file's crate: the paths below name
/// the web tier, the CLI and the deploy script from one origin, so a future move of this suite cannot silently
/// point them at nothing.
fn repo_source(relative: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the suite lives in tests/, one level below the repository root");
    fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"))
}

/// The production smoke, which is the only caller of these two addresses — the contract is between these two files.
fn smoke_source() -> String {
    repo_source("cli/src/smoke.rs")
}

fn deploy_script() -> String {
    repo_source("scripts/deploy-prod.sh")
}

/// The variable that carries the deployed commit from the deploy to the running container. Named once here so a rename
/// on either side fails this test rather than a release.
const STAMP_VARIABLE: &str = "CULEBRALUXE_BUILD_SHA";

#[test]
fn the_smoke_urls_are_exactly_what_the_smoke_asks_for() {
    // If the smoke ever changes which path it calls, the mismatch has to be handled on purpose, not discovered in
    // production by a failed release.
    let smoke = smoke_source();
    for path in ["/api/build-info", "/api/rust-ready"] {
        assert!(
            smoke.contains(path),
            "the smoke no longer asks for {path}; this fence must move with it"
        );
    }
}

#[test]
fn the_router_mounts_both_addresses_the_smoke_asks_for() {
    let routes = repo_source("web/src/api/routes.rs");
    for path in ["/api/build-info", "/api/rust-ready"] {
        assert!(
            routes.contains(&format!("\"{path}\"")),
            "src/api/routes.rs no longer declares {path}, so the release gate's check cannot pass"
        );
    }
    assert!(
        routes.contains("build_info::build_info"),
        "the build-info route is declared but nothing is mounted on it"
    );
}

#[test]
fn readiness_has_one_answer_not_two() {
    // `/api/rust-ready` must mount the same handler as `/readyz`: a second readiness implementation could drift from
    // the one the container's HEALTHCHECK trusts, and then two sources would answer one fact.
    let routes = repo_source("web/src/api/routes.rs");
    let ready_routes = routes
        .lines()
        .filter(|line| line.contains("\"/readyz\"") || line.contains("\"/api/rust-ready\""))
        .count();
    assert_eq!(
        ready_routes, 2,
        "expected /readyz and /api/rust-ready to be declared"
    );
    let mounted = routes.matches("get(ready)").count();
    assert_eq!(
        mounted, 2,
        "both readiness routes must mount the same `ready` handler; found {mounted}"
    );
}

#[test]
fn the_build_stamp_variable_is_the_one_the_server_reads() {
    let handler = repo_source("web/src/api/build_info.rs");
    assert!(
        handler.contains(STAMP_VARIABLE),
        "build_info.rs no longer reads {STAMP_VARIABLE}"
    );
    assert!(
        handler.contains("VERCEL_GIT_COMMIT_SHA"),
        "the fallback a Vercel build stamp arrives in is gone"
    );

    let deploy = deploy_script();
    assert!(
        deploy.contains(STAMP_VARIABLE),
        "scripts/deploy-prod.sh no longer writes {STAMP_VARIABLE}: the live build would answer with no sha"
    );
    assert!(
        deploy.contains("CULEBRALUXE_BUILT_AT"),
        "the deploy no longer stamps the build time it serves"
    );
}

#[test]
fn an_unstamped_build_refuses_instead_of_inventing_a_sha() {
    // The honest failure, asserted at the source level because it is the property that matters: an empty `sha` with a
    // note naming what to set. A build that answered "unknown" would pass a gate that compares nothing.
    let handler = repo_source("web/src/api/build_info.rs");
    assert!(
        handler.contains("no build stamp: set"),
        "the unstamped case must name the variable to set rather than serve a placeholder"
    );
}
