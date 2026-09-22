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
    //
    // A BROKEN KEY MUST NOT BECOME THE HOMEPAGE. This resolved each key with `unwrap_or("/")`, so a typo in the table
    // above sent the visitor Home — the menu looked like it worked, the destination was wrong, and nothing anywhere said
    // so. Now an unresolvable key is a development failure (`debug_assert!`, live in tests and debug builds and compiled
    // out of release) and the item is omitted, so the worst a release can do is show one link fewer. It can never show
    // the wrong destination, and the exact-hrefs test below fails on the typo before it ships.
    let href_of = |key: &str| -> Option<&'static str> {
        let found = screen(key);
        debug_assert!(
            found.is_some(),
            "the site menu names '{key}', which is not a screen in the registry"
        );
        found.map(|screen| screen.path)
    };
    let desktop = LINKS
        .iter()
        .filter_map(|(label, key)| {
            let href = href_of(key)?;
            Some(format!(
                "<a href=\"{href}\" class=\"{CAPSULE}\">{label}</a>",
                href = escape(href),
                label = escape(label)
            ))
        })
        .collect::<String>();
    let mobile = LINKS
        .iter()
        .filter_map(|(label, key)| {
            let href = href_of(key)?;
            Some(format!(
                "<a href=\"{href}\" class=\"{MOBILE_CAPSULE}\">{label}</a>",
                href = escape(href),
                label = escape(label)
            ))
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
               <summary class=\"flex cursor-pointer list-none flex-col items-end gap-1.5 text-brand-ivory \
                 [&::-webkit-details-marker]:hidden\" aria-label=\"Menu\">\
                 <span class=\"block h-px w-6 bg-current\"></span>\
                 <span class=\"block h-px w-6 bg-current\"></span>\
                 <span class=\"block h-px w-6 bg-current\"></span>\
               </summary>\
               <nav class=\"absolute inset-x-0 top-full flex max-h-[75svh] flex-col gap-2 overflow-y-auto \
                 border-t border-brand-gold/25 bg-brand-navy px-4 py-4 backdrop-blur-md\" aria-label=\"Mobile\">\
                 {mobile}<a href=\"/portal/dashboard\" class=\"{MOBILE_CAPSULE}\">Portal</a>\
               </nav>\
             </details>\
           </div>\
         </header>\
         <div class=\"h-[76px] flex-none shrink-0 lg:h-[92px]\" aria-hidden=\"true\"></div>"
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
    // NO FOOTER HERE: `render()` wraps every site screen with the header and the footer, so appending one from a page is
    // how a page ends up with two. The chrome belongs to the shell; a page renders sections.
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

/// `components/page-hero.tsx` — the header every interior page opens with: a full-bleed photograph, two scrims, the
/// eyebrow and the statement over it, and an optional intro beneath.
///
/// ONE COMPONENT, FIVE PAGES, SO IT IS PORTED ONCE. About, FAQ, Contact, Guide and Sellers all opened with this, and
/// building it once is what stops five copies of it drifting apart: the height (`68svh`), the padding that clears the
/// fixed header (`pt-40`), the gradient that makes ivory text legible over a photograph, and the rule that an absent
/// intro renders nothing rather than an empty line.
///
/// The values come from the call site, because that is how the pages had them: Sellers and About carried their hero copy
/// as literals in the page, while FAQ and Contact read theirs from the managed page-hero slots. Same component, two
/// sources, and the source is the page's business rather than the hero's.
fn page_hero(eyebrow: &str, title: &str, intro: Option<&str>, image: &str, image_alt: &str) -> String {
    // `image || '/placeholder.svg'` in the component. An empty path is not a missing image, it is a broken one.
    let image = if image.is_empty() {
        "/placeholder.svg"
    } else {
        image
    };
    let intro = match intro {
        Some(text) if !text.trim().is_empty() => format!(
            "<p class=\"mt-8 max-w-2xl text-pretty text-base font-light leading-relaxed text-background/80 \
             md:text-lg\">{}</p>",
            escape(text)
        ),
        _ => String::new(),
    };
    format!(
        "<section class=\"relative flex min-h-[68svh] items-end overflow-hidden\">\
           <img src=\"{image}\" alt=\"{alt}\" sizes=\"100vw\" \
             class=\"absolute inset-0 h-full w-full object-cover\" />\
           <div class=\"absolute inset-0 bg-gradient-to-t from-black/70 via-black/25 to-black/40\"></div>\
           <div class=\"relative w-full px-6 pb-16 pt-40 md:px-12 md:pb-24\">\
             <div class=\"mx-auto max-w-[1600px]\">\
               <p class=\"mb-5 text-xs font-light uppercase tracking-[0.4em] text-background/70\">{eyebrow}</p>\
               <h1 class=\"max-w-4xl text-balance font-serif text-4xl font-light leading-[1.05] text-background \
                 md:text-6xl\">{title}</h1>\
               {intro}\
             </div>\
           </div>\
         </section>",
        image = escape(image),
        alt = escape(image_alt),
        eyebrow = escape(eyebrow),
        title = escape(title),
        intro = intro,
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

use crate::icons::icon;

/// `app/services/page.tsx` — the Services page.
///
/// IT IS NOT `components/services.tsx`. That component is the homepage's buyers/sellers band, and the first version of
/// this page was built from it: the /services page rendered the homepage's words, with a "For Buyers" heading on a page
/// that has no such section. The real page is this file's own design and its own copy — eight service cards with their
/// own images and enquiry links, a three-step process, four reasons, a strip of three principles, and a closing band.
///
/// The copy is literal here because it is literal there: this page reads no managed content. The payload still arrives,
/// because the screen is editorial like every other public page, and it is deliberately unused — inventing a slot for
/// copy that never had one would be a second source of truth for words the page already owns.
fn site_services(model: &Model) -> String {
    let _ = model;
    format!(
        "{hero}{intro}{cards}{how}{why}{strip}{cta}",
        hero = page_hero(
            "Services",
            "Real estate services, quietly handled.",
            Some(
                "Thoughtful advisory, research, coordination, and property support for owners, buyers, and clients across Culebra."
            ),
            "/images/coastline.png",
            "Aerial view of Culebra coastline and turquoise Caribbean water",
        ),
        intro = service_intro(),
        cards = service_cards(),
        how = service_process(),
        why = service_reasons(),
        strip = service_principles(),
        cta = service_cta(),
    )
}

/// "More than transactions. / Thoughtful support at every step."
fn service_intro() -> String {
    "<section class=\"border-b border-border bg-[#f8f6f1] px-6 py-20 md:px-12 md:py-24\">\
       <div class=\"mx-auto max-w-[1600px]\">\
         <div class=\"mx-auto max-w-3xl text-center\">\
           <h2 class=\"font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl\">\
             More than transactions.<br />Thoughtful support at every step.</h2>\
           <div class=\"mx-auto mt-6 h-px w-12 bg-accent\"></div>\
           <p class=\"mx-auto mt-7 max-w-2xl text-sm font-light leading-relaxed text-muted-foreground\">\
             From valuations and research to coordination and marketing, our services are designed to simplify decisions, \
             connect the right expertise, and protect your interests on Culebra.</p>\
           <p class=\"mt-7 text-[10px] font-medium uppercase tracking-[0.24em] text-accent\">\
             Local knowledge · Thoughtful coordination · Exceptional discretion</p>\
         </div>\
       </div>\
     </section>"
        .to_string()
}

/// The eight services, in the order the page lists them, with their own images and enquiry links.
///
/// A TABLE RATHER THAN EIGHT BLOCKS OF MARKUP, because the page renders them from an array and the array is the thing a
/// reader needs to check against the live site: number, title, body, call to action, destination, image — in that order.
const SERVICES: [(&str, &str, &str, &str, &str, &str); 8] = [
    (
        "01",
        "Market Analysis / CMA",
        "Comprehensive market data and local insight to help you understand current value and position with confidence.",
        "Request analysis",
        "/contact?service=market-analysis",
        "/images/services/service-01-market-analysis-cma.jpg",
    ),
    (
        "02",
        "Property Evaluation",
        "A considered evaluation of your home or land based on property characteristics, location, and current market conditions.",
        "Request evaluation",
        "/contact?service=property-evaluation",
        "/images/services/service-02-property-evaluation.jpg",
    ),
    (
        "03",
        "Comparable Research",
        "Detailed comparable-property research to support informed decisions when buying, selling, or evaluating an opportunity.",
        "Request comparables",
        "/contact?service=comparable-research",
        "/images/services/service-03-comparable-research.jpg",
    ),
    (
        "04",
        "Land Survey Coordination",
        "Coordination with trusted local professionals for surveys, boundary work, and related property documentation.",
        "Request survey",
        "/contact?service=land-survey",
        "/images/services/service-04-land-survey-coordination.jpg",
    ),
    (
        "05",
        "Appraisal Coordination",
        "Assistance arranging professional appraisal services for lending, estate planning, investment, or personal decision-making.",
        "Request appraisal",
        "/contact?service=appraisal",
        "/images/services/service-05-appraisal-coordination.jpg",
    ),
    (
        "06",
        "Deed & Title Research",
        "Coordination of title history, deed research, lien checks, and document retrieval with the appropriate local professionals.",
        "Request research",
        "/contact?service=title-research",
        "/images/services/service-06-deed-title-research.jpg",
    ),
    (
        "07",
        "Real Estate Consultation",
        "Personalized guidance for property ownership, purchases, sales, investment questions, and long-range planning on Culebra.",
        "Book consultation",
        "/contact?service=consultation",
        "/images/services/service-07-real-estate-consultation.jpg",
    ),
    (
        "08",
        "Property Marketing Services",
        "Discreet, elevated property presentation and marketing support designed around the property, audience, and objective.",
        "Discuss marketing",
        "/contact?service=property-marketing",
        "/images/services/service-08-property-marketing-services.jpg",
    ),
];

// "How it works": number, title, body, icon.
const SERVICE_PROCESS: [(&str, &str, &str, &str); 3] = [
    ("1", "Share your needs", "Tell us about the property, your objectives, and the support you are looking for.", "message-circle"),
    ("2", "We review & coordinate", "We research the situation, connect the right professionals, and organize the details.", "map-pinned"),
    ("3", "Clear next steps", "You receive thoughtful guidance, timely updates, and a clear path forward.", "clipboard-check"),
];

// The four reasons in the "Why clients come to CulebraLuxe" split.
const SERVICE_REASONS: [(&str, &str, &str); 4] = [
    ("Island-specific knowledge", "Deep understanding of Culebra's properties, neighborhoods, infrastructure, market, and way of life.", "compass"),
    ("Personally handled", "Thoughtful, attentive service with direct involvement rather than a high-volume handoff model.", "user-round"),
    ("Trusted local network", "Established relationships with surveyors, attorneys, appraisers, contractors, and other island professionals.", "network"),
    ("Boutique service", "Selective client relationships, careful coordination, and discreet high-touch support.", "handshake"),
];

// The three-column strip of principles: icon, heading, body.
const SERVICE_PRINCIPLES: [(&str, &str, &str); 3] = [
    ("file-search", "Research before action", "Decisions begin with understanding the property, context, documentation, and objective."),
    ("handshake", "The right people", "We help connect each need with appropriate local expertise rather than treating every request the same."),
    ("check-circle-2", "Follow-through", "Thoughtful coordination and clear communication keep small details from becoming large problems."),
];

/// The eight service cards, four across on a wide screen.
fn service_cards() -> String {
    let cards = SERVICES
        .iter()
        .map(|(number, title, body, cta, href, image)| {
            format!(
                "<article class=\"group flex h-full flex-col overflow-hidden border border-border bg-[#fbfaf7] \
                   transition-transform duration-500 hover:-translate-y-1\">\
                   <div class=\"relative aspect-[4/2.7] overflow-hidden bg-muted\">\
                     <img src=\"{image}\" alt=\"{title}\" sizes=\"(min-width: 1024px) 25vw, (min-width: 640px) 50vw, 100vw\" \
                       class=\"absolute inset-0 h-full w-full object-cover transition-transform duration-700 \
                       group-hover:scale-[1.03]\" /></div>\
                   <div class=\"flex flex-1 flex-col px-6 pb-7 pt-6\">\
                     <p class=\"text-[10px] font-light uppercase tracking-[0.22em] text-accent\">{number}</p>\
                     <h3 class=\"mt-3 font-serif text-xl font-light leading-tight text-foreground\">{title}</h3>\
                     <p class=\"mt-4 flex-1 text-sm font-light leading-relaxed text-muted-foreground\">{body}</p>\
                     <a href=\"{href}\" class=\"group/link mt-7 inline-flex items-center gap-3 text-[10px] font-medium \
                       uppercase tracking-[0.18em] text-accent\">{cta}\
                       <span class=\"inline-block h-px w-6 bg-accent transition-all duration-500 \
                         group-hover/link:w-10\"></span></a>\
                   </div>\
                 </article>",
                image = escape(image),
                title = escape(title),
                number = escape(number),
                body = escape(body),
                href = escape(href),
                cta = escape(cta),
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"border-b border-border bg-[#f3efe8] px-6 py-20 md:px-12 md:py-28\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"mb-12 md:mb-16\">\
               <p class=\"text-xs font-light uppercase tracking-[0.28em] text-accent\">What we can help with</p>\
               <h2 class=\"mt-4 max-w-2xl font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl\">\
                 Practical expertise around island property.</h2>\
             </div>\
             <div class=\"grid gap-5 sm:grid-cols-2 lg:grid-cols-4\">{cards}</div>\
           </div>\
         </section>"
    )
}

/// "How it works" — three numbered steps, each with its icon.
///
/// AN ICON IS NEVER DRAWN WITHOUT AN ANSWER: `icon()` returns `None` for a name that is not in the table, and that is
/// faced here rather than papered over. A missing icon is an invisible hole in a layout.
fn service_process() -> String {
    let steps = SERVICE_PROCESS
        .iter()
        .map(|(number, title, body, icon_name)| {
            let icon = icon(icon_name, "h-11 w-11", "1.15").unwrap_or_else(|| {
                panic!("the Services page asks for an icon that is not in the table: {icon_name}")
            });
            format!(
                "<div class=\"h-full px-6 text-center md:border-r md:border-border md:px-14 last:md:border-r-0\">\
                   <div class=\"mx-auto flex h-14 w-14 items-center justify-center text-accent\">{icon}</div>\
                   <div class=\"mt-7 flex items-baseline justify-center gap-4\">\
                     <span class=\"font-serif text-3xl font-light text-accent\">{number}</span>\
                     <h3 class=\"font-serif text-xl font-light text-foreground\">{title}</h3>\
                   </div>\
                   <p class=\"mx-auto mt-4 max-w-xs text-sm font-light leading-relaxed text-muted-foreground\">{body}</p>\
                 </div>",
                icon = icon,
                number = escape(number),
                title = escape(title),
                body = escape(body),
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"border-b border-border bg-[#efebe3] px-6 py-20 md:px-12 md:py-24\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"text-center\">\
               <h2 class=\"font-serif text-3xl font-light text-foreground md:text-4xl\">How it works</h2>\
               <div class=\"mx-auto mt-5 h-px w-12 bg-accent\"></div>\
             </div>\
             <div class=\"mx-auto mt-14 grid max-w-6xl gap-12 md:grid-cols-3 md:gap-0\">{steps}</div>\
           </div>\
         </section>"
    )
}

/// "Why clients come to CulebraLuxe" — the property photograph beside the four reasons.
fn service_reasons() -> String {
    let reasons = SERVICE_REASONS
        .iter()
        .map(|(title, body, icon_name)| {
            let icon = icon(icon_name, "h-4 w-4", "1.3").unwrap_or_else(|| {
                panic!("the Services page asks for an icon that is not in the table: {icon_name}")
            });
            format!(
                "<div class=\"flex gap-5\">\
                   <div class=\"mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-full \
                     border border-accent/60 text-accent\">{icon}</div>\
                   <div>\
                     <h3 class=\"text-sm font-medium text-foreground\">{title}</h3>\
                     <p class=\"mt-1.5 max-w-lg text-sm font-light leading-relaxed text-muted-foreground\">{body}</p>\
                   </div>\
                 </div>",
                icon = icon,
                title = escape(title),
                body = escape(body),
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"border-b border-border bg-[#f8f6f1]\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"grid items-stretch md:grid-cols-2\">\
               <div class=\"relative min-h-[420px] overflow-hidden bg-muted md:min-h-[620px]\">\
                 <img src=\"/images/hero-villa.png\" alt=\"Culebra property overlooking the Caribbean\" \
                   sizes=\"(min-width: 768px) 50vw, 100vw\" \
                   class=\"absolute inset-0 h-full w-full object-cover\" /></div>\
               <div class=\"flex h-full flex-col justify-center px-6 py-16 md:px-14 md:py-20 lg:px-20\">\
                 <p class=\"text-xs font-light uppercase tracking-[0.28em] text-accent\">Why CulebraLuxe</p>\
                 <h2 class=\"mt-4 font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl\">\
                   Why clients come to CulebraLuxe</h2>\
                 <div class=\"mt-9 space-y-7\">{reasons}</div>\
               </div>\
             </div>\
           </div>\
         </section>"
    )
}

/// The three-column strip of principles: research, the right people, follow-through.
fn service_principles() -> String {
    let columns = SERVICE_PRINCIPLES
        .iter()
        .map(|(icon_name, heading, body)| {
            let icon = icon(icon_name, "mx-auto h-8 w-8", "1.2").unwrap_or_else(|| {
                panic!("the Services page asks for an icon that is not in the table: {icon_name}")
            });
            format!(
                "<div class=\"px-5 text-center md:border-r md:border-border md:px-12 last:md:border-r-0\">\
                   {icon}\
                   <p class=\"mt-5 text-xs font-medium uppercase tracking-[0.16em] text-foreground\">{heading}</p>\
                   <p class=\"mx-auto mt-3 max-w-sm text-sm font-light leading-relaxed text-muted-foreground\">{body}</p>\
                 </div>",
                icon = icon,
                heading = escape(heading),
                body = escape(body),
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"border-b border-border bg-[#f2ede5] px-6 py-16 md:px-12 md:py-20\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"grid gap-10 md:grid-cols-3 md:gap-0\">{columns}</div>\
           </div>\
         </section>"
    )
}

/// The closing band: the same words the page ends on.
fn service_cta() -> String {
    "<section class=\"bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-28\">\
       <div class=\"mx-auto max-w-[1600px] text-center\">\
         <p class=\"text-xs font-light uppercase tracking-[0.28em] text-primary-foreground/60\">Culebra · Puerto Rico</p>\
         <h2 class=\"mx-auto mt-5 max-w-3xl font-serif text-3xl font-light leading-[1.1] md:text-4xl\">\
           Let&#39;s begin a quiet conversation.</h2>\
         <p class=\"mx-auto mt-5 max-w-xl text-sm font-light leading-relaxed text-primary-foreground/70\">\
           Tell us what you need. We&#39;ll help determine the right next step and whether CulebraLuxe can help.</p>\
         <a href=\"/contact\" class=\"mt-10 inline-flex border border-primary-foreground/40 px-8 py-4 text-xs \
           font-light uppercase tracking-[0.22em] transition-colors hover:bg-primary-foreground hover:text-primary\">\
           Start a conversation</a>\
       </div>\
     </section>"
        .to_string()
}

// "What we value": title, body, icon.
const ABOUT_VALUES: [(&str, &str, &str); 3] = [
    ("Fit over volume", "We measure success not in transactions but in fit — pairing the right stewards with the right homes.", "waves"),
    ("Local, truly", "Founded by island residents, we know Culebra beyond its coordinates — the trade winds, the tide charts, and the people who shape it.", "palmtree"),
    ("Quiet stewardship", "We protect the character that makes this place rare, advising with discretion and patience at every turn.", "leaf"),
];

// "Why clients choose CulebraLuxe": title and body.
const ABOUT_REASONS: [(&str, &str); 4] = [
    ("Boutique by design", "We intentionally work with a limited number of clients."),
    ("Island-specific expertise", "We understand unique homes, land, waterfront, and the realities of island ownership."),
    ("Personally handled", "Every search, showing, and negotiation is handled directly and deliberately."),
    ("Trusted by referral", "Much of our work comes through personal introductions and word of mouth."),
];

// The four figures, in the order the page shows them.
const ABOUT_STATS: [(&str, &str); 4] = [
    ("14", "Years on island"),
    ("1", "Island, entirely"),
    ("40+", "Homes stewarded"),
    ("100%", "By referral"),
];

// The founder's four credentials, and the eight photographs in the "Life on the island" strip.
const ABOUT_CREDENTIALS: [&str; 4] = [
    "Former Windsurfing World Champion",
    "Licensed Puerto Rico Real Estate Broker",
    "Full-time Culebra resident",
    "Referral-led, boutique practice",
];

const ABOUT_LIFE: [&str; 8] = [
    "/images/about/life-01.jpg",
    "/images/about/life-02.jpg",
    "/images/about/life-03.jpg",
    "/images/about/life-04.jpg",
    "/images/about/life-05.jpg",
    "/images/about/life-06.jpg",
    "/images/about/life-07.jpg",
    "/images/about/life-08.jpg",
];

/// `app/about/page.tsx` — the About page.
///
/// THE COPY IS LITERAL BECAUSE IT IS LITERAL THERE. This page reads no managed content: the founder's biography, the
/// values, the reasons, the four figures and the gallery captions are all written into the page. The payload still
/// arrives — the screen is editorial like every other public page — and it is deliberately unused, because routing these
/// words through a content slot that never existed would be a second source of truth for copy the page already owns.
fn site_about(model: &Model) -> String {
    let _ = model;
    format!(
        "{hero}{founder}{values}{reasons}{stats}{life}{cta}",
        hero = page_hero(
            "About Us",
            "Devoted to a single island.",
            Some(
                "CulebraLuxe is a boutique brokerage working with few clients, few homes, and an uncommon amount of care."
            ),
            "/images/about/about-hero.jpg",
            "Aerial view across Culebra and the surrounding Caribbean water",
        ),
        founder = about_founder(),
        values = about_values(),
        reasons = about_reasons(),
        stats = about_stats(),
        life = about_life(),
        cta = about_cta(),
    )
}

/// "Why clients choose CulebraLuxe" — the four reasons beside a photograph of Lisa at work.
fn about_reasons() -> String {
    let reasons = ABOUT_REASONS
        .iter()
        .map(|(title, body)| {
            format!(
                "<div class=\"flex gap-4\">\
                   <div class=\"mt-1 flex h-6 w-6 shrink-0 items-center justify-center rounded-full \
                     border border-accent text-xs text-accent\">✓</div>\
                   <p class=\"text-sm font-light leading-relaxed text-muted-foreground\">\
                     <span class=\"font-medium text-foreground\">{title}</span> — {body}</p>\
                 </div>",
                title = escape(title),
                body = escape(body),
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"bg-muted/30 px-6 py-20 md:px-12 md:py-28\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"grid items-center gap-12 md:grid-cols-2 md:gap-16\">\
               <div>\
                 <h2 class=\"font-serif text-3xl font-light leading-tight text-foreground md:text-4xl\">\
                   Why clients choose CulebraLuxe</h2>\
                 <div class=\"mt-10 space-y-7\">{reasons}</div>\
               </div>\
               <div class=\"aspect-[4/3] overflow-hidden bg-muted\">\
                 <img src=\"/images/about/lisa-work.jpg\" \
                   alt=\"Lisa Penfield working with clients in Culebra\" class=\"h-full w-full object-cover\" /></div>\
             </div>\
           </div>\
         </section>"
    )
}

/// The four figures: "14 / Years on island" and its neighbours.
fn about_stats() -> String {
    let stats = ABOUT_STATS
        .iter()
        .map(|(figure, label)| {
            format!(
                "<div class=\"text-center md:border-r md:border-border last:md:border-r-0\">\
                   <p class=\"font-serif text-4xl font-light text-accent md:text-5xl\">{figure}</p>\
                   <p class=\"mt-3 text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground\">{label}</p>\
                 </div>",
                figure = escape(figure),
                label = escape(label),
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"px-6 py-16 md:px-12 md:py-20\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"grid grid-cols-2 gap-y-10 md:grid-cols-4\">{stats}</div>\
           </div>\
         </section>"
    )
}

/// "Life on the island" — eight photographs in a single scrolling row.
fn about_life() -> String {
    let frames = ABOUT_LIFE
        .iter()
        .enumerate()
        .map(|(index, src)| {
            format!(
                "<div class=\"h-[220px] w-[180px] shrink-0 overflow-hidden bg-muted md:h-[260px] md:w-[220px]\">\
                   <img src=\"{src}\" alt=\"Life on Culebra {number}\" class=\"h-full w-full object-cover\" /></div>",
                src = escape(src),
                number = index + 1,
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"px-6 pb-20 pt-8 md:px-12 md:pb-28\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"text-center\">\
               <h2 class=\"font-serif text-3xl font-light text-foreground md:text-4xl\">Life on the island</h2>\
               <p class=\"mx-auto mt-4 max-w-xl text-sm font-light leading-relaxed text-muted-foreground\">\
                 A quiet collection of business and personal moments that reflect the pace, place, and perspective \
                 behind the brand.</p>\
             </div>\
             <div class=\"mt-12 flex gap-2 overflow-x-auto pb-2\">{frames}</div>\
           </div>\
         </section>"
    )
}

/// The closing band: "We would be glad to know what you are looking for."
fn about_cta() -> String {
    "<section class=\"bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-32\">\
       <div class=\"mx-auto flex max-w-[1600px] flex-col items-start gap-8\">\
         <h2 class=\"max-w-3xl text-balance font-serif text-3xl font-light leading-[1.1] md:text-4xl\">\
           We would be glad to know what you are looking for.</h2>\
         <a href=\"/contact\" class=\"group inline-flex items-center gap-3 text-xs font-light uppercase \
           tracking-[0.24em]\">Start a conversation\
           <span class=\"inline-block h-px w-10 bg-primary-foreground transition-all duration-500 \
             group-hover:w-16\"></span></a>\
       </div>\
     </section>"
        .to_string()
}

// The guide's nine sections, in the order the page presents them: id, number, title, headline, description.
const GUIDE_SECTIONS: [(&str, &str, &str, &str, &str); 9] = [
    ("beaches", "01", "BEACHES", "The edges of the island.", "From world-famous shores to quiet hidden coves, every beach in Culebra has its own character."),
    ("water", "02", "WATER", "The island from the water.", "Reefs, protected bays, open water and surrounding cays make the sea part of everyday life on Culebra."),
    ("wildlife-land", "03", "WILDLIFE & LAND", "A landscape worth protecting.", "Refuge lands, dry forest, trails and coastal habitat reveal the quieter natural side of the island."),
    ("coffee-casual", "04", "COFFEE & CASUAL", "Easy mornings and simple stops.", "Coffee, breakfast, beach kiosks and casual favorites for the unhurried rhythm of island days."),
    ("dining", "05", "DINING", "Where the island gathers.", "Waterfront tables, local seafood, pizza, tacos and relaxed evening spots across Culebra."),
    ("getting-here", "06", "GETTING HERE", "Arriving is part of the experience.", "Flights and ferry service connect Culebra with San Juan, Ceiba and Puerto Rico's main island."),
    ("getting-around", "07", "GETTING AROUND", "Small island, easy rhythm.", "Jeeps, carts and local taxis make it simple to move between town, beaches and the hills."),
    ("essentials", "08", "ISLAND ESSENTIALS", "The practical side of island life.", "Groceries, medical care, banking, hardware and everyday services that keep life on Culebra moving."),
    ("island-story", "09", "ISLAND STORY", "An island shaped by history.", "Indigenous roots, Spanish rule, military history, community resistance and conservation all shaped modern Culebra."),
];

/// The founder: her portrait, her biography, her words and her credentials.
/// `app/guide/page.tsx` — the Island Guide.
///
/// THE PAGE THAT IS A CATALOGUE RATHER THAN COPY. Its words come from `guide_item` through the page route, grouped into
/// nine sections with each entry a card carrying its own photograph. The section headings are literal because they are
/// the page's structure rather than its content; everything inside a section arrives from the database.
fn site_guide(model: &Model) -> String {
    let entries: &[crate::model::GuideItem] = model
        .page
        .as_ref()
        .map(|page| page.guide.as_slice())
        .unwrap_or(&[]);
    let nav = GUIDE_SECTIONS
        .iter()
        .map(|(id, _, title, _, _)| {
            format!(
                "<a href=\"#{id}\" class=\"text-[11px] font-light uppercase tracking-[0.22em] text-muted-foreground \
                 transition-colors hover:text-foreground\">{title}</a>",
                id = escape(id),
                title = escape(title)
            )
        })
        .collect::<String>();
    let sections = GUIDE_SECTIONS
        .iter()
        .map(|(id, number, title, headline, description)| {
            let cards = entries
                .iter()
                .filter(|item| item.section == *id)
                .map(guide_card)
                .collect::<String>();
            format!(
                "<section id=\"{id}\" class=\"scroll-mt-24\">\
                   <div class=\"grid gap-10 md:grid-cols-12 md:gap-12\">\
                     <div class=\"md:col-span-3\">\
                       <p class=\"text-xs font-light uppercase tracking-[0.28em] text-accent\">{number} / {title}</p>\
                       <h2 class=\"mt-5 font-serif text-2xl font-light leading-[1.2] text-foreground md:text-3xl\">{headline}</h2>\
                       <p class=\"mt-5 max-w-sm text-sm font-light leading-relaxed text-muted-foreground\">{description}</p>\
                     </div>\
                     <div class=\"min-w-0 md:col-span-9\">\
                       <div class=\"flex gap-5 overflow-x-auto pb-4\">{cards}</div>\
                     </div>\
                   </div>\
                 </section>",
                id = escape(id),
                number = escape(number),
                title = escape(title),
                headline = escape(headline),
                description = escape(description),
                cards = cards,
            )
        })
        .collect::<String>();
    format!(
        "{hero}\
         <section class=\"border-b border-border px-6 md:px-12\">\
           <div class=\"mx-auto max-w-[1600px] overflow-x-auto\">\
             <nav class=\"flex min-w-max gap-8 py-6 md:gap-10\">{nav}</nav>\
           </div>\
         </section>\
         <section class=\"px-6 py-20 md:px-12 md:py-28\">\
           <div class=\"mx-auto max-w-[1600px]\"><div class=\"space-y-24 md:space-y-32\">{sections}</div></div>\
         </section>\
         <section class=\"bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-32\">\
           <div class=\"mx-auto flex max-w-[1600px] flex-col items-start gap-8\">\
             <h2 class=\"max-w-3xl text-balance font-serif text-3xl font-light leading-[1.1] md:text-4xl\">\
               When you are ready to find your place here.</h2>\
             <a href=\"/buyers\" class=\"group inline-flex items-center gap-3 text-xs font-light uppercase \
               tracking-[0.24em]\">Explore buying on Culebra\
               <span class=\"inline-block h-px w-10 bg-primary-foreground transition-all duration-500 \
                 group-hover:w-16\"></span></a>\
           </div>\
         </section>",
        hero = page_hero(
            "Island Guide",
            "A slower rhythm, kept intentionally intact.",
            Some(
                "No traffic lights. No high-rises. Fishing boats at dawn, reef-clear water by noon, and evenings measured in shades of gold."
            ),
            "/images/culture.png",
            "The white sand crescent and turquoise water of Flamenco Beach, Culebra",
        ),
        nav = nav,
        sections = sections,
    )
}

/// One place in the guide: its photograph, its area, its name, what it is, and how to reach it.
fn guide_card(item: &crate::model::GuideItem) -> String {
    let image = match item.image_path.as_deref() {
        Some(src) => format!(
            "<img src=\"{src}\" alt=\"{alt}\" class=\"h-full w-full object-cover\" />",
            src = escape(src),
            alt = escape(item.image_alt.as_deref().unwrap_or(&item.name)),
        ),
        // An entry with no photograph says so, rather than showing a broken frame.
        None => "<div class=\"flex h-full items-center justify-center\"><span class=\"text-[10px] font-light \
                 uppercase tracking-[0.2em] text-muted-foreground\">Image coming soon</span></div>"
            .to_string(),
    };
    // The line above the name: whichever of the three the entry actually has.
    let overline = item
        .subtitle
        .as_deref()
        .or(item.area.as_deref())
        .or(item.eyebrow.as_deref())
        .unwrap_or("");
    let address = item
        .address
        .as_deref()
        .map(|value| {
            format!(
                "<p class=\"mt-4 text-xs font-light leading-relaxed text-muted-foreground\">{}</p>",
                escape(value)
            )
        })
        .unwrap_or_default();
    let phone = item
        .phone
        .as_deref()
        .map(|value| {
            format!(
                "<a href=\"tel:{}\" class=\"mt-2 block text-xs font-light text-foreground\">{}</a>",
                escape(value),
                escape(value)
            )
        })
        .unwrap_or_default();
    let website = item
        .website_url
        .as_deref()
        .map(|value| {
            format!(
                "<a href=\"{}\" target=\"_blank\" rel=\"noreferrer\" class=\"mt-3 inline-block text-[10px] \
                 font-light uppercase tracking-[0.2em] text-foreground\">Visit website →</a>",
                escape(value)
            )
        })
        .unwrap_or_default();
    format!(
        "<article class=\"w-[78vw] max-w-[280px] shrink-0 sm:w-[240px] lg:w-[220px]\">\
           <div class=\"aspect-[4/3] overflow-hidden bg-muted\">{image}</div>\
           <div class=\"pt-5\">\
             <p class=\"text-[10px] font-light uppercase tracking-[0.22em] text-accent\">{overline}</p>\
             <h3 class=\"mt-2 font-serif text-xl font-light leading-tight text-foreground\">{name}</h3>\
             <p class=\"mt-3 text-sm font-light leading-relaxed text-muted-foreground\">{description}</p>\
             {address}{phone}{website}\
           </div>\
         </article>",
        image = image,
        overline = escape(overline),
        name = escape(&item.name),
        description = escape(&item.description),
        address = address,
        phone = phone,
        website = website,
    )
}

// The four steps of "A considered path from first look to ownership".
const BUYER_STEPS: [(&str, &str, &str); 4] = [
    ("01", "A quiet conversation", "We begin by understanding what you are truly seeking — the light, the outlook, the rhythm of days. No pressure, no listings sheet. Just a considered discussion of possibility."),
    ("02", "Private viewings", "Many of the finest homes on Culebra never reach a public listing. We arrange discreet, unhurried viewings — including off-market residences held within our private network."),
    ("03", "Diligence & title", "We coordinate title research, survey review, and legal counsel, translating the particulars of Puerto Rico property law into clear, unhurried guidance."),
    ("04", "Closing & beyond", "From closing logistics to introductions for architects, builders, and island life, we remain a steady presence well after the keys change hands."),
];

// The six supporting services, listed as one ruled column.
const BUYER_SERVICES: [&str; 6] = [
    "Private, unlisted viewings",
    "Legal, title & closing guidance",
    "Architecture & renovation introductions",
    "Residency & relocation support",
    "Property management referrals",
    "Long-term stewardship advice",
];

// Sellers, section 01: title, body, icon.
const SELLER_WHY_US: [(&str, &str, &str); 3] = [
    ("Local intelligence", "Deep knowledge of properties, places, and local conditions that do not appear neatly in a database.", "compass"),
    ("Individual attention", "Every property receives its own positioning, strategy, presentation, and path to market.", "user-round"),
    ("Selective representation", "We maintain a limited portfolio so each listing receives meaningful focus and care.", "gem"),
];

// Section 02: the six steps of the process.
const SELLER_PROCESS: [(&str, &str, &str); 6] = [
    ("Understand", "The property, circumstances, and objectives.", "search"),
    ("Position", "Market analysis and pricing strategy.", "target"),
    ("Prepare", "Property preparation and media production.", "camera"),
    ("Launch", "Market introduction and targeted exposure.", "send"),
    ("Represent", "Showings, offers, and skilled negotiation.", "users"),
    ("Close", "Contract-to-closing coordination and follow-through.", "check-circle-2"),
];

// Section 03: the five dimensions that feed the market position, and the two on the other side of it.
const SELLER_MARKET_LEFT: [(&str, &str, &str); 3] = [
    ("Property", "Home, improvements, land, views, condition.", "home"),
    ("Place", "Micro-location, access, infrastructure, island context.", "map-pin"),
    ("Market", "Comparable sales, competition, supply, and current conditions.", "bar-chart-3"),
];

const SELLER_MARKET_RIGHT: [(&str, &str, &str); 2] = [
    ("Buyer", "Likely buyer pool, motivations, ability, and timing.", "user-round"),
    ("Objectives", "Your goals, timing, flexibility, and desired outcome.", "flag"),
];

// Section 04: the two lists under Presentation & Exposure. Titles only — that is all the page shows.
const SELLER_PRESENTATION: [(&str, &str); 6] = [
    ("Professional photography", "camera"),
    ("Video & walkthroughs", "video"),
    ("Staging & presentation guidance", "home"),
    ("Editorial property story", "file-text"),
    ("Digital property marketing", "megaphone"),
    ("Print & marketing materials", "clipboard-check"),
];

const SELLER_DISTRIBUTION: [(&str, &str); 6] = [
    ("CulebraLuxe buyer relationships", "users"),
    ("Direct qualified-buyer outreach", "user-round"),
    ("Targeted email campaigns", "mail"),
    ("Puerto Rico listing channels & MLS", "network"),
    ("Major real estate platforms", "megaphone"),
    ("Broker network & referrals", "handshake"),
];

// Section 05: the six stages of representation.
const SELLER_REPRESENTATION: [(&str, &str, &str); 6] = [
    ("Private showings", "Personally presenting the property to qualified buyers.", "users"),
    ("Offers & negotiation", "Evaluating offers and negotiating terms that align with your goals.", "file-text"),
    ("Contract progression", "Moving from accepted offer into the appropriate purchase-and-sale process.", "pen-line"),
    ("Due diligence coordination", "Survey, appraisal, inspections, financing, title, and other diligence items.", "clipboard-check"),
    ("Closing coordination", "Coordinating with attorneys, title professionals, and all parties.", "handshake"),
    ("Successful close", "Following through until the transaction is complete.", "key-round"),
];

/// `app/sellers/page.tsx` — the Sellers page.
///
/// SEVEN SECTIONS, ALL OF THEM THE PAGE'S OWN COPY: why CulebraLuxe, the six-step process, the market-positioning
/// diagram, presentation and exposure, representation, and the closing band. The page reads no managed content, so the
/// payload arrives (it is an editorial screen) and is deliberately unused.
fn site_sellers(model: &Model) -> String {
    let _ = model;
    format!(
        "{hero}{why}{process}{market}{presentation}{representation}{cta}",
        hero = page_hero(
            "Selling on Culebra",
            "Presented to the few who truly belong here.",
            Some(
                "Extraordinary properties deserve more than exposure — they deserve understanding, strategy, and representation."
            ),
            "/images/coastline.png",
            "Culebra coastline and homes overlooking the Caribbean",
        ),
        why = seller_why_us(),
        process = seller_process(),
        market = seller_market(),
        presentation = seller_presentation(),
        representation = seller_representation(),
        cta = seller_cta(),
    )
}

/// 01 — Why CulebraLuxe: the argument on the left, three reasons to its right.
fn seller_why_us() -> String {
    let cards = SELLER_WHY_US
        .iter()
        .map(|(title, body, icon_name)| {
            let icon = icon(icon_name, "h-10 w-10 text-accent", "1.25").unwrap_or_else(|| {
                panic!("the Sellers page asks for an icon that is not in the table: {icon_name}")
            });
            format!(
                "<div class=\"h-full md:border-l md:border-border md:px-10\">{icon}\
                   <h3 class=\"mt-6 text-sm font-medium uppercase tracking-[0.15em] text-foreground\">{title}</h3>\
                   <p class=\"mt-4 max-w-xs text-sm font-light leading-relaxed text-muted-foreground\">{body}</p>\
                 </div>",
                icon = icon,
                title = escape(title),
                body = escape(body),
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"border-b border-border bg-[#f6f3ed] px-6 py-20 md:px-12 md:py-28\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"grid gap-14 md:grid-cols-12 md:gap-16\">\
               <div class=\"md:col-span-3\">\
                 <p class=\"text-xs font-light uppercase tracking-[0.28em] text-accent\">01</p>\
                 <h2 class=\"mt-4 font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl\">Why CulebraLuxe</h2>\
                 <p class=\"mt-6 text-sm font-medium leading-relaxed text-foreground\">Selling on Culebra is different.</p>\
                 <p class=\"mt-5 max-w-sm text-sm font-light leading-relaxed text-muted-foreground\">\
                   This is not a conventional real estate market. Inventory is limited, data is fragmented, and every \
                   property is unique. Value is shaped by factors an algorithm will never see.</p>\
               </div>\
               <div class=\"grid gap-10 md:col-span-9 md:grid-cols-3 md:gap-0\">{cards}</div>\
             </div>\
           </div>\
         </section>"
    )
}

/// 02 — Our Process: six steps, each cut into an arrow on a wide screen.
fn seller_process() -> String {
    let steps = SELLER_PROCESS
        .iter()
        .map(|(title, body, icon_name)| {
            let icon = icon(icon_name, "mx-auto h-8 w-8 text-accent", "1.25").unwrap_or_else(|| {
                panic!("the Sellers page asks for an icon that is not in the table: {icon_name}")
            });
            format!(
                "<div class=\"relative h-full bg-[#f8f6f1] px-5 py-7 text-center \
                   lg:[clip-path:polygon(0_0,88%_0,100%_50%,88%_100%,0_100%,12%_50%)] lg:px-7\">{icon}\
                   <p class=\"mt-5 text-[10px] font-medium uppercase tracking-[0.16em] text-foreground\">{title}</p>\
                   <p class=\"mt-4 text-xs font-light leading-relaxed text-muted-foreground\">{body}</p>\
                 </div>",
                icon = icon,
                title = escape(title),
                body = escape(body),
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"border-b border-border bg-[#efebe3] px-6 py-20 md:px-12 md:py-24\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"grid gap-12 md:grid-cols-12 md:gap-12\">\
               <div class=\"md:col-span-2\">\
                 <p class=\"text-xs font-light uppercase tracking-[0.28em] text-accent\">02</p>\
                 <h2 class=\"mt-4 font-serif text-3xl font-light text-foreground\">Our Process</h2>\
                 <p class=\"mt-5 max-w-xs text-sm font-light leading-relaxed text-muted-foreground\">\
                   A considered approach from first conversation to closing.</p>\
               </div>\
               <div class=\"md:col-span-10\">\
                 <div class=\"grid gap-2 sm:grid-cols-2 lg:grid-cols-6\">{steps}</div>\
               </div>\
             </div>\
           </div>\
         </section>"
    )
}

// The two braces in the market diagram. Held as constants because a `{` or `}` inside a format string has to be doubled,
// and a brace that means a brace is easier to read than an escape sequence.
const SELLER_BRACE_RIGHT: &str =
    "<span class=\"select-none font-serif text-[150px] font-extralight leading-none text-accent/35\">}</span>";
const SELLER_BRACE_LEFT: &str =
    "<span class=\"select-none font-serif text-[150px] font-extralight leading-none text-accent/35\">{</span>";

/// 03 — Market Positioning: five dimensions in, one considered position out.
fn seller_market() -> String {
    let inputs = |rows: &[(&str, &str, &str)]| {
        rows.iter()
            .map(|(title, body, icon_name)| {
                let icon = icon(icon_name, "mt-1 h-7 w-7 shrink-0 text-accent", "1.25")
                    .unwrap_or_else(|| {
                        panic!("the Sellers page asks for an icon that is not in the table: {icon_name}")
                    });
                format!(
                    "<div class=\"flex gap-4\">{icon}<div>\
                       <p class=\"text-[10px] font-medium uppercase tracking-[0.15em] text-foreground\">{title}</p>\
                       <p class=\"mt-2 text-xs font-light leading-relaxed text-muted-foreground\">{body}</p>\
                     </div></div>",
                    icon = icon,
                    title = escape(title),
                    body = escape(body),
                )
            })
            .collect::<String>()
    };
    format!(
        "<section class=\"border-b border-border bg-[#f8f6f1] px-6 py-24 md:px-12 md:py-32\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"grid gap-14 md:grid-cols-12 md:gap-14\">\
               <div class=\"md:col-span-3\">\
                 <p class=\"text-xs font-light uppercase tracking-[0.28em] text-accent\">03</p>\
                 <h2 class=\"mt-4 font-serif text-3xl font-light leading-[1.05] text-foreground md:text-4xl\">\
                   Market<br />Positioning</h2>\
                 <p class=\"mt-5 text-sm font-medium text-foreground\">Position before promotion.</p>\
                 <p class=\"mt-5 max-w-sm text-sm font-light leading-relaxed text-muted-foreground\">\
                   We analyze five key dimensions to determine how your property should be positioned in today&#39;s \
                   Culebra market.</p>\
               </div>\
               <div class=\"md:col-span-9\">\
                 <div class=\"rounded-sm border border-border bg-[#fcfbf8] px-6 py-10 md:px-10 md:py-12\">\
                   <div class=\"grid items-center gap-10 lg:grid-cols-[1fr_auto_1.3fr_auto_1fr] lg:gap-5\">\
                     <div class=\"space-y-9\">{left}</div>\
                     <div class=\"hidden items-center justify-center lg:flex\">{brace_right}</div>\
                     <div class=\"flex min-w-0 flex-col items-center\">\
                       <div class=\"flex aspect-square w-full max-w-[310px] flex-col items-center justify-center \
                         rounded-full border border-accent/30 bg-[#eee8de] px-10 text-center \
                         shadow-[0_8px_30px_rgba(0,0,0,0.025)]\">\
                         <p class=\"font-serif text-4xl font-light text-accent\">CL</p>\
                         <p class=\"mt-5 text-xs font-medium uppercase tracking-[0.2em] text-foreground\">\
                           CulebraLuxe<br />Analysis</p>\
                         <div class=\"mt-5 space-y-1 text-xs font-light text-muted-foreground\">\
                           <p>Local knowledge.</p><p>Market intelligence.</p><p>Individual judgment.</p>\
                         </div>\
                       </div>\
                       <div class=\"h-12 w-px bg-accent/30\"></div>\
                       <div class=\"border border-accent/20 bg-[#f3eee6] px-10 py-5 text-center\">\
                         <p class=\"text-[10px] font-medium uppercase tracking-[0.16em] text-foreground\">Market Position</p>\
                         <p class=\"mt-2 font-serif text-sm font-light text-muted-foreground\">\
                           Price · Strategy · Timing</p>\
                       </div>\
                     </div>\
                     <div class=\"hidden items-center justify-center lg:flex\">{brace_left}</div>\
                     <div class=\"space-y-12\">{right}</div>\
                   </div>\
                   <div class=\"mt-10 border-t border-border pt-6 text-center lg:hidden\">\
                     <p class=\"text-[10px] font-light uppercase tracking-[0.18em] text-muted-foreground\">\
                       Five dimensions inform one considered market position.</p>\
                   </div>\
                 </div>\
               </div>\
             </div>\
           </div>\
         </section>",
        left = inputs(&SELLER_MARKET_LEFT),
        right = inputs(&SELLER_MARKET_RIGHT),
        brace_right = SELLER_BRACE_RIGHT,
        brace_left = SELLER_BRACE_LEFT,
    )
}

/// 04 — Presentation & Exposure: the two lists, and the line that ends the section.
fn seller_presentation() -> String {
    let list = |rows: &[(&str, &str)]| {
        rows.iter()
            .map(|(title, icon_name)| {
                let icon = icon(icon_name, "h-5 w-5 shrink-0 text-accent", "1.25").unwrap_or_else(|| {
                    panic!("the Sellers page asks for an icon that is not in the table: {icon_name}")
                });
                format!(
                    "<div class=\"flex items-center gap-4\">{icon}\
                       <p class=\"text-sm font-light text-foreground\">{title}</p></div>",
                    icon = icon,
                    title = escape(title),
                )
            })
            .collect::<String>()
    };
    let panel = |label: &str, rows: &[(&str, &str)]| {
        format!(
            "<div class=\"h-full border-border bg-[#faf8f4] p-8 md:border-l md:px-12 md:py-10\">\
               <p class=\"text-xs font-medium uppercase tracking-[0.18em] text-foreground\">{label}</p>\
               <div class=\"mt-8 space-y-6\">{rows}</div>\
             </div>",
            label = escape(label),
            rows = list(rows),
        )
    };
    format!(
        "<section class=\"border-b border-border bg-[#f0ece5] px-6 py-24 md:px-12 md:py-28\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"grid gap-14 md:grid-cols-12 md:gap-14\">\
               <div class=\"md:col-span-3\">\
                 <p class=\"text-xs font-light uppercase tracking-[0.28em] text-accent\">04</p>\
                 <h2 class=\"mt-4 font-serif text-3xl font-light leading-[1.05] text-foreground md:text-4xl\">\
                   Presentation<br />&amp; Exposure</h2>\
                 <p class=\"mt-5 max-w-xs text-sm font-light leading-relaxed text-muted-foreground\">\
                   The right property deserves more than a listing.</p>\
                 <div class=\"relative mt-10 aspect-[4/3] overflow-hidden\">\
                   <img src=\"/images/hero-villa.png\" \
                     alt=\"Culebra property prepared for market presentation\" \
                     sizes=\"(min-width: 768px) 25vw, 100vw\" \
                     class=\"absolute inset-0 h-full w-full object-cover\" /></div>\
               </div>\
               <div class=\"grid gap-12 md:col-span-9 md:grid-cols-2 md:gap-0\">{presentation}{distribution}</div>\
             </div>\
             <p class=\"mt-14 border-t border-border pt-8 text-center font-serif text-lg font-light text-foreground\">\
               Exposure is not the strategy. Exposure serves the strategy.</p>\
           </div>\
         </section>",
        presentation = panel("Presentation", &SELLER_PRESENTATION),
        distribution = panel("Distribution", &SELLER_DISTRIBUTION),
    )
}

/// 05 — Representation: six stages across the width.
fn seller_representation() -> String {
    let stages = SELLER_REPRESENTATION
        .iter()
        .map(|(title, body, icon_name)| {
            let icon = icon(icon_name, "mx-auto h-8 w-8 text-accent", "1.25").unwrap_or_else(|| {
                panic!("the Sellers page asks for an icon that is not in the table: {icon_name}")
            });
            format!(
                "<div class=\"h-full text-center lg:border-l lg:border-border lg:px-6\">{icon}\
                   <h3 class=\"mt-5 text-[10px] font-medium uppercase tracking-[0.15em] text-foreground\">{title}</h3>\
                   <p class=\"mt-4 text-xs font-light leading-relaxed text-muted-foreground\">{body}</p>\
                 </div>",
                icon = icon,
                title = escape(title),
                body = escape(body),
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"bg-[#f7f4ef] px-6 py-24 md:px-12 md:py-28\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"grid gap-12 md:grid-cols-12 md:gap-12\">\
               <div class=\"md:col-span-2\">\
                 <p class=\"text-xs font-light uppercase tracking-[0.28em] text-accent\">05</p>\
                 <h2 class=\"mt-4 font-serif text-3xl font-light text-foreground\">Representation</h2>\
                 <p class=\"mt-5 text-sm font-light leading-relaxed text-muted-foreground\">\
                   From first showing to closing.</p>\
               </div>\
               <div class=\"grid gap-10 sm:grid-cols-2 md:col-span-10 lg:grid-cols-6 lg:gap-0\">{stages}</div>\
             </div>\
           </div>\
         </section>"
    )
}

/// The closing band.
fn seller_cta() -> String {
    "<section class=\"bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-28\">\
       <div class=\"mx-auto max-w-[1600px] text-center\">\
         <h2 class=\"font-serif text-3xl font-light leading-[1.1] md:text-4xl\">\
           Every property starts with a conversation.</h2>\
         <p class=\"mx-auto mt-5 max-w-xl text-sm font-light leading-relaxed text-primary-foreground/70\">\
           Tell us about your property. We&#39;ll discuss your objectives, the market, and whether working together \
           makes sense.</p>\
         <a href=\"/contact\" class=\"mt-10 inline-flex border border-primary-foreground/40 px-8 py-4 text-xs \
           font-light uppercase tracking-[0.22em] transition-colors hover:bg-primary-foreground hover:text-primary\">\
           Discuss your property</a>\
       </div>\
     </section>"
        .to_string()
}

/// `app/buyers/page.tsx` — the parent: the showroom's inventory, and the guidance around it.
///
/// THE INVENTORY IS THE PAGE. It arrives as listings — the same public properties and the same card renderer the homepage
/// uses — rather than as editorial copy, because a buyers page that shows no homes is a brochure. What is NOT here yet is
/// the showroom's own machinery: the filter bar, the URL contract that drives it, saving, comparing and the carousel are
/// interactive state, and they are the next slice rather than something to fake with a static grid.
fn site_buyers(model: &Model) -> String {
    let listings: &[crate::model::Listing] = model
        .page
        .as_ref()
        .map(|page| page.listings.as_slice())
        .unwrap_or(&[]);
    let steps = BUYER_STEPS
        .iter()
        .map(|(number, title, body)| {
            format!(
                "<div class=\"border-t border-border pt-7\">\
                   <span class=\"font-serif text-2xl font-light text-accent\">{number}</span>\
                   <h3 class=\"mt-7 font-serif text-2xl font-light leading-snug text-foreground\">{title}</h3>\
                   <p class=\"mt-4 text-sm font-light leading-relaxed text-muted-foreground\">{body}</p>\
                 </div>",
                number = escape(number),
                title = escape(title),
                body = escape(body),
            )
        })
        .collect::<String>();
    let services = BUYER_SERVICES
        .iter()
        .map(|item| {
            format!(
                "<li class=\"py-5 text-sm font-light tracking-wide text-foreground/80\">{}</li>",
                escape(item)
            )
        })
        .collect::<String>();
    format!(
        "{hero}{inventory}\
         <section class=\"px-6 py-24 md:px-12 md:py-32\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"mb-16 max-w-3xl md:mb-20\">\
               <p class=\"mb-5 text-xs font-light uppercase tracking-[0.34em] text-accent\">Buying on Culebra</p>\
               <h2 class=\"text-balance font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl\">\
                 A considered path from first look to ownership.</h2>\
               <p class=\"mt-6 max-w-2xl text-sm font-light leading-relaxed text-muted-foreground\">\
                 Finding the right property is only the beginning. We guide the details that follow — privately, \
                 carefully, and with an understanding of how transactions work on the island.</p>\
             </div>\
             <div class=\"grid gap-12 md:grid-cols-2 lg:grid-cols-4 lg:gap-8\">{steps}</div>\
           </div>\
         </section>\
         <section class=\"bg-foreground px-6 py-24 text-background md:px-12 md:py-32\">\
           <div class=\"mx-auto grid max-w-[1600px] gap-14 lg:grid-cols-12 lg:items-center\">\
             <div class=\"lg:col-span-7\">\
               <p class=\"mb-5 text-xs font-light uppercase tracking-[0.34em] text-background/50\">Private Opportunities</p>\
               <h2 class=\"max-w-4xl text-balance font-serif text-4xl font-light leading-[1.05] md:text-5xl lg:text-6xl\">\
                 Not every exceptional property is publicly listed.</h2>\
             </div>\
             <div class=\"lg:col-span-4 lg:col-start-9\">\
               <p class=\"text-sm font-light leading-relaxed text-background/70\">\
                 Culebra remains a small island with a highly relationship-driven property market. Some owners prefer \
                 discretion. Tell us what you are looking for, and we can widen the search beyond the public inventory.</p>\
               <a href=\"/contact\" class=\"group mt-8 inline-flex items-center gap-3 border border-background/30 px-8 \
                 py-4 text-xs font-light uppercase tracking-[0.2em] transition-colors duration-500 \
                 hover:border-background\">Begin a private search\
                 <span class=\"inline-block h-px w-8 bg-background transition-all duration-500 group-hover:w-12\"></span></a>\
             </div>\
           </div>\
         </section>\
         <section class=\"px-6 py-24 md:px-12 md:py-32\">\
           <div class=\"mx-auto grid max-w-[1600px] gap-14 lg:grid-cols-12 lg:gap-20\">\
             <div class=\"lg:col-span-5\">\
               <p class=\"mb-5 text-xs font-light uppercase tracking-[0.34em] text-accent\">Beyond the Search</p>\
               <h2 class=\"max-w-lg text-balance font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl\">\
                 Every detail, quietly handled.</h2>\
             </div>\
             <div class=\"lg:col-span-6 lg:col-start-7\">\
               <ul class=\"flex flex-col divide-y divide-border border-y border-border\">{services}</ul>\
             </div>\
           </div>\
         </section>",
        hero = page_hero(
            "For Buyers",
            "Find your place on Culebra.",
            Some(
                "Exceptional homes, villas, and land — presented with the perspective of people who know the island intimately."
            ),
            "/images/hero-villa.png",
            "A modern luxury villa overlooking the Culebra coastline",
        ),
        // The inventory, through the card renderer the homepage already uses, so a listing looks the same wherever it
        // appears. An island with nothing published renders as an empty grid, not as invented homes.
        inventory = featured_properties(listings),
    )
}

fn about_founder() -> String {
    let credentials = ABOUT_CREDENTIALS
        .iter()
        .map(|item| {
            format!(
                "<div class=\"flex items-center gap-4 py-4\"><span class=\"text-accent\">○</span>\
                   <p class=\"text-sm font-light text-foreground\">{}</p></div>",
                escape(item)
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"px-6 py-20 md:px-12 md:py-28\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <div class=\"grid items-stretch gap-12 md:grid-cols-2 md:gap-0\">\
               <div class=\"h-full min-h-[520px] overflow-hidden bg-muted md:min-h-[680px]\">\
                 <img src=\"/images/about/lisa-portrait.jpg\" \
                   alt=\"Lisa Penfield, founder and broker of CulebraLuxe\" \
                   class=\"h-full w-full object-cover\" /></div>\
               <div class=\"flex h-full flex-col justify-center px-0 py-4 md:px-16 lg:px-20\">\
                 <p class=\"text-xs font-light uppercase tracking-[0.28em] text-accent\">Lisa Penfield</p>\
                 <h2 class=\"mt-5 font-serif text-4xl font-light leading-[1.1] text-foreground md:text-5xl\">\
                   Founder &amp; Broker</h2>\
                 <div class=\"mt-8 max-w-xl space-y-5 text-sm font-light leading-relaxed text-muted-foreground\">\
                   <p>Lisa Penfield has spent more than two decades reading Culebra — first as a world-champion \
                     windsurfer studying its wind and water, later as a broker studying its land and light.</p>\
                   <p>The same discipline that wins a championship applies just as well to representing a home: \
                     patience, precision, and knowing exactly when to act.</p>\
                   <p>Licensed in Puerto Rico since 2002, Lisa also brings 20 years of experience in luxury \
                     hospitality and corporate travel at Hyatt Dorado Beach. A full-time resident of Culebra, she works \
                     with a deliberately small number of clients each year — enough to give every search, listing, and \
                     negotiation her full attention.</p>\
                 </div>\
                 <p class=\"mt-8 font-serif text-xl font-light italic text-accent\">\
                   “Here, it’s personal. Always has been.”</p>\
                 <div class=\"mt-10 divide-y divide-border border-y border-border\">{credentials}</div>\
               </div>\
             </div>\
           </div>\
         </section>"
    )
}

/// "What we value" — three centred cards, each with its own icon.
fn about_values() -> String {
    let cards = ABOUT_VALUES
        .iter()
        .map(|(title, body, icon_name)| {
            let icon = icon(icon_name, "h-10 w-10", "1.25").unwrap_or_else(|| {
                panic!("the About page asks for an icon that is not in the table: {icon_name}")
            });
            format!(
                "<div class=\"px-4 text-center md:border-r md:border-border md:px-12 last:md:border-r-0\">\
                   <div class=\"mx-auto mb-6 flex h-12 w-12 items-center justify-center text-accent\">{icon}</div>\
                   <h3 class=\"text-sm font-medium uppercase tracking-[0.16em] text-foreground\">{title}</h3>\
                   <p class=\"mx-auto mt-4 max-w-sm text-sm font-light leading-relaxed text-muted-foreground\">{body}</p>\
                 </div>",
                icon = icon,
                title = escape(title),
                body = escape(body),
            )
        })
        .collect::<String>();
    format!(
        "<section class=\"px-6 py-20 md:px-12 md:py-24\">\
           <div class=\"mx-auto max-w-[1600px]\">\
             <h2 class=\"text-center font-serif text-3xl font-light text-foreground md:text-4xl\">What we value</h2>\
             <div class=\"mt-14 grid gap-14 md:grid-cols-3 md:gap-0\">{cards}</div>\
           </div>\
         </section>"
    )
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
        "site-buyers" => Some(site_buyers(model)),
        "site-guide" => Some(site_guide(model)),
        "site-sellers" => Some(site_sellers(model)),
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

    /// THE ROUTE CONTRACT, EXACTLY — the destination of every menu item, not its label.
    ///
    /// The existing header test checks that the labels are present and that there are eight capsules. That is satisfied by
    /// a menu where every link points somewhere wrong, which is precisely the failure this header has already had once:
    /// `unwrap_or("/")` turned an unresolvable key into Home, silently. A label is what a visitor reads; the href is where
    /// they end up, and only the second one is the contract.
    #[test]
    fn the_site_header_links_to_the_exact_urls_of_the_public_site() {
        let html = render(&Model {
            screen: target("site-home"),
            ..Model::default()
        });
        for (label, href) in [
            ("Buyers", "/buyers"),
            ("Sellers", "/sellers"),
            ("Services", "/services"),
            ("Guide", "/guide"),
            ("About", "/about"),
            ("FAQ", "/faq"),
            ("Contact", "/contact"),
            ("Portal", "/portal/dashboard"),
        ] {
            let expected = format!(
                "href=\"{href}\" class=\"top-nav-capsule top-nav-capsule--tight\">{label}</a>"
            );
            assert!(
                html.contains(&expected),
                "{label} does not link to {href} with the shared capsule class; expected {expected}"
            );
        }
        // The logo is home, and it is the only element that promises home.
        assert!(
            html.contains("href=\"/\" aria-label=\"CulebraLuxe home\""),
            "the logo is what goes home"
        );
    }

    /// The page hero, which five pages share, so it is tested once on its own rather than through each of them.
    #[test]
    fn the_page_hero_renders_its_copy_and_only_an_intro_when_there_is_one() {
        let with_intro = page_hero(
            "Selling on Culebra",
            "Presented to the few who truly belong here.",
            Some("Extraordinary properties deserve more than exposure."),
            "/images/coastline.png",
            "Culebra coastline and homes overlooking the Caribbean",
        );
        assert!(with_intro.contains("Selling on Culebra"));
        assert!(with_intro.contains("Presented to the few who truly belong here."));
        assert!(with_intro.contains("Extraordinary properties deserve more than exposure."));
        assert!(with_intro.contains("src=\"/images/coastline.png\""));
        // The things that make it the same hero on every page: the height, the padding that clears the fixed header, and
        // the scrim that makes ivory text legible over a photograph.
        assert!(with_intro.contains("min-h-[68svh]"));
        assert!(with_intro.contains("pb-16 pt-40"));
        assert!(with_intro.contains("bg-gradient-to-t from-black/70 via-black/25 to-black/40"));

        // No intro means no paragraph, rather than an empty line where a sentence should be.
        let bare = page_hero("About Us", "Devoted to a single island.", None, "", "");
        assert!(!bare.contains("mt-8 max-w-2xl"), "an absent intro renders nothing");
        // And an empty image path falls back rather than rendering a broken image.
        assert!(bare.contains("src=\"/placeholder.svg\""));
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
