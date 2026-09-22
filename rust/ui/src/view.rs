//! The view: a pure function of the model, rendering the class names this application already uses.
//!
//! The nav is generated from `SCREENS`, so a screen cannot exist without appearing in the menu and a menu entry
//! cannot point at a screen that does not exist. Every interpolated value is escaped — this crate renders data from a
//! database and from third-party sources, and a Rust renderer that formats HTML owns that risk.

use crate::model::{home, screen, Block, Listing, Model, Row, Surface, PAGE_SIZE, SCREENS};

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

/// Render the whole screen: the chrome its surface calls for, plus the current screen's body.
///
/// TWO CHROMES, ONE PER KIND OF SURFACE. The public site is a website: a header across the top, content at full width,
/// a footer. The portal is an application: a fixed left rail of sections. Rendering the portal rail on the public site —
/// which is what this did — makes the marketing pages look like a control panel, which is a worse failure than a plain
/// one: the shape of the page is the first thing a visitor reads.
pub fn render(model: &Model) -> String {
    if model.screen.surface == Surface::Site {
        return format!(
            "<div class=\"flex min-h-screen flex-col bg-background text-foreground\" \
             data-rust-screen=\"{}\">{}{}{}</div>",
            escape(model.screen.key),
            site_header(model),
            format!(
                "<main class=\"min-w-0 flex-1\">{}{}{}</main>",
                site_error_banner(model),
                loading_banner(model),
                body(model)
            ),
            site_footer()
        );
    }
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

/// The public site's header, ported from `components/site-header.tsx`.
///
/// THE FIRST VERSION WAS MINE, NOT THE DESIGN'S, and it showed: a light bar with a text wordmark and a menu of registry
/// titles. The real header is fixed, navy with a gold hairline, carries the logo image, and its menu is the seven site
/// links plus Favorites and Portal — no "Home" (the logo is home) and Portal always present. Written from the component
/// this time, class for class.
///
/// THE MOBILE MENU IS A `<details>`. The component used React state and a hamburger that animates into an X; a details
/// element gives the same open/close behaviour with no JavaScript and no island, and it degrades to a working menu if
/// the stylesheet never arrives. The animated X is not reproduced — the summary is the same three rules, static, and
/// that is a deliberate simplification rather than an oversight.
fn site_header(model: &Model) -> String {
    let _ = model;
    const CAPSULE: &str = "top-nav-capsule top-nav-capsule--tight";
    const MOBILE_CAPSULE: &str = "top-nav-capsule top-nav-capsule--full";
    // The site's menu, as the component lists it: these are the labels and the order the live header uses. Favorites is
    // deliberately not among them: the user's call — it does nothing yet (there is no favorites model behind it), and a
    // nav item that leads to an empty promise is worse than no nav item. The screen stays in the registry, so the page
    // is not lost when the feature is.
    const LINKS: [(&str, &str); 7] = [
        ("Buyers", "site-buyers"),
        ("Sellers", "site-sellers"),
        ("Services", "site-services"),
        ("Guide", "site-guide"),
        ("About", "site-about"),
        ("FAQ", "site-faq"),
        ("Contact", "site-contact"),
    ];
    // A link is a link: real hrefs, so the address bar, the back button and a bookmark all behave. The Rust UI
    // navigates on the paths in the registry rather than intercepting clicks, which is also what lets the same markup
    // be served by a Rust document route later without changing the page.
    let href_of = |key: &str| screen(key).map(|screen| screen.path).unwrap_or("/");
    let desktop = LINKS
        .iter()
        .map(|(label, key)| {
            format!(
                "<a href=\"{href}\" class=\"{CAPSULE}\">{label}</a>",
                href = escape(href_of(key)),
                label = escape(label)
            )
        })
        .collect::<String>();
    let mobile = LINKS
        .iter()
        .map(|(label, key)| {
            format!(
                "<a href=\"{href}\" class=\"{MOBILE_CAPSULE}\">{label}</a>",
                href = escape(href_of(key)),
                label = escape(label)
            )
        })
        .collect::<String>();
    format!(
        "<header class=\"fixed inset-x-0 top-0 z-50 border-b border-brand-gold/15 bg-brand-navy py-6\">\
           <div class=\"mx-auto flex max-w-[1600px] items-center justify-between px-6 md:px-12\">\
             <a href=\"/\" aria-label=\"CulebraLuxe home\" class=\"flex h-7 w-[250px] flex-none items-center\">\
               <img src=\"/images/culebraluxe-header-logo-test.png\" alt=\"CulebraLuxe\" width=\"2050\" \
                 height=\"300\" class=\"h-9 max-h-9 w-auto max-w-full flex-none object-contain\" />\
             </a>\
             <nav class=\"hidden items-center gap-1 lg:flex\" aria-label=\"Primary\">{desktop}\
               <a href=\"/portal/dashboard\" class=\"{CAPSULE}\">Portal</a>\
             </nav>\
             <details class=\"lg:hidden\">\
               <summary class=\"flex cursor-pointer list-none flex-col items-end gap-1.5 text-brand-ivory\" \
                 aria-label=\"Menu\">\
                 <span class=\"block h-px w-6 bg-current\"></span>\
                 <span class=\"block h-px w-6 bg-current\"></span>\
                 <span class=\"block h-px w-6 bg-current\"></span>\
               </summary>\
               <nav class=\"mt-4 flex flex-col gap-2 border-t border-brand-gold/25 pt-4\" aria-label=\"Mobile\">\
                 {mobile}<a href=\"/portal/dashboard\" class=\"{MOBILE_CAPSULE}\">Portal</a>\
               </nav>\
             </details>\
           </div>\
         </header>\
         <div class=\"h-[76px] lg:h-[92px]\" aria-hidden=\"true\"></div>"
    )
}

/// The error banner on the public site, with the same escaping and the same wording as the portal's.
fn site_error_banner(model: &Model) -> String {
    match model.error.as_deref() {
        Some(message) => format!(
            "<div class=\"border-b border-destructive/40 bg-destructive/10 px-6 py-3 text-sm\" role=\"alert\">\
             Could not load: {}</div>",
            escape(message)
        ),
        None => String::new(),
    }
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

/// The public homepage, ported from `app/page.tsx` and the components it composes.
///
/// THIS IS A REPRODUCTION, NOT AN INTERPRETATION. Every class below is one the TypeScript components use, in the same
/// structure: the hero is a full-bleed image with two gradient scrims and its copy pinned to the bottom; the services
/// block is a dark primary section with buyers over sellers; culture is a full-bleed image with an editorial column and
/// a row of stats under it. Nothing is positioned with an inline style, and no value is invented — where the page used
/// the block's own content, so does this.
///
/// A PAGE, NOT A LIST. Its data arrives as blocks (`PageContent`), which is the whole reason the flip was wrong before:
/// rendered from rows, this page was a heading and three lines of text with none of the design that makes it the
/// homepage.
fn site_home(model: &Model) -> String {
    let Some(page) = model.page.as_ref() else {
        // The chrome and the loading line are already on screen; an empty body here is honest, and filling it with
        // invented copy would be worse than a page that is still arriving.
        return String::new();
    };
    let mut out = String::new();
    out.push_str(&hero(&page.hero));
    // The live page put the property grids between the hero and the services band, and only when there were listings
    // to show — a portfolio section with nothing in it is worse than no portfolio section.
    if !page.listings.is_empty() {
        out.push_str(&featured_properties(&page.featured));
        out.push_str(&home_properties(&page.buyers, &page.listings));
    }
    out.push_str(&services(&page.buyers, &page.sellers));
    out.push_str(&culture(&page.culture));
    out.push_str(&about_section(&page.about));
    out.push_str(&site_footer());
    out
}

/// `components/hero.tsx` — a full-viewport image, two scrims, and the block's copy pinned to the bottom edge.
fn hero(block: &Block) -> String {
    let image = block.image_path.as_deref().unwrap_or("/images/hero-villa.png");
    let alt = block
        .image_alt
        .as_deref()
        .unwrap_or("Cliffside modern villa overlooking the turquoise Caribbean sea in Culebra");
    format!(
        "<section id=\"top\" class=\"relative h-[100svh] w-full overflow-hidden\">\
           <img src=\"{image}\" alt=\"{alt}\" sizes=\"100vw\" \
             class=\"absolute inset-0 h-full w-full object-cover\" />\
           <div class=\"absolute inset-0 bg-gradient-to-b from-black/40 via-black/10 to-black/50\"></div>\
           <div class=\"absolute inset-0 bg-gradient-to-t from-black/40 to-transparent\"></div>\
           <div class=\"relative flex h-full flex-col justify-end px-6 pb-20 md:px-12 md:pb-28\">\
             <div class=\"mx-auto w-full max-w-[1600px]\">\
               <p class=\"mb-6 text-xs font-light uppercase tracking-[0.4em] text-background/70\">{eyebrow}</p>\
               <h1 class=\"max-w-4xl text-balance font-serif text-5xl font-light leading-[1.02] text-background \
                 md:text-7xl lg:text-8xl\">{title}</h1>\
               <div class=\"mt-10 flex flex-col gap-6 border-t border-background/25 pt-8 md:flex-row md:items-end \
                 md:justify-between\">\
                 <p class=\"max-w-md text-pretty text-sm font-light leading-relaxed text-background/80\">{body}</p>\
                 {cta}\
               </div>\
             </div>\
           </div>\
         </section>",
        image = escape(image),
        alt = escape(alt),
        eyebrow = escape(&block.eyebrow),
        title = escape(&block.title),
        body = escape(&block.body),
        cta = rule_cta(block, "#properties", "bg-background"),
    )
}

/// `components/services.tsx` — the dark primary block: buyers over sellers, buyers as copy plus a ruled list, sellers
/// as a portrait image beside their copy.
///
/// THE ITEMS ARE TYPED, NOT POSITIONAL. The buyers' list comes from the block's `items`, filtered to the ones whose key
/// says `list` — the same filter the TSX used. Rendering every item regardless would mix stats into a services list the
/// moment someone added one to the slot.
fn services(buyers: &Block, sellers: &Block) -> String {
    let buyer_items = block_items(buyers, "list");
    format!(
        "<div class=\"bg-primary text-primary-foreground\">\
           <section id=\"buyers\" class=\"border-b border-primary-foreground/10 px-6 py-28 md:px-12 md:py-40\">\
             <div class=\"mx-auto grid max-w-[1600px] gap-14 md:grid-cols-2 md:gap-24\">\
               <div>\
                 <p class=\"mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50\">{b_eyebrow}</p>\
                 <h2 class=\"text-balance font-serif text-4xl font-light leading-[1.06] md:text-5xl\">{b_title}</h2>\
               </div>\
               <div class=\"flex flex-col justify-center gap-10\">\
                 <p class=\"max-w-md text-pretty text-sm font-light leading-relaxed text-primary-foreground/75\">{b_body}</p>\
                 <ul class=\"flex flex-col divide-y divide-primary-foreground/10 border-y border-primary-foreground/10\">{b_items}</ul>\
                 {b_cta}\
               </div>\
             </div>\
           </section>\
           <section id=\"sellers\" class=\"px-6 py-28 md:px-12 md:py-40\">\
             <div class=\"mx-auto grid max-w-[1600px] items-center gap-14 md:grid-cols-2 md:gap-24\">\
               <div class=\"relative aspect-[4/5] w-full overflow-hidden\">\
                 <img src=\"{s_image}\" alt=\"{s_alt}\" sizes=\"(min-width: 768px) 50vw, 100vw\" \
                   class=\"absolute inset-0 h-full w-full object-cover\" />\
               </div>\
               <div class=\"flex flex-col gap-10\">\
                 <div>\
                   <p class=\"mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50\">{s_eyebrow}</p>\
                   <h2 class=\"text-balance font-serif text-4xl font-light leading-[1.06] md:text-5xl\">{s_title}</h2>\
                 </div>\
                 <p class=\"max-w-md text-pretty text-sm font-light leading-relaxed text-primary-foreground/75\">{s_body}</p>\
                 {s_cta}\
               </div>\
             </div>\
           </section>\
         </div>",
        b_eyebrow = escape(&buyers.eyebrow),
        b_title = escape(&buyers.title),
        b_body = escape(&buyers.body),
        b_items = buyer_items,
        b_cta = rule_cta(buyers, "#contact", "bg-primary-foreground"),
        s_image = escape(
            sellers
                .image_path
                .as_deref()
                .unwrap_or("/images/coastline.png")
        ),
        s_alt = escape(sellers.image_alt.as_deref().unwrap_or(
            "Aerial view of the Culebra coastline with jade and turquoise water"
        )),
        s_eyebrow = escape(&sellers.eyebrow),
        s_title = escape(&sellers.title),
        s_body = escape(&sellers.body),
        s_cta = rule_cta(sellers, "#contact", "bg-primary-foreground"),
    )
}

/// The items of one kind, so a list renders as a list and a stat as a stat rather than everything as everything.
fn block_items(block: &Block, key: &str) -> String {
    block
        .items
        .iter()
        .filter(|item| item.key == key)
        .filter_map(|item| item.value.as_deref())
        .map(|value| {
            format!(
                "<li class=\"py-5 text-sm font-light tracking-wide text-primary-foreground/85\">{}</li>",
                escape(value)
            )
        })
        .collect::<String>()
}

/// `components/culture.tsx` — a full-bleed image with the title over it, then an editorial column: the subtitle in the
/// accent colour, the body in display serif, and a row of stats under a rule.
fn culture(block: &Block) -> String {
    let stats = block
        .items
        .iter()
        .filter(|item| item.key == "stat")
        .map(|item| {
            format!(
                "<div><p class=\"font-serif text-xl font-light text-foreground\">{label}</p>\
                   <p class=\"mt-2 text-sm font-light leading-relaxed text-muted-foreground\">{value}</p></div>",
                label = escape(item.label.as_deref().unwrap_or("")),
                value = escape(item.value.as_deref().unwrap_or(""))
            )
        })
        .collect::<String>();
    format!(
        "<section id=\"culture\" class=\"relative\">\
           <div class=\"relative h-[85svh] w-full overflow-hidden\">\
             <img src=\"{image}\" alt=\"{alt}\" sizes=\"100vw\" \
               class=\"absolute inset-0 h-full w-full object-cover\" />\
             <div class=\"absolute inset-0 bg-gradient-to-t from-black/55 via-black/15 to-black/25\"></div>\
             <div class=\"absolute inset-0 flex flex-col justify-end px-6 pb-20 md:px-12 md:pb-28\">\
               <div class=\"mx-auto w-full max-w-[1600px]\">\
                 <p class=\"mb-5 text-xs font-light uppercase tracking-[0.4em] text-background/70\">{eyebrow}</p>\
                 <h2 class=\"max-w-3xl text-balance font-serif text-4xl font-light leading-[1.05] text-background \
                   md:text-6xl\">{title}</h2>\
               </div>\
             </div>\
           </div>\
           <div class=\"px-6 py-24 md:px-12 md:py-32\">\
             <div class=\"mx-auto grid max-w-[1600px] gap-14 md:grid-cols-12 md:gap-24\">\
               <p class=\"text-xs font-light uppercase tracking-[0.28em] text-accent md:col-span-4\">{subtitle}</p>\
               <div class=\"md:col-span-8\">\
                 <p class=\"max-w-3xl text-balance font-serif text-2xl font-light leading-[1.4] text-foreground \
                   md:text-3xl\">{body}</p>\
                 <div class=\"mt-14 grid gap-10 border-t border-border pt-10 sm:grid-cols-3\">{stats}</div>\
               </div>\
             </div>\
           </div>\
         </section>",
        image = escape(block.image_path.as_deref().unwrap_or("/images/culture.png")),
        alt = escape(block.image_alt.as_deref().unwrap_or(
            "The white sand crescent and clear turquoise water of Flamenco Beach, Culebra"
        )),
        eyebrow = escape(&block.eyebrow),
        title = escape(&block.title),
        subtitle = escape(&block.subtitle),
        body = escape(&block.body),
        stats = stats,
    )
}

/// `components/featured-properties.tsx` — "The Collection": three estates, each an image beside its numeral, name,
/// location, facts and price, alternating sides down the page.
///
/// THE SIDES ALTERNATE BY INDEX, the way the component did it: `md:[direction:rtl]` on every second article, with the
/// inner columns set back to `ltr`. Reproduced rather than simplified to a single side, because the alternation is
/// most of what the section looks like.
fn featured_properties(items: &[Listing]) -> String {
    let heading = "<div class=\"mb-20 md:mb-28\">\
         <div class=\"flex flex-col gap-6 border-b border-border pb-10 md:flex-row md:items-end md:justify-between\">\
           <div>\
             <p class=\"mb-4 text-xs font-light uppercase tracking-[0.34em] text-accent\">The Collection</p>\
             <h2 class=\"max-w-2xl text-balance font-serif text-4xl font-light leading-[1.05] text-foreground md:text-6xl\">\
               Residences chosen for their silence.</h2>\
           </div>\
           <p class=\"max-w-xs text-pretty text-sm font-light leading-relaxed text-muted-foreground\">\
             Each estate is selected in person, for its light, its outlook, and its relationship to the sea.</p>\
         </div>\
       </div>";
    // Three, as the component's default limit. A different number is a decision for whoever asks for one.
    let shown = &items[..items.len().min(3)];
    let body = if shown.is_empty() {
        "<p class=\"max-w-xl text-sm font-light leading-relaxed text-muted-foreground\">\
         The next collection is being prepared.</p>"
            .to_string()
    } else {
        shown
            .iter()
            .enumerate()
            .map(|(index, listing)| {
                let reversed = if index % 2 == 1 {
                    " md:[direction:rtl]"
                } else {
                    ""
                };
                format!(
                    "<article class=\"grid items-center gap-10 md:grid-cols-12 md:gap-16{reversed}\">\
                       <div class=\"md:col-span-8 md:[direction:ltr]\">\
                         <a href=\"/properties/{slug}\" aria-label=\"View {name}\" \
                           class=\"group relative block aspect-[16/10] w-full overflow-hidden\">\
                           <img src=\"{image}\" alt=\"{alt}\" sizes=\"(min-width: 768px) 66vw, 100vw\" \
                             class=\"absolute inset-0 h-full w-full object-cover transition-transform duration-[1600ms] \
                             ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:scale-[1.04]\" />\
                         </a>\
                       </div>\
                       <div class=\"md:col-span-4 md:[direction:ltr]\">\
                         <span class=\"font-serif text-sm font-light text-accent\">({numeral})</span>\
                         <h3 class=\"mt-4 font-serif text-3xl font-light leading-tight text-foreground md:text-4xl\">\
                           <a href=\"/properties/{slug}\" \
                             class=\"transition-colors duration-300 hover:text-accent\">{name}</a></h3>\
                         {location}\
                         <p class=\"mt-8 max-w-xs text-sm font-light leading-relaxed text-foreground/80\">{facts}</p>\
                         <div class=\"mt-8 flex items-center justify-between border-t border-border pt-6\">\
                           <span class=\"text-xs font-light uppercase tracking-[0.2em] text-muted-foreground\">{price}</span>\
                           <a href=\"#contact\" \
                             class=\"inline-flex items-center gap-2 text-xs font-light uppercase tracking-[0.2em] \
                             text-foreground\">Enquire<span class=\"inline-block h-px w-6 bg-foreground\"></span></a>\
                         </div>\
                       </div>\
                     </article>",
                    reversed = reversed,
                    slug = escape(&listing.slug),
                    name = escape(&listing.name),
                    image = escape(listing.image_path.as_deref().unwrap_or("/placeholder.svg")),
                    alt = escape(listing.image_alt.as_deref().unwrap_or(&listing.name)),
                    numeral = format!("{:02}", index + 1),
                    location = match listing.location.as_deref().filter(|value| !value.is_empty()) {
                        Some(location) => format!(
                            "<p class=\"mt-3 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground\">{}</p>",
                            escape(location)
                        ),
                        None => String::new(),
                    },
                    facts = escape(&listing_facts(listing)),
                    price = escape(listing.price.as_deref().unwrap_or("Price upon request")),
                )
            })
            .collect::<String>()
    };
    format!(
        "<section id=\"properties\" class=\"px-6 py-28 md:px-12 md:py-40\">\
           <div class=\"mx-auto max-w-[1600px]\">{heading}\
             <div class=\"flex flex-col gap-28 md:gap-40\">{body}</div>\
           </div>\
         </section>"
    )
}

/// The one-line facts under an estate's name: beds and baths, or the lot size for land, joined the way the TypeScript
/// `propertyFacts` joined them. Missing numbers are omitted rather than printed as zero, and a whole number prints
/// without a decimal point — "8 Bed", not "8.0 Bed" — while a half-bath keeps its half.
fn listing_facts(listing: &Listing) -> String {
    let count = |value: f64| -> String {
        if value.fract() == 0.0 {
            format!("{}", value as i64)
        } else {
            format!("{value}")
        }
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(beds) = listing.beds {
        parts.push(format!("{} Bed", count(beds)));
    }
    if let Some(baths) = listing.baths {
        parts.push(format!("{} Bath", count(baths)));
    }
    if let Some(area) = listing.area.as_deref().filter(|value| !value.is_empty()) {
        parts.push(area.to_string());
    }
    parts.join("  ·  ")
}

/// `components/home-properties.tsx` — the dark portfolio band: a heading and a "View All Properties" button, then four
/// cards in a row, each an image, a name, a price and a facts line.
///
/// THE SAVE CONTROL IS NOT HERE YET. The TypeScript card carries a `SaveProperty` island in its top-right corner, and
/// saving is a session-backed action with its own state — so it arrives with the island work rather than as a button
/// that looks like it saves and does not. The Featured badge is here, because that one is just a flag on the card.
fn home_properties(block: &Block, items: &[Listing]) -> String {
    let shown = &items[..items.len().min(4)];
    if shown.is_empty() {
        // The component rendered nothing at all with no listings, rather than an empty band.
        return String::new();
    }
    let cards = shown
        .iter()
        .map(|listing| {
            let badge = if listing.featured {
                "<span class=\"absolute left-3 top-3 bg-background/90 px-3 py-1 text-[10px] font-light uppercase \
                 tracking-[0.18em] text-foreground\">Featured</span>"
            } else {
                ""
            };
            format!(
                "<article class=\"group flex flex-col\">\
                   <div class=\"relative aspect-[5/4] w-full overflow-hidden bg-background/10\">\
                     <a href=\"/properties/{slug}\" aria-label=\"{name}\">\
                       <img src=\"{image}\" alt=\"{name}\" \
                         sizes=\"(min-width: 1024px) 22vw, (min-width: 640px) 45vw, 90vw\" \
                         class=\"absolute inset-0 h-full w-full object-cover transition-transform duration-[1400ms] \
                         ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:scale-[1.05]\" />\
                     </a>{badge}\
                   </div>\
                   <div class=\"mt-5 flex items-baseline justify-between gap-4\">\
                     <a href=\"/properties/{slug}\" \
                       class=\"font-serif text-xl font-light transition-colors duration-300 hover:text-background/70\">{name}</a>\
                     <span class=\"whitespace-nowrap text-sm font-light text-background/80\">{price}</span>\
                   </div>\
                   <p class=\"mt-2 text-[11px] font-light uppercase tracking-[0.16em] text-background/55\">{facts}</p>\
                 </article>",
                slug = escape(&listing.slug),
                name = escape(&listing.name),
                image = escape(listing.image_path.as_deref().unwrap_or("/placeholder.svg")),
                badge = badge,
                price = escape(listing.price.as_deref().unwrap_or("Price upon request")),
                facts = escape(&listing_facts(listing)),
            )
        })
        .collect::<String>();
    let eyebrow = if block.eyebrow.is_empty() {
        "The Portfolio"
    } else {
        block.eyebrow.as_str()
    };
    let title = if block.title.is_empty() {
        "Find your place in Culebra."
    } else {
        block.title.as_str()
    };
    let intro = if block.body.is_empty() {
        "Exquisite properties on an extraordinary island — each chosen for its light, its outlook, and its \
         relationship to the sea."
    } else {
        block.body.as_str()
    };
    format!(
        "<section class=\"bg-foreground px-6 py-24 text-background md:px-12 md:py-32\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"mb-14 flex flex-col gap-8 md:flex-row md:items-end md:justify-between\">\
               <div class=\"max-w-xl\">\
                 <p class=\"mb-4 text-xs font-light uppercase tracking-[0.34em] text-background/60\">{eyebrow}</p>\
                 <h2 class=\"text-balance font-serif text-4xl font-light leading-[1.05] md:text-5xl\">{title}</h2>\
                 <p class=\"mt-5 max-w-md text-pretty text-sm font-light leading-relaxed text-background/70\">{intro}</p>\
               </div>\
               <a href=\"#properties\" class=\"inline-flex items-center gap-3 self-start border border-background/30 \
                 px-8 py-4 text-xs font-light uppercase tracking-[0.2em] transition-colors duration-500 \
                 hover:border-background md:self-auto\">View All Properties<span aria-hidden=\"true\">&rarr;</span></a>\
             </div>\
             <div class=\"grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4\">{cards}</div>\
           </div>\
         </section>",
        eyebrow = escape(eyebrow),
        title = escape(title),
        intro = escape(intro),
        cards = cards,
    )
}

/// `components/about.tsx` — the eyebrow, the display-serif statement, and three columns: the block's body, its first
/// paragraph, and a two-by-two of its stats.
///
/// THE THIRD COLUMN IS THE STATS, and the middle one is the block's first `paragraph` item — that is the component's
/// structure, not a simplification: the stats sit beside the prose rather than under it.
fn about_section(block: &Block) -> String {
    let first_paragraph = block
        .items
        .iter()
        .filter(|item| item.key == "paragraph")
        .filter_map(|item| item.value.as_deref())
        .next()
        .unwrap_or("");
    let stats = block
        .items
        .iter()
        .filter(|item| item.key == "stat")
        .map(|item| {
            format!(
                "<div><p class=\"font-serif text-4xl font-light text-foreground\">{label}</p>\
                   <p class=\"mt-2 text-xs font-light uppercase tracking-[0.2em] text-muted-foreground\">{value}</p></div>",
                label = escape(item.label.as_deref().unwrap_or("")),
                value = escape(item.value.as_deref().unwrap_or(""))
            )
        })
        .collect::<String>();
    format!(
        "<section id=\"about\" class=\"px-6 py-28 md:px-12 md:py-40\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <p class=\"mb-16 text-xs font-light uppercase tracking-[0.34em] text-accent md:mb-24\">{eyebrow}</p>\
             <h2 class=\"max-w-5xl text-balance font-serif text-3xl font-light leading-[1.2] text-foreground \
               md:text-5xl md:leading-[1.18]\">{title}</h2>\
             <div class=\"mt-20 grid gap-14 border-t border-border pt-16 md:mt-28 md:grid-cols-3 md:gap-16\">\
               <p class=\"max-w-sm text-sm font-light leading-relaxed text-muted-foreground\">{body}</p>\
               <p class=\"max-w-sm text-sm font-light leading-relaxed text-muted-foreground\">{paragraph}</p>\
               <div class=\"flex flex-col justify-between gap-8\">\
                 <div class=\"grid grid-cols-2 gap-8\">{stats}</div>\
               </div>\
             </div>\
           </div>\
         </section>",
        eyebrow = escape(&block.eyebrow),
        title = escape(&block.title),
        body = escape(&block.body),
        paragraph = escape(first_paragraph),
        stats = stats,
    )
}

/// The items of one kind rendered as paragraphs. Kept for the sections that need more than the first one.
/// The items of one kind rendered as paragraphs. Kept for the sections that need more than the first one.
///
/// The year is the host's, not the page's: a page rendered once and cached would carry last year's date forever, which
/// is the kind of detail nobody notices until January.
fn site_footer() -> String {
    let links = [
        ("Buyers", "/buyers"),
        ("Sellers", "/sellers"),
        ("Services", "/services"),
        ("Guide", "/guide"),
        ("About", "/about"),
        ("FAQ", "/faq"),
        ("Contact", "/contact"),
    ]
    .iter()
    .map(|(label, href)| {
        format!(
            "<a href=\"{href}\" class=\"text-xs font-light uppercase tracking-[0.2em] text-muted-foreground \
             transition-colors hover:text-foreground\">{label}</a>",
            href = escape(href),
            label = escape(label)
        )
    })
    .collect::<String>();
    format!(
        "<footer class=\"border-t border-border px-6 py-16 md:px-12\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"flex flex-col gap-12 md:flex-row md:items-end md:justify-between\">\
               <div>\
                 <p class=\"font-serif text-lg font-normal uppercase tracking-[0.35em] text-foreground\">CulebraLuxe</p>\
                 <p class=\"mt-4 max-w-xs text-sm font-light leading-relaxed text-muted-foreground\">\
                   Architectural estates and beachfront residences on the island of Culebra, Puerto Rico.</p>\
               </div>\
               <nav class=\"flex flex-wrap gap-x-8 gap-y-3\" aria-label=\"Footer\">{links}</nav>\
             </div>\
             <div class=\"mt-14 flex flex-col gap-3 border-t border-border pt-8 text-xs font-light uppercase \
               tracking-[0.16em] text-muted-foreground md:flex-row md:justify-between\">\
               <p>&copy; CulebraLuxe. All rights reserved.</p>\
               <p>Culebra &middot; Puerto Rico</p>\
             </div>\
           </div>\
         </footer>"
    )
}

/// The call to action whose rule takes a colour, because the hero draws it in the page background and the dark sections
/// draw it in the primary foreground. One function with the colour passed in, rather than two that drift.
fn rule_cta(block: &Block, fallback: &str, rule: &str) -> String {
    let Some(label) = block.cta_label.as_deref().filter(|label| !label.is_empty()) else {
        return String::new();
    };
    format!(
        "<a href=\"{href}\" class=\"inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]\">\
         {label}<span class=\"inline-block h-px w-10 {rule}\"></span></a>",
        href = escape(block.cta_href.as_deref().unwrap_or(fallback)),
        label = escape(label),
        rule = rule
    )
}

///
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

/// The site-privacy page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. `kind` is the tag it had, so the render below can put
/// it back in the same shape.
const PRIVACY_VIEW_CONTENT: [(&str, &str); 29] = [
    ("p", "CulebraLuxe LLC"),
    ("h1", "Privacy Policy"),
    ("p", "Last updated: September 1, 2026"),
    ("h2", "Overview"),
    ("p", "CulebraLuxe LLC respects your privacy. This Privacy Policy explains how we collect, use, store, and protect information when you use our website, communicate with us, or interact with services and integrations operated by CulebraLuxe, including WhatsApp and Meta services."),
    ("h2", "Information we collect"),
    ("p", "We may collect information you provide directly to us, such as your name, email address, phone number, property information, communication preferences, and other information you choose to provide when contacting CulebraLuxe or using our services."),
    ("p", "When you communicate with CulebraLuxe through WhatsApp or other messaging services, we may receive information associated with those communications, including identifiers, phone numbers, timestamps, message status information, and message content when required to provide the requested communication service."),
    ("h2", "Meta and WhatsApp integrations"),
    ("p", "CulebraLuxe may use Meta Platforms, Inc. services, including Facebook Login for Business, the WhatsApp Business Platform, and WhatsApp Business App coexistence features. When you authorize or interact with these services, Meta may provide CulebraLuxe with information necessary to operate the integration, such as business account identifiers, WhatsApp Business Account information, phone number identifiers, access authorization information, webhook events, and messaging data associated with CulebraLuxe communications."),
    ("p", "We use this information only to operate CulebraLuxe business communications, maintain our customer and relationship records, provide requested services, troubleshoot integrations, and comply with applicable legal or platform requirements."),
    ("h2", "How we use information"),
    ("p", "We may use collected information to:"),
    ("li", "respond to inquiries and communicate with clients and prospective clients;"),
    ("li", "provide real estate brokerage and related services;"),
    ("li", "maintain client, property, transaction, and relationship records;"),
    ("li", "operate and improve our website, internal systems, and messaging integrations;"),
    ("li", "protect against fraud, misuse, security incidents, or unauthorized access; and"),
    ("li", "comply with legal, regulatory, contractual, and platform obligations."),
    ("h2", "Sharing of information"),
    ("p", "We do not sell personal information. We may share information with service providers and technology platforms only as needed to operate CulebraLuxe services, including hosting, communications, document, authentication, and messaging services. We may also disclose information when required by law or when reasonably necessary to protect CulebraLuxe, our clients, or others."),
    ("h2", "Data retention and security"),
    ("p", "We retain information only for as long as reasonably necessary for the purposes described in this policy, for legitimate business and recordkeeping needs, and as required by law. We use reasonable administrative, technical, and organizational safeguards designed to protect information from unauthorized access, loss, misuse, or disclosure."),
    ("h2", "Your choices and requests"),
    ("p", "You may contact us to ask about personal information associated with you, request a correction, or request deletion where applicable. Some information may be retained when required for legal, regulatory, transaction-record, security, or legitimate business purposes."),
    ("h2", "Third-party services"),
    ("p", "Our services may interact with third-party platforms such as Meta and WhatsApp. Those services operate under their own privacy policies and terms. CulebraLuxe is not responsible for the privacy practices of third-party services except for our own collection and use of information received through them."),
    ("h2", "Contact us"),
    ("p", "Questions or privacy requests may be sent to CulebraLuxe LLC through our public contact page at https://www.culebraluxe.com/contact."),
];

/// The static page, rendered from that text. No read model and no fetch: a static page is content, and content does not
/// belong in a database query.
fn privacy_view() -> String {
    let body = PRIVACY_VIEW_CONTENT
        .iter()
        .map(|(kind, text)| match *kind {
            "h1" | "h2" | "h3" => format!(
                "<h2 class=\"font-serif text-2xl font-light text-foreground\">{}</h2>",
                escape(text)
            ),
            "li" => format!("<li class=\"ml-6 list-disc\">{}</li>", escape(text)),
            _ => format!(
                "<p class=\"mt-4 text-sm font-light leading-7 text-muted-foreground\">{}</p>",
                escape(text)
            ),
        })
        .collect::<String>();
    format!(
        "<article class=\"px-6 py-20 md:px-12 md:py-28\"><div class=\"mx-auto max-w-4xl space-y-6\">{body}</div></article>"
    )
}


/// The site-services page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. `kind` is the tag it had, so the render below can put
/// it back in the same shape.

/// `app/services/page.tsx` — the Services page, which is `components/services.tsx`.
///
/// THE BODY ALREADY EXISTED, and reusing it is the point: `services()` above renders exactly these two blocks, ported
/// from the same component for the homepage, so the page and the homepage's summary of it are one renderer and cannot
/// drift apart. The homepage shows the buyers and sellers blocks as a band between the listings and the culture section;
/// this page is where they are the page.
///
/// WHAT WAS HERE INSTEAD was a hardcoded article of hand-typed paragraphs, on the stated theory that "a static page is
/// content, and content does not belong in a database query". That is the opposite of how this site is built: the copy
/// is managed content in Neon, addressed by slot, and the live page read it from there. Retyping it into Rust would have
/// created a second copy that silently diverges from the one the client edits — and the deleted text had already drifted
/// from the slot it claimed to reproduce.
fn site_services(model: &Model) -> String {
    let Some(page) = model.page.as_ref() else {
        // The chrome and the loading line are already on screen; an empty body is honest here and invented copy is not.
        return String::new();
    };
    format!(
        "{}{}",
        services(&page.buyers, &page.sellers),
        site_footer()
    )
}

/// `app/about/page.tsx` — the About page, which is `components/about.tsx`.
///
/// The same shape as the Services page above, for the same reason: `about_section()` is the renderer the homepage already
/// uses for this block, and it renders exactly what the component does — the eyebrow, the statement, the body paragraph,
/// the first `paragraph` item and the `stat` items. One renderer, so the About page and the homepage's about section
/// cannot disagree about what the about block looks like.
fn site_about(model: &Model) -> String {
    let Some(page) = model.page.as_ref() else {
        return String::new();
    };
    format!("{}{}", about_section(&page.about), site_footer())
}


/// The login-unauthorized page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. `kind` is the tag it had, so the render below can put
/// it back in the same shape.
const LOGIN_UNAUTHORIZED_VIEW_CONTENT: [(&str, &str); 3] = [
    ("h1", "Access not authorized"),
    ("p", "This account is authenticated but is not authorized for CulebraLuxe. Accounts are provisioned by an administrator — there is no self-service sign-up or automatic access."),
    ("p", "If you believe this is a mistake, contact a CulebraLuxe administrator and provide the identity shown by your sign-in provider. Your password, secret, and provider credentials are never shared or displayed here."),
];

/// The static page, rendered from that text. No read model and no fetch: a static page is content, and content does not
/// belong in a database query.
fn login_unauthorized_view() -> String {
    let body = LOGIN_UNAUTHORIZED_VIEW_CONTENT
        .iter()
        .map(|(kind, text)| match *kind {
            "h1" | "h2" | "h3" => format!(
                "<h2 class=\"font-serif text-2xl font-light text-foreground\">{}</h2>",
                escape(text)
            ),
            "li" => format!("<li class=\"ml-6 list-disc\">{}</li>", escape(text)),
            _ => format!(
                "<p class=\"mt-4 text-sm font-light leading-7 text-muted-foreground\">{}</p>",
                escape(text)
            ),
        })
        .collect::<String>();
    format!(
        "<article class=\"px-6 py-20 md:px-12 md:py-28\"><div class=\"mx-auto max-w-4xl space-y-6\">{body}</div></article>"
    )
}


/// The auth-error page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. `kind` is the tag it had, so the render below can put
/// it back in the same shape.
const AUTH_ERROR_VIEW_CONTENT: [(&str, &str); 2] = [
    ("h1", "Authentication failed"),
    ("p", "Sign-in could not be completed. Please try again."),
];

/// The static page, rendered from that text. No read model and no fetch: a static page is content, and content does not
/// belong in a database query.
fn auth_error_view() -> String {
    let body = AUTH_ERROR_VIEW_CONTENT
        .iter()
        .map(|(kind, text)| match *kind {
            "h1" | "h2" | "h3" => format!(
                "<h2 class=\"font-serif text-2xl font-light text-foreground\">{}</h2>",
                escape(text)
            ),
            "li" => format!("<li class=\"ml-6 list-disc\">{}</li>", escape(text)),
            _ => format!(
                "<p class=\"mt-4 text-sm font-light leading-7 text-muted-foreground\">{}</p>",
                escape(text)
            ),
        })
        .collect::<String>();
    format!(
        "<article class=\"px-6 py-20 md:px-12 md:py-28\"><div class=\"mx-auto max-w-4xl space-y-6\">{body}</div></article>"
    )
}


/// The portal-auth-proof page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. `kind` is the tag it had, so the render below can put
/// it back in the same shape.
const PORTAL_AUTH_PROOF_VIEW_CONTENT: [(&str, &str); 2] = [
    ("h1", "Portal Auth Proof"),
    ("p", "Safe session diagnostics only."),
];

/// The static page, rendered from that text. No read model and no fetch: a static page is content, and content does not
/// belong in a database query.
fn portal_auth_proof_view() -> String {
    let body = PORTAL_AUTH_PROOF_VIEW_CONTENT
        .iter()
        .map(|(kind, text)| match *kind {
            "h1" | "h2" | "h3" => format!(
                "<h2 class=\"font-serif text-2xl font-light text-foreground\">{}</h2>",
                escape(text)
            ),
            "li" => format!("<li class=\"ml-6 list-disc\">{}</li>", escape(text)),
            _ => format!(
                "<p class=\"mt-4 text-sm font-light leading-7 text-muted-foreground\">{}</p>",
                escape(text)
            ),
        })
        .collect::<String>();
    format!(
        "<article class=\"px-6 py-20 md:px-12 md:py-28\"><div class=\"mx-auto max-w-4xl space-y-6\">{body}</div></article>"
    )
}



/// The seller-strategy page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. `kind` is the tag it had, so the render below can put
/// it back in the same shape.
const SELLER_STRATEGY_VIEW_CONTENT: [(&str, &str); 21] = [
    ("p", "Core · Strategic disposition"),
    ("h1", "Seller Strategy"),
    ("p", "Strategic disposition analysis for sellers — change any assumption and the model recalculates live."),
    ("p", "Recommended"),
    ("p", "expected PV · months to liquidity"),
    ("p", "Why this strategy?"),
    ("p", "Expected PV Ranking"),
    ("p", "Shared facts"),
    ("h2", "Live Assumptions"),
    ("p", "Sunk · basis"),
    ("p", "mo · future"),
    ("p", "ASSUMPTIONS"),
    ("p", "expected PV · changes apply immediately"),
    ("p", "Hero visual"),
    ("h2", "Decision Map"),
    ("p", "Live decision tree with probabilities, outcomes, and present values"),
    ("p", "Read-out"),
    ("h2", "Key Takeaways"),
    ("p", "Evidence"),
    ("h2", "Analysis Detail"),
    ("p", "branches across enabled paths · winner at PV. Open Show All Branches for the full probability, price, after-tax and discounted-PV breakdown."),
];

/// The static page, rendered from that text. No read model and no fetch: a static page is content, and content does not
/// belong in a database query.
fn seller_strategy_view() -> String {
    let body = SELLER_STRATEGY_VIEW_CONTENT
        .iter()
        .map(|(kind, text)| match *kind {
            "h1" | "h2" | "h3" => format!(
                "<h2 class=\"font-serif text-2xl font-light text-foreground\">{}</h2>",
                escape(text)
            ),
            "li" => format!("<li class=\"ml-6 list-disc\">{}</li>", escape(text)),
            _ => format!(
                "<p class=\"mt-4 text-sm font-light leading-7 text-muted-foreground\">{}</p>",
                escape(text)
            ),
        })
        .collect::<String>();
    format!(
        "<article class=\"px-6 py-20 md:px-12 md:py-28\"><div class=\"mx-auto max-w-4xl space-y-6\">{body}</div></article>"
    )
}



/// The login-recovery page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. `kind` is the tag it had, so the render below can put
/// it back in the same shape.
const LOGIN_RECOVERY_VIEW_CONTENT: [(&str, &str); 2] = [
    ("h1", "Emergency administrative access"),
    ("p", "For CulebraLuxe administrators only. This path is independent of the normal sign-in provider and is intended solely for outage recovery."),
];

/// The static page, rendered from that text. No read model and no fetch: a static page is content, and content does not
/// belong in a database query.
fn login_recovery_view() -> String {
    let body = LOGIN_RECOVERY_VIEW_CONTENT
        .iter()
        .map(|(kind, text)| match *kind {
            "h1" | "h2" | "h3" => format!(
                "<h2 class=\"font-serif text-2xl font-light text-foreground\">{}</h2>",
                escape(text)
            ),
            "li" => format!("<li class=\"ml-6 list-disc\">{}</li>", escape(text)),
            _ => format!(
                "<p class=\"mt-4 text-sm font-light leading-7 text-muted-foreground\">{}</p>",
                escape(text)
            ),
        })
        .collect::<String>();
    format!(
        "<article class=\"px-6 py-20 md:px-12 md:py-28\"><div class=\"mx-auto max-w-4xl space-y-6\">{body}</div></article>"
    )
}

/// The portal entry: sign-in, rendered by Rust.
///
/// WHY A BODY RATHER THAN TEXT EXTRACTION. The page this replaces used a Next *server action* to start the OAuth flow,
/// and a server action is not markup — it is a Next mechanism that cannot survive the port. So the body is written, and
/// it links to the endpoint Auth.js already exposes for exactly this (`/api/auth/signin/google`), which is the same
/// flow its own default sign-in page uses. Sign-in therefore still works from Rust; what is lost is the env-guarded
/// "Portal temporarily unavailable" branch, which needs the server's configuration and is a follow-up rather than
/// something to fake here.
///
/// The copy is the copy that was on the page, taken from the extraction this replaces rather than retyped.
fn login_view() -> String {
    "<main class=\"flex min-h-screen items-center justify-center bg-[#f5f2ec] px-6\">\
       <div class=\"w-full max-w-sm\">\
         <div class=\"text-center\">\
           <div class=\"font-serif text-2xl font-light uppercase tracking-[0.08em] text-[#030f23]\">CulebraLuxe</div>\
           <div class=\"mt-1 text-[10px] font-light uppercase tracking-[0.32em] text-[#030f23]/50\">Private Portal</div>\
         </div>\
         <div class=\"mt-10 rounded-sm border border-[#030f23]/10 bg-white p-8\">\
           <h1 class=\"font-serif text-xl font-light\">Sign in</h1>\
           <p class=\"mt-2 text-sm font-light leading-6 text-black/50\">Access is for the CulebraLuxe team. There is no \
             public sign-up — accounts are provisioned by an administrator.</p>\
           <a href=\"/api/auth/signin/google?callbackUrl=%2Fportal-auth-proof\" \
             class=\"mt-8 flex min-h-12 w-full items-center justify-center gap-3 rounded-sm border \
             border-[#030f23]/15 px-4 text-sm font-light text-[#030f23] transition hover:border-[#030f23]\">\
             Continue with Google</a>\
           <p class=\"mt-4 text-xs font-light text-black/40\">Having trouble? Contact a CulebraLuxe administrator.</p>\
         </div>\
         <div class=\"mt-6 text-center\">\
           <a href=\"/login/recovery\" class=\"text-xs font-light text-[#030f23]/45 underline-offset-2 hover:underline\">\
             Emergency administrative access</a>\
         </div>\
       </div>\
     </main>"
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
        "login" => Some(login_view()),
        "login-recovery" => Some(login_recovery_view()),
        "seller-strategy" => Some(seller_strategy_view()),
        "portal-auth-proof" => Some(portal_auth_proof_view()),
        "auth-error" => Some(auth_error_view()),
        "login-unauthorized" => Some(login_unauthorized_view()),
        "site-services" => Some(site_services(model)),
        "site-about" => Some(site_about(model)),
        "site-privacy" => Some(privacy_view()),
        "site-whatsapp" => Some(whatsapp_view()),
        "site-home" => Some(site_home(model)),
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

/// A site screen that has its page but no renderer yet.
///
/// THIS EXISTS BECAUSE A BLANK PAGE IS INVISIBLE. Sellers, FAQ, Guide, Contact and Buyers are served their payload and
/// have no body written yet — so before this they rendered chrome, nothing, and no explanation, which is indistinguishable
/// from a page that is broken, a module that failed to load, or a screen that navigated to the wrong place. Four
/// different problems, one symptom, no information.
///
/// It names the screen, so a stuck page is also a pointer: the key in the message is the key to look up in the registry,
/// the route and the view.
fn not_yet_ported(model: &Model) -> String {
    format!(
        "<section class=\"mx-auto max-w-[1600px] px-6 py-28 md:px-12 md:py-40\">\
           <p class=\"mb-4 text-xs font-light uppercase tracking-[0.34em] text-accent\">Not yet ported</p>\
           <h1 class=\"max-w-4xl text-balance font-serif text-3xl font-light leading-[1.15] text-foreground md:text-4xl\">\
             This page is still TypeScript. Its content has arrived and its design has not.</h1>\
           <p class=\"mt-8 max-w-2xl text-sm font-light leading-relaxed text-muted-foreground\">\
             The Rust view has no renderer for this screen yet, so it is showing this instead of an empty page. \
             The screen is <code class=\"text-foreground\">{key}</code> and its content is being served correctly: \
             nothing is broken, the body is simply not written.</p>\
         </section>",
        key = escape(model.screen.key),
    )
}

/// The body: a screen's own markup when it has any, else a header and rows.
fn body(model: &Model) -> String {
    // THE SITE HAS NO SCREEN HEADER. On the portal, a title and its route help you know where you are; on the public
    // site they were the first two things a visitor read — "Home" then "Replaces /" — the UI talking about itself over
    // the top of the page. Site chrome is the navy header and the footer; the page itself says what it is.
    let header = if model.screen.surface == Surface::Site {
        String::new()
    } else {
        screen_header(model)
    };
    if let Some(custom) = custom_body(model) {
        return format!("{header}{custom}");
    }
    // A site screen whose page has arrived but whose renderer has not says so, rather than rendering nothing at all.
    if model.screen.surface == Surface::Site && model.page.is_some() {
        return format!("{header}{}", not_yet_ported(model));
    }
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
    fn the_portal_nav_lists_each_of_its_screens_once_and_never_a_retired_one() {
        // Scoped to the PORTAL nav, because the site header is no longer built from the registry at all (below). This is
        // the invariant the table can still guarantee: a listed screen of the current surface appears exactly once, and
        // a retired one never appears.
        for &screen in SCREENS.iter().filter(|s| s.surface != Surface::Site) {
            let html = render(&Model {
                screen,
                ..Model::default()
            });
            let menu = html
                .split("aria-label=\"Portal\"")
                .nth(1)
                .and_then(|rest| rest.split("</nav>").next())
                .unwrap_or(html.as_str());
            for candidate in SCREENS.iter().filter(|c| c.surface != Surface::Site) {
                let count = menu
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

    /// The user's report, pinned. The first version of the header listed registry titles — twelve of them, "Home" first,
    /// Portal nowhere — and the report was: the logo is missing, the bar should be navy, "Home" should not be there,
    /// Portal is missing. This is the design's menu, and the registry's bookkeeping does not leak into it.
    /// The Services page renders the two blocks it is made of, through the same renderer the homepage uses — which is
    /// the point: one renderer, so the page and the homepage's summary of it cannot drift.
    ///
    /// It guards the two halves of the bug this page had. It carried a second, hand-typed copy of copy that already
    /// lives in the content store; and it fetched nothing, because the route that mounts it never handed it a `pagePath`,
    /// so the Rust screen had no page to read and rendered an empty body under the chrome.
    /// EVERY URL PAINTS ITS OWN SCREEN — pinned at the level the tests above cannot reach.
    ///
    /// Those tests build a `Model` by hand and call `render`, so they exercise the view and nothing else. A real page
    /// takes a longer path: the host calls `mount(id, key)`, `update` opens that screen, and `render` paints it. Nothing
    /// tested that path, so a bug in it shows up as "the Services page shows the homepage" with a green test suite.
    ///
    /// The second half is the landmine in the header: its links resolve their hrefs from the registry with a fallback of
    /// "/", so a key that fails to resolve does not fail — it silently sends the visitor to the homepage. That is exactly
    /// what a "Services" page showing the landing page looks like from the outside, so it is asserted rather than trusted.
    #[test]
    fn each_site_url_paints_its_own_screen_and_links_to_its_own_url() {
        for key in [
            "site-services",
            "site-sellers",
            "site-about",
            "site-faq",
            "site-guide",
        ] {
            let screen = target(key);
            let mut program = crate::Program::new();
            program.open(screen);
            let html = program.html();
            assert!(
                !html.contains("/images/hero-villa.png"),
                "{key} painted the landing page's hero"
            );
            assert_eq!(
                program.model().screen.key,
                key,
                "{key} did not stay open after being opened"
            );
            assert!(
                html.contains(&format!("href=\"{}\"", screen.path)),
                "the header of {key} does not link to {} — the fallback sends the visitor to the homepage instead",
                screen.path
            );
        }
    }

    /// The About page renders the managed about block through the same renderer the homepage's about section uses — so
    /// the page and the homepage section cannot disagree about what the about block looks like.
    #[test]
    fn the_about_page_renders_the_about_block() {
        let page = crate::model::PageContent {
            about: Block {
                eyebrow: "About Us".into(),
                title: "A boutique brokerage devoted to a single island.".into(),
                body: "We work with few clients.".into(),
                items: vec![
                    crate::model::BlockItem {
                        key: "paragraph".into(),
                        label: None,
                        value: Some("Founded by island residents.".into()),
                    },
                    crate::model::BlockItem {
                        key: "stat".into(),
                        label: Some("14".into()),
                        value: Some("Years on island".into()),
                    },
                ],
                ..Block::default()
            },
            ..crate::model::PageContent::default()
        };
        let html = render(&Model {
            screen: target("site-about"),
            page: Some(page),
            ..Model::default()
        });
        assert!(html.contains("About Us"));
        assert!(html.contains("Founded by island residents."));
        // The stats come from items keyed `stat`, the same filter the component used: a stat is a pair, not a line.
        assert!(html.contains("Years on island"));

        // And no copy before the payload arrives.
        let bare = render(&Model {
            screen: target("site-about"),
            ..Model::default()
        });
        assert!(!bare.contains("About Us"));
    }

    #[test]
    fn the_services_page_renders_the_blocks_it_is_made_of() {
        let page = crate::model::PageContent {
            buyers: Block {
                eyebrow: "For Buyers".into(),
                title: "A considered path".into(),
                body: "Guided, and not hurried.".into(),
                items: vec![crate::model::BlockItem {
                    key: "list".into(),
                    label: None,
                    value: Some("Search the island, not the portals".into()),
                }],
                ..Block::default()
            },
            sellers: Block {
                eyebrow: "For Sellers".into(),
                title: "Presented to the few".into(),
                ..Block::default()
            },
            ..crate::model::PageContent::default()
        };
        let html = render(&Model {
            screen: target("site-services"),
            page: Some(page),
            ..Model::default()
        });
        assert!(html.contains("For Buyers"));
        assert!(html.contains("For Sellers"));
        // The buyers' list comes from items keyed `list`, so it renders as the ruled list and not as loose text.
        assert!(html.contains("Search the island, not the portals"));
        // The sellers block keeps its portrait image, falling back to the coastline the component falls back to.
        assert!(html.contains("/images/coastline.png"));

        // NO PAYLOAD, NO COPY. Until the page arrives the body is empty and the host owns the loading line. Inventing the
        // page's words while it loads is how a port stops being a port.
        let bare = render(&Model {
            screen: target("site-services"),
            ..Model::default()
        });
        assert!(
            !bare.contains("For Buyers"),
            "the page does not invent its copy while the payload is still in flight"
        );
    }

    #[test]
    fn the_site_header_is_the_designs_menu_and_not_the_registry() {
        let html = render(&Model {
            screen: target("site-home"),
            ..Model::default()
        });
        let menu = html
            .split("aria-label=\"Primary\"")
            .nth(1)
            .and_then(|rest| rest.split("</nav>").next())
            .expect("the site header has a primary nav");
        for label in [
            "Buyers",
            "Sellers",
            "Services",
            "Guide",
            "About",
            "FAQ",
            "Contact",
            "Portal",
        ] {
            assert!(
                menu.contains(&format!(">{label}</a>")),
                "{label} missing from the site header"
            );
        }
        assert!(
            !menu.contains(">Home<"),
            "the logo is home; Home is not a menu item"
        );
        // EXACTLY THE DESIGN'S EIGHT ENTRIES, and nothing else. A title-matching check cannot work here: the registry's
        // own screens are called "FAQ" and "Portal", and the design's header links to both. Counting is the honest
        // invariant — it catches a registry entry leaking in and a label going missing, which is what the first version
        // of this header got wrong in both directions.
        assert_eq!(
            menu.matches("top-nav-capsule top-nav-capsule--tight").count(),
            8,
            "the site menu is the design's eight entries: seven links and Portal"
        );
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
            screen: target("client-record"),
            scope: Some("villa-del-mar".into()),
            ..Model::default()
        };
        assert!(render(&model).contains("Record <code>villa-del-mar</code>"));

        // The slug arrives from a URL, so it is data like any other and gets escaped like any other.
        let hostile = Model {
            screen: target("client-record"),
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
