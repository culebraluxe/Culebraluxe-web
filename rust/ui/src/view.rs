//! The view: a pure function of the model, rendering the class names this application already uses.
//!
//! The nav is generated from `SCREENS`, so a screen cannot exist without appearing in the menu and a menu entry
//! cannot point at a screen that does not exist. Every interpolated value is escaped — this crate renders data from a
//! database and from third-party sources, and a Rust renderer that formats HTML owns that risk.

use crate::model::{home, Model, Row, Surface, PAGE_SIZE, SCREENS};

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

// ---------------------------------------------------------------------------
// THE CONTROL VOCABULARY.
//
// Each function below renders one control and nothing else. Three rules hold for all of them, and they are what make a
// control Rust's rather than the DOM's:
//
//   1. The rendered value IS the model's value: `value="{query}"`, `checked`, `selected` all come from `Controls`. A
//      control therefore never has to be read back out of the DOM to know what it holds.
//   2. Each carries the one attribute the shell turns into a named message (`data-field` on input, `data-select` and
//      `data-toggle` on change, `data-tab`, `data-page`, `data-clear` on click). No control dispatches anything itself.
//   3. No inline styles and no magic numbers: the classes are ones this application already defines, so the stylesheet
//      keeps deciding how it looks.
// ---------------------------------------------------------------------------

/// The screen's search field.
fn search_field(value: &str, placeholder: &str) -> String {
    format!(
        "<div class=\"flex items-center gap-2\">\
           <input type=\"search\" data-field=\"query\" value=\"{value}\" placeholder=\"{placeholder}\" \
             class=\"w-full rounded-md border bg-background px-3 py-2 text-sm text-foreground \
             placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring\" />\
           <button type=\"button\" data-clear=\"query\" class=\"shrink-0 rounded-md border px-2.5 py-2 text-sm \
             text-muted-foreground hover:bg-muted/60\">Clear</button>\
         </div>",
        value = escape(value),
        placeholder = escape(placeholder)
    )
}

/// A dropdown. Options are `(value, label)` pairs and the choice travels by VALUE, never by label, so renaming an
/// option cannot change what a saved choice means.
fn select_field(options: &[(&str, &str)], selected: Option<&str>) -> String {
    let items = options
        .iter()
        .map(|(value, label)| {
            let chosen = if selected == Some(*value) {
                " selected"
            } else {
                ""
            };
            format!(
                "<option value=\"{value}\"{chosen}>{label}</option>",
                value = escape(value),
                label = escape(label)
            )
        })
        .collect::<String>();
    format!(
        "<select data-select=\"filter\" aria-label=\"Filter\" class=\"rounded-md border bg-background px-3 py-2 \
         text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring\">{items}</select>"
    )
}

/// A switch, rendered as a checkbox inside a label so the whole row is the hit target — which is also what gives it a
/// keyboard path for free.
fn switch_field(on: bool, label: &str) -> String {
    format!(
        "<label class=\"flex cursor-pointer items-center gap-2 text-sm text-foreground\">\
           <input type=\"checkbox\" data-toggle=\"true\"{checked} class=\"h-4 w-4 rounded border\" />{label}\
         </label>",
        checked = if on { " checked" } else { "" },
        label = escape(label)
    )
}

/// A row of tabs. The active tab is read from the model, not from a class a click added.
fn tab_row(tabs: &[(&str, &str)], active: Option<&str>) -> String {
    let items = tabs
        .iter()
        .map(|(key, label)| {
            let current = active == Some(*key);
            format!(
                "<button type=\"button\" role=\"tab\" aria-selected=\"{selected}\" data-tab=\"{key}\" \
                 class=\"rounded-md px-3 py-1.5 text-sm {state}\">{label}</button>",
                selected = if current { "true" } else { "false" },
                key = escape(key),
                state = if current {
                    "bg-muted font-medium text-foreground"
                } else {
                    "text-muted-foreground hover:bg-muted/60"
                },
                label = escape(label)
            )
        })
        .collect::<String>();
    format!("<div role=\"tablist\" class=\"flex items-center gap-1\">{items}</div>")
}

/// Paging as two buttons that ask for a DELTA. The reducer clamps, so a button never has to know the last page; the
/// buttons disable at the ends so the pointer says what the reducer would do anyway.
fn page_controls(page: usize, pages: usize) -> String {
    let button = |delta: i64, label: &str, disabled: bool| {
        format!(
            "<button type=\"button\" data-page=\"{delta}\"{disabled} class=\"rounded-md border px-2.5 py-1.5 \
             text-sm {state}\">{label}</button>",
            disabled = if disabled { " disabled" } else { "" },
            state = if disabled {
                "text-muted-foreground/50"
            } else {
                "text-foreground hover:bg-muted/60"
            }
        )
    };
    format!(
        "<div class=\"flex items-center gap-2\">{prev}<span class=\"text-xs text-muted-foreground\">page {shown} of \
         {pages}</span>{next}</div>",
        prev = button(-1, "Previous", page == 0),
        next = button(1, "Next", pages == 0 || page + 1 >= pages),
        shown = page + 1,
        pages = pages.max(1)
    )
}
/// The lab's own data. Deliberately a constant: a lab that fetched rows would be testing the network, and the controls
/// and the reducer are what are under examination here. `kind` is which tab a row belongs to, `status` what the
/// dropdown filters on.
const LAB_ROWS: [(&str, &str, &str, &str); 11] = [
    ("ctl-search", "Search field", "controls", "ready"),
    ("ctl-select", "Dropdown", "controls", "ready"),
    ("ctl-switch", "Switch", "controls", "ready"),
    ("ctl-tabs", "Tabs", "controls", "ready"),
    ("ctl-pager", "Paging", "controls", "ready"),
    ("st-empty", "Empty state", "states", "ready"),
    ("st-error", "Error state", "states", "ready"),
    ("st-loading", "Loading state", "states", "ready"),
    ("lay-grid", "Grid card", "layout", "planned"),
    ("lay-panel", "Side panel", "layout", "planned"),
    ("lay-toolbar", "Toolbar", "layout", "planned"),
];

/// Statuses the dropdown offers. `""` is "no filter" rather than a status, so an unset dropdown filters nothing.
const LAB_STATUSES: [(&str, &str); 3] = [("", "Any status"), ("ready", "Ready"), ("planned", "Planned")];

/// The tabs, and what each one narrows to.
const LAB_TABS: [(&str, &str); 4] = [
    ("all", "All"),
    ("controls", "Controls"),
    ("states", "States"),
    ("layout", "Layout"),
];

/// The Rust Design Lab — this crate's own controls, on the application's own design tokens.
///
/// WHY THERE IS A SECOND LAB. The TypeScript lab at `/portal/design-lab` is a catalogue of React components and stays
/// TypeScript, deliberately: it is about those components, and nothing Rust renders would tell the same story. This
/// one is about the CONTROLS this crate owns, and its job is to make them visible and operable — a text field, a
/// dropdown, a switch, tabs, a pager — each wired through the model, so what is on screen is evidence that a keystroke
/// travels to the reducer and back, rather than an illustration of what it might look like.
///
/// THE PANEL AT THE BOTTOM IS THE POINT. It prints the model's control state. If a control were keeping its own value
/// in the DOM, that panel would disagree with the control above it — which is the failure the model owns this state to
/// prevent.
fn rust_lab(model: &Model) -> String {
    let controls = &model.controls;
    let query = controls.query.trim().to_lowercase();
    let status = controls.filter.as_deref().unwrap_or("");
    let tab = controls.tab.as_deref().unwrap_or("all");

    let matched: Vec<&(&str, &str, &str, &str)> = LAB_ROWS
        .iter()
        .filter(|(id, label, kind, row_status)| {
            (tab == "all" || *kind == tab)
                && (status.is_empty() || *row_status == status)
                && (query.is_empty() || label.to_lowercase().contains(&query) || id.contains(&query))
                // The switch is a real filter, not a decoration: it hides what is not built yet.
                && !(controls.toggled && *row_status == "planned")
        })
        .collect();

    let pages = matched.len().div_ceil(PAGE_SIZE);
    // Clamped here as well as in the reducer: the reducer knows a row count only when the host supplied rows, and this
    // body's list is its own. The two agree because they use the same PAGE_SIZE.
    let page = controls.page.min(pages.saturating_sub(1));
    let slice = matched
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .copied()
        .collect::<Vec<_>>();

    let table = if slice.is_empty() {
        "<p class=\"rounded-md border bg-muted/40 px-3 py-6 text-center text-sm text-muted-foreground\">\
         Nothing matches those controls. That is what an empty state looks like.</p>"
            .to_string()
    } else {
        let rows = slice
            .iter()
            .map(|(id, label, kind, row_status)| {
                format!(
                    "<tr class=\"border-t\">\
                       <td class=\"px-3 py-2 font-mono text-xs text-muted-foreground\">{id}</td>\
                       <td class=\"px-3 py-2 text-sm\">{label}</td>\
                       <td class=\"px-3 py-2 text-xs text-muted-foreground\">{kind}</td>\
                       <td class=\"px-3 py-2\"><span class=\"rounded-full border px-2 py-0.5 text-xs {state}\">\
                         {status}</span></td>\
                     </tr>",
                    id = escape(id),
                    label = escape(label),
                    kind = escape(kind),
                    status = escape(row_status),
                    state = if *row_status == "ready" {
                        "bg-primary/10 text-primary"
                    } else {
                        "text-muted-foreground"
                    }
                )
            })
            .collect::<String>();
        format!(
            "<table class=\"w-full border-collapse text-left\">\
               <thead><tr class=\"text-xs uppercase tracking-wide text-muted-foreground\">\
                 <th class=\"px-3 py-2\">id</th><th class=\"px-3 py-2\">label</th>\
                 <th class=\"px-3 py-2\">tab</th><th class=\"px-3 py-2\">status</th>\
               </tr></thead><tbody>{rows}</tbody></table>"
        )
    };

    let state_row = |name: &str, value: String| {
        format!(
            "<div class=\"flex items-baseline justify-between gap-3 border-t py-1.5\">\
               <dt class=\"text-xs uppercase tracking-wide text-muted-foreground\">{}</dt>\
               <dd class=\"font-mono text-xs\">{}</dd></div>",
            escape(name),
            value
        )
    };

    format!(
        "<div class=\"space-y-4\">\
           <p class=\"max-w-3xl text-sm text-muted-foreground\">\
             The controls this crate renders, wired to the model. Every value below comes from that state, which is why \
             the panel at the bottom cannot disagree with what you see: a keystroke is a named message, the reducer is \
             the only thing that changes the model, and the view is a pure function of it.\
           </p>\
           <div class=\"rounded-lg border bg-card p-4\">\
             <div class=\"flex flex-wrap items-center gap-3\">{search}{select}{switch}</div>\
             <div class=\"mt-3 flex flex-wrap items-center justify-between gap-3\">{tabs}{pager}</div>\
           </div>\
           <div class=\"overflow-hidden rounded-lg border bg-card\">{table}</div>\
           <dl class=\"rounded-lg border bg-card p-4\">{q}{f}{t}{s}{p}</dl>\
         </div>",
        search = search_field(&controls.query, "Filter by label or id…"),
        select = select_field(&LAB_STATUSES, controls.filter.as_deref()),
        switch = switch_field(controls.toggled, "Hide what is not built yet"),
        tabs = tab_row(&LAB_TABS, Some(tab)),
        pager = page_controls(page, pages),
        q = state_row("query", format!("\"{}\"", escape(&controls.query))),
        f = state_row("filter", escape(controls.filter.as_deref().unwrap_or("(none)"))),
        t = state_row("tab", escape(tab)),
        s = state_row("toggled", controls.toggled.to_string()),
        p = state_row("page", format!("{page} of {}", pages.saturating_sub(1).max(0))),
    )
}

/// The WhatsApp contact page: static copy, rendered by Rust.
///
/// A body rather than rows because there is no read model here and never was — the page is a phone number and a link.
/// Porting it means the route stops being TypeScript, which is the point; inventing a "contact" read model so a generic
/// list could render one line would be the wrong shape for the same result.
fn whatsapp_view() -> String {
    "<div class=\"bg-[var(--brand-navy)] px-6 py-20 text-[var(--brand-ivory)] md:px-12\">\
       <section class=\"mx-auto max-w-3xl text-center\">\
         <p class=\"mb-4 text-sm uppercase tracking-[0.3em] text-[var(--brand-gold)]\">Official Business Contact</p>\
         <h1 class=\"font-serif text-4xl font-medium md:text-5xl\">CulebraLuxe WhatsApp</h1>\
         <p class=\"mx-auto mt-6 max-w-xl text-base leading-7 text-[var(--brand-ivory)]/80\">\
           Contact CulebraLuxe through our official WhatsApp Business number.\
         </p>\
         <a href=\"https://wa.me/17876383333\" rel=\"noopener\" \
           class=\"mt-10 inline-flex rounded-full border border-[var(--brand-gold)]/60 px-8 py-4 text-lg tracking-wide \
           transition hover:border-[var(--brand-gold)] hover:text-[var(--brand-gold)]\">+1 (787) 638-3333</a>\
       </section>\
     </div>"
        .to_string()
}

///
/// Everything else in this crate renders rows because that is what a read model is. A lab, a board or a widget host is
/// not a list, and pretending it is produces a screen that looks nothing like the one being ported. So a screen may own
/// its body; the model, the messages, the effects and the shell are unchanged, which is the point of putting this here
/// rather than in the shell.
/// Screens that render markup of their own instead of a generic list of rows.
///
/// Everything else in this crate renders rows because that is what a read model is. A lab, a board or a widget host is
/// not a list, and pretending it is produces a screen that looks nothing like the one being ported. So a screen may own
/// its body; the model, the messages, the effects and the shell are unchanged, which is the point of putting this here
/// rather than in the shell.
fn custom_body(model: &Model) -> Option<String> {
    match model.screen.key {
        "tech-lab" => Some(tech_lab()),
        "rust-lab" => Some(rust_lab(model)),
        "site-whatsapp" => Some(whatsapp_view()),
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
    use crate::model::{Msg, Nav, Screen};

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

    // ---- the Rust lab's controls --------------------------------------------------------------------------------

    /// The contract with the shell. Every control the lab renders must carry the one attribute the shell reads, and a
    /// control that loses its attribute goes deaf without failing anything else — which is exactly the kind of bug that
    /// survives a green build and a happy click-through.
    #[test]
    fn every_control_the_lab_renders_carries_its_intent_attribute() {
        let html = render(&Model {
            screen: target("rust-lab"),
            ..Model::default()
        });
        for attribute in [
            "data-field=\"query\"",
            "data-select=\"filter\"",
            "data-toggle=\"true\"",
            "data-tab=",
            "data-page=",
            "data-clear=\"query\"",
        ] {
            assert!(
                html.contains(attribute),
                "the lab renders no control carrying {attribute}, so that control cannot reach the reducer"
            );
        }
    }

    #[test]
    fn the_lab_filters_on_what_the_model_holds() {
        let mut model = Model {
            screen: target("rust-lab"),
            ..Model::default()
        };
        let before = render(&model);
        assert!(before.contains("Search field") && before.contains("Side panel"));

        crate::update::update(&mut model, Msg::QueryChanged("dropdown".into()));
        let after = render(&model);
        assert!(after.contains("Dropdown"), "the match must survive");
        assert!(
            !after.contains("Side panel"),
            "a row that does not match must not be rendered"
        );
    }

    #[test]
    fn the_lab_prints_the_state_its_controls_act_on() {
        let mut model = Model {
            screen: target("rust-lab"),
            ..Model::default()
        };
        crate::update::update(&mut model, Msg::Toggled(true));
        crate::update::update(&mut model, Msg::TabSelected("layout".into()));
        let html = render(&model);
        // The switch is a filter, not a decoration: `layout` is all-planned, so switching it on empties the table.
        assert!(
            html.contains("Nothing matches those controls"),
            "the switch must hide what is not built yet"
        );
        assert!(
            html.contains(">true<"),
            "the panel must show the state the controls are acting on, not a copy of it"
        );
    }
}
