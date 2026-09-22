//! Icons, as inline SVG.
//!
//! WHY INLINE AND NOT A FONT OR A SPRITE. The pages carry lucide icons — a wave, a palm, a leaf on About; a compass, a
//! target, a camera on Sellers. A Rust view cannot call a React component, so the geometry has to live somewhere, and the
//! choice was: inline the paths here, put a sprite in the stylesheet, or drop the icons. Inline is what the user picked,
//! and it is also the one that cannot drift: the icon is in the markup, so it renders before the stylesheet arrives and
//! it cannot 404.
//!
//! THE PATHS ARE EXTRACTED, NOT REMEMBERED. They were rendered out of the installed `lucide-react`
//! (`renderToStaticMarkup`) and pasted from that output. That is not fussiness: in lucide v1.17 the icon called `waves`
//! draws what the library now names `waves-horizontal`, and `palmtree` draws `tree-palm` — the names and the shapes have
//! changed between versions. Path data recalled from memory or copied from a different version would have drawn a
//! different icon than the page it is replacing, silently, which is this project's most expensive recurring mistake.
//!
//! `stroke-width` is an argument because the design asks for `1.25` on the About cards — a real difference at 2.5rem, not
//! a rounding detail. The class is an argument for the same reason: the size belongs to the stylesheet.

/// The lucide icon named `name`, as inline SVG, with `class` and `stroke_width` applied.
///
/// `None` for an unknown name. An unknown icon is a mistake in the view, and a mistake that renders an empty box in
/// place of a drawing is one nobody sees; `None` makes the call site decide, and the call sites here fail loudly.
pub fn icon(name: &str, class: &str, stroke_width: &str) -> Option<String> {
    let paths = match name {
        // lucide `waves` (v1.17: waves-horizontal)
        "waves" => {
            "<path d=\"M2 12q2.5 2 5 0t5 0 5 0 5 0\"></path>\
             <path d=\"M2 19q2.5 2 5 0t5 0 5 0 5 0\"></path>\
             <path d=\"M2 5q2.5 2 5 0t5 0 5 0 5 0\"></path>"
        }
        // lucide `palmtree` (v1.17: tree-palm)
        "palmtree" => {
            "<path d=\"M13 8c0-2.76-2.46-5-5.5-5S2 5.24 2 8h2l1-1 1 1h4\"></path>\
             <path d=\"M13 7.14A5.82 5.82 0 0 1 16.5 6c3.04 0 5.5 2.24 5.5 5h-3l-1-1-1 1h-3\"></path>\
             <path d=\"M5.89 9.71c-2.15 2.15-2.3 5.47-.35 7.43l4.24-4.25.7-.7.71-.71 2.12-2.12c-1.95-1.96-5.27-1.8-7.42.35\"></path>\
             <path d=\"M11 15.5c.5 2.5-.17 4.5-1 6.5h4c2-5.5-.5-12-1-14\"></path>"
        }
        // lucide `leaf`
        "leaf" => {
            "<path d=\"M11 20A7 7 0 0 1 9.8 6.1C15.5 5 17 4.48 19 2c1 2 2 4.18 2 8 0 5.5-4.78 10-10 10Z\"></path>\
             <path d=\"M2 21c0-3 1.85-5.36 5.08-6C9.5 14.52 12 13 13 12\"></path>"
        }
        // ----- The Sellers page's set, for `app/sellers/page.tsx`. Every one extracted from the installed lucide and
        // pasted from that output, for the reason above: this library has renamed icons and reshaped them across
        // versions, and a remembered path draws a different picture than the page it replaces.
        "compass" => {
            "<circle cx=\"12\" cy=\"12\" r=\"10\"></circle>\
             <path d=\"m16.24 7.76-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z\"></path>"
        }
        "user-round" => {
            "<circle cx=\"12\" cy=\"8\" r=\"5\"></circle>\
             <path d=\"M20 21a8 8 0 0 0-16 0\"></path>"
        }
        "gem" => {
            "<path d=\"M10.5 3 8 9l4 13 4-13-2.5-6\"></path>\
             <path d=\"M17 3a2 2 0 0 1 1.6.8l3 4a2 2 0 0 1 .013 2.382l-7.99 10.986a2 2 0 0 1-3.247 0l-7.99-10.986A2 2 0 0 1 2.4 7.8l2.998-3.997A2 2 0 0 1 7 3z\"></path>\
             <path d=\"M2 9h20\"></path>"
        }
        "search" => {
            "<path d=\"m21 21-4.34-4.34\"></path>\
             <circle cx=\"11\" cy=\"11\" r=\"8\"></circle>"
        }
        "target" => {
            "<circle cx=\"12\" cy=\"12\" r=\"10\"></circle>\
             <circle cx=\"12\" cy=\"12\" r=\"6\"></circle>\
             <circle cx=\"12\" cy=\"12\" r=\"2\"></circle>"
        }
        "camera" => {
            "<path d=\"M13.997 4a2 2 0 0 1 1.76 1.05l.486.9A2 2 0 0 0 18.003 7H20a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2h1.997a2 2 0 0 0 1.759-1.048l.489-.904A2 2 0 0 1 10.004 4z\"></path>\
             <circle cx=\"12\" cy=\"13\" r=\"3\"></circle>"
        }
        "send" => {
            "<path d=\"M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z\"></path>\
             <path d=\"m21.854 2.147-10.94 10.939\"></path>"
        }
        "users" => {
            "<path d=\"M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2\"></path>\
             <path d=\"M16 3.128a4 4 0 0 1 0 7.744\"></path>\
             <path d=\"M22 21v-2a4 4 0 0 0-3-3.87\"></path>\
             <circle cx=\"9\" cy=\"7\" r=\"4\"></circle>"
        }
        "check-circle-2" => {
            "<circle cx=\"12\" cy=\"12\" r=\"10\"></circle>\
             <path d=\"m9 12 2 2 4-4\"></path>"
        }
        "home" => {
            "<path d=\"M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8\"></path>\
             <path d=\"M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z\"></path>"
        }
        "map-pin" => {
            "<path d=\"M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0\"></path>\
             <circle cx=\"12\" cy=\"10\" r=\"3\"></circle>"
        }
        "bar-chart-3" => {
            "<path d=\"M3 3v16a2 2 0 0 0 2 2h16\"></path>\
             <path d=\"M18 17V9\"></path>\
             <path d=\"M13 17V5\"></path>\
             <path d=\"M8 17v-3\"></path>"
        }
        "flag" => {
            "<path d=\"M4 22V4a1 1 0 0 1 .4-.8A6 6 0 0 1 8 2c3 0 5 2 7.333 2q2 0 3.067-.8A1 1 0 0 1 20 4v10a1 1 0 0 1-.4.8A6 6 0 0 1 16 16c-3 0-5-2-8-2a6 6 0 0 0-4 1.528\"></path>"
        }
        "video" => {
            "<path d=\"m16 13 5.223 3.482a.5.5 0 0 0 .777-.416V7.87a.5.5 0 0 0-.752-.432L16 10.5\"></path>\
             <rect x=\"2\" y=\"6\" width=\"14\" height=\"12\" rx=\"2\"></rect>"
        }
        "file-text" => {
            "<path d=\"M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z\"></path>\
             <path d=\"M14 2v5a1 1 0 0 0 1 1h5\"></path>\
             <path d=\"M10 9H8\"></path>\
             <path d=\"M16 13H8\"></path>\
             <path d=\"M16 17H8\"></path>"
        }
        "megaphone" => {
            "<path d=\"M11 6a13 13 0 0 0 8.4-2.8A1 1 0 0 1 21 4v12a1 1 0 0 1-1.6.8A13 13 0 0 0 11 14H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2z\"></path>\
             <path d=\"M6 14a12 12 0 0 0 2.4 7.2 2 2 0 0 0 3.2-2.4A8 8 0 0 1 10 14\"></path>\
             <path d=\"M8 6v8\"></path>"
        }
        "clipboard-check" => {
            "<rect width=\"8\" height=\"4\" x=\"8\" y=\"2\" rx=\"1\" ry=\"1\"></rect>\
             <path d=\"M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2\"></path>\
             <path d=\"m9 14 2 2 4-4\"></path>"
        }
        "mail" => {
            "<path d=\"m22 7-8.991 5.727a2 2 0 0 1-2.009 0L2 7\"></path>\
             <rect x=\"2\" y=\"4\" width=\"20\" height=\"16\" rx=\"2\"></rect>"
        }
        "network" => {
            "<rect x=\"16\" y=\"16\" width=\"6\" height=\"6\" rx=\"1\"></rect>\
             <rect x=\"2\" y=\"16\" width=\"6\" height=\"6\" rx=\"1\"></rect>\
             <rect x=\"9\" y=\"2\" width=\"6\" height=\"6\" rx=\"1\"></rect>\
             <path d=\"M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3\"></path>\
             <path d=\"M12 12V8\"></path>"
        }
        "handshake" => {
            "<path d=\"m11 17 2 2a1 1 0 1 0 3-3\"></path>\
             <path d=\"m14 14 2.5 2.5a1 1 0 1 0 3-3l-3.88-3.88a3 3 0 0 0-4.24 0l-.88.88a1 1 0 1 1-3-3l2.81-2.81a5.79 5.79 0 0 1 7.06-.87l.47.28a2 2 0 0 0 1.42.25L21 4\"></path>\
             <path d=\"m21 3 1 11h-2\"></path>\
             <path d=\"M3 3 2 14l6.5 6.5a1 1 0 1 0 3-3\"></path>\
             <path d=\"M3 4h8\"></path>"
        }
        "key-round" => {
            "<path d=\"M2.586 17.414A2 2 0 0 0 2 18.828V21a1 1 0 0 0 1 1h3a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h1a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h.172a2 2 0 0 0 1.414-.586l.814-.814a6.5 6.5 0 1 0-4-4z\"></path>\
             <circle cx=\"16.5\" cy=\"7.5\" r=\".5\" fill=\"currentColor\"></circle>"
        }
        _ => return None,
    };
    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"none\" \
         stroke=\"currentColor\" stroke-width=\"{stroke_width}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" \
         class=\"{class}\" aria-hidden=\"true\">{paths}</svg>",
        // The view's own escaper, which covers the attribute case too (`"` and `'` as well as the text ones). `class` is
        // a literal at every call site, so this is belt and braces rather than a live hazard.
        class = crate::view::escape(class),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_the_pages_use_is_present_and_draws_something() {
        for name in ["waves", "palmtree", "leaf"] {
            let svg = icon(name, "h-10 w-10", "1.25").expect("the icon is in the table");
            assert!(svg.starts_with("<svg "), "{name} is an svg");
            assert!(svg.contains("<path d=\""), "{name} draws a path");
            assert!(svg.contains("stroke-width=\"1.25\""), "{name} keeps its weight");
            assert!(
                svg.contains("class=\"h-10 w-10\""),
                "{name} keeps the size the design asks for"
            );
            // Decorative: the meaning is in the copy beside it, so a screen reader must skip it rather than read it.
            assert!(svg.contains("aria-hidden=\"true\""));
        }
    }

    #[test]
    fn an_unknown_icon_is_none_rather_than_an_empty_box() {
        // An empty `<svg>` renders as nothing at all, which is a mistake nobody sees. `None` is a mistake the call site
        // has to handle.
        assert!(icon("not-an-icon", "h-4 w-4", "2").is_none());
    }
}
