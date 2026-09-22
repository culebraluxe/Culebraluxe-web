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
