//! The view: a pure function of the model, rendering the class names this application already uses.
//!
//! The nav is generated from `SCREENS`, so a screen cannot exist without appearing in the menu and a menu entry
//! cannot point at a screen that does not exist. Every interpolated value is escaped — this crate renders data from a
//! database and from third-party sources, and a Rust renderer that formats HTML owns that risk.

use crate::model::{home, Model, Row, Surface, SCREENS};

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
        escape(model.screen.key),
        nav(model),
        error_banner(model),
        loading_banner(model),
        body(model)
    )
}

fn nav(model: &Model) -> String {
    let surface = model.screen.surface;
    let mut out = format!(
        "<p class=\"mb-2 px-2 text-xs font-medium uppercase tracking-wide text-muted-foreground\">{}</p>",
        escape(surface.label())
    );
    // The surface the current screen belongs to, which is what the live portal does: the registry is the single source
    // of truth for what navigation belongs under a surface, and only LISTED screens appear here. Retired and unlisted
    // routes are reached from the screens that own them.
    for screen in SCREENS
        .iter()
        .copied()
        .filter(|candidate| candidate.surface == surface && candidate.is_listed())
    {
        let active = screen == model.screen;
        out.push_str(&format!(
            "<button type=\"button\" data-nav=\"{key}\" class=\"block w-full rounded-md px-2 py-1.5 text-left \
             text-sm {state}\">{label}{suffix}</button>",
            key = escape(screen.key),
            state = if active { "bg-muted font-medium" } else { "hover:bg-muted/60 text-muted-foreground" },
            label = escape(screen.title),
            suffix = if screen.is_deferred() { " (no data yet)" } else { "" }
        ));
    }
    out.push_str(&surface_switcher(model));
    out
}

/// One nav entry per surface, so a host that mounts one surface can still reach the others. Each carries
/// `data-surface` rather than `data-nav`: the two are different intents (go to a screen, versus go to a surface's
/// home), and one attribute meaning two things is how a click ends up doing the wrong one.
fn surface_switcher(model: &Model) -> String {
    let mut out = String::from(
        "<p class=\"mt-6 mb-2 px-2 text-xs font-medium uppercase tracking-wide text-muted-foreground\">Surfaces</p>",
    );
    for surface in Surface::ALL.iter().copied() {
        let Some(home) = home(surface) else { continue };
        let active = surface == model.screen.surface;
        out.push_str(&format!(
            "<button type=\"button\" data-surface=\"{key}\" title=\"home: {home}\" class=\"block w-full rounded-md \
             px-2 py-1 text-left text-xs {state}\">{label}</button>",
            key = escape(surface.key()),
            home = escape(home.key),
            state = if active { "text-foreground font-medium" } else { "text-muted-foreground hover:bg-muted/60" },
            label = escape(surface.label())
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
           <p class=\"text-sm text-muted-foreground\">{path}</p>\
           {subject}\
         </header>",
        title = escape(model.screen.title),
        // A screen with no live route says that, rather than naming one that does not exist.
        path = if model.screen.path.is_empty() {
            "No live route yet: this screen reads data the app exposes through no route of its own."
                .to_string()
        } else {
            format!("Replaces <code>{}</code>", escape(model.screen.path))
        },
        subject = subject
    );
    if model.screen.is_deferred() {
        return format!("{header}{}", deferred_notice(model));
    }
    format!("{header}{}", rows(model))
}

/// A screen with no data says so in its own words. The reason travels with the screen rather than being one hardcoded
/// sentence, because "no read model exists", "this is a demo placeholder" and "this is a widget host deliberately not
/// moved yet" are three different situations — and a screen that cannot tell them apart teaches the reader nothing.
fn deferred_notice(model: &Model) -> String {
    let reason = model
        .screen
        .deferred
        .unwrap_or("This screen has no data source yet.");
    format!(
        "<div class=\"rounded-lg border bg-card p-6 text-sm text-muted-foreground\">{}</div>",
        escape(reason)
    )
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
    // Exactly one record, or none: a row that could open two different records opens neither, because one click cannot
    // mean two things. (`workflows` is the case: it has both an instance record and a runtime inspector.)
    let records = SCREENS
        .iter()
        .filter(|candidate| candidate.detail_of == Some(model.screen.key))
        .count();
    let opens = if records == 1 {
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
    use crate::model::{Nav, Screen};

    /// Screens are addressed by KEY in these tests. The table is the source of truth, so a test that named a variant
    /// would be asserting a name that only exists in a previous version of this file.
    fn target(key: &str) -> Screen {
        crate::model::screen(key).expect("a screen the table defines")
    }

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

    /// The nav shows its own surface's listed screens, plus one switcher entry per surface — and NEVER a retired,
    /// unlisted or record screen. That last part is the invariant worth having: "the code stays, the links go" is a
    /// decision recorded in the registry, and a port that quietly re-lists a retired screen undoes it.
    #[test]
    fn the_nav_shows_its_surface_and_never_a_retired_screen() {
        for &screen in SCREENS {
            let html = render(&Model {
                screen,
                ..Model::default()
            });
            for candidate in SCREENS {
                let count = html
                    .matches(&format!("data-nav=\"{}\"", candidate.key))
                    .count();
                match candidate.nav {
                    Nav::Listed if candidate.surface == screen.surface => {
                        assert_eq!(
                            count,
                            1,
                            "{} missing from the {} nav",
                            candidate.key,
                            screen.surface.label()
                        )
                    }
                    // A listed screen of another surface appears at most once, as that surface's home.
                    Nav::Listed => assert!(count <= 1, "{} appeared twice", candidate.key),
                    Nav::Retired | Nav::Unlisted | Nav::Record => {
                        assert_eq!(count, 0, "{} must never be a nav entry", candidate.key)
                    }
                }
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
            screen: target("site-properties"),
            rows: rows.clone(),
            ..Model::default()
        });
        assert!(listing.contains("data-open-record=\"villa-del-mar\""));
        // Activity has no record screen: it is a feed, and a row of it is history rather than a thing to open.
        let feed = render(&Model {
            screen: target("activity"),
            rows,
            ..Model::default()
        });
        assert!(!feed.contains("data-open-record"));
    }

    #[test]
    fn a_detail_screen_names_the_record_it_is_about() {
        let model = Model {
            screen: target("site-property-detail"),
            scope: Some("villa-del-mar".into()),
            ..Model::default()
        };
        assert!(render(&model).contains("Record <code>villa-del-mar</code>"));

        // The slug arrives from a URL, so it is data like any other and gets escaped like any other.
        let hostile = Model {
            screen: target("site-property-detail"),
            scope: Some("<b>x</b>".into()),
            ..Model::default()
        };
        assert!(render(&hostile).contains("&lt;b&gt;x&lt;/b&gt;"));
    }

    #[test]
    fn the_active_screen_is_marked_and_the_others_are_not() {
        let model = Model {
            screen: target("deals"),
            ..Model::default()
        };
        let html = render(&model);
        assert!(html.contains("data-rust-screen=\"deals\""));
        assert_eq!(html.matches("bg-muted font-medium").count(), 1);
    }

    #[test]
    fn a_screen_with_no_data_says_why_in_its_own_words() {
        // The reason travels with the screen rather than being one hardcoded sentence. Projects is wired now, so the
        // example is the screen that is a placeholder by design.
        let placeholder = render(&Model {
            screen: target("accounting-receipt-scanner"),
            ..Model::default()
        });
        assert!(placeholder.contains("FAKE V1"));
        assert!(placeholder.contains("(no data yet)"));
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
