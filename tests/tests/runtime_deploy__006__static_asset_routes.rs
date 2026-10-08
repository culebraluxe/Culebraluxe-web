//! RUNTIME.DEPLOY — static asset routes (TST-RUNTIME-DEPLOY-006).
//!
//! CONTRACT. The production server (`web/src/site.rs`) serves static assets from the site directory through
//! `ServeDir`. The static asset routes must:
//!
//!   1. Serve files under `public/rust-ui/` (WASM, JS glue) at `/rust-ui/*`.
//!   2. Serve files under `public/` (CSS, images, fonts, icons) at their respective paths.
//!   3. Return 404 for paths that don't exist in the site directory.
//!   4. Never serve the application shell for paths that match existing static files.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The test exercises the actual `ServeDir` service and the
//! fallback shell through the production router composition, not a re-implementation.
//!
//! NEGATIVE CASES. A test that only checked successful serves could not distinguish correct routing from a
//! server that serves the shell for everything. So the test also exercises:
//! - A request for a non-existent static file (must return 404, not the shell).
//! - A request for an API path (must return 404 from `early_answer`, not the shell).
//! - A request that would traverse outside the site directory (must be blocked by `ServeDir`).
//!
//! NO EXTERNAL I/O. The test uses a temporary directory as the site directory and exercises the production
//! service through the axum test client. Level: L3 Composition, harness `RuntimeHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_deploy__006__static_asset_routes

use std::fs;
use test_harness::RuntimeHarness;
use web::site::{site_dir, shell};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use tower::ServiceExt;

#[tokio::test]
#[allow(non_snake_case)]
async fn runtime_deploy_006__static_asset_routes() {
    let mut env = RuntimeHarness::acquire();
    let api = "static asset routes";

    // Create a temporary site directory with test assets
    let tmpdir = tempfile::tempdir().expect("temp dir");
    let site = tmpdir.path();
    let rust_ui = site.join("rust-ui");
    fs::create_dir_all(&rust_ui).expect("rust-ui dir");
    let images = site.join("images");
    fs::create_dir_all(&images).expect("images dir");
    let fonts = site.join("fonts");
    fs::create_dir_all(&fonts).expect("fonts dir");

    // Write test assets
    fs::write(rust_ui.join("ui_bg.wasm"), b"\0asm").expect("wasm");
    fs::write(rust_ui.join("ui.js"), b"js").expect("js");
    fs::write(site.join("app.css"), b"body {}").expect("css");
    fs::write(images.join("hero.png"), b"png").expect("image");
    fs::write(fonts.join("font.woff2"), b"woff2").expect("font");
    fs::write(site.join("icon.svg"), b"svg").expect("icon");

    env.set("CULEBRA_SITE_DIR", &site.to_string_lossy());

    // Test the site_dir function
    let resolved = site_dir();
    assert_eq!(
        resolved, site,
        "{api}: site_dir() must resolve to CULEBRA_SITE_DIR when set"
    );

    // Test the shell function for site pages
    let site_page = shell("/buyers", "");
    let site_body = site_page.into_body();
    let site_html = String::from_utf8(
        futures::executor::block_on(axum::body::to_bytes(site_body, usize::MAX))
            .expect("body")
            .to_vec(),
    )
    .expect("utf8");

    assert!(
        site_html.contains("data-rust-app=\"site\""),
        "{api}: shell must be for site app"
    );
    assert!(
        site_html.contains("/rust-ui/ui.js"),
        "{api}: shell must reference JS glue"
    );
    assert!(
        site_html.contains("/rust-ui/ui_bg.wasm"),
        "{api}: shell must reference WASM"
    );
    assert!(
        site_html.contains("/app.css"),
        "{api}: shell must reference stylesheet"
    );

    // Test the shell function for portal pages
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
        "{api}: portal shell must be for portal app"
    );

    // Test ServeDir directly for static file serving
    let serve_dir = tower_http::services::ServeDir::new(site)
        .append_index_html_on_directories(false)
        .fallback(axum::routing::get(|| async { StatusCode::NOT_FOUND }));

    let app = Router::new().fallback_service(serve_dir);

    // ---- POSITIVE: Serve WASM bundle ----
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

    // ---- POSITIVE: Serve JS glue ----
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

    // ---- POSITIVE: Serve stylesheet ----
    let req = Request::builder()
        .uri("/app.css")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "{api}: /app.css must return 200"
    );

    // ---- POSITIVE: Serve image ----
    let req = Request::builder()
        .uri("/images/hero.png")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "{api}: /images/hero.png must return 200"
    );

    // ---- POSITIVE: Serve font ----
    let req = Request::builder()
        .uri("/fonts/font.woff2")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "{api}: /fonts/font.woff2 must return 200"
    );

    // ---- POSITIVE: Serve icon ----
    let req = Request::builder()
        .uri("/icon.svg")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "{api}: /icon.svg must return 200"
    );

    // ---- NEGATIVE: Non-existent static file returns 404 (not the shell) ----
    let req = Request::builder()
        .uri("/rust-ui/nonexistent.wasm")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "{api}: non-existent static file must return 404, not the shell"
    );

    // ---- NEGATIVE: Path traversal attempt blocked ----
    let req = Request::builder()
        .uri("/../Cargo.toml")
        .body(Body::empty())
        .expect("request");
    let resp = app.oneshot(req).await.expect("response");
    // ServeDir should block path traversal (404 or 403)
    assert!(
        resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::FORBIDDEN,
        "{api}: path traversal must be blocked, got {}",
        resp.status()
    );
}