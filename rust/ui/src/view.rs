//! The view: a pure function of the model, rendering the class names this application already uses.
//!
//! The nav is generated from `Screen::ALL`, so a screen cannot exist without appearing in the menu and a menu entry
//! cannot point at a screen that does not exist. Every interpolated value is escaped — this crate renders data from a
//! database and from third-party sources, and a Rust renderer that formats HTML owns that risk.

use crate::model::{Model, Row, Screen};

/// Escape text for HTML text and attribute positions. Quotes matter because the same helper fills `data-` attributes,
/// where an unescaped quote would end the attribute early.
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

/// Render the whole screen: nav plus the current screen's body. Pure and decision-free.
pub fn render(model: &Model) -> String {
    format!(
        "<div class=\"flex min-h-screen text-foreground\" data-rust-screen=\"{}\">\
           <nav class=\"w-60 shrink-0 border-r bg-card p-4\" aria-label=\"Portal\">{}</nav>\
           <main class=\"min-w-0 flex-1 p-6\">{}{}{}</main>\
         </div>",
        escape(model.screen.key()),
        nav(model),
        error_banner(model),
        loading_banner(model),
        body(model)
    )
}

fn nav(model: &Model) -> String {
    let mut out = String::new();
    let mut group = "";
    // Only the area this screen belongs to. A host mounts one area, so the public host can never be offered the
    // expenses screen and the portal host can never be offered the listings.
    for &screen in Screen::ALL
        .iter()
        .filter(|screen| screen.area() == model.screen.area())
    {
        if screen.group() != group {
            group = screen.group();
            out.push_str(&format!(
                "<p class=\"mt-4 mb-1 px-2 text-xs font-medium uppercase tracking-wide text-muted-foreground\">{}</p>",
                escape(group)
            ));
        }
        let active = screen == model.screen;
        out.push_str(&format!(
            "<button type=\"button\" data-nav=\"{key}\" class=\"block w-full rounded-md px-2 py-1.5 text-left \
             text-sm {state}\">{label}{suffix}</button>",
            key = escape(screen.key()),
            state = if active { "bg-muted font-medium" } else { "hover:bg-muted/60 text-muted-foreground" },
            label = escape(screen.title()),
            suffix = if screen.is_deferred() { " (not ported)" } else { "" }
        ));
    }
    out
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

/// The body: a header naming the screen and the live route it replaces, then either the placeholder or its rows.
fn body(model: &Model) -> String {
    // When the screen is about one record, say which — a detail screen that does not name its subject is a page you
    // cannot tell apart from a wrong one.
    let subject = match model.scope.as_deref() {
        Some(scope) => format!(
            "<p class=\"text-sm text-muted-foreground\">Record <code>{}</code></p>",
            escape(scope)
        ),
        None => String::new(),
    };
    let header = format!(
        "<header class=\"mb-4\">\
           <h1 class=\"text-xl font-semibold\">{title}</h1>\
           <p class=\"text-sm text-muted-foreground\">Replaces <code>{path}</code></p>\
           {subject}\
         </header>",
        title = escape(model.screen.title()),
        path = escape(model.screen.live_path()),
        subject = subject
    );
    if model.screen.is_deferred() {
        return format!("{header}{}", deferred_notice());
    }
    format!("{header}{}", rows(model))
}

/// Project Management says why it is empty, on the screen, rather than looking like a broken page.
fn deferred_notice() -> String {
    "<div class=\"rounded-lg border bg-card p-6 text-sm text-muted-foreground\">\
       This screen is intentionally empty. It holds three third-party widgets in the TypeScript application — a tree,\
       a Gantt, and a calendar — and the plan for letting Rust own the container while each widget keeps its own\
       subtree has to be decided before any of them moves. The other screens come first.\
     </div>"
        .to_string()
}

fn rows(model: &Model) -> String {
    if model.rows.is_empty() {
        return "<p class=\"text-sm text-muted-foreground\">Nothing to show yet.</p>".to_string();
    }
    let items = model
        .rows
        .iter()
        .map(|row| row_item(model, row))
        .collect::<Vec<_>>()
        .join("");
    format!("<ul class=\"space-y-2\">{items}</ul>")
}

fn row_item(model: &Model, row: &Row) -> String {
    let selected = model.selected_row_id.as_deref() == Some(row.id.as_str());
    // A row on a screen with a detail view carries the intent to open that record, and the shell turns the attribute
    // into `RecordOpened`. A row anywhere else carries only `data-select-row`.
    let opens = if model.screen.detail().is_some() {
        format!(" data-open-record=\"{}\"", escape(&row.id))
    } else {
        String::new()
    };
    let cells = row
        .cells
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            let class = if index == 0 {
                "font-medium"
            } else {
                "text-muted-foreground"
            };
            format!("<span class=\"{class}\">{}</span>", escape(cell))
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<li><button type=\"button\" data-select-row=\"{id}\"{opens} aria-pressed=\"{pressed}\" \
           class=\"flex w-full items-center justify-between gap-3 rounded-lg border bg-card p-3 text-left text-sm \
           {state}\">{cells}{badge}</button></li>",
        id = escape(&row.id),
        opens = opens,
        pressed = if selected { "true" } else { "false" },
        state = if selected { "ring-1 ring-ring" } else { "hover:bg-muted/40" },
        cells = cells,
        badge = match row.badge.as_deref() {
            Some(badge) => format!(
                "<span class=\"rounded-full bg-muted px-2 py-0.5 text-xs\">{}</span>",
                escape(badge)
            ),
            None => String::new(),
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Model, Row};

    /// The row that matters. Data reaches this crate from a database and from third-party feeds, and a renderer that
    /// formats HTML owns the escaping — the test goes through `render`, not through `escape`, because the bug that
    /// ships is the one where the helper is correct and one call site forgets it.
    #[test]
    fn text_from_the_database_cannot_become_markup() {
        let hostile = "<img src=x onerror=\"alert(1)\">";
        let model = Model {
            rows: vec![Row {
                id: "1".into(),
                cells: vec![hostile.into()],
                badge: Some("<b>bad</b>".into()),
            }],
            ..Model::default()
        };
        let html = render(&model);
        assert!(
            !html.contains("<img"),
            "a cell must never be able to open a tag"
        );
        assert!(!html.contains("<b>bad</b>"));
        assert!(html.contains("&lt;img src=x onerror=&quot;alert(1)&quot;&gt;"));
        assert!(html.contains("&lt;b&gt;bad&lt;/b&gt;"));
    }

    /// A hostile row id lands in a `data-` attribute, where an unescaped quote would end the attribute and let the
    /// rest of the value become markup.
    #[test]
    fn a_row_id_cannot_escape_its_attribute() {
        let model = Model {
            rows: vec![Row {
                id: "\" onmouseover=\"steal()".into(),
                cells: vec!["x".into()],
                badge: None,
            }],
            ..Model::default()
        };
        let html = render(&model);
        assert!(!html.contains("onmouseover=\"steal()\""));
        assert!(html.contains("&quot;"));
    }

    /// The nav is per area: a host mounts one half of the application, so the public host must not be offered the
    /// expenses screen and the portal host must not be offered the listings. Checked from both sides, for every screen.
    #[test]
    fn the_nav_shows_exactly_the_screens_of_its_area() {
        for screen in Screen::ALL {
            let html = render(&Model {
                screen: *screen,
                ..Model::default()
            });
            for candidate in Screen::ALL {
                let expected = usize::from(candidate.area() == screen.area());
                assert_eq!(
                    html.matches(&format!("data-nav=\"{}\"", candidate.key()))
                        .count(),
                    expected,
                    "{} on the {} nav while showing {}",
                    candidate.key(),
                    screen.area().label(),
                    screen.key()
                );
            }
        }
    }

    /// A listing row opens its record; a row on a screen with no detail view must not pretend it can.
    #[test]
    fn only_a_screen_with_a_detail_view_offers_to_open_a_row() {
        let rows = vec![Row {
            id: "villa-del-mar".into(),
            cells: vec!["Villa del Mar".into()],
            badge: None,
        }];
        let listing = render(&Model {
            screen: Screen::SiteProperties,
            rows: rows.clone(),
            ..Model::default()
        });
        assert!(listing.contains("data-open-record=\"villa-del-mar\""));
        // Activity has no record screen: it is a feed, and a row of it is history rather than a thing to open.
        let feed = render(&Model {
            screen: Screen::Activity,
            rows,
            ..Model::default()
        });
        assert!(!feed.contains("data-open-record"));
    }

    #[test]
    fn a_detail_screen_names_the_record_it_is_about() {
        let model = Model {
            screen: Screen::SitePropertyDetail,
            scope: Some("villa-del-mar".into()),
            ..Model::default()
        };
        assert!(render(&model).contains("Record <code>villa-del-mar</code>"));

        // The slug arrives from a URL, so it is data like any other and gets escaped like any other.
        let hostile = Model {
            screen: Screen::SitePropertyDetail,
            scope: Some("<b>x</b>".into()),
            ..Model::default()
        };
        assert!(render(&hostile).contains("&lt;b&gt;x&lt;/b&gt;"));
    }

    #[test]
    fn the_active_screen_is_marked_and_the_others_are_not() {
        let model = Model {
            screen: Screen::Deals,
            ..Model::default()
        };
        let html = render(&model);
        assert!(html.contains("data-rust-screen=\"deals\""));
        assert_eq!(html.matches("bg-muted font-medium").count(), 1);
    }

    #[test]
    fn the_deferred_screen_says_so_instead_of_looking_broken() {
        let model = Model {
            screen: Screen::Projects,
            ..Model::default()
        };
        let html = render(&model);
        assert!(html.contains("intentionally empty"));
        assert!(html.contains("Projects (not ported)"));
    }

    #[test]
    fn the_error_banner_is_escaped_too() {
        let model = Model {
            error: Some("<script>".into()),
            ..Model::default()
        };
        let html = render(&model);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
