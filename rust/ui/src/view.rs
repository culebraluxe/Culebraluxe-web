//! The view: a pure function of the model, rendering the same class names the TypeScript screens already use.
//!
//! TWO THINGS TO NOTICE, both deliberate:
//!
//! * **The class names are Tailwind v4 / shadcn tokens already in `app/globals.css`** (`bg-card`,
//!   `text-muted-foreground`, `rounded-lg border`). Styling transfers because the markup speaks the same vocabulary
//!   — no parallel stylesheet, no theme to keep in sync. Tailwind only has to be told to scan this crate with an
//!   `@source` line when the shell lands.
//! * **Every interpolated string is escaped here.** A Rust renderer that formats HTML owns that risk, and this crate
//!   will render values that came from a database and from third-party sources. The escaping test is not decoration.
//!
//! The `data-*` attributes are the bridge: a third-party widget gets a container identified by a stable id, and any
//! selection it reports comes back as a `Msg` carrying that same id. Nothing else crosses.

use crate::model::{Model, PropertySummary, Screen};

/// Escape text for HTML text and attribute positions. Quotes are escaped because the same helper is used inside
/// `data-` attributes, where an unescaped quote would end the attribute early.
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
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

fn acreage(property: &PropertySummary) -> String {
    match property.acreage {
        Some(acres) => format!("{acres:.2} acres"),
        None => "acreage unknown".to_string(),
    }
}

/// Render the whole screen. Pure: same model in, same string out, and it decides nothing.
pub fn render(model: &Model) -> String {
    let body = match model.screen {
        Screen::Portfolio => portfolio(model),
        Screen::PropertyDetail => detail(model),
    };
    format!(
        "<div class=\"mx-auto max-w-4xl p-6\" data-rust-screen=\"{}\">{}{}{}</div>",
        match model.screen {
            Screen::Portfolio => "portfolio",
            Screen::PropertyDetail => "property-detail",
        },
        error_banner(model),
        loading_banner(model),
        body
    )
}

fn error_banner(model: &Model) -> String {
    match model.error.as_deref() {
        Some(message) => format!(
            "<div class=\"mb-4 rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-sm\" \
             role=\"alert\">Could not load: {}</div>",
            escape(message)
        ),
        None => String::new(),
    }
}

fn loading_banner(model: &Model) -> String {
    if model.loading {
        "<p class=\"mb-4 text-sm text-muted-foreground\">Loading…</p>".to_string()
    } else {
        String::new()
    }
}

fn portfolio(model: &Model) -> String {
    if model.properties.is_empty() && !model.loading {
        return "<p class=\"text-sm text-muted-foreground\">No properties yet.</p>".to_string();
    }
    let known = model
        .properties
        .iter()
        .filter(|property| property.owner_known)
        .count();
    let unknown = model.properties.len().saturating_sub(known);
    let rows = model
        .properties
        .iter()
        .map(|property| {
            format!(
                "<li class=\"flex items-center justify-between rounded-lg border bg-card p-3\">\
                   <button type=\"button\" data-select-property=\"{id}\" \
                     class=\"text-left text-sm font-medium hover:underline\">{label}</button>\
                   <span class=\"text-xs text-muted-foreground\">{acreage}{owner}</span>\
                 </li>",
                id = escape(&property.id),
                label = escape(&property.label),
                acreage = acreage(property),
                owner = if property.owner_known {
                    String::new()
                } else {
                    " · owner unknown".to_string()
                }
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<header class=\"mb-4\">\
           <h1 class=\"text-xl font-semibold\">Portfolio</h1>\
           <p class=\"text-sm text-muted-foreground\">{total} properties · {known} with an owner of record · \
             {unknown} with none</p>\
         </header>\
         <ul class=\"space-y-2\">{rows}</ul>",
        total = model.properties.len(),
        known = known,
        unknown = unknown,
        rows = rows
    )
}

fn detail(model: &Model) -> String {
    match model.selected() {
        None => "<p class=\"text-sm text-muted-foreground\">That property is no longer in the list.</p>".to_string(),
        Some(property) => format!(
            "<header class=\"mb-4\">\
               <button type=\"button\" data-action=\"back\" class=\"text-sm text-muted-foreground hover:underline\">\
                 ← Portfolio</button>\
               <h1 class=\"mt-2 text-xl font-semibold\">{label}</h1>\
               <p class=\"text-sm text-muted-foreground\">{acreage} · {owner}</p>\
             </header>",
            label = escape(&property.label),
            acreage = acreage(property),
            owner = if property.owner_known { "owner of record" } else { "owner unknown" }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Model, PropertySummary, Screen};

    fn property() -> PropertySummary {
        PropertySummary {
            id: "p1".into(),
            label: "Bo. Delicias 17a".into(),
            owner_known: true,
            acreage: Some(1.25),
        }
    }

    #[test]
    fn escaping_covers_text_and_attribute_positions() {
        assert_eq!(
            escape("<script>alert('x')</script>"),
            "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;"
        );
        assert_eq!(escape("a\"b&c"), "a&quot;b&amp;c");
    }

    #[test]
    fn a_label_that_looks_like_markup_is_rendered_as_text() {
        let model = Model {
            properties: vec![PropertySummary {
                id: "p1\" onmouseover=\"alert(1)".into(),
                label: "<img src=x onerror=alert(1)>".into(),
                owner_known: true,
                acreage: None,
            }],
            ..Model::default()
        };
        let html = render(&model);
        assert!(
            !html.contains("<img"),
            "markup from data must never reach the DOM as markup"
        );
        assert!(
            html.contains("&lt;img"),
            "the escaped form is what gets rendered"
        );
        assert!(
            !html.contains("onmouseover=\"alert"),
            "an attribute must not be breakable from data"
        );
    }

    #[test]
    fn the_portfolio_reports_owner_known_and_unknown_separately() {
        let model = Model {
            properties: vec![
                property(),
                PropertySummary {
                    id: "p2".into(),
                    label: "Parcel".into(),
                    owner_known: false,
                    acreage: None,
                },
            ],
            ..Model::default()
        };
        let html = render(&model);
        assert!(html.contains("2 properties"));
        assert!(html.contains("1 with an owner of record"));
        assert!(html.contains("1 with none"));
        assert!(
            html.contains("owner unknown"),
            "the unknown-owner case is real data here, not an edge case"
        );
    }

    #[test]
    fn the_detail_screen_renders_the_selection_and_a_way_back() {
        let model = Model {
            properties: vec![property()],
            selected_property_id: Some("p1".into()),
            screen: Screen::PropertyDetail,
            ..Model::default()
        };
        let html = render(&model);
        assert!(html.contains("data-rust-screen=\"property-detail\""));
        assert!(html.contains("Bo. Delicias 17a"));
        assert!(
            html.contains("data-action=\"back\""),
            "the way back belongs to the screen, not the browser"
        );
    }

    #[test]
    fn a_dangling_selection_says_so_instead_of_rendering_nothing() {
        let model = Model {
            screen: Screen::PropertyDetail,
            ..Model::default()
        };
        assert!(render(&model).contains("no longer in the list"));
    }

    #[test]
    fn the_markup_speaks_the_applications_class_vocabulary() {
        let html = render(&Model {
            properties: vec![property()],
            ..Model::default()
        });
        for class in ["bg-card", "text-muted-foreground", "rounded-lg", "border"] {
            assert!(
                html.contains(class),
                "styling transfers by reusing tokens; {class} is missing"
            );
        }
    }

    #[test]
    fn the_selection_id_travels_to_the_dom_for_the_bridge() {
        let html = render(&Model {
            properties: vec![property()],
            ..Model::default()
        });
        assert!(
            html.contains("data-select-property=\"p1\""),
            "a widget can only report a selection if the id is on the element"
        );
    }
}
