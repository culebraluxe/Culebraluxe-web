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

/// Screens that render markup of their own instead of a generic list of rows.
///
/// Everything else in this crate renders rows because that is what a read model is. A lab, a board or a widget host is
/// not a list, and pretending it is produces a screen that looks nothing like the one being ported. So a screen may own
/// its body; the model, the messages, the effects and the shell are unchanged, which is the point of putting this here
/// rather than in the shell.
fn custom_body(model: &Model) -> Option<String> {
    match model.screen.key {
        "tech-lab" => Some(tech_lab()),
        "projects" => Some(projects_view(model)),
        _ => None,
    }
}

/// The Tech Lab: what Rust does with layout that the TypeScript pages do not, on the same design tokens.
///
/// The claim worth testing here is layout honesty: every class below is one this application already defines, nothing
/// is positioned with an inline style or a magic number, and no value is copied from a design file. If it looks
/// sharper, it is because the markup says what it means and the stylesheet decides how that looks.
fn tech_lab() -> String {
    let tokens = [
        ("Surface", "bg-card border text-card-foreground"),
        ("Muted text", "text-muted-foreground"),
        ("Accent", "bg-primary text-primary-foreground"),
        ("Danger", "bg-destructive/10 border-destructive/40"),
    ];
    let token_rows = tokens
        .iter()
        .map(|(name, class)| {
            format!(
                "<div class=\"flex items-center gap-3 rounded-lg border p-3\">\
                   <span class=\"w-32 text-xs text-muted-foreground\">{}</span>\
                   <span class=\"rounded-md border px-3 py-1 text-xs {}\">{}</span>\
                 </div>",
                escape(name),
                escape(class),
                escape(class)
            )
        })
        .collect::<String>();

    let badges = ["new", "active", "under contract", "closed"]
        .iter()
        .map(|status| {
            format!(
                "<span class=\"rounded-full bg-muted px-2.5 py-1 text-xs\">{}</span>",
                escape(status)
            )
        })
        .collect::<String>();

    let grid = (1..=6)
        .map(|index| {
            format!(
                "<div class=\"rounded-lg border bg-card p-4\">\
                   <p class=\"text-2xl font-semibold tabular-nums\">{index}</p>\
                   <p class=\"text-xs text-muted-foreground\">Column {index}</p>\
                 </div>"
            )
        })
        .collect::<String>();

    let table = [
        ("Villa Rosada", "Under Contract", "$1,250,000", "9 days"),
        ("Casa Verde", "Showing", "$780,000", "—"),
        ("La Colina", "Offer", "$2,400,000", "3 days"),
    ]
    .iter()
    .map(|(name, stage, value, next)| {
        format!(
            "<tr class=\"border-t\">\
               <td class=\"px-3 py-2 text-sm font-medium\">{}</td>\
               <td class=\"px-3 py-2 text-sm text-muted-foreground\">{}</td>\
               <td class=\"px-3 py-2 text-right text-sm tabular-nums\">{}</td>\
               <td class=\"px-3 py-2 text-right text-sm text-muted-foreground\">{}</td>\
             </tr>",
            escape(name),
            escape(stage),
            escape(value),
            escape(next)
        )
    })
    .collect::<String>();

    format!(
        "{}{}{}{}{}",
        lab_intro(),
        lab_tokens(&token_rows),
        lab_badges(&badges),
        lab_grid(&grid),
        lab_table(&table)
    )
}

fn lab_section(title: &str, body: &str) -> String {
    format!(
        "<section>\
           <h2 class=\"mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground\">{}</h2>\
           {}\
         </section>",
        escape(title),
        body
    )
}

fn lab_intro() -> String {
    "<section class=\"rounded-xl border bg-card p-5\">\
       <h2 class=\"text-sm font-medium\">Why this screen exists</h2>\
       <p class=\"mt-2 max-w-3xl text-sm text-muted-foreground\">\
         Every class on this page is one this application already defines. Nothing here is positioned with an inline\
         style or a magic number, and no value was copied from a design file — the markup says what it means and the\
         stylesheet decides how that looks. That is the whole difference: the same tokens, applied without a compromise\
         being made on the way.\
       </p>\
     </section>"
        .to_string()
}

fn lab_tokens(rows: &str) -> String {
    lab_section("Tokens", &format!("<div class=\"space-y-2\">{rows}</div>"))
}

fn lab_badges(badges: &str) -> String {
    lab_section(
        "Status badges",
        &format!("<div class=\"flex flex-wrap gap-2\">{badges}</div>"),
    )
}

fn lab_grid(grid: &str) -> String {
    lab_section(
        "Grid",
        &format!(
            "<div class=\"grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-6\">{grid}</div>"
        ),
    )
}

fn lab_table(table: &str) -> String {
    let head =
        "<thead class=\"bg-muted/50 text-xs uppercase tracking-wide text-muted-foreground\"><tr>\
         <th class=\"px-3 py-2 text-left font-medium\">Property</th>\
         <th class=\"px-3 py-2 text-left font-medium\">Stage</th>\
         <th class=\"px-3 py-2 text-right font-medium\">Value</th>\
         <th class=\"px-3 py-2 text-right font-medium\">Next date</th>\
       </tr></thead>";
    lab_section(
        "Table",
        &format!(
            "<div class=\"overflow-hidden rounded-lg border bg-card\">\
               <table class=\"w-full border-collapse\">{head}<tbody>{table}</tbody></table>\
             </div>"
        ),
    )
}

/// The heading every screen gets: its title, the live route it replaces, and the record when it is about one.
fn screen_header(model: &Model) -> String {
    // A detail screen that does not name its subject is a page you cannot tell apart from a wrong one.
    let subject = match model.scope.as_deref() {
        Some(scope) => format!(
            "<p class=\"text-sm text-muted-foreground\">Record <code>{}</code></p>",
            escape(scope)
        ),
        None => String::new(),
    };
    format!(
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
    )
}

/// Projects, mirroring `components/portal/projects-workspace.tsx` as it is today: the 86px scope rail, the glass view
/// rail with the same six views, the status control with the same four words, the work plan table, and the catch-up
/// panel whose source is Apple Calendar through EventKit.
///
/// NOT FINISHED HERE, ON PURPOSE: the Apple Calendar path is a seam. The live screen got one calendar event and one
/// todo to happen, and finishing it is a separate piece of work — so this panel names the source and shows the seam
/// rather than pretending to sync. The tree, the timeline and the calendar widgets stay in TypeScript until they are
/// hosted as islands.
fn projects_view(model: &Model) -> String {
    let views = [
        "Work Plan",
        "Timeline",
        "Calendar",
        "Financials",
        "Documents",
        "Activity",
    ];
    let view_tabs = views
        .iter()
        .enumerate()
        .map(|(index, view)| {
            let active = index == 0;
            format!(
                "<button type=\"button\" class=\"portal-glass-tab{}\"{}>{}</button>",
                if active {
                    " bg-[var(--portal-navy)] text-white shadow-sm"
                } else {
                    ""
                },
                if active { " aria-current=\"page\"" } else { "" },
                escape(view)
            )
        })
        .collect::<String>();

    let statuses = ["Open", "In progress", "Complete", "Archived"];
    let status_options = statuses
        .iter()
        .map(|status| format!("<option>{}</option>", escape(status)))
        .collect::<String>();

    let scope_rail = "<div class=\"flex w-[86px] shrink-0 flex-col items-center gap-2 border-r \
         border-[var(--portal-panel-border)] py-3\" aria-label=\"Project scope and domain\">\
         <button type=\"button\" class=\"rounded-full bg-[var(--portal-navy)] px-2.5 py-1 text-[10px] font-medium \
           uppercase tracking-[0.12em] text-white\">Catch-Up</button>\
       </div>";

    let rows = if model.rows.is_empty() {
        "<tr><td colspan=\"3\" class=\"px-3 py-6 text-center text-sm text-muted-foreground\">No projects returned.</td></tr>"
            .to_string()
    } else {
        model
            .rows
            .iter()
            .map(|row| {
                let cells = row
                    .cells
                    .iter()
                    .enumerate()
                    .map(|(index, cell)| {
                        let class = if index == 0 { "font-medium text-[var(--portal-navy)]" } else { "text-[var(--portal-blue-gray)]" };
                        format!("<td class=\"px-3 py-2 text-sm {class}\">{}</td>", escape(cell))
                    })
                    .collect::<String>();
                format!("<tr class=\"border-t border-[var(--portal-panel-border)]\">{cells}{}</tr>", match row.badge.as_deref() {
                    Some(badge) => format!(
                        "<td class=\"px-3 py-2 text-right text-[11px] uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]\">{}</td>",
                        escape(badge)
                    ),
                    None => String::new(),
                })
            })
            .collect::<String>()
    };

    let work_plan = format!(
        "<section class=\"min-w-0 flex-1 overflow-hidden rounded-lg border border-[var(--portal-panel-border)] bg-white/70\">\
           <header class=\"flex items-center gap-3 border-b border-[var(--portal-panel-border)] px-3 py-2\">\
             <p class=\"w-[190px] shrink-0 text-left text-[14px] font-medium uppercase leading-tight tracking-[0.14em] \
               text-[var(--portal-gold)]\">{title}</p>\
             <nav aria-label=\"Project workspace views\" class=\"portal-glass-rail h-11 w-max\">{view_tabs}</nav>\
             <div class=\"ml-auto flex shrink-0 items-center gap-2\">\
               <select aria-label=\"Project status\" class=\"rounded-full bg-white/50 px-2.5 py-1 text-[9px] font-medium \
                 uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] outline-none\">{status_options}</select>\
               <button type=\"button\" class=\"rounded-full bg-[var(--portal-navy)] px-3.5 py-2 text-[12px] font-medium \
                 text-white shadow-sm\">New project</button>\
             </div>\
           </header>\
           <table class=\"w-full border-collapse\">\
             <thead class=\"text-[11px] uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]\"><tr>\
               <th class=\"px-3 py-2 text-left font-medium\">Work item</th>\
               <th class=\"px-3 py-2 text-left font-medium\">Owner</th>\
               <th class=\"px-3 py-2 text-left font-medium\">Status</th>\
             </tr></thead>\
             <tbody>{rows}</tbody>\
           </table>\
         </section>",
        title = escape(model.screen.title),
        view_tabs = view_tabs,
        status_options = status_options,
        rows = rows
    );

    let catch_up = catch_up_panel();
    format!("<div class=\"flex gap-3\">{scope_rail}{work_plan}{catch_up}</div>")
}

/// The catch-up panel, and the seam that is not finished: Apple Calendar, through EventKit, one event and one todo
/// deep. It says so rather than implying a sync that is not there.
fn catch_up_panel() -> String {
    "<aside class=\"w-[300px] shrink-0 rounded-lg border border-[var(--portal-panel-border)] bg-white/70 p-3\">\
       <h2 class=\"text-[11px] font-medium uppercase tracking-[0.12em] text-[var(--portal-gold)]\">Catch-Up</h2>\
       <p class=\"mt-2 text-[13px] leading-snug text-[var(--portal-blue-gray)]\">\
         Scheduled from <strong>Apple Calendar</strong> through EventKit. This path is a seam, not a finished sync: the\
         live screen produced one calendar event and one todo, and completing it is separate work.\
       </p>\
       <p class=\"mt-2 text-[11px] uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]\">source: apple_calendar</p>\
     </aside>"
        .to_string()
}

/// The body: a screen's own markup when it has any, else a header and rows.
fn body(model: &Model) -> String {
    if let Some(custom) = custom_body(model) {
        return format!("{}{}", screen_header(model), custom);
    }
    let header = screen_header(model);
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

    /// The Projects screen mirrors the live workspace: the same six views in the same order, the same four status
    /// words, and the Apple Calendar seam named rather than implied.
    #[test]
    fn the_projects_screen_mirrors_the_live_workspace() {
        let html = render(&Model {
            screen: target("projects"),
            rows: vec![Row {
                id: "p1".into(),
                cells: vec!["Listing onboarding".into(), "Ada".into()],
                badge: Some("active".into()),
            }],
            ..Model::default()
        });
        for view in [
            "Work Plan",
            "Timeline",
            "Calendar",
            "Financials",
            "Documents",
            "Activity",
        ] {
            assert!(
                html.contains(view),
                "the {view} view is part of the live workspace"
            );
        }
        for status in ["Open", "In progress", "Complete", "Archived"] {
            assert!(
                html.contains(status),
                "the live status control offers {status}"
            );
        }
        assert!(
            html.contains("portal-glass-tab"),
            "the live view rail is glass tabs"
        );
        assert!(
            html.contains("Listing onboarding"),
            "the wired rows are the work plan"
        );
        // The seam, stated: Apple Calendar through EventKit is not finished, and the screen says so.
        assert!(html.contains("Apple Calendar"));
        assert!(html.contains("one calendar event and one todo"));
    }
    #[test]
    fn a_screen_can_own_its_body() {
        let lab = render(&Model {
            screen: target("tech-lab"),
            rows: vec![Row {
                id: "ignored".into(),
                cells: vec!["a".into(), "b".into()],
                badge: None,
            }],
            ..Model::default()
        });
        assert!(lab.contains("Why this screen exists"));
        assert!(
            !lab.contains("data-select-row"),
            "a screen that owns its body does not render a row list"
        );
        // And it still gets the standard header, so it is a screen like any other.
        assert!(lab.contains("Replaces <code>/portal/tech/lab</code>"));
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
