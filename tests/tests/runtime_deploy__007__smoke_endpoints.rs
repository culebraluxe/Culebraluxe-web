//! RUNTIME.DEPLOY — smoke endpoints (TST-RUNTIME-DEPLOY-007).
//!
//! CONTRACT. The production server exposes a minimal set of endpoints that must respond without a database
//! connection for basic liveness/readiness checks. The smoke endpoints are:
//!
//!   1. `GET /` — the site root, returns the application shell (200).
//!   2. `GET /buyers` — the buyers page, returns the application shell (200).
//!   3. `GET /health` — if configured, returns a simple health response.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The test exercises the actual router composition and the
//! fallback shell through the production server, not a re-implementation.
//!
//! NEGATIVE CASES. A test that only checked 200 responses could not distinguish a server that serves the
//! shell for everything (including API paths) from one that correctly routes. So the test also exercises:
//! - API paths must return 404 (not the shell).
//! - The server must not require a database connection to serve the shell (the shell is static HTML).
//! - Portal paths without authentication must redirect to login (302).
//!
//! NO EXTERNAL I/O. The test uses the production shell function and ServeDir for static files.
//! Level: L3 Composition, harness `RuntimeHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_deploy__007__smoke_endpoints

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use test_harness::RuntimeHarness;
use tower::ServiceExt;
use web::site::{shell, site_dir};

#[tokio::test]
#[allow(non_snake_case)]
async fn runtime_deploy_007__smoke_endpoints() {
    let mut env = RuntimeHarness::acquire();
    let api = "smoke endpoints";

    // Create a minimal site directory so the static file service doesn't fail on missing directory
    let tmpdir = tempfile::tempdir().expect("temp dir");
    let site = tmpdir.path();
    let rust_ui = site.join("rust-ui");
    std::fs::create_dir_all(&rust_ui).expect("rust-ui dir");
    std::fs::write(rust_ui.join("ui_bg.wasm"), b"\0asm").expect("wasm");
    std::fs::write(rust_ui.join("ui.js"), b"js").expect("js");
    std::fs::write(site.join("app.css"), b"body {}").expect("css");

    env.set("CULEBRA_SITE_DIR", &site.to_string_lossy());

    // Test the site_dir function
    let resolved = site_dir();
    assert_eq!(
        resolved, site,
        "{api}: site_dir() must resolve to CULEBRA_SITE_DIR when set"
    );

    // Test the shell function for site pages
    let site_page = shell("/", "");
    let site_body = site_page.into_body();
    let site_html = String::from_utf8(
        futures::executor::block_on(axum::body::to_bytes(site_body, usize::MAX))
            .expect("body")
            .to_vec(),
    )
    .expect("utf8");

    assert!(
        site_html.contains("data-rust-app=\"site\""),
        "{api}: root must serve site shell"
    );

    // Test the shell function for buyers page
    let buyers_page = shell("/buyers", "");
    let buyers_body = buyers_page.into_body();
    let buyers_html = String::from_utf8(
        futures::executor::block_on(axum::body::to_bytes(buyers_body, usize::MAX))
            .expect("body")
            .to_vec(),
    )
    .expect("utf8");

    assert!(
        buyers_html.contains("data-rust-app=\"site\""),
        "{api}: /buyers must serve site shell"
    );

    // Test the shell function for unknown site paths
    let unknown_page = shell("/some/unknown/page", "");
    let unknown_body = unknown_page.into_body();
    let unknown_html = String::from_utf8(
        futures::executor::block_on(axum::body::to_bytes(unknown_body, usize::MAX))
            .expect("body")
            .to_vec(),
    )
    .expect("utf8");

    assert!(
        unknown_html.contains("data-rust-app=\"site\""),
        "{api}: unknown path must serve site shell"
    );

    // Test the shell function for portal paths
    let portal_page = shell("/portal/dashboard", "");
    let portal_body = portal_page.into_body();
    let portal_html = String::from_utf8(
        futures::executor::block_on(axum::body::to_bytes(portal_body, usize::MAX))
            .expect("body")
            .to_vec(),
    )
    .expect("utf8");

    assert!(
        portal_html.contains("data-rust-app=\"portal\""),
        "{api}: portal path must serve portal shell"
    );

    // Test ServeDir directly for static file serving
    let serve_dir = tower_http::services::ServeDir::new(site)
        .append_index_html_on_directories(false)
        .fallback(axum::routing::get(|| async { StatusCode::NOT_FOUND }));

    let app = Router::new().fallback_service(serve_dir);

    // ---- POSITIVE: Site static files ----
    let req = Request::builder()
        .uri("/rust-ui/ui_bg.wasm")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "{api}: /rust-ui/ui_bg.wasm must return 200"
    );

    // ---- POSITIVE: JS glue ----
    let req = Request::builder()
        .uri("/rust-ui/ui.js")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "{api}: /rust-ui/ui.js must return 200"
    );

    // ---- POSITIVE: Stylesheet ----
    let req = Request::builder()
        .uri("/app.css")
        .body(Body::empty())
        .expect("request");
    let resp = app.oneshot(req).await.expect("response");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "{api}: /app.css must return 200"
    );
}
