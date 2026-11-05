//! THE WEBSITE — served by this Rust server. The Yew application is the whole UI; this module hands it to the browser.
//!
//! Every URL the API does not claim lands here (the router's fallback): a file under the site directory (`public/`:
//! the wasm, the JS glue, the stylesheet, images) is served as itself; any other path gets the application shell, and
//! the Yew router inside it decides which screen that URL is. There is no other web server and no TypeScript.
//!
//! THE SHELL is one small HTML document: fonts, the stylesheet, the mount point, and a module script that loads the
//! wasm and starts it on that mount point. For a portal path it also carries the signed-in user's projection
//! (`#rust-actor`), which the portal chrome reads for what to offer (see `crate::api::ui_auth`).
//!
//! THE ASSET NAMES ARE CONTENT-ADDRESSED. `scripts/asset-hash.sh` writes `app.<hash>.css`, `rust-ui/ui.<hash>.js`
//! and `rust-ui/ui_bg.<hash>.wasm` and a manifest beside them; the shell names those files, never the stable names,
//! so a page can never be served a different build's stylesheet or wasm under one URL. The stable names are still
//! served (a page from the machine a rolling deploy just replaced asks for an address this machine may not have —
//! `stale_asset` falls back to the canonical file), and a build with no manifest renders the canonical names exactly
//! as it always did.

use std::collections::HashMap;
use std::path::PathBuf;

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Request, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};
use tower_http::services::ServeDir;

/// Where the built site lives: `CULEBRA_SITE_DIR`, else `public/` — looked for in the working directory and then one
/// level above it, because the server is started either from the repository root or from the crate it lives in.
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

/// What `scripts/asset-hash.sh` recorded for this build: canonical name -> content-addressed name. Missing in a
/// developer checkout, which is why every lookup falls back to the canonical name rather than failing.
fn asset_manifest() -> HashMap<String, String> {
    std::fs::read_to_string(site_dir().join("asset-manifest.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// The address to name for `canonical`, preferring the content-addressed file when the build recorded one.
fn asset_in(manifest: &HashMap<String, String>, canonical: &str) -> String {
    manifest
        .get(canonical)
        .cloned()
        .unwrap_or_else(|| canonical.to_owned())
}

fn asset(canonical: &str) -> String {
    asset_in(&asset_manifest(), canonical)
}

/// The canonical file behind a content-addressed address (`/app.0123456789ab.css`), when the address is one of the
/// three the shell names. A page handed out by the machine a rolling deploy has just replaced asks for a hash this
/// machine may not carry; serving the canonical file keeps that address working for the length of the deploy.
fn stale_asset(path: &str) -> Option<&'static str> {
    const RULES: [(&str, &str, &str); 3] = [
        ("/app.", ".css", "app.css"),
        ("/rust-ui/ui.", ".js", "rust-ui/ui.js"),
        ("/rust-ui/ui_bg.", ".wasm", "rust-ui/ui_bg.wasm"),
    ];
    for (prefix, suffix, canonical) in RULES {
        let Some(hash) = path
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix))
        else {
            continue;
        };
        if hash.len() >= 8 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Some(canonical);
        }
    }
    None
}

fn asset_content_type(canonical: &str) -> &'static str {
    match canonical {
        "app.css" => "text/css; charset=utf-8",
        "rust-ui/ui.js" => "text/javascript; charset=utf-8",
        _ => "application/wasm",
    }
}

/// The router's fallback: static files first, then a replaced build's asset address, then the shell.
pub fn service(state: crate::api::ApiState) -> ServeDir<axum::routing::MethodRouter> {
    ServeDir::new(site_dir())
        .append_index_html_on_directories(false)
        .fallback(axum::routing::get(asset_or_shell).with_state(state))
}

async fn asset_or_shell(
    axum::extract::State(state): axum::extract::State<crate::api::ApiState>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    if let Some(canonical) = stale_asset(uri.path()) {
        if let Ok(bytes) = std::fs::read(site_dir().join(canonical)) {
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, asset_content_type(canonical)),
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                Body::from(bytes),
            )
                .into_response();
        }
    }
    shell_now(axum::extract::State(state), uri, headers).await
}

async fn shell_now(
    axum::extract::State(state): axum::extract::State<crate::api::ApiState>,
    uri: Uri,
    headers: HeaderMap,
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
    let head = page_head(&state, path, &headers, &uri).await;
    shell_with_head(path, &actor, &head)
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

/// The application shell for `path`, with the site's default share metadata.
pub fn shell(path: &str, actor: &str) -> Response {
    let head = default_head("", path);
    shell_with_head(path, actor, &head)
}

/// The shell with `head` (title, description and the `og:`/`twitter:` card) supplied by the caller.
fn shell_with_head(path: &str, actor: &str, head: &str) -> Response {
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
    // The stylesheet, the glue and the wasm are named for their bytes (scripts/asset-hash.sh), so a cached page can
    // never be run against a different deploy's stylesheet or code — the "button jumped" a signer saw when the two
    // came from different builds. A developer checkout has no manifest and therefore gets the stable names.
    let css = asset("app.css");
    let js = asset("rust-ui/ui.js");
    let wasm = asset("rust-ui/ui_bg.wasm");
    let html = format!(
        r##"<!doctype html>
<html lang="en" class="light bg-background">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="theme-color" content="#f5f2ec">
{head}
<link rel="icon" href="/icon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-icon.png">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Cormorant+Garamond:wght@300;400;500;600&family=Instrument+Sans:wght@400;500;600&display=swap">
<style>:root{{--font-cormorant:'Cormorant Garamond',serif;--font-instrument:'Instrument Sans',sans-serif}}</style>
<link rel="stylesheet" href="/{css}">
</head>
<body class="font-sans antialiased">
{actor_script}<div id="rust-ui" data-rust-app="{app}"></div>
<script type="module">
import init, {{ start_in }} from '/{js}';
await init({{ module_or_path: '/{wasm}' }});
start_in(document.getElementById('rust-ui'));
</script>
</body>
</html>
"##
    );
    (StatusCode::OK, Html(html)).into_response()
}

/// Cache-Control for a response whose handler did not pick one (the media routes pick their own).
///
/// Cloudflare caches by URL, so the rule follows from whether the address is content-addressed: a hashed asset never
/// changes and is immutable for a year; the stable aliases are short-lived; images are a day; the anonymous shell is
/// cached for five minutes at the edge and revalidated in the background; everything personal is never stored.
pub fn cache_control_for(path: &str, is_html: bool) -> &'static str {
    if path == "/login"
        || path.starts_with("/portal")
        || path.starts_with("/api/")
        || path.starts_with("/v1/")
    {
        return "private, no-store";
    }
    if stale_asset(path).is_some() {
        return "public, max-age=31536000, immutable";
    }
    if path.starts_with("/images/") {
        return "public, max-age=86400";
    }
    if path == "/app.css" || path.starts_with("/rust-ui/") {
        return "public, max-age=300";
    }
    if is_html {
        return "public, s-maxage=300, stale-while-revalidate=86400";
    }
    "public, max-age=86400"
}

/// Applied to the whole application router: it only fills in Cache-Control where a handler left it unset.
pub async fn cache_policy(request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    let mut response = next.run(request).await;
    if response.headers().contains_key(header::CACHE_CONTROL) {
        return response;
    }
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.starts_with("text/html"))
        .unwrap_or(false);
    if let Ok(value) = HeaderValue::from_str(cache_control_for(&path, is_html)) {
        response.headers_mut().insert(header::CACHE_CONTROL, value);
    }
    response
}

/// The share metadata a page carries: `<title>`, the description, and the Open Graph / Twitter card.
///
/// WHY THE SERVER AND NOT THE BROWSER: a link preview is fetched by the receiving app (Messages, WhatsApp, Mail) as
/// plain HTML — it never runs the Yew application. Without these tags a shared property link has nothing to show, and
/// the apps fall back to whatever they cached from a previous host (the Vercel-era card a person saw in November).
/// The property page resolves its own record here so the card carries that property's name and hero photograph.
fn head_tags(title: &str, description: &str, url: &str, image: &str) -> String {
    let (title, description, url, image) = (
        escape_attr(title),
        escape_attr(description),
        escape_attr(url),
        escape_attr(image),
    );
    format!(
        r#"<title>{title}</title>
<meta name="description" content="{description}">
<meta property="og:type" content="website">
<meta property="og:site_name" content="CulebraLuxe">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{description}">
<meta property="og:url" content="{url}">
<meta property="og:image" content="{image}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{title}">
<meta name="twitter:description" content="{description}">
<meta name="twitter:image" content="{image}">"#
    )
}

/// The site's own card, for every page that is not a property.
fn default_head(base: &str, path: &str) -> String {
    head_tags(
        "CulebraLuxe — Culebra Caribbean Estates",
        "CulebraLuxe presents an editorial collection of architectural estates and beachfront residences on the island of Culebra, Puerto Rico.",
        &format!("{base}{path}"),
        &format!("{base}/images/culebraluxe-email-logo.png"),
    )
}

/// The card for one page: a property's own record when the address names one, the site's card otherwise.
async fn page_head(
    state: &crate::api::ApiState,
    path: &str,
    headers: &HeaderMap,
    uri: &Uri,
) -> String {
    let base = base_url(headers);
    let Some(key) = path.strip_prefix("/properties/") else {
        return default_head(&base, path);
    };
    let key = key.trim_end_matches('/');
    if key.is_empty() {
        return default_head(&base, path);
    }
    let context = crate::api::public_guest_context(headers);
    let Ok(Some(record)) = state
        .services()
        .public_listings()
        .property(key, &context)
        .await
    else {
        return default_head(&base, path);
    };
    let image = match record.hero_media_id.as_deref() {
        Some(id) if !id.is_empty() => match state
            .services()
            .public_listings()
            .media_public_url(id, "card")
            .await
        {
            Ok(Some(url)) => url,
            _ => format!("{base}/images/culebraluxe-email-logo.png"),
        },
        _ => format!("{base}/images/culebraluxe-email-logo.png"),
    };
    // The card's description: the property's own copy when it has any, else a sentence built from the facts a
    // person reading the preview can use — the kind, the place. "Culebra" alone is not a description.
    let description = record
        .short_description
        .as_deref()
        .map(str::trim)
        .filter(|copy| !copy.is_empty())
        .map(|copy| truncate_words(copy, 200))
        .unwrap_or_else(|| {
            let place = [record.city.as_deref(), record.state_or_province.as_deref()]
                .into_iter()
                .flatten()
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            let kind = record
                .property_type
                .as_deref()
                .map(str::trim)
                .filter(|kind| !kind.is_empty());
            match (kind, place.is_empty()) {
                (Some(kind), false) => format!("{kind} in {place}, represented by CulebraLuxe."),
                (None, false) => format!("A property in {place}, represented by CulebraLuxe."),
                _ => "A property represented by CulebraLuxe on the island of Culebra, Puerto Rico."
                    .to_owned(),
            }
        });
    head_tags(
        &format!("{} — CulebraLuxe", record.name),
        &description,
        &format!("{base}{}", uri.path()),
        &image,
    )
}

/// The absolute origin the page was requested on: `https://` plus the `Host` header, or the configured site.
fn base_url(headers: &HeaderMap) -> String {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|host| !host.is_empty() && !host.contains('/'))
        .unwrap_or("www.culebraluxe.com");
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("https");
    format!("{scheme}://{host}")
}

/// `value`, cut to at most `max` characters on a word boundary — a description tag, not a paragraph.
fn truncate_words(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    let mut out = String::new();
    for word in value.split_whitespace() {
        if out.chars().count() + word.chars().count() + 1 > max {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// Attribute-safe text: the model's own copy reaches a `content="..."`, so `&`, `<`, `>`, `"` and `'` are escaped.
fn escape_attr(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
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
        assert!(
            site.contains(&format!("href=\"/{}\"", asset("app.css"))),
            "the stylesheet is named by its bytes"
        );
        assert!(
            site.contains(&format!("from '/{}'", asset("rust-ui/ui.js"))),
            "the glue is named by its bytes"
        );
        assert!(site.contains(&format!(
            "module_or_path: '/{}'",
            asset("rust-ui/ui_bg.wasm")
        )));
        let portal = body(shell("/portal/clients/abc", "")).await;
        assert!(portal.contains(r#"data-rust-app="portal""#));
    }

    #[test]
    fn a_manifest_names_the_content_addressed_file() {
        let mut manifest = HashMap::new();
        manifest.insert("app.css".to_owned(), "app.0123456789ab.css".to_owned());
        assert_eq!(asset_in(&manifest, "app.css"), "app.0123456789ab.css");
        assert_eq!(
            asset_in(&manifest, "rust-ui/ui.js"),
            "rust-ui/ui.js",
            "an unlisted asset keeps its stable name"
        );
    }

    #[test]
    fn a_replaced_builds_asset_address_falls_back_to_the_canonical_file() {
        assert_eq!(stale_asset("/app.0123456789ab.css"), Some("app.css"));
        assert_eq!(
            stale_asset("/rust-ui/ui.0123456789ab.js"),
            Some("rust-ui/ui.js")
        );
        assert_eq!(
            stale_asset("/rust-ui/ui_bg.0123456789ab.wasm"),
            Some("rust-ui/ui_bg.wasm")
        );
        for stable in ["/app.css", "/rust-ui/ui.js", "/rust-ui/ui_bg.wasm"] {
            assert_eq!(
                stale_asset(stable),
                None,
                "{stable} is not content-addressed"
            );
        }
        for foreign in [
            "/images/app.01234567.css",
            "/portal/app.01234567.css",
            "/app.nothex.css",
        ] {
            assert_eq!(
                stale_asset(foreign),
                None,
                "{foreign} is not one of the three"
            );
        }
    }

    #[test]
    fn cache_control_follows_the_kind_of_response() {
        assert_eq!(
            cache_control_for("/app.0123456789ab.css", false),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(
            cache_control_for("/images/hero.jpg", false),
            "public, max-age=86400"
        );
        assert_eq!(cache_control_for("/app.css", false), "public, max-age=300");
        assert_eq!(
            cache_control_for("/buyers", true),
            "public, s-maxage=300, stale-while-revalidate=86400"
        );
        for personal in ["/login", "/portal/dashboard", "/api/media/x", "/v1/me"] {
            assert_eq!(
                cache_control_for(personal, false),
                "private, no-store",
                "{personal}"
            );
        }
    }

    #[test]
    fn the_default_card_names_the_site_and_an_image() {
        let head = default_head("https://www.culebraluxe.com", "/buyers");
        assert!(head.contains(
            r#"<meta property="og:title" content="CulebraLuxe — Culebra Caribbean Estates">"#
        ));
        assert!(head
            .contains(r#"<meta property="og:url" content="https://www.culebraluxe.com/buyers">"#));
        assert!(head.contains(r#"<meta property="og:image" content="https://www.culebraluxe.com/images/culebraluxe-email-logo.png">"#));
        assert!(head.contains(r#"<meta name="twitter:card" content="summary_large_image">"#));
    }

    #[test]
    fn a_property_card_carries_its_own_title_and_hero() {
        let head = head_tags(
            "Sea to Soul — CulebraLuxe",
            "A beachfront parcel at Playa Sardinas II.",
            "https://www.culebraluxe.com/properties/80b65fa3",
            "https://br-snowy-fog-axg3jae2.storage.c-4.us-east-2.aws.neon.tech/culebraluxe-property-media-public/media/80b65fa3/web.jpg",
        );
        assert!(head.contains("Sea to Soul — CulebraLuxe"));
        assert!(head.contains("culebraluxe-property-media-public"));
        assert!(head.contains("https://www.culebraluxe.com/properties/80b65fa3"));
    }

    #[test]
    fn model_copy_cannot_break_out_of_a_content_attribute() {
        let head = head_tags(
            r#"A "quoted" & <tagged> name's"#,
            "text",
            "https://example.test/x",
            "https://example.test/i.png",
        );
        assert!(head.contains("&quot;quoted&quot;"));
        assert!(head.contains("&amp;"));
        assert!(head.contains("&lt;tagged&gt;"));
        assert!(head.contains("&#39;"));
        assert!(!head.contains(r#"content="A "quoted""#));
    }

    #[test]
    fn a_long_description_is_cut_on_a_word_boundary() {
        let text = "word ".repeat(100);
        let cut = truncate_words(&text, 40);
        assert!(cut.chars().count() <= 40);
        assert!(cut.ends_with("word"));
        assert_eq!(truncate_words("short", 40), "short");
    }

    #[test]
    fn the_origin_comes_from_the_request_host() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "culebraluxe-prod.fly.dev".parse().unwrap());
        assert_eq!(base_url(&headers), "https://culebraluxe-prod.fly.dev");
        let empty = HeaderMap::new();
        assert_eq!(base_url(&empty), "https://www.culebraluxe.com");
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
