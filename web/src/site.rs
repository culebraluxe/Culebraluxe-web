//! THE WEBSITE — served by this Rust server. The Yew application is the whole UI; this module hands it to the browser.
//!
//! Every URL the API does not claim lands here (the router's fallback): a file under the site directory (`public/`:
//! the wasm, the JS glue, the stylesheet, images) is served as itself; any other path gets the application shell, and
//! the Yew router inside it decides which screen that URL is. There is no other web server and no TypeScript.
//!
//! THE SHELL is one small HTML document: fonts, the stylesheet, the mount point, and a module script that loads the
//! wasm and starts it on that mount point. For a portal path it also carries the signed-in user's projection
//! (`#rust-actor`), which the portal chrome reads for what to offer (see `crate::api::ui_auth`).

use std::path::PathBuf;

use axum::http::{header, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use tower_http::services::ServeDir;

/// Where the built site lives: `CULEBRA_SITE_DIR`, else `public/` from the repository root or from `rust/`.
pub fn site_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CULEBRA_SITE_DIR") {
        return PathBuf::from(dir);
    }
    ["public", "../public"]
        .into_iter()
        .map(PathBuf::from)
        .find(|dir| dir.join("rust-ui").is_dir())
        .unwrap_or_else(|| PathBuf::from("public"))
}

/// The router's fallback: static files first, then the shell.
pub fn service(state: crate::api::ApiState) -> ServeDir<axum::routing::MethodRouter> {
    ServeDir::new(site_dir())
        .append_index_html_on_directories(false)
        .fallback(axum::routing::get(shell_now).with_state(state))
}

async fn shell_now(
    axum::extract::State(state): axum::extract::State<crate::api::ApiState>,
    uri: Uri,
    headers: axum::http::HeaderMap,
) -> Response {
    let path = uri.path();
    if let Some(answer) = early_answer(path) {
        return answer;
    }
    let portal = path.starts_with("/portal/");
    // A portal page for someone not signed in goes to the sign-in page, and comes back here after.
    let actor = if portal {
        match crate::api::ui_auth::resolve_portal_context(&state, &headers).await {
            Ok(resolved) => crate::api::ui_auth::actor_projection(&resolved),
            Err(_) => {
                let back = uri
                    .path_and_query()
                    .map(|p| p.as_str())
                    .unwrap_or("/portal/dashboard");
                return axum::response::Redirect::to(&format!(
                    "/login?callbackUrl={}",
                    crate::api::google_auth::encode(back)
                ))
                .into_response();
            }
        }
    } else {
        String::new()
    };
    shell(path, &actor)
}

/// The answers that need no page: an API address nobody answers is a 404 (never a web page a caller would try to parse
/// as JSON), and the portal, which has no page of its own, opens on the dashboard.
fn early_answer(path: &str) -> Option<Response> {
    if path.starts_with("/v1/") || path.starts_with("/api/") {
        return Some(StatusCode::NOT_FOUND.into_response());
    }
    if path == "/portal" || path == "/portal/" {
        return Some(axum::response::Redirect::temporary("/portal/dashboard").into_response());
    }
    None
}

/// The application shell for `path`.
pub fn shell(path: &str, actor: &str) -> Response {
    let portal = path == "/portal" || path.starts_with("/portal/");
    let actor_script = if actor.is_empty() {
        String::new()
    } else {
        // `</` cannot appear inside a script element's text; JSON never needs it escaped otherwise.
        format!(
            r#"<script id="rust-actor" type="application/json">{}</script>"#,
            actor.replace("</", "<\\/")
        )
    };
    let app = if portal { "portal" } else { "site" };
    let html = format!(
        r##"<!doctype html>
<html lang="en" class="light bg-background">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="theme-color" content="#f5f2ec">
<title>CulebraLuxe — Culebra Caribbean Estates</title>
<meta name="description" content="CulebraLuxe presents an editorial collection of architectural estates and beachfront residences on the island of Culebra, Puerto Rico.">
<link rel="icon" href="/icon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-icon.png">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Cormorant+Garamond:wght@300;400;500;600&family=Instrument+Sans:wght@400;500;600&display=swap">
<style>:root{{--font-cormorant:'Cormorant Garamond',serif;--font-instrument:'Instrument Sans',sans-serif}}</style>
<link rel="stylesheet" href="/app.css">
</head>
<body class="font-sans antialiased">
{actor_script}<div id="rust-ui" data-rust-app="{app}"></div>
<script type="module">
import init, {{ start_in }} from '/rust-ui/ui.js';
await init({{ module_or_path: '/rust-ui/ui_bg.wasm' }});
start_in(document.getElementById('rust-ui'));
</script>
</body>
</html>
"##
    );
    (
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-cache")],
        Html(html),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn every_page_is_the_same_shell_and_the_portal_is_marked() {
        let body = |response: Response| async {
            String::from_utf8(
                axum::body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap()
        };
        let site = body(shell("/buyers", "")).await;
        assert!(site.contains(r#"data-rust-app="site""#));
        assert!(site.contains("import init, { start_in } from '/rust-ui/ui.js'"));
        let portal = body(shell("/portal/clients/abc", "")).await;
        assert!(portal.contains(r#"data-rust-app="portal""#));
    }

    #[tokio::test]
    async fn an_unknown_api_address_is_a_404_not_the_page() {
        for path in ["/v1/nothing", "/api/portal/rust-ui/nothing"] {
            assert_eq!(
                early_answer(path).unwrap().status(),
                StatusCode::NOT_FOUND,
                "{path}"
            );
        }
        assert!(early_answer("/buyers").is_none(), "a page is the shell");
        let portal = early_answer("/portal").unwrap();
        assert_eq!(portal.headers()[header::LOCATION], "/portal/dashboard");
    }
}
