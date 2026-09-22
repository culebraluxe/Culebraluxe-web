//! The document: the HTML shell the Rust UI owns.
//!
//! WHY THIS EXISTS. Until now the Rust UI was a component tree painted into a container Next had already rendered: Next
//! owned the `<html>`, the `<head>`, the stylesheet link, the fonts and the assets, and Rust owned what went inside one
//! `<div>`. That is not a UI migration, it is an inclusion — `cargo run` produced nothing a browser could open, and the
//! stylesheet the Rust markup depends on only existed because a Tailwind pass inside the Next build happened to scan
//! this crate's source.
//!
//! This module closes that: the same pure `view::render` that the WASM shell paints with is wrapped in a full document
//! here, server-side, with the stylesheet and fonts the Rust UI owns. One view, two hosts — and the document no longer
//! depends on Next, on `.next/`, or on any generated JavaScript.

use crate::model::Model;
use crate::view::{escape, render};

/// Where the Rust UI's own stylesheet is served from. One URL, so the document and the server cannot drift.
pub const STYLESHEET_URL: &str = "/ui/app.css";

/// The Rust UI's assets, mounted at the origin root by the server.
pub const ASSETS_ROOT: &str = "/ui";

/// The site name and description the document carries. Kept here rather than read from a Next metadata export, because
/// the document is this crate's now.
const SITE_NAME: &str = "CulebraLuxe";

/// Render `model` as a complete HTML document.
///
/// `surface` decides the body class: the public site and the portal want different grounds, and a document that cannot
/// say which one it is cannot set the colour the first paint uses before any stylesheet arrives.
pub fn document(model: &Model) -> String {
    let title = format!("{} — {}", model.screen.title, SITE_NAME);
    format!(
        "<!doctype html>\
         <html lang=\"en\">\
         <head>\
           <meta charset=\"utf-8\" />\
           <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\
           <title>{title}</title>\
           <meta name=\"description\" content=\"{description}\" />\
           <link rel=\"stylesheet\" href=\"{stylesheet}\" />\
           <link rel=\"preload\" as=\"font\" type=\"font/woff2\" href=\"{serif}\" crossorigin />\
           <link rel=\"preload\" as=\"font\" type=\"font/woff2\" href=\"{sans}\" crossorigin />\
         </head>\
         <body class=\"font-sans antialiased\">\
           <div id=\"rust-ui\">{body}</div>\
         </body>\
         </html>",
        title = escape(&title),
        description = escape(
            "Architectural estates and beachfront residences on the island of Culebra, Puerto Rico."
        ),
        stylesheet = STYLESHEET_URL,
        // The two faces the design uses, self-hosted under the assets root: next/font is a Next mechanism and cannot be
        // the hidden dependency of a document this crate renders.
        serif = format!("{ASSETS_ROOT}/fonts/cormorant-garamond-latin.woff2"),
        sans = format!("{ASSETS_ROOT}/fonts/instrument-sans-latin.woff2"),
        body = render(model),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{screen, Model};

    #[test]
    fn the_document_carries_what_a_browser_needs_to_render_the_page() {
        let model = Model {
            screen: screen("site-home").expect("a screen the table defines"),
            ..Model::default()
        };
        let html = document(&model);
        for required in [
            "<!doctype html>",
            "<meta charset=\"utf-8\" />",
            "<title>",
            STYLESHEET_URL,
            "rel=\"preload\"",
            "id=\"rust-ui\"",
        ] {
            assert!(
                html.contains(required),
                "a document without {required} is not one a browser can render"
            );
        }
        assert!(
            html.contains("</html>") && html.contains("</body>"),
            "the document must close what it opens"
        );
    }

    #[test]
    fn the_document_body_is_the_same_view_the_wasm_shell_paints() {
        // One view, two hosts: a document that rendered different markup from the shell would make the server-rendered
        // page and the hydrated page disagree, which is a hydration mismatch with no React in sight.
        let model = Model {
            screen: screen("site-home").expect("a screen the table defines"),
            ..Model::default()
        };
        assert!(document(&model).contains(&render(&model)));
    }
}
