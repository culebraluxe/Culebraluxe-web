//! The Model: every screen the application serves, and the state the shell renders.
//!
//! THE SCOPE IS THE APPLICATION, NOT A MENU. This module used to list only the portal menu, which meant screens that
//! exist and are reachable were "out of scope" by my judgement rather than by fact. The captain's rule now: every
//! route that serves a page gets a screen. `SCREENS` is that list, and it is checked against the live route tree by a
//! test so the two cannot drift.
//!
//! A TABLE, NOT AN ENUM. At this size an enum needs five match arms per screen (title, path, key, area, group) and
//! stays in sync by hand. One row per screen is one place to look and one place to be wrong.
//!
//! `nav` records what the *registry* says about a route, not what I think of it: `lib/navigation/registry.ts` is the
//! single source of truth for "what navigation belongs under this surface", and it documents four routes as RETIRED
//! FROM THE NAV with the code left in place. Those are ported like everything else and simply not listed.

use std::collections::BTreeMap;

/// Which operating surface a screen belongs to. The first six mirror `lib/navigation/registry.ts`; `Site` is the
/// public site, which the registry does not cover because it is not part of the portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Core,
    Accounting,
    Marketing,
    Ops,
    Support,
    Tech,
    Site,
}

impl Surface {
    /// The label the registry uses. `OPPS` is the registry's own spelling and is kept rather than corrected here.
    pub fn label(self) -> &'static str {
        match self {
            Self::Core => "CORE",
            Self::Accounting => "ACCOUNTING",
            Self::Marketing => "MARKETING",
            Self::Ops => "OPPS",
            Self::Support => "SUPPORT",
            Self::Tech => "TECH",
            Self::Site => "SITE",
        }
    }

    pub const ALL: &'static [Surface] = &[
        Surface::Core,
        Surface::Accounting,
        Surface::Marketing,
        Surface::Ops,
        Surface::Support,
        Surface::Tech,
        Surface::Site,
    ];

    /// A stable key for the DOM, in the same style as `Screen::key`. The label is for reading; this is for addressing.
    pub fn key(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Accounting => "accounting",
            Self::Marketing => "marketing",
            Self::Ops => "ops",
            Self::Support => "support",
            Self::Tech => "tech",
            Self::Site => "site",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|surface| surface.key() == key)
    }
}

/// What navigation the registry gives a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    /// A normal nav entry under its surface.
    Listed,
    /// A route the registry RETIRED FROM THE NAV, with the code deliberately left in place. Ported, not listed.
    Retired,
    /// A real route the registry never listed: reachable from its parent screen, not a nav entry of its own.
    Unlisted,
    /// A record screen: reached by opening a row, never from the nav, because "one client, but which one?" is not
    /// something a menu can offer.
    Record,
}

/// One screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub key: &'static str,
    pub title: &'static str,
    /// The live route this port replaces. A `[bracket]` marks the part the record key supplies.
    pub path: &'static str,
    pub surface: Surface,
    pub nav: Nav,
    /// Why this screen has no rows, when it has none. A screen that cannot be wired must say so in its own words
    /// rather than looking broken — and "no read model exists" is a different sentence from "this is a placeholder".
    pub deferred: Option<&'static str>,
    /// For a record screen: the key of the list screen it is opened from.
    pub detail_of: Option<&'static str>,
}

impl Screen {
    pub fn is_deferred(self) -> bool {
        self.deferred.is_some()
    }

    /// Listed in the nav, and in the surface the registry puts it in.
    pub fn is_listed(self) -> bool {
        self.nav == Nav::Listed
    }
}

/// Look a screen up by its key. `Option` is deliberate: an unknown key is a stale page or a typo, and neither should
/// navigate anywhere.
pub fn screen(key: &str) -> Option<Screen> {
    SCREENS
        .iter()
        .copied()
        .find(|candidate| candidate.key == key)
}

/// The record screen a list screen opens into, if it has one.
pub fn record_for(list_key: &str) -> Option<Screen> {
    SCREENS
        .iter()
        .copied()
        .find(|candidate| candidate.detail_of == Some(list_key))
}

/// The screen a surface opens on: its first listed screen in table order. That is the surface's home, and the one
/// destination the switcher can offer without guessing.
pub fn home(surface: Surface) -> Option<Screen> {
    SCREENS
        .iter()
        .copied()
        .find(|candidate| candidate.surface == surface && candidate.is_listed())
}

/// The listed screens of one surface, in table order.
pub fn listed(surface: Surface) -> impl Iterator<Item = Screen> {
    SCREENS
        .iter()
        .copied()
        .filter(move |candidate| candidate.surface == surface && candidate.is_listed())
}

/// Every screen the application serves. Labels, paths and nav status come from the code, not from memory:
/// `lib/navigation/registry.ts` for the portal surfaces, and the route tree under `app/` for the rest.
pub const SCREENS: &[Screen] = &[
    // ---- CORE (the registry's surface is NEXUS, labelled CORE) ----
    Screen { key: "dashboard", title: "Cockpit", path: "/portal/dashboard", surface: Surface::Core, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "clients", title: "Clients", path: "/portal/clients", surface: Surface::Core, nav: Nav::Listed, deferred: None, detail_of: None },
    // The captain's call: the model is proven, so Project Management starts. The list is wired; the three widgets
    // (React Arborist tree, Gantt, FullCalendar) stay in TypeScript until the port can render a screen of its own markup
    // and host them as islands.
    Screen { key: "projects", title: "Projects", path: "/portal/projects", surface: Surface::Core, nav: Nav::Listed, deferred: None, detail_of: None },
    // Keys are addresses, not labels: this one stays `deals` because the rows route and the host already address it by
    // that name, while the user sees the registry's word for it.
    Screen { key: "deals", title: "Contracts", path: "/portal/deals", surface: Surface::Core, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "cabinet", title: "Cabinet", path: "/portal/documents", surface: Surface::Core, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "workflows", title: "Workflows", path: "/portal/workflows", surface: Surface::Core, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "forms", title: "Forms", path: "/portal/forms", surface: Surface::Core, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "seller-strategy", title: "Seller Strategy", path: "/portal/core/seller-strategy", surface: Surface::Core, nav: Nav::Listed, deferred: None, detail_of: None },

    // ---- ACCOUNTING ----
    Screen { key: "accounting", title: "Dashboard", path: "/portal/accounting", surface: Surface::Accounting, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "accounting-receivables", title: "Receivables", path: "/portal/accounting/receivables", surface: Surface::Accounting, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "accounting-expenses", title: "Expenses", path: "/portal/accounting/expenses", surface: Surface::Accounting, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "accounting-pnl", title: "P&L Statement", path: "/portal/accounting/pnl", surface: Surface::Accounting, nav: Nav::Listed, deferred: None, detail_of: None },
    // The page's own header says it: "Receipt Scanner (FAKE V1). Polished visual placeholder for the future OCR
    // workflow. Deterministic demo extraction only — real OCR is deferred to a separate story." A placeholder BY
    // DESIGN is a different thing from a screen nobody wired, and it says which one it is.
    // The scanner is a REAL screen now — `portal_accounting_receipt_scanner.rs` renders the workflow, the drop surface, the
    // reviewed draft and the prototype notice itself — so the registry no longer calls it deferred. That note used to be the
    // screen's whole body under the string renderer, and it also put "(no data yet)" beside the rail entry, which would now
    // be wrong: this screen reads nothing, and it says so in its own words where the reader can see them.
    Screen { key: "accounting-receipt-scanner", title: "Receipt Scanner", path: "/portal/accounting/receipt-scanner", surface: Surface::Accounting, nav: Nav::Listed, deferred: None, detail_of: None },

    // ---- MARKETING ----
    Screen { key: "marketing", title: "Dashboard", path: "/portal/marketing", surface: Surface::Marketing, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "marketing-syndication", title: "Syndication", path: "/portal/marketing/syndication", surface: Surface::Marketing, nav: Nav::Listed, deferred: None, detail_of: None },

    // ---- OPPS (the registry's own spelling) ----
    Screen { key: "issues", title: "Issue Queue", path: "/portal/issues", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "needs-review", title: "Needs Review", path: "/portal/needs-review", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "property-admin", title: "Data Workbench", path: "/portal/property-admin", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "media-admin", title: "Media Audit", path: "/portal/media-admin", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "property-media", title: "Property Media", path: "/portal/property-media", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "identity-quality", title: "Identity Quality", path: "/portal/identity-quality", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "client-admin", title: "Client Administration", path: "/portal/client-admin", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "reporting", title: "Reporting", path: "/portal/reporting", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
    // Reachable from Property Admin, never a nav entry of its own.
    Screen { key: "decision-analysis", title: "Decision Analysis", path: "/portal/decision-analysis", surface: Surface::Ops, nav: Nav::Unlisted, deferred: Some("Converted into Seller Strategy; this route is what is left of it. There is nothing here to render, and the screen that replaced it is the one to look at."), detail_of: None },

    // ---- SUPPORT ----
    Screen { key: "system-health", title: "System Health", path: "/portal/system-health", surface: Surface::Support, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "db-test", title: "DB Test", path: "/portal/db-test", surface: Surface::Support, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "whatsapp-meta", title: "WhatsApp Diagnostic", path: "/portal/admin/whatsapp-meta", surface: Surface::Support, nav: Nav::Listed, deferred: None, detail_of: None },
    // The registry puts Security under SUPPORT even though its route is /portal/settings. It is ONE screen: an earlier
    // draft listed it twice, under two surfaces, for the same route.
    Screen { key: "security", title: "Security", path: "/portal/settings", surface: Surface::Support, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "settings-authorities", title: "Authorities", path: "/portal/settings/authorities", surface: Surface::Support, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "settings-roles", title: "Roles", path: "/portal/settings/roles", surface: Surface::Support, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "settings-users", title: "Users", path: "/portal/settings/users", surface: Surface::Support, nav: Nav::Unlisted, deferred: None, detail_of: None },
    // Moved to TECH at the captain's instruction: this is the screen used to prove the WhatsApp integration works, and
    // TECH is where the integration screens live.
    Screen { key: "whatsapp-coexistence", title: "WhatsApp Coexistence", path: "/portal/admin/whatsapp-coexistence", surface: Surface::Tech, nav: Nav::Unlisted, deferred: None, detail_of: None },

    // ---- screens that exist and are reached from elsewhere (not in the registry) ----
    Screen { key: "attention", title: "Attention", path: "/portal/attention", surface: Surface::Core, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "activity", title: "Activity", path: "/portal/activity", surface: Surface::Core, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "showings", title: "Showings", path: "/portal/showings", surface: Surface::Core, nav: Nav::Unlisted, deferred: None, detail_of: None },

    // ---- TECH ----
    Screen { key: "tech", title: "Cockpit", path: "/portal/tech", surface: Surface::Tech, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "storyboard", title: "Story Board", path: "/portal/storyboard", surface: Surface::Tech, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "design-lab", title: "UI Lab", path: "/portal/design-lab", surface: Surface::Tech, nav: Nav::Listed, deferred: None, detail_of: None },
    // GPT's lab, merged from origin/main. It is a TypeScript MVI lab (`ui/framer-ui-lab/model.ts` +
    // `framer-ui-lab-controller.ts`) rendering a React view with its own CSS module
    // (`components/portal/tech/framer-ui-lab.tsx`, 438 lines), and it has no read model — so there is nothing for a
    // rows endpoint to return, and the experiments are promoted into the Rust view language one at a time if the
    // captain likes them. Registered under TECH as "Framer UI Lab" with authority tech.access.
    Screen { key: "framer-ui-lab", title: "Framer UI Lab", path: "/portal/tech/framer-ui-lab", surface: Surface::Tech, nav: Nav::Unlisted, deferred: Some("A client-rendered experiment lab: its model and controller live in ui/framer-ui-lab and its view is a React component with its own CSS module, so there is no read model to return. Promoting an experiment into the shared Rust view language is a separate, deliberate step."), detail_of: None },
    Screen { key: "media-test", title: "Media Test", path: "/portal/media-test", surface: Surface::Tech, nav: Nav::Unlisted, deferred: Some("A manual harness for the media pipeline: it uploads, transforms and inspects. There is no read model to render, and a list of rows would not be the screen that was here.") , detail_of: None },
    // A screen the captain asked for: what Rust does with layout that the TypeScript pages do not, rendering on the same
    // design tokens. Its route is the only one this port adds on purpose, and it serves the port itself.
    // The sibling of the TypeScript design lab, which stays TypeScript deliberately: that one is a catalogue of React
    // components, and nothing Rust renders would tell the same story. This one is the control vocabulary this crate
    // owns, wired to the model, and it is meant to be OPERATED rather than read.
    Screen { key: "rust-lab", title: "Rust Lab", path: "/portal/tech/rust-lab", surface: Surface::Tech, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "tech-lab", title: "Tech Lab", path: "/portal/tech/lab", surface: Surface::Tech, nav: Nav::Listed, deferred: None, detail_of: None },
    // RETIRED FROM THE NAV (2026-09-13, the registry's own note): "the code stays, the links go". Command Center,
    // Command Console, GROK and the Flight Recorder LIST were each an attempt at the same problem that never got
    // used, and their names were close enough to "the cockpit" that one conversation could mean five. Ported like
    // everything else, and not listed — which is exactly what the registry says about them.
    Screen { key: "command-center", title: "Command Center", path: "/portal/command-center", surface: Surface::Tech, nav: Nav::Retired, deferred: None, detail_of: None },
    Screen { key: "command-console", title: "Command Console", path: "/portal/command-console", surface: Surface::Tech, nav: Nav::Retired, deferred: None, detail_of: None },
    Screen { key: "tech-grok", title: "GROK", path: "/portal/tech/grok", surface: Surface::Tech, nav: Nav::Retired, deferred: Some("A retired route, kept in the code deliberately. It has no read model and nothing links to it."), detail_of: None },
    Screen { key: "tech-flight-recorder", title: "Flight Recorder", path: "/portal/tech/flight-recorder", surface: Surface::Tech, nav: Nav::Retired, deferred: None, detail_of: None },
    Screen { key: "tech-app-errors", title: "App Errors", path: "/portal/tech/app-errors", surface: Surface::Tech, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "tech-runs", title: "Runs", path: "/portal/tech/runs", surface: Surface::Tech, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "tech-kanban", title: "Kanban", path: "/portal/tech/kanban", surface: Surface::Tech, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "tech-line", title: "Line", path: "/portal/tech/line", surface: Surface::Tech, nav: Nav::Unlisted, deferred: None, detail_of: None },

    // ---- Record screens: opened from a row, never from the nav ----
    Screen { key: "client-record", title: "Client", path: "/portal/clients/[personId]", surface: Surface::Core, nav: Nav::Record, deferred: None, detail_of: Some("clients") },
    Screen { key: "deal-record", title: "Deal", path: "/portal/deals/[dealId]", surface: Surface::Core, nav: Nav::Record, deferred: None, detail_of: Some("deals") },
    Screen { key: "form-record", title: "Form", path: "/portal/forms/[formId]", surface: Surface::Core, nav: Nav::Record, deferred: None, detail_of: Some("forms") },
    Screen { key: "workflow-record", title: "Workflow instance", path: "/portal/workflows/[instanceId]", surface: Surface::Core, nav: Nav::Record, deferred: None, detail_of: Some("workflows") },
    Screen { key: "property-record", title: "Property record", path: "/portal/property-admin/[propertyId]", surface: Surface::Ops, nav: Nav::Record, deferred: None, detail_of: Some("property-admin") },
    Screen { key: "story-record", title: "Story", path: "/portal/storyboard/[id]", surface: Surface::Tech, nav: Nav::Record, deferred: None, detail_of: Some("storyboard") },
    // The registry explains where this belongs: the Flight Recorder "is reached from the SELECTED STORY's own detail
    // pane — for the instance that actually ran it". That link is NOT in the data: db/storyboard.ts carries no
    // workflow instance id, and listTraceEvents filters by instance/trace/correlation, none of which is a story run.
    // So it stays a real screen with nothing current to open it, recorded as a finding rather than papered over with a
    // row id that would fetch an empty trace.
    Screen { key: "trace-record", title: "Trace", path: "/portal/tech/flight-recorder/[instanceId]", surface: Surface::Tech, nav: Nav::Unlisted, deferred: None, detail_of: None },
    // Reached from a workflow instance, not from a row of the workflows list — so it is a real screen that no click
    // currently opens. Listed here so it is not forgotten, and unlisted in the nav so it does not pretend to be.
    Screen { key: "runtime-record", title: "Runtime inspector", path: "/portal/runtime-inspector/[instanceId]", surface: Surface::Support, nav: Nav::Unlisted, deferred: None, detail_of: None },

    // ---- SITE: the public front. Not covered by the portal registry, so these come from the route tree. ----
    Screen { key: "site-home", title: "Home", path: "/", surface: Surface::Site, nav: Nav::Listed, deferred: None , detail_of: None },
    // NO LIVE ROUTE: the app serves public properties only at /properties/[slug]; there is no index page, and
    // nothing links to one. This screen is a view over the public inventory read model (getProperties/getFilteredProperties)
    // which the app exposes only through the detail route. Empty `path` means exactly that, and the header says so
    // rather than naming a route that does not exist.
    // The public property index. It had NO LIVE ROUTE for a long time - the public read model was reachable only
    // through `/properties/[slug]` and nothing linked to an index - and the empty `path` here is what recorded that.
    // The route exists now, and this page is the reason the site does: every listing, rendered by Rust from the public
    // listing read model.
    Screen { key: "site-properties", title: "Properties", path: "/properties", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-property-detail", title: "Property", path: "/properties/[slug]", surface: Surface::Site, nav: Nav::Record, deferred: None, detail_of: Some("site-properties") },
    Screen { key: "site-about", title: "About", path: "/about", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-buyers", title: "Buyers", path: "/buyers", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-sellers", title: "Sellers", path: "/sellers", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-faq", title: "FAQ", path: "/faq", surface: Surface::Site, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "site-contact", title: "Contact", path: "/contact", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-guide", title: "Guide", path: "/guide", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-services", title: "Services", path: "/services", surface: Surface::Site, nav: Nav::Unlisted, deferred: None , detail_of: None },
    Screen { key: "site-privacy", title: "Privacy", path: "/privacy", surface: Surface::Site, nav: Nav::Unlisted, deferred: None , detail_of: None },
    Screen { key: "site-video", title: "Video", path: "/video", surface: Surface::Site, nav: Nav::Unlisted, deferred: Some("A tool-test page for video playback: one Mux player and a hardcoded playback id. The player is a third-party island and is not ported, so what is left is the note instead of the embed.") , detail_of: None },
    Screen { key: "site-whatsapp", title: "WhatsApp", path: "/whatsapp", surface: Surface::Site, nav: Nav::Unlisted, deferred: None , detail_of: None },
    Screen { key: "site-favorites", title: "Favorites", path: "/favorites", surface: Surface::Site, nav: Nav::Unlisted, deferred: Some("Never finished: this screen was meant to hold saved properties and never held anything. A Rust body belongs with the feature, not ahead of it - and the feature is a real decision, not a port.") , detail_of: None },

    // ---- ROUTES THE TABLE WAS MISSING. It is now checked against `find app -name page.tsx` (80 routes, minus the two
    // preview hosts this port adds), rather than against a registry that says of itself "Only EXISTING routes are
    // listed" — a statement about NAVIGATION, not about which routes exist. Titles are the route's own name, not a name
    // I invented for it.
    Screen { key: "login", title: "Login", path: "/login", surface: Surface::Site, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "login-recovery", title: "Login recovery", path: "/login/recovery", surface: Surface::Site, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "login-unauthorized", title: "Login unauthorized", path: "/login/unauthorized", surface: Surface::Site, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "auth-error", title: "Auth error", path: "/auth/error", surface: Surface::Site, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "review", title: "Review", path: "/review/[token]/[page]", surface: Surface::Site, nav: Nav::Unlisted, deferred: Some("Never got working: a multi-panel client review tool built out of five components that were never finished. The screen says so rather than pretending, and finishing the feature is a decision rather than a port."), detail_of: None },
    Screen { key: "portal-root", title: "Portal", path: "/portal", surface: Surface::Core, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "portal-auth-proof", title: "Portal auth proof", path: "/portal-auth-proof", surface: Surface::Support, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "dev-apple-map-test", title: "Apple map test", path: "/dev/apple-map-test", surface: Surface::Support, nav: Nav::Unlisted, deferred: Some("A manual integration harness for Apple Maps. Nothing to render from a read model; the page exists to be driven by hand."), detail_of: None },
    Screen { key: "dev-google-map-test", title: "Google map test", path: "/dev/google-map-test", surface: Surface::Support, nav: Nav::Unlisted, deferred: Some("A manual integration harness for Google Maps. Nothing to render from a read model; the page exists to be driven by hand."), detail_of: None },
    Screen { key: "console-story", title: "Command Console story", path: "/portal/command-console/[storyId]", surface: Surface::Tech, nav: Nav::Record, deferred: None, detail_of: Some("command-console") },
];

/// One editorial block of a public page: exactly the shape `lib/marketing-content` gives the TypeScript pages.
///
/// WHY A PAYLOAD AND NOT ROWS. A list screen's data is a list, and `Row` is the right shape for it. A page is not a
/// list: its hero has an image and an alt text, its sections have eyebrows and calls to action, and rendering those as
/// `cells[0]`/`cells[1]` is how a converted page ends up as a list of strings with no design. This is the shape the
/// page needs, and it is the same shape the TypeScript page read, so the port is a reproduction rather than a guess.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Block {
    pub eyebrow: String,
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub cta_label: Option<String>,
    pub cta_href: Option<String>,
    /// The image this block is built around, and what it shows. Both parts are required for an accessible page, so they
    /// travel together rather than the alt text being optional in practice and omitted in fact.
    pub image_path: Option<String>,
    pub image_alt: Option<String>,
    /// The block's list items, for the blocks that have them (a buyer's services, a set of stats).
    pub items: Vec<BlockItem>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BlockItem {
    /// `list`, `stat`, `office`, `email`, `faq` — what the item is FOR, so the view can render it as what it is.
    pub key: String,
    pub label: Option<String>,
    pub value: Option<String>,
}

/// A card in the featured/property grids: the public shape of a listing.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Listing {
    pub slug: String,
    pub name: String,
    pub location: Option<String>,
    pub price: Option<String>,
    pub kind: Option<String>,
    pub image_path: Option<String>,
    pub image_alt: Option<String>,
    /// Bedrooms and bathrooms are NOT integers. Half-baths are ordinary, so `7.5` is a real value in this data — and
    /// declaring these as `i64` made the whole page payload fail to deserialize the first time a listing had one, which
    /// presents as a page with no hero and no sections rather than as a bad number.
    pub beds: Option<f64>,
    pub baths: Option<f64>,
    pub area: Option<String>,
    /// Whether the estate is in the featured set, which is what draws the badge on its card.
    pub featured: bool,
}

/// One entry in the Island Guide: a place, with its photograph.
///
/// THE FIRST PAYLOAD TYPE THAT IS NOT EDITORIAL COPY. The guide is a catalogue read from `guide_item` — beaches, dining,
/// essentials — and each entry is a card with a picture, an area, contact details and a website. Forcing that into a
/// `Block` would have meant cramming a place into `cells`, which is the abstraction the page route exists to avoid.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GuideItem {
    /// Which section of the guide it belongs to (`beaches`, `dining`, …), so the page can group it.
    pub section: String,
    pub name: String,
    /// The line above the name: the neighbourhood if there is one, else the area, else the entry's own eyebrow.
    pub subtitle: Option<String>,
    pub area: Option<String>,
    pub eyebrow: Option<String>,
    pub description: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub website_url: Option<String>,
    /// The card photograph. Absent is a real state — the page says "Image coming soon" rather than showing a broken
    /// frame, and it keeps the alt text travelling with the image for the case where there is one.
    pub image_path: Option<String>,
    pub image_alt: Option<String>,
}

/// One piece of a property's media: a gallery frame, a video, or a document.
///
/// PERMISSIVE ON PURPOSE. The read model owns the shape of its media rows — a gallery image and a video do not carry the
/// same fields, and a document carries neither an alt text nor a duration. The route passes them through untouched, so
/// this accepts whatever arrives and the view reads whichever fields are present rather than assuming a shape the read
/// model never promised.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MediaItem {
    pub id: Option<String>,
    pub url: Option<String>,
    pub href: Option<String>,
    pub media_url: Option<String>,
    pub media_id: Option<String>,
    pub alt: Option<String>,
    pub alt_text: Option<String>,
    pub title: Option<String>,
    pub name: Option<String>,
    pub label: Option<String>,
    pub caption: Option<String>,
    pub playback_id: Option<String>,
    pub role: Option<String>,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
}

impl MediaItem {
    /// Where the item lives, whichever of the read model's fields carried it. The guide's photographs arrive as a media
    /// id that has to be turned into a route; these arrive as a URL, and both must work.
    pub fn src(&self) -> Option<String> {
        self.url
            .clone()
            .or_else(|| self.href.clone())
            .or_else(|| self.media_url.clone())
            .or_else(|| self.media_id.clone().map(|id| format!("/api/media/{id}")))
    }

    /// What it is called, for the alt text and the label.
    pub fn text(&self) -> Option<&str> {
        self.alt
            .as_deref()
            .or(self.alt_text.as_deref())
            .or(self.title.as_deref())
            .or(self.name.as_deref())
            .or(self.label.as_deref())
            .or(self.caption.as_deref())
    }
}

/// A property record: the facts the card shows, and the media the page is made of.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PropertyRecord {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub kind: Option<String>,
    pub price: Option<String>,
    pub beds: Option<f64>,
    pub baths: Option<f64>,
    pub area: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub year_built: Option<i64>,
    pub architecture: Option<String>,
    pub status: Option<String>,
    pub neighborhood: Option<String>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub lot_size: Option<String>,
    pub living_area: Option<f64>,
    pub bathrooms_full: Option<f64>,
    pub bathrooms_half: Option<f64>,
    pub stories: Option<f64>,
    pub parking_spaces: Option<f64>,
    pub water_access: bool,
    pub beach_access: bool,
    pub amenities: Vec<String>,
    pub view_type: Vec<String>,
    pub lifestyle_tags: Vec<String>,
    pub short_description: Option<String>,
    pub listing_agent_name: Option<String>,
    pub listing_agent_phone: Option<String>,
    pub listing_agent_email: Option<String>,
    pub listing_office: Option<String>,
    pub listing_id: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub hero_url: Option<String>,
    pub gallery: Vec<MediaItem>,
    pub videos: Vec<MediaItem>,
    pub documents: Vec<MediaItem>,
    pub similar: Vec<Listing>,
    pub public_slugs: Vec<String>,
}

/// The portal's own page payload: what one portal screen renders, in the screen's real shape.
///
/// WHY THIS EXISTS. A portal screen used to arrive as `RustUiRow { id, cells, badge }` — a generic list with the fields
/// flattened in and the rest thrown away. That is a fine transport for a table and a wrong one for a screen: the
/// Activity feed renders a channel, a direction, a person, a summary and the property or deal a line belongs to, and
/// `cells` keeps none of those as fields. So a screen that is really ported gets a DTO of its own here — the read
/// model's fields, not a column list it has to decode.

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechPage {
    pub ready: bool,
    pub total_stories: i64,
    pub open_count: i64,
    pub backlog_count: i64,
    pub closed_count: i64,
    pub completion_percent: f64,
    pub active_work: Vec<PortalTechStory>,
    pub selected_story: Option<PortalTechStory>,
    pub selected_runs: Vec<PortalTechRun>,
    pub recorder_instance_id: Option<String>,
    pub hold: Option<PortalTechHold>,
    pub sorter_cards: Vec<PortalTechSorterCard>,
    pub sorter_columns: Vec<PortalTechSorterColumn>,
    pub engine_runs: Vec<PortalTechEngineRun>,
    pub queued_cards: Vec<PortalTechQueuedCard>,
    pub engine_read_ok: bool,
    pub queue_read_ok: bool,
    pub ledger: Option<PortalTechLedger>,
    pub staging_flight: Option<PortalTechFlight>,
    pub recent_flights: Vec<PortalTechFlight>,
    pub recent_history: Vec<PortalTechHistory>,
    pub freshness: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechStory {
    pub id: String,
    pub workstream: String,
    pub operating_surface: Option<String>,
    pub title: String,
    pub priority: String,
    pub status: String,
    pub notes: Option<String>,
    pub batch: Option<i64>,
    pub goal: Option<String>,
    pub scope: Option<String>,
    pub dependencies: Option<String>,
    pub preconditions: Option<String>,
    pub architect_brief: Option<String>,
    pub context_refs: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub postconditions: Option<String>,
    pub completion: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechRun {
    pub id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub result_status: Option<String>,
    pub run_type: Option<String>,
    pub agent_runtime: Option<String>,
    pub completion: Option<f64>,
    pub notes: Option<String>,
    pub commit_hash: Option<String>,
    pub tests_summary: Option<String>,
    pub execution_environment: Option<String>,
    pub run_phase: Option<String>,
    pub lead_decision: Option<String>,
    pub model_used: Option<String>,
    pub cost_widgets: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechHold {
    pub reason: Option<String>,
    pub originating_node: Option<String>,
    pub failure_class: Option<String>,
    pub resume_target: Option<String>,
    pub since: Option<String>,
    pub process_instance_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechSorterCard {
    pub id: String,
    pub column: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub completion: f64,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechSorterColumn {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechEngineRun {
    pub story_id: String,
    pub title: String,
    pub instance_id: String,
    pub last_node: Option<String>,
    pub status: String,
    pub attempts: i64,
    pub at: Option<String>,
    pub stale: bool,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechQueuedCard {
    pub story_id: String,
    pub title: String,
    pub state: String,
    pub since: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechLedger {
    pub total_attempts: i64,
    pub stories: i64,
    pub completed: i64,
    pub failed: i64,
    pub interrupted: i64,
    pub worst_story_id: Option<String>,
    pub worst_attempts: Option<i64>,
    pub as_of: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechFlight {
    pub id: String,
    pub label: Option<String>,
    pub status: String,
    pub scheduled_for: Option<String>,
    pub fired_at: Option<String>,
    pub created_at: String,
    pub model_policy: String,
    pub story_count: i64,
    pub queued_count: i64,
    pub skipped_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechHistory {
    pub id: String,
    pub title: String,
    pub latest_run_at: Option<String>,
    pub latest_run_result: Option<String>,
}


#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsRow {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub status: String,
    pub meta: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsStellar {
    pub listing_contract_date: Option<String>,
    pub expiration_date: Option<String>,
    pub listing_type: Option<String>,
    pub agent_mls_id: Option<String>,
    pub tax_id: Option<String>,
    pub tax_year: Option<String>,
    pub annual_tax: Option<String>,
    pub legal_description: Option<String>,
    pub zoning: Option<String>,
    pub total_area_sqft: Option<String>,
    pub heated_area_source: Option<String>,
    pub ownership_type: Option<String>,
    pub hoa_details: Option<String>,
    pub showing_instructions: Option<String>,
    pub occupant_type: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsProperty {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub status: String,
    pub featured: bool,
    pub is_active_listing: bool,
    pub is_published: bool,
    pub property_type: Option<String>,
    pub list_price: Option<String>,
    pub location: Option<String>,
    pub address_line1: Option<String>,
    pub street_number: Option<String>,
    pub street_name: Option<String>,
    pub unit_number: Option<String>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub neighborhood: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub iso_country_code: Option<String>,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    pub bedrooms: Option<String>,
    pub bathrooms: Option<String>,
    pub bathrooms_full: Option<String>,
    pub bathrooms_half: Option<String>,
    pub square_feet: Option<String>,
    pub lot_size: Option<String>,
    pub lot_size_units: Option<String>,
    pub year_built: Option<String>,
    pub stories: Option<String>,
    pub parking_spaces: Option<String>,
    pub short_description: Option<String>,
    pub editorial_description: Option<String>,
    pub public_remarks: Option<String>,
    pub listing_agent_name: Option<String>,
    pub listing_agent_email: Option<String>,
    pub listing_agent_phone: Option<String>,
    pub listing_office: Option<String>,
    pub legal_owner_name: Option<String>,
    pub listing_identifier: Option<String>,
    pub registry_entry: Option<String>,
    pub finca_number: Option<String>,
    pub registry_section: Option<String>,
    pub seller_person_id: Option<String>,
    pub seller_name: Option<String>,
    pub seller_email: Option<String>,
    pub seller_phone: Option<String>,
    pub seller_location: Option<String>,
    pub archived: bool,
    pub image_count: i64,
    pub video_count: i64,
    pub document_count: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub stellar: PortalOpsStellar,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsPerson {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub company: Option<String>,
    pub location: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsProject {
    pub id: String,
    pub name: String,
    pub owner: Option<String>,
    pub status: String,
    pub description: String,
    pub areas: Vec<String>,
    pub project_type: Option<String>,
    pub playbook_id: Option<String>,
    pub playbook_version: Option<i32>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsMediaAsset {
    pub id: String,
    pub property_id: String,
    pub media_type: String,
    pub role: String,
    pub sort_order: i32,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub created_at: Option<String>,
    pub mux_asset_id: Option<String>,
    pub mux_playback_id: Option<String>,
    pub duration_seconds: Option<String>,
    pub aspect_ratio: Option<String>,
    pub source_url: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsWorkbenchPage {
    pub entity: String,
    pub rows: Vec<PortalOpsRow>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub selected_id: Option<String>,
    pub property: Option<PortalOpsProperty>,
    pub person: Option<PortalOpsPerson>,
    pub project: Option<PortalOpsProject>,
    pub media: Vec<PortalOpsMediaAsset>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalRecordProperty {
    pub id: String,
    pub name: String,
    pub status: String,
    pub location: String,
    pub list_price: Option<String>,
    pub slug: Option<String>,
    pub archived: bool,
    pub image_count: i64,
    pub video_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalRecordsPage {
    pub rows: Vec<PortalRecordProperty>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub selected_id: Option<String>,
    pub selected: Option<PortalRecordProperty>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalListingProperty {
    pub id: String,
    pub name: String,
    pub status: String,
    pub slug: Option<String>,
    pub image_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalListingMediaPage {
    pub properties: Vec<PortalListingProperty>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub selected_id: Option<String>,
    pub selected: Option<PortalListingProperty>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardStory {
    pub id: String,
    pub title: String,
    pub priority: String,
    pub status: String,
    pub completion: f64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardGroup {
    pub group: String,
    pub stories: Vec<PortalStoryboardStory>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardPanel {
    pub bucket: String,
    pub count: i64,
    pub groups: Vec<PortalStoryboardGroup>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardKpis {
    pub total: i64,
    pub open: i64,
    pub backlog: i64,
    pub blocked_hold: i64,
    pub complete: i64,
    pub next_version: i64,
    pub completion_percent: f64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardPanels {
    pub open: PortalStoryboardPanel,
    pub backlog: PortalStoryboardPanel,
    pub closed: PortalStoryboardPanel,
    pub next_version: PortalStoryboardPanel,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardPage {
    pub kpis: PortalStoryboardKpis,
    pub panels: PortalStoryboardPanels,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalPage {
    /// TECH / Engineering Cockpit — story supply, Flight staging, Forge execution and recent history.
    pub tech: Option<PortalTechPage>,
    /// CORE Cockpit — the situational-awareness landing page.
    pub cockpit: Option<PortalCockpitPage>,
    /// CORE Cabinet — canonical immutable issued-document repository.
    pub cabinet: Option<PortalCabinetPage>,
    /// `/portal/activity` — the unified feed, ordered as the read model returned it.
    pub activity: Vec<PortalActivityEntry>,
    /// `/portal/workflows` — definition-driven transaction workflow cards.
    pub workflows: Option<PortalWorkflowList>,
    /// `/portal/workflows/[instanceId]` — one workflow instance in its real timeline shape.
    pub workflow: Option<PortalWorkflowDetail>,
    /// CORE Clients — directory plus whichever person is selected or directly addressed.
    pub clients: Option<PortalClientsPage>,
    /// CORE Forms — saved sessions plus the working record/editor payload.
    pub forms: Option<PortalFormsPage>,
    /// CORE Projects — authoritative Rust Project/WBS data plus reducer-owned workspace selection.
    pub projects: Option<PortalProjectsPage>,
    /// CORE Contracts — canonical Deal portfolio plus form-created Contract artifacts.
    pub deals: Option<PortalDealsPage>,
    /// Accounting V1 — the dashboard's projections, the two lists, and the P&L for a requested period. One word per
    /// screen, in the same shape the other surfaces use, so a screen reads `portal.accounting.<what it renders>`.
    pub accounting: Option<PortalAccountingPage>,
    /// SUPPORT — the four diagnostic screens. One word per screen, as above.
    pub support: Option<PortalSupportPage>,
    /// OPPS universal Data Workbench — one selector/editor shell over typed domain adapters.
    pub ops: Option<PortalOpsWorkbenchPage>,
    /// OPPS Records — legacy bounded property projection retained while routes outside the workbench converge.
    pub records: Option<PortalRecordsPage>,
    /// OPPS Listing Media — bounded listing/property projection for media attachment.
    pub listing_media: Option<PortalListingMediaPage>,
    /// TECH Story Board — canonical legacy cockpit projection rendered natively by Yew.
    pub storyboard: Option<PortalStoryboardPage>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealsPage {
    pub deals: Vec<PortalDeal>,
    pub contracts: Vec<PortalDealContract>,
    pub properties: Vec<PortalDealableProperty>,
    pub users: Vec<PortalDealOwnerCandidate>,
    pub workspace: Option<PortalDealWorkspace>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDeal {
    pub id: String,
    pub property_id: String,
    pub property_name: String,
    pub property_location: String,
    pub property_descriptor: Option<String>,
    pub hero_media_id: Option<String>,
    pub client_id: String,
    pub client_name: String,
    pub stage: String,
    pub list_price: Option<f64>,
    pub offer_price: Option<f64>,
    pub owner: String,
    pub closing_date: Option<String>,
    pub next_milestone: Option<String>,
    pub next_milestone_at: Option<String>,
    pub last_activity: Option<String>,
    pub last_activity_at: Option<String>,
    pub showing_count: i64,
    pub offer_count: i64,
    pub participant_count: i64,
    pub latest_offer_amount: Option<f64>,
    pub latest_offer_status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealContract {
    pub id: String,
    pub form_template_id: String,
    pub contract_type: String,
    pub property_id: String,
    pub property_label: Option<String>,
    pub status: String,
    pub process_instance_id: Option<String>,
    pub executed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealableProperty {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealOwnerCandidate {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealPersonCandidate {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub location: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealPeopleSearch {
    pub people: Vec<PortalDealPersonCandidate>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspace {
    pub deal: Option<PortalDealWorkspaceDeal>,
    pub property: Option<PortalDealWorkspaceProperty>,
    pub client: Option<PortalDealWorkspaceClient>,
    pub participants: Vec<PortalDealWorkspaceParticipant>,
    pub open_tasks: Vec<PortalDealWorkspaceTask>,
    pub activity: Vec<PortalDealWorkspaceActivity>,
    pub offers: Vec<PortalDealWorkspaceOffer>,
    pub showings: Vec<PortalDealWorkspaceShowing>,
    pub contracts: Vec<PortalDealContract>,
    pub owner_candidates: Vec<PortalDealOwnerCandidate>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceDeal {
    pub id: String,
    pub stage: String,
    pub list_price: Option<f64>,
    pub offer_price: Option<f64>,
    pub closing_date_label: Option<String>,
    pub closed_at_label: Option<String>,
    pub notes: Option<String>,
    pub created_at_label: String,
    pub updated_at_label: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceProperty {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
    pub property_type: Option<String>,
    pub bedrooms: Option<f64>,
    pub bathrooms: Option<f64>,
    pub square_feet: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceClient {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceParticipant {
    pub id: String,
    pub role_category: String,
    pub role_label: Option<String>,
    pub kind: String,
    pub person_id: Option<String>,
    pub user_id: Option<String>,
    pub name: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceTask {
    pub id: String,
    pub title: String,
    pub detail: Option<String>,
    pub due_at_label: Option<String>,
    pub is_overdue: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceActivity {
    pub id: String,
    pub person_id: Option<String>,
    pub channel: String,
    pub direction: Option<String>,
    pub occurred_at_label: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub person_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceOffer {
    pub id: String,
    pub person_id: String,
    pub person_name: Option<String>,
    pub parent_offer_id: Option<String>,
    pub amount: f64,
    pub status: String,
    pub submitted_at_label: String,
    pub responded_at_label: Option<String>,
    pub note: Option<String>,
    pub is_counter: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceShowing {
    pub id: String,
    pub person_id: String,
    pub person_name: String,
    pub status: String,
    pub requested_at_label: String,
    pub scheduled_at_label: Option<String>,
    pub completed_at_label: Option<String>,
    pub cancelled_at_label: Option<String>,
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PortalDealCommand {
    CreateTask {
        title: String,
        detail: Option<String>,
        due_at: Option<String>,
    },
    CompleteTask {
        task_id: String,
    },
    CreateShowing {
        person_id: String,
        property_id: Option<String>,
    },
    ScheduleShowing {
        showing_id: String,
        scheduled_at: String,
    },
    CancelShowing {
        showing_id: String,
    },
    CompleteShowing {
        showing_id: String,
    },
    SubmitOffer {
        person_id: String,
        amount: String,
        parent_offer_id: Option<String>,
    },
    WithdrawOffer {
        offer_id: String,
    },
    RejectOffer {
        offer_id: String,
    },
    AddOtherParticipant {
        person_id: String,
        role_label: String,
    },
    UpdateOtherParticipant {
        participant_id: String,
        role_label: String,
    },
    EndOtherParticipant {
        participant_id: String,
    },
    SetStructuralParticipant {
        role: String,
        person_id: Option<String>,
        user_id: Option<String>,
    },
    EndStructuralParticipant {
        participant_id: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCabinetPage {
    pub documents: Vec<PortalCabinetDocument>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCabinetDocument {
    pub id: String,
    pub deal_id: Option<String>,
    pub property_id: Option<String>,
    pub document_type_label: Option<String>,
    pub title: Option<String>,
    pub state: String,
    pub template_id: Option<String>,
    pub template_version: Option<i32>,
    pub issued_version: Option<i32>,
    pub issued_checksum_sha256: Option<String>,
    pub issued_by_display_name: Option<String>,
    pub party_name: Option<String>,
    pub property_name: Option<String>,
    pub deal_name: Option<String>,
    pub created_at: String,
    pub signed_artifact_available: bool,
    pub signed_audit_available: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCockpitPage {
    pub active_client_count: i64,
    pub live_deal_count: i64,
    pub upcoming_count: i64,
    pub under_contract_count: i64,
    pub active_workflow_count: i64,
    pub blocked_workflow_count: i64,
    pub overdue_tasks: Vec<PortalCockpitTask>,
    pub tasks_due_soon: Vec<PortalCockpitTask>,
    pub recent_interactions: Vec<PortalCockpitInteraction>,
    pub featured_deal: Option<PortalCockpitDeal>,
    pub pipeline: Vec<PortalCockpitStageCount>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCockpitTask {
    pub id: String,
    pub person_id: Option<String>,
    pub title: String,
    pub detail: Option<String>,
    pub due_at: Option<String>,
    pub due_at_label: Option<String>,
    pub context_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCockpitInteraction {
    pub id: String,
    pub person_name: String,
    pub channel: String,
    pub occurred_at_label: String,
    pub summary: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCockpitDeal {
    pub id: String,
    pub property_name: String,
    pub hero_media_id: Option<String>,
    pub stage: String,
    pub list_price: Option<f64>,
    pub offer_price: Option<f64>,
    pub closing_date: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCockpitStageCount {
    pub stage: String,
    pub count: i64,
}

/// One line of the unified activity feed, with the fields the live screen renders.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalActivityEntry {
    pub id: String,
    /// `website`, `email`, `call`, `imessage`, `sms`, `meeting`, `showing`, `document`, `manual`, `whatsapp` — labelled
    /// for display by the screen, not here.
    pub channel: String,
    /// Which way it went, when the channel has a direction.
    pub direction: Option<String>,
    /// Already formatted by the read model: the screen shows the label, it does not compute a date.
    pub occurred_at_label: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    /// The person this line is about, and the key its link uses when there is one.
    pub person_id: Option<String>,
    pub person_name: Option<String>,
    pub property_name: Option<String>,
    /// The deal this line belongs to, and the property that deal is about — two different names, which is why both are
    /// here.
    pub deal_id: Option<String>,
    pub deal_property_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowList {
    pub configured: bool,
    pub items: Vec<PortalWorkflowSummary>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowSummary {
    pub instance_id: String,
    pub workflow_name: String,
    pub workflow_version: i64,
    pub property_name: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub active_milestones: Vec<String>,
    pub open_task_count: i64,
    pub blocker_count: i64,
    pub responsible_party: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowDetail {
    pub instance_id: String,
    pub workflow_name: String,
    pub workflow_version: i64,
    pub property_name: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub responsible_party: Option<String>,
    pub started_at_label: String,
    pub timeline: Vec<PortalWorkflowTimelineItem>,
    pub milestones: Vec<PortalWorkflowMilestone>,
    pub open_task_count: i64,
    pub pending_timer_count: i64,
    pub blockers: Vec<String>,
    pub events: Vec<PortalWorkflowEvent>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowTimelineItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub deadline: Option<String>,
    pub completed: bool,
    pub active: bool,
    pub optional: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowMilestone {
    pub id: String,
    pub label: String,
    pub owner: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowEvent {
    pub id: String,
    pub event_type: String,
    pub node_label: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientsPage {
    pub rows: Vec<PortalClientSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub selected_id: Option<String>,
    pub selected: Option<PortalClientDetail>,
    pub comms: Option<PortalCommsPanel>,
    pub properties: Vec<PortalClientProperty>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientSummary {
    pub id: String,
    pub display_name: String,
    pub name_resolved: bool,
    pub role: String,
    pub status: String,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub observed_count: i64,
    pub two_way: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientDetail {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub budget_min: Option<f64>,
    pub budget_max: Option<f64>,
    pub timeline: Option<String>,
    pub assigned_agent: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCommsPanel {
    pub person_id: String,
    pub aggregate: PortalCommsAggregate,
    pub sources: Vec<PortalCommsSource>,
    pub moments: Vec<PortalCommsMoment>,
    pub moment_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCommsAggregate {
    pub observed_count: i64,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub two_way: bool,
    pub first_observed_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub last_contact_at: Option<String>,
    pub last_contact_label: Option<String>,
    pub active_source_count: i64,
    pub source_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCommsSource {
    pub source: String,
    pub channel: String,
    pub label: String,
    pub total_count: i64,
    pub two_way: bool,
    pub last_context: Option<String>,
    pub last_contact_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCommsMoment {
    pub id: String,
    pub channel: Option<String>,
    pub direction: Option<String>,
    pub occurred_at: String,
    pub title: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientProperty {
    pub id: String,
    pub display_name: String,
    pub relation: String,
    pub relation_status: Option<String>,
    pub address: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormsPage {
    pub items: Vec<PortalFormSummary>,
    pub selected: Option<PortalFormRecord>,
    pub template: Option<PortalFormTemplate>,
    pub issued: Option<PortalIssuedFormDocument>,
    pub signers: Vec<PortalFormSigner>,
    pub template_choices: Vec<PortalFormTemplateChoice>,
    /// Working-editor state owned by the reducer, never by Yew hooks.
    pub dirty: bool,
    pub saving: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormSummary {
    pub id: String,
    pub template_id: String,
    pub template_version: i32,
    pub template_name: String,
    pub active_version: i32,
    pub status: String,
    pub deal_id: Option<String>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub deal_label: Option<String>,
    pub property_label: Option<String>,
    pub client_name: Option<String>,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormRecord {
    pub id: String,
    pub template_id: String,
    pub template_version: i32,
    pub template_name: String,
    pub active_version: i32,
    pub status: String,
    pub deal_id: Option<String>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub deal_label: Option<String>,
    pub property_label: Option<String>,
    pub client_name: Option<String>,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
    pub updated_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormTemplate {
    pub id: String,
    pub version: i32,
    pub active_version: i32,
    pub display_name: String,
    pub document_type_label: String,
    pub rendering_title: String,
    pub presentation: String,
    pub fields: Vec<PortalFormField>,
    pub sections: Vec<PortalFormSection>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormField {
    pub name: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub required: bool,
    pub options: Vec<String>,
    pub when: Option<PortalFormWhen>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormSection {
    pub name: String,
    pub label: String,
    pub editable: bool,
    pub when: Option<PortalFormWhen>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormWhen {
    pub field: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalIssuedFormDocument {
    pub document_id: String,
    pub issued_version: i32,
    pub checksum: String,
    pub created_at: String,
    pub media_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormSigner {
    pub person_id: Option<String>,
    pub name: String,
    pub email: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormTemplateChoice {
    pub id: String,
    pub display_name: String,
    pub active_version: i32,
}


#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectsPage {
    pub projects: Vec<PortalProject>,
    pub items: Vec<PortalProjectWorkItem>,
    pub documents: Vec<PortalProjectDocument>,
    pub media: Vec<PortalProjectMedia>,
    pub activity: Vec<PortalProjectActivity>,
    pub calendar: Vec<PortalProjectCalendarEvent>,
    pub identity_names: BTreeMap<String, String>,
    /// Workspace state lives with the payload and changes only in update().
    pub active_domain: String,
    pub selected_project_id: Option<String>,
    pub selected_node_id: Option<String>,
    pub active_view: String,
    pub catch_up: bool,
    /// The bottom selected-work pane is collapsible, matching the mature Projects workspace.
    pub work_collapsed: bool,
    pub work_dirty: bool,
    pub saving: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProject {
    pub id: String,
    pub name: String,
    pub owner: Option<String>,
    pub status: String,
    pub description: String,
    pub areas: Vec<String>,
    pub project_type: Option<String>,
    pub playbook_id: Option<String>,
    pub playbook_version: Option<i32>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectWorkItem {
    pub id: String,
    pub title: String,
    pub notes: String,
    pub category: String,
    pub status: String,
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
    pub due_at: Option<String>,
    pub owner: Option<String>,
    pub order: Option<i32>,
    pub entity: Option<PortalProjectEntity>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectEntity {
    pub entity_type: String,
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectDocument {
    pub id: String,
    pub property_id: Option<String>,
    pub title: String,
    pub state: String,
    pub template_id: Option<String>,
    pub template_version: Option<i32>,
    pub issued_version: Option<i32>,
    pub created_at: String,
    pub signed_artifact_available: bool,
    pub signed_audit_available: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectMedia {
    pub id: String,
    pub property_id: String,
    pub media_type: String,
    pub role: String,
    pub sort_order: i32,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub created_at: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectActivity {
    pub id: String,
    pub person_id: Option<String>,
    pub deal_id: Option<String>,
    pub property_id: Option<String>,
    pub channel: String,
    pub direction: Option<String>,
    pub occurred_at: String,
    pub occurred_at_label: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub person_name: Option<String>,
    pub property_name: Option<String>,
    pub deal_property_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectCalendarEvent {
    pub id: String,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub all_day: bool,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
    pub property_name: Option<String>,
    pub kind: String,
    pub source: String,
}

/// Everything a public page renders from.
///
/// A page is a set of named blocks, not an ordered list, because the page decides where each one goes — the hero is a
/// full-bleed image with the title over it, the culture block is an image followed by an editorial column. Flattening
/// them into an ordered list would put the layout in the data, where the view could no longer decide it.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PageContent {
    pub hero: Block,
    pub buyers: Block,
    pub sellers: Block,
    pub culture: Block,
    pub about: Block,
    pub contact: Block,
    /// The FAQ page's accordion source (screen `site-faq`).
    ///
    /// Served as the `faq.list` block whole rather than as a list of questions, because the block carries more than its
    /// items: its `subtitle` is the heading over the closing call to action and its `ctaLabel`/`ctaHref` are the link
    /// itself. Splitting the questions out would have meant a second field for each of those and a second shape to keep
    /// in step with the content store. The questions are its items keyed `faq` — a label and a value, question and
    /// answer — which is the same filter `faqEntries()` applies in TypeScript.
    pub faq: Block,
    pub featured: Vec<Listing>,
    pub listings: Vec<Listing>,
    /// The Island Guide's catalogue (screen `site-guide`). Empty for every other page, which is what `default` is for.
    pub guide: Vec<GuideItem>,
    /// The property record (screen `site-property-detail`), when the page is about one property.
    pub property: Option<PropertyRecord>,
    /// The portal screen's payload, when the screen is one that has been ported to a real component.
    ///
    /// `None` for every screen still rendering rows: a screen with no DTO yet keeps the generic list, and the two live
    /// side by side while the port goes screen by screen.
    pub portal: Option<PortalPage>,
}

///
/// Deliberately generic. This sweep ports screen *structure* — route, heading, nav, state boundary — and per-screen
/// data contracts come after. A generic row means a new screen is a table entry rather than a new module, and it keeps
/// the port honest: nothing here invents columns for a screen whose real columns have not been read yet.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    pub cells: Vec<String>,
    #[serde(default)]
    pub badge: Option<String>,
}

/// How many rows one page holds.
///
/// The MODEL owns this rather than the view, because the reducer is what clamps `Controls::page` and it cannot clamp
/// against a number it cannot see. The view reads it instead of deciding it, so a page button can never disagree with
/// the bound the reducer enforces.
pub const PAGE_SIZE: usize = 25;

/// What the user has set on the current screen's controls.
///
/// One struct rather than four fields on the model, because these share a lifecycle: they are the screen's own input,
/// and navigation clears them (`update::open`). A filter typed on Clients must not follow the user to Deals.
///
/// WHY THE MODEL OWNS THIS: a control that keeps its own value in the DOM is state the reducer cannot see, cannot
/// test, and cannot restore. Holding it here means a keystroke is a named message, filtering is a pure function of the
/// model, and the rendered value always *is* the model's value rather than whatever the DOM last held.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Controls {
    /// Text in the screen's search field.
    pub query: String,
    /// The chosen dropdown option, by the option's value rather than its label.
    pub filter: Option<String>,
    /// The active tab, by key.
    pub tab: Option<String>,
    /// A switch or checkbox the user has set.
    pub toggled: bool,
    /// THE NAMED DROPDOWNS, by the name the control carries — the Buyers bar's `price`, `beds` and `sort`.
    ///
    /// A map rather than a field per control. The portal screens have one dropdown and it is `filter` above; the Buyers
    /// inventory bar has three, and giving the model a `price`, a `beds` and a `sort` field would put one screen's
    /// vocabulary into state every screen shares — and make every future screen that wants its own named control edit
    /// the model, the reducer and the shell. The name is the one the markup already carries (`data-select="price"`), so
    /// there is a single vocabulary rather than one in the DOM and another here.
    ///
    /// It lives in `Controls` because it has the same lifecycle: navigation clears it (`update::open`), so a price
    /// range chosen on Buyers cannot follow the visitor to another screen and silently narrow a list they never
    /// filtered.
    pub named: BTreeMap<String, String>,
    /// Which page of rows the user is on, 0-based. Clamped by the reducer; see `Msg::PageChanged`.
    pub page: usize,
}

/// Reducer-owned state for the Contracts create panel. The DOM never owns a second copy of these values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DealCreateState {
    pub open: bool,
    pub property_id: String,
    pub client_person_id: String,
    pub client_label: String,
    pub client_query: String,
    pub owner_user_id: String,
    pub notes: String,
    pub people: Vec<PortalDealPersonCandidate>,
    pub searching: bool,
    pub submitting: bool,
}

/// Reducer-owned state for the Accounting screens' forms and commands.
///
/// WHY ONE STRUCT RATHER THAN A FIELD PER INPUT ON `Model`: the three Accounting screens are one module with three
/// shapes — an expense form, a receivable form, and a P&L period — and grouping them keeps `Model`'s own list of a dozen
/// screen states from growing by another dozen. Nothing here is duplicated in the DOM: every input reads its value from
/// this and writes it back through a message, which is the whole point of the exercise.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AccountingState {
    /// Whether the New Expense panel is open.
    pub expense_open: bool,
    pub expense_vendor: String,
    pub expense_category: String,
    /// The digits the operator typed, as a string. Deliberately not a number: it is validated in Rust, and a `f64` here
    /// would round it before validation ever saw it.
    pub expense_amount: String,
    pub expense_on: String,
    pub expense_memo: String,
    /// Whether the New Receivable panel is open, and its six fields.
    pub receivable_open: bool,
    pub receivable_reference: String,
    pub receivable_description: String,
    pub receivable_category: String,
    pub receivable_amount: String,
    pub receivable_issued_on: String,
    pub receivable_due_on: String,
    /// The paid date per receivable, keyed by id — only where the operator has changed it. A row with no entry shows the
    /// book's `today`, so the map holds edits rather than copies of a default, and a row that arrives later needs no
    /// seeding.
    pub paid_on: std::collections::BTreeMap<String, String>,
    /// The P&L's period, as the operator has set it. Empty until the screen's first payload tells it what was projected —
    /// the bridge defaults the first request to the current month, which is what the live page projected.
    pub pnl_from: String,
    pub pnl_to: String,
    /// The receipt scanner's demonstration.
    pub scanner: ScannerState,
    /// A command is in flight: the form is disabled and the button says so.
    pub submitting: bool,
    /// What the last command said, if it has said anything. Cleared when a new one starts.
    pub notice: Option<CommandNotice>,
}

/// The outcome of a command, as the screen reports it: the live forms printed a green "Created." or a red message.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandNotice {
    pub ok: bool,
    pub message: String,
}

/// The receipt scanner's state, all of it reducer-owned.
///
/// IT IS A DEMONSTRATION, AND THE STATE SAYS SO. There is no OCR here and none is implied: pressing Scan cycles four fixed
/// receipts, the operator reviews one, and saving it records an expense exactly as the form does. The point of this screen in
/// V1 is the WORKFLOW — attach, extract, review, save — and a demonstration of a workflow is still a workflow worth having
/// the state of.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScannerState {
    /// The name of the receipt the operator attached, or the demonstration's own once one has been scanned.
    pub file_name: String,
    /// Whether a file is being dragged over the surface, which is what highlights it.
    pub dragging: bool,
    /// How many receipts have been scanned, so the next one is the next seed — the cycle the live component ran.
    pub demo_index: usize,
    /// The extracted draft under review, or nothing before the first scan.
    pub draft: Option<ScannerDraft>,
}

/// One reviewed receipt: what the demonstration "extracted", editable where a human would correct it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScannerDraft {
    pub vendor: String,
    /// The digits as the seed carries them, on their way to Rust's validation like any other amount.
    pub amount: String,
    pub category: String,
    pub memo: String,
    pub expense_on: String,
}

impl CommandNotice {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
        }
    }
}



#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlightRecorderState {
    /// The exact process instance from the route. A story id never belongs here.
    pub instance_id: String,
    /// Canonical Flight Recorder transaction returned by the authenticated trace API.
    ///
    /// The specialized console renderer adapts this immutable snapshot for SVG/virtualized presentation, but does not
    /// own fetching or application state.
    pub transaction: Option<serde_json::Value>,
}


#[derive(Debug, Clone, PartialEq)]
pub struct OpsWorkbenchState {
    pub entity: String,
    pub section: String,
    pub rail_collapsed: bool,
    pub dirty: bool,
    pub saving: bool,
    pub creating: bool,
    pub new_name: String,
    pub form: BTreeMap<String, String>,
    pub person_query: String,
    pub person_people: Vec<PortalDealPersonCandidate>,
    pub person_searching: bool,
    pub selected_person: Option<PortalDealPersonCandidate>,
    pub media_index: usize,
    pub media_role: String,
    pub media_alt: String,
    pub media_file_name: Option<String>,
    pub media_uploading: bool,
    pub media_uploader_open: bool,
}

impl Default for OpsWorkbenchState {
    fn default() -> Self {
        Self {
            entity: "property".into(),
            section: "property".into(),
            rail_collapsed: false,
            dirty: false,
            saving: false,
            creating: false,
            new_name: String::new(),
            form: BTreeMap::new(),
            person_query: String::new(),
            person_people: Vec::new(),
            person_searching: false,
            selected_person: None,
            media_index: 0,
            media_role: "gallery".into(),
            media_alt: String::new(),
            media_file_name: None,
            media_uploading: false,
            media_uploader_open: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListingMediaState {
    pub role: String,
    pub alt: String,
    pub file_name: Option<String>,
    pub uploading: bool,
}

impl Default for ListingMediaState {
    fn default() -> Self {
        Self {
            role: "gallery".into(),
            alt: String::new(),
            file_name: None,
            uploading: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TechCockpitState {
    /// Browser-local value from the `datetime-local` Flight scheduler.
    pub schedule_at: String,
    /// The command whose write is currently in flight. One command at a time keeps double-clicks from duplicating work.
    pub busy_action: Option<String>,
    /// The last operator command result. Reads do not erase it; a new command does.
    pub notice: Option<CommandNotice>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DealWorkspaceState {
    pub task_title: String,
    pub task_detail: String,
    pub task_due_at: String,
    pub offer_amounts: BTreeMap<String, String>,
    pub showing_times: BTreeMap<String, String>,
    pub participant_query: String,
    pub participant_person_id: String,
    pub participant_label: String,
    pub participant_role_label: String,
    pub participant_people: Vec<PortalDealPersonCandidate>,
    pub participant_searching: bool,
    pub other_role_labels: BTreeMap<String, String>,
    pub structural_role: String,
    pub structural_query: String,
    pub structural_person_id: String,
    pub structural_label: String,
    pub structural_owner_user_id: String,
    pub structural_people: Vec<PortalDealPersonCandidate>,
    pub structural_searching: bool,
    pub busy_action: Option<String>,
}

/// Reducer-owned state for the public property photo viewer.
///
/// The pre-Rust PropertyMediaPanel was interactive: selecting a thumbnail changed the hero,
/// arrows moved through the canonical photo order, and "View all photos" opened a lightbox.
/// That state belongs here rather than in a Yew hook so the property page obeys the same MVI
/// invariant as the rest of the application.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PropertyTab {
    #[default]
    Overview,
    Details,
    Video,
    Documents,
    Map,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PropertyRecent {
    pub slug: String,
    pub id: String,
    pub name: String,
    pub at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyMediaState {
    pub tab: PropertyTab,
    pub saved: bool,
    pub recent: Vec<PropertyRecent>,
    pub active_index: usize,
    pub lightbox_open: bool,
    pub lightbox_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub screen: Screen,
    pub loading: bool,
    pub error: Option<String>,
    pub rows: Vec<Row>,
    /// Selection is an id, never an index or a copied row: a refreshed list must not re-point the selection at a
    /// different record.
    pub selected_row_id: Option<String>,
    /// What the current screen is *about*, when it is about a single record — the property slug or person id. Kept
    /// separate from `selected_row_id` because selecting a row and opening a record are different acts: one is
    /// browsing a list, the other is a different screen with its own fetch.
    pub scope: Option<String>,
    /// What the user has typed, chosen and paged to on this screen.
    pub controls: Controls,
    /// A public page's own content, when the screen is an editorial page rather than a list.
    ///
    /// `None` is a real state and not an error: a list screen has no page payload, and a page that has not loaded yet
    /// shows the chrome and its loading line exactly as a list does.
    pub page: Option<PageContent>,
    /// Public property gallery selection/lightbox state.
    pub property_media: PropertyMediaState,
    /// Seller Strategy is deterministic local application state: no fetch and no parallel React model.
    pub seller_strategy: crate::seller_strategy::SellerStrategyState,
    /// Contracts create/search state is reducer-owned just like every other interactive portal surface.
    pub deal_create: DealCreateState,
    /// Accounting V1's forms and their command state: the expense form, the receivable form, the P&L period and the
    /// receipt scanner's demonstration.
    pub accounting: AccountingState,
    /// System Health's Workflow Diagnostics: which instance row is open, its detail, and what the read said.
    ///
    /// THIS IS THE STATE THE EARLIER CONVERSION LOST. The pre-cutover component held it in React (`selectedId`, `detail`,
    /// `loadingId`, `error`) and fetched the detail when a row was clicked; the rows cutover dropped the interaction and the
    /// screen became a static list. It lives on the model for the same reason every other screen's input does: so the click
    /// is a message, the read is an effect, and `update` decides what the answer means.
    pub workflow: WorkflowDiagnosticsState,
    /// Flight Recorder route/read ownership. The React console is only a renderer over this state.
    pub flight_recorder: FlightRecorderState,
    /// TECH Cockpit operator controls and command state.
    pub tech: TechCockpitState,
    /// OPPS universal Data Workbench state. Field values are reducer-owned; the DOM never keeps a second draft.
    pub ops: OpsWorkbenchState,
    /// OPPS Listing Media upload controls. The file bytes stay in the browser input; only metadata lives here.
    pub listing_media: ListingMediaState,
    /// One Deal workspace's forms and transient command state.
    pub deal_workspace: DealWorkspaceState,
    /// WHICH MOUNT THIS STATE BELONGS TO.
    ///
    /// A host run has a generation, the shell stamps it on the program and on every effect it asks for, and every
    /// response has to present it back before it is allowed to touch the model. Without it, a request issued for screen
    /// A can land while screen B is mounted and write A's rows or blocks into B's screen — the browser shows one page
    /// and the model quietly holds another. Order of arrival is not ownership, and this is the value that says whose
    /// answer it is.
    pub generation: u64,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            // The first table entry, i.e. what a host that says nothing gets. A host that cares which screen opens
            // says so (the shell's `mount` takes the key), because the URL is the host's business.
            screen: SCREENS[0],
            loading: false,
            error: None,
            rows: Vec::new(),
            selected_row_id: None,
            scope: None,
            controls: Controls::default(),
            page: None,
            property_media: PropertyMediaState::default(),
            seller_strategy: crate::seller_strategy::SellerStrategyState::default(),
            deal_create: DealCreateState::default(),
            accounting: AccountingState::default(),
            workflow: WorkflowDiagnosticsState::default(),
            flight_recorder: FlightRecorderState::default(),
            tech: TechCockpitState::default(),
            ops: OpsWorkbenchState::default(),
            listing_media: ListingMediaState::default(),
            deal_workspace: DealWorkspaceState::default(),
            // Generation zero is "no host has said", which is what a program built by a test or an example holds.
            generation: 0,
        }
    }
}

impl Model {
    pub fn selected(&self) -> Option<&Row> {
        let id = self.selected_row_id.as_deref()?;
        self.rows.iter().find(|row| row.id == id)
    }
}

/// Every intent the shell can receive. This is the whole vocabulary a widget may speak.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    /// The screen mounted, or navigation arrived that needs data.
    ScreenOpened(Screen),
    /// A HOST RUN OPENED THIS SCREEN, and this is the generation it belongs to.
    ///
    /// The difference from `ScreenOpened` is the generation: the host numbers its runs, and the number travels with
    /// every request this mount makes so the answer can be matched to the question. A run that is replaced — the host
    /// re-mounted, the route changed, an old async run resuming after its cleanup — must not be able to open its screen
    /// over the current one, and this is where that is refused.
    Mount {
        screen: Screen,
        generation: u64,
    },
    /// A dynamic portal record mounted directly from its Next route.
    MountScoped {
        screen: Screen,
        scope: Option<String>,
        generation: u64,
    },
    /// The user picked a screen from the nav.
    Navigate(Screen),
    RowsLoaded {
        screen: String,
        generation: u64,
        rows: Vec<Row>,
    },
    RowSelected(String),
    /// A row was opened as a record rather than merely selected: on a listing, this navigates to that record's own
    /// screen. A screen with no record treats it as a selection instead, so the same click is never ambiguous.
    RecordOpened(String),
    /// The host reports a failed request. The model keeps what it had; the message is the record.
    /// A request failed — and it says WHOSE failure it was, for the same reason a successful answer does.
    ///
    /// A FAILURE CAN BE STALE TOO, and an unowned one is worse than an unowned payload: a rejected request for a screen
    /// the visitor has left would put its error message on the screen they are on now, and clear its loading state while
    /// its own request is still in flight. `update::owns` refuses it on the same rule as `PageLoaded` and `RowsLoaded`.
    EffectFailed {
        screen: String,
        generation: u64,
        message: String,
    },

    /// A public page's content arrived: the blocks an editorial page is built from.
    ///
    /// IT ARRIVES WITH ITS OWNER. `screen` and `generation` are the ones the request was issued under, and the reducer
    /// refuses the answer if they are not the ones the model holds now — a payload for screen A must never mutate screen
    /// B, however the network reorders things.
    PageLoaded {
        screen: String,
        generation: u64,
        page: PageContent,
    },

    /// A portal screen's payload arrived, with its owner — the same rule as a public page's.
    PortalLoaded {
        screen: String,
        generation: u64,
        page: PortalPage,
    },

    // ---- Public property media -----------------------------------------------------------------------------------
    /// Select one image in the canonical hero + gallery order.
    PropertyMediaSelected(usize),
    PropertyMediaPrevious,
    PropertyMediaNext,
    PropertyLightboxOpened(usize),
    PropertyLightboxClosed,
    /// Move the lightbox by -1 or +1, wrapping at both ends.
    PropertyLightboxMoved(i8),
    PropertyTabSelected(PropertyTab),
    PropertyBrowserLoaded { id: String, saved: bool, recent: Vec<PropertyRecent> },
    PropertyFavoriteToggled,
    PropertyFavoriteStored { id: String, saved: bool },

    // ---- Flight Recorder -----------------------------------------------------------------------------------------
    FlightRecorderRefreshRequested,
    FlightRecorderLoaded {
        screen: String,
        generation: u64,
        instance_id: String,
        transaction: serde_json::Value,
    },

    // ---- TECH / Engineering Cockpit -----------------------------------------------------------------------------
    TechStorySelected(String),
    TechRefreshRequested,
    TechScheduleChanged(String),
    TechClearWorkbenchRequested,
    TechGoodToGoRequested,
    TechScopedRunRequested(String),
    TechMoveWorkbenchRequested(String),
    TechLaunchFlightRequested,
    TechScheduleFlightRequested { scheduled_for: String },
    TechCancelFlightRequested(String),
    TechCommandCompleted {
        screen: String,
        generation: u64,
        ok: bool,
        message: String,
    },

    // ---- OPPS / universal Data Workbench -------------------------------------------------------------------------
    OpsEntitySelected(String),
    OpsSectionSelected(String),
    OpsRailToggled,
    OpsFieldChanged { key: String, value: String },
    OpsSaveRequested,
    OpsRevertRequested,
    OpsCreateToggled,
    OpsCreateNameChanged(String),
    OpsCreateRequested,
    OpsPersonQueryChanged(String),
    OpsPersonSelected(String),
    OpsPeopleLoaded {
        screen: String,
        generation: u64,
        query: String,
        people: Vec<PortalDealPersonCandidate>,
    },
    OpsMediaSelected(usize),
    OpsMediaUploaderToggled,
    OpsMediaRoleChanged(String),
    OpsMediaAltChanged(String),
    OpsMediaFileChosen(String),
    OpsMediaUploadRequested,
    OpsMediaUploadCompleted {
        screen: String,
        generation: u64,
    },
    OpsVideoRefreshRequested,

    // ---- OPPS / legacy Records + Listing Media -------------------------------------------------------------------
    RecordArchiveRequested,
    ListingMediaRoleChanged(String),
    ListingMediaAltChanged(String),
    ListingMediaFileChosen(String),
    ListingMediaUploadRequested,
    ListingMediaUploadCompleted {
        screen: String,
        generation: u64,
    },

    // ---- Contracts / Deal workspace -----------------------------------------------------------------------------
    DealCreateToggled,
    DealCreatePropertyChanged(String),
    DealCreateClientQueryChanged(String),
    DealCreateClientSelected {
        id: String,
        label: String,
    },
    DealCreateOwnerChanged(String),
    DealCreateNotesChanged(String),
    DealPeopleLoaded {
        screen: String,
        generation: u64,
        query: String,
        people: Vec<PortalDealPersonCandidate>,
    },
    DealCreateRequested,
    DealCreated {
        screen: String,
        generation: u64,
        id: String,
    },
    DealWorkspaceTaskTitleChanged(String),
    DealWorkspaceTaskDetailChanged(String),
    DealWorkspaceTaskDueChanged(String),
    DealWorkspaceCreateTaskRequested,
    DealWorkspaceCompleteTaskRequested { task_id: String },
    DealWorkspaceOfferAmountChanged { key: String, value: String },
    DealWorkspaceSubmitOfferRequested { parent_offer_id: Option<String> },
    DealWorkspaceWithdrawOfferRequested { offer_id: String },
    DealWorkspaceRejectOfferRequested { offer_id: String },
    DealWorkspaceCreateShowingRequested,
    DealWorkspaceShowingTimeChanged { showing_id: String, value: String },
    DealWorkspaceScheduleShowingRequested { showing_id: String },
    DealWorkspaceCancelShowingRequested { showing_id: String },
    DealWorkspaceCompleteShowingRequested { showing_id: String },
    DealWorkspaceParticipantQueryChanged(String),
    DealWorkspaceParticipantSelected { id: String, label: String },
    DealWorkspaceParticipantRoleChanged(String),
    DealWorkspaceAddParticipantRequested,
    DealWorkspaceOtherRoleChanged { participant_id: String, value: String },
    DealWorkspaceUpdateOtherRequested { participant_id: String },
    DealWorkspaceEndOtherRequested { participant_id: String },
    DealWorkspaceStructuralRoleChanged(String),
    DealWorkspaceStructuralQueryChanged(String),
    DealWorkspaceStructuralPersonSelected { id: String, label: String },
    DealWorkspaceStructuralOwnerChanged(String),
    DealWorkspaceSetStructuralRequested,
    DealWorkspaceEndStructuralRequested { participant_id: String },
    DealWorkspacePeopleLoaded {
        screen: String,
        generation: u64,
        purpose: String,
        query: String,
        people: Vec<PortalDealPersonCandidate>,
    },
    DealWorkspaceCommandCompleted {
        screen: String,
        generation: u64,
        id: String,
    },

    // ---- controls: the screen's own input, one message per act --------------------------------------------------
    /// The user typed in the screen's search field.
    QueryChanged(String),
    /// The user chose a dropdown option, by value.
    FilterChanged(String),
    /// The user chose one of the screen's NAMED dropdowns: `key` is the name the control carries (`price`, `beds`,
    /// `sort` on the Buyers bar) and `value` is the chosen option.
    ///
    /// A pair rather than one message per control, for the same reason `Controls::named` is a map: the model should not
    /// have to learn a screen's vocabulary to hold a screen's input. An empty value is the "no filter" option and
    /// REMOVES the entry rather than storing an empty string, so "unset" is one state and not two.
    FilterSelected {
        key: String,
        value: String,
    },
    /// The user picked a tab, by key.
    TabSelected(String),
    /// The user set a switch or checkbox.
    Toggled(bool),
    /// The user asked to move by `delta` pages. A DELTA rather than a target page, so the reducer owns the bounds and
    /// a stale button cannot land the user past the end of a list that shrank while they were reading it.
    PageChanged(i64),

    // ---- accounting: the expense form's own input, one message per act -------------------------------------------
    /// The user opened or closed the New Expense panel.
    ExpenseFormToggled,
    ExpenseVendorChanged(String),
    ExpenseCategoryChanged(String),
    ExpenseAmountChanged(String),
    ExpenseDateChanged(String),
    ExpenseMemoChanged(String),
    /// The user submitted the form.
    ///
    /// NOTHING IS VALIDATED HERE. The rules — vendor required, canonical category, a real non-negative amount — belong to
    /// Rust, and a second copy of them in the reducer is the copy that goes stale. This asks for the command; the answer
    /// decides what the form says.
    ExpenseSubmitted,

    // ---- accounting: the receivable form, and mark-paid ----------------------------------------------
    ReceivableFormToggled,
    ReceivableReferenceChanged(String),
    ReceivableDescriptionChanged(String),
    ReceivableCategoryChanged(String),
    ReceivableAmountChanged(String),
    ReceivableIssuedOnChanged(String),
    ReceivableDueOnChanged(String),
    ReceivableSubmitted,
    /// The operator chose the date one receivable was paid on.
    ReceivablePaidDateChanged { id: String, value: String },
    /// The operator marked a receivable paid. The transition itself is Rust's and the database's: this asks for it.
    ReceivablePaidSubmitted { id: String },

    // ---- accounting: the P&L's period ---------------------------------------------------------------------
    PnlFromChanged(String),
    PnlToChanged(String),
    /// The operator applied the period. The request is for exactly what the two fields hold: a filter that is quietly
    /// widened or dropped is the defect this screen exists to avoid.
    PnlApplied,

    // ---- accounting: the receipt scanner's demonstration ----------------------------------------------------
    /// A file is being dragged over the drop surface (or has left it).
    ScannerDragging(bool),
    /// The operator attached a file.
    ScannerFileChosen(String),
    /// The operator pressed Scan Receipt: the demonstration extracts its next seed.
    ScannerScanned,
    ScannerCategoryChanged(String),
    /// The operator saved the reviewed draft as an expense. The command is the same one the Expenses form issues.
    ScannerSubmitted,

    // ---- system-health: the workflow diagnostics instance list ----------------------------------------------
    /// The operator opened or closed the row for one instance.
    ///
    /// ONE MESSAGE FOR BOTH DIRECTIONS, exactly as `toggleInstance` was one function: whether this is an open or a close is
    /// decided in `update` against what the model already holds, so the view never has to know which it is asking for and two
    /// rows can never both be open.
    WorkflowInstanceToggled {
        instance_id: String,
    },

    /// Cockpit task command. The Rust engine owns application-task completion.
    CockpitTaskCompleteRequested {
        task_id: String,
    },

    /// Forms editor intents. The working draft remains inside Model -> PortalFormsPage.
    FormFieldChanged {
        name: String,
        value: String,
    },
    FormSectionChanged {
        name: String,
        value: String,
    },
    FormSaveRequested,
    FormCreateRequested {
        template_id: String,
    },
    FormCreated {
        form_id: String,
    },

    /// Seller Strategy intents. The reducer owns all assumptions and disclosure state.
    SellerStrategyFieldChanged {
        key: String,
        raw: String,
        percent: bool,
    },
    SellerStrategyOptionToggled {
        option: u8,
        enabled: bool,
    },
    SellerStrategyEditAllToggled,
    SellerStrategyActiveEditChanged(Option<u8>),
    SellerStrategyDetailToggled,
    SellerStrategyReset,

    /// Project Management intents. Vendor widgets are views only; these own workspace state.
    ProjectDomainSelected(String),
    ProjectSelected(String),
    ProjectNodeSelected(Option<String>),
    ProjectViewSelected(String),
    ProjectCatchUpToggled(bool),
    ProjectCatchUpItemSelected {
        project_id: String,
        node_id: String,
    },
    ProjectCatchUpItemCompleteRequested {
        project_id: String,
        node_id: String,
    },
    ProjectStatusRequested(String),
    ProjectWorkTitleChanged(String),
    ProjectWorkNotesChanged(String),
    ProjectWorkOwnerChanged(String),
    ProjectWorkDueChanged(String),
    ProjectWorkStatusChanged(String),
    ProjectWorkCollapsedToggled,
    ProjectWorkSaveRequested,
}

impl Msg {
    /// Parse the JSON the host fetched. A bad payload becomes a visible error rather than a panic, because a panic in
    /// WASM takes the whole screen down and explains nothing.
    pub fn rows_loaded_json(screen: &str, generation: u64, payload: &str) -> Msg {
        match serde_json::from_str::<Vec<Row>>(payload) {
            Ok(rows) => Msg::RowsLoaded {
                screen: screen.to_string(),
                generation,
                rows,
            },
            // A payload that will not parse is a failure, and it knows whose it is: the screen and the generation the
            // request was made under travel with the parse, so a malformed answer to a question nobody is asking any
            // more is discarded rather than shown.
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("could not read the screen payload: {error}"),
            },
        }
    }

    /// The same contract for a page: the host fetched blocks, and a payload that does not parse is an error the user
    /// can see rather than a page that silently renders empty.
    pub fn page_loaded_json(screen: &str, generation: u64, payload: &str) -> Msg {
        match serde_json::from_str::<PageContent>(payload) {
            Ok(page) => Msg::PageLoaded {
                screen: screen.to_string(),
                generation,
                page,
            },
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("could not read the page payload: {error}"),
            },
        }
    }
    /// The same contract for a portal screen: the fields its real component renders, not a column list.
    pub fn portal_loaded_json(screen: &str, generation: u64, payload: &str) -> Msg {
        match serde_json::from_str::<PortalPage>(payload) {
            Ok(page) => Msg::PortalLoaded {
                screen: screen.to_string(),
                generation,
                page,
            },
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("could not read the portal payload: {error}"),
            },
        }
    }
}

/// What the host must do next. Requests, never decisions.
///
/// THE WIRE FORMAT IS A CONTRACT WITH THE HOST, and it is not `camelCase`: the tag is the variant name exactly as the
/// TypeScript side spells it (`FetchRows`, `FetchPage`). `rename_all = "camelCase"` renamed the TAG as well as the
/// fields, so Rust emitted `"fetchRows"` while the host looked for `"FetchRows"` — and a host that does not recognise an
/// effect ignores it. Every request was silently discarded: no rows, no page content, no error, just the chrome that
/// renders before the first effect. That is what every blank screen was.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "effect")]
pub enum Effect {
    PropertyBrowserRead { id: String, slug: String, title: String, valid_slugs: Vec<String> },
    PropertyFavoriteWrite { id: String, slug: String, title: String, saved: bool },
    /// Fetch rows for this screen, optionally about one record.
    ///
    /// The screen travels with the effect rather than being scraped back out of the DOM, and `scope` is the record key
    /// when the screen is about one record. Turning that into a request — which path, which query parameter — stays
    /// the host's business, because the host is what owns the network.
    FetchRows {
        screen: &'static str,
        scope: Option<String>,
        /// Which mount asked. The host puts it on the request and presents it back with the answer.
        generation: u64,
    },
    /// Fetch one canonical Flight Recorder transaction by process-instance UUID.
    FetchFlightRecorder {
        screen: &'static str,
        instance_id: String,
        generation: u64,
    },
    /// Fetch the TECH Engineering Cockpit assembly-line projection.
    FetchTech {
        screen: &'static str,
        selected: Option<String>,
        generation: u64,
    },
    /// Run one TECH Cockpit operator command. The reducer owns the intent; the browser runner only transports it.
    TechCommand {
        screen: &'static str,
        generation: u64,
        body: serde_json::Value,
    },
    /// Fetch the CORE Cabinet from the authoritative Rust Vault service.
    FetchCabinet {
        screen: &'static str,
        generation: u64,
    },
    /// Fetch the CORE Cockpit from its dedicated Rust-backed bridge.
    FetchCockpit {
        screen: &'static str,
        generation: u64,
    },
    /// Complete one application task from the Cockpit and return a refreshed snapshot.
    CompleteCockpitTask {
        screen: &'static str,
        generation: u64,
        task_id: String,
    },

    /// OPPS universal workbench read. The active entity is part of the request rather than a new screen.
    FetchOps {
        screen: &'static str,
        entity: String,
        selected: Option<String>,
        search: String,
        page: usize,
        generation: u64,
    },
    /// Persist whichever typed entity is active through the reviewed workbench bridge.
    SaveOps {
        screen: &'static str,
        entity: String,
        id: String,
        fields: BTreeMap<String, String>,
        search: String,
        page: usize,
        generation: u64,
    },
    /// Property is the first entity with create enabled in the universal shell.
    CreateOpsProperty {
        screen: &'static str,
        name: String,
        generation: u64,
    },
    /// Search canonical people by human identity for Property relations.
    SearchOpsPeople {
        screen: &'static str,
        query: String,
        generation: u64,
    },
    /// Upload a Property image from the workbench Media tab.
    UploadOpsMedia {
        screen: &'static str,
        property_id: String,
        role: String,
        alt: String,
        generation: u64,
    },
    /// OPPS Records read with server-side search, paging and selected property.
    FetchRecords {
        screen: &'static str,
        selected: Option<String>,
        search: String,
        page: usize,
        generation: u64,
    },
    /// OPPS Listing Media read with server-side search, paging and selected property.
    FetchListingMedia {
        screen: &'static str,
        selected: Option<String>,
        search: String,
        page: usize,
        generation: u64,
    },
    /// Archive or restore the selected property and return the refreshed Records payload.
    RecordArchive {
        screen: &'static str,
        property_id: String,
        archived: bool,
        search: String,
        page: usize,
        generation: u64,
    },
    /// Upload the browser-selected listing photo; bytes are read from the DOM by the effect runner.
    UploadListingMedia {
        screen: &'static str,
        property_id: String,
        role: String,
        alt: String,
        generation: u64,
    },
    /// Fetch a portal screen's payload.
    ///
    /// A SEPARATE EFFECT FROM `FetchPage` because it is a separate route with a separate audience: the public page feed
    /// is unauthenticated and reads what the site already publishes, while this one answers only for an authenticated
    /// portal user. One effect meaning two audiences is how a public request ends up asking a private route.
    FetchPortal {
        screen: &'static str,
        scope: Option<String>,
        generation: u64,
    },
    /// One of Accounting V1's commands, and the refreshed screen as its answer.
    ///
    /// THE BODY IS THE COMMAND, in the bridge's own vocabulary (`action`, `vendor`, `amount`, …), and it is built by the
    /// reducer rather than by the view: an effect is a request, and what a screen is asking for is the reducer's decision.
    /// The answer is a portal payload, so a successful write and the refresh that follows it are one round trip and the
    /// list can never be a row behind what was just saved.
    AccountingCommand {
        screen: &'static str,
        generation: u64,
        body: serde_json::Value,
    },
    /// The P&L for a period the visitor chose.
    ///
    /// ITS OWN EFFECT because it is the one Accounting read that carries a request parameter: the range is what the screen
    /// is asking about, and an effect that dropped it would answer a question nobody asked. The two ends travel as the
    /// strings the date inputs hold, and the service validates them — a backwards period is a refusal, not an empty report.
    FetchAccountingPnl {
        screen: &'static str,
        generation: u64,
        from: String,
        to: String,
    },
    /// CORE Clients uses server-side search and paging, plus one selected person's detail bundle.
    FetchClients {
        screen: &'static str,
        scope: Option<String>,
        selected: Option<String>,
        search: String,
        page: usize,
        generation: u64,
    },
    /// CORE Forms read transport.
    FetchForms {
        screen: &'static str,
        scope: Option<String>,
        generation: u64,
    },
    /// Persist the reducer-owned working draft.
    SaveForm {
        screen: &'static str,
        generation: u64,
        form_id: String,
        field_values: BTreeMap<String, String>,
        sections: BTreeMap<String, String>,
    },
    /// Start another form from the current transaction/client/property context.
    CreateForm {
        screen: &'static str,
        generation: u64,
        template_id: String,
        deal_id: Option<String>,
        person_id: Option<String>,
        property_id: Option<String>,
    },
    /// CORE Contracts reads the canonical Deal portfolio and its Contract artifacts from Rust.
    FetchDeals {
        screen: &'static str,
        scope: Option<String>,
        generation: u64,
    },
    /// Search canonical people for the Contracts create panel.
    SearchDealPeople {
        screen: &'static str,
        generation: u64,
        query: String,
    },
    /// Create one canonical Deal through the reviewed Rust command bridge.
    CreateDeal {
        screen: &'static str,
        generation: u64,
        property_id: String,
        client_person_id: String,
        owner_user_id: Option<String>,
        notes: Option<String>,
    },
    /// Search people while editing one Deal workspace role.
    SearchDealWorkspacePeople {
        screen: &'static str,
        generation: u64,
        purpose: String,
        query: String,
    },
    /// Run one Rust-owned Deal workspace command.
    RunDealWorkspaceCommand {
        screen: &'static str,
        generation: u64,
        deal_id: String,
        command: PortalDealCommand,
    },
    /// CORE Project Management reads only from Rust Project/WBS/Vault APIs.
    FetchProjects {
        screen: &'static str,
        generation: u64,
    },
    UpdateProjectStatus {
        screen: &'static str,
        generation: u64,
        project_id: String,
        status: String,
    },
    SaveProjectWork {
        screen: &'static str,
        generation: u64,
        item_id: String,
        title: String,
        notes: String,
        status: String,
        due_at: Option<String>,
        owner: Option<String>,
    },
    /// Browser navigation is an effect, not a view mutation.
    BrowserNavigate { href: String },
    /// Fetch a public page's content: the blocks, not the rows.
    ///
    /// A SEPARATE EFFECT because it is a different request and a different shape. A list screen asks what its rows are;
    /// an editorial page asks for its hero, its sections and its cards, and the answer is a `PageContent`. Sending that
    /// through `FetchRows` would mean overloading one payload with two meanings, which is how a page ends up rendered
    /// as a list of strings.
    ///
    /// `scope` IS THE RECORD KEY FOR A PAGE ABOUT ONE RECORD, and it is carried here for the same reason it is carried
    /// on `FetchRows` — because the alternative is a request that cannot name what it is about. `/properties/<slug>` is
    /// an editorial page *and* a record page: it is served by the page feed, it renders blocks, and the slug is the
    /// whole of what makes it that property rather than another. This effect used to carry the screen and nothing else,
    /// so a slug the screen already knew was dropped on the floor between the reducer and the host; the request went out
    /// without it, the page route correctly refused to guess (`if (!slug)`), and the child page showed a host error
    /// instead of a property. The key travels with the request, in both of the two requests that can be about a record.
    FetchPage {
        screen: &'static str,
        scope: Option<String>,
        generation: u64,
    },
}

// ---------------------------------------------------------------------------
// Accounting V1, on the wire.
//
// MONEY IS A STRING IN ALL OF THESE, and that is the point rather than an oversight: the amounts are Postgres `numeric`
// and they arrive as the digits the database holds. A `f64` here would round a cent away somewhere between the server and
// the screen, and a total that is a cent out is a total nobody can reconcile.
//
// These mirror `domain::accounting` field for field. The UI crate does not depend on the domain crate — the browser only
// ever sees JSON — so the two are kept in step by the payload being the contract.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSupportPage {
    /// `/portal/db-test` — the database/client diagnostic the pre-cutover screen showed.
    pub db_test: Option<PortalDbTest>,
    /// `/portal/settings` — the Security landing screen: its counts and the break-glass posture.
    pub security: Option<PortalSecurity>,
    /// `/portal/admin/whatsapp-meta` — what Meta says about this deployment's WhatsApp number.
    pub whats_app_meta: Option<PortalWhatsAppMeta>,
    /// `/portal/system-health` — the operational health snapshot, the environment posture, and the workflow diagnostics.
    pub system_health: Option<PortalSystemHealthPage>,
}

/// The system-health screen's whole payload: three reads the pre-cutover page made together.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSystemHealthPage {
    pub health: PortalSystemHealthSnapshot,
    pub environment: PortalEnvironmentReadiness,
    pub diagnostics: PortalWorkflowDiagnostics,
}

/// The operational health snapshot: counts and signals, field for field as `legacy/db/system-health.ts` returns them.
///
/// EVERY FIELD IS A COUNT OR A LABEL about something the application can be wrong about. Nothing here is a name, an address
/// or a credential — the snapshot answers "how is the book", not "who is in it".
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSystemHealthSnapshot {
    pub unresolved_intake_count: i64,
    pub open_task_count: i64,
    pub overdue_task_count: i64,
    pub active_deal_count: i64,
    pub under_contract_count: i64,
    pub active_property_count: i64,
    pub recent_interaction_at_label: Option<String>,
    pub interactions_last7_days: i64,
    pub persons_without_email_identity: i64,
    pub persons_without_phone_identity: i64,
    pub open_tasks_without_due_date: i64,
    pub active_properties_without_hero_media: i64,
    pub completed_showings_missing_completed_at: i64,
    pub scheduled_showings_missing_scheduled_at: i64,
    pub active_participants_with_ended_at: i64,
    pub other_participants_missing_role_label: i64,
    pub offers_with_cross_deal_parent: i64,
    pub showings_with_deal_property_mismatch: i64,
    pub completed_showings_missing_showing_interaction: i64,
    pub inactive_participants_without_ended_at: i64,
    pub public_properties_with_multiple_heroes: i64,
    pub hero_media_not_image: i64,
    pub account_type_mismatch_count: i64,
    pub active_app_users_without_role: i64,
    pub auth_identity_inactive_app_user: i64,
    pub owner_assignments: i64,
    pub multiple_owners: i64,
    pub auth_identity_without_usable_app_user: i64,
}

/// Environment and secrets readiness: POSTURE, never values.
///
/// THIRTEEN BOOLEANS. Not one URL, key, token or secret — the projection itself is built that way, and this type carries it
/// through without widening it. "Configured" is the most any of these screens is allowed to know about a secret.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalEnvironmentReadiness {
    pub is_production: bool,
    pub database_configured: bool,
    pub database_dev_prod_separated: bool,
    pub auth_secret_configured: bool,
    pub auth_provider_configured: bool,
    pub break_glass_configured: bool,
    pub break_glass_enabled: bool,
    pub google_maps_key_configured: bool,
    pub google_maps_demo_key_absent_in_production: bool,
    pub mux_configured: bool,
    pub broker_signature_configured: bool,
    pub broker_signature_enabled: bool,
    pub all_production_required_configured: bool,
}

/// The workflow diagnostics snapshot: the counts, the definitions, the instances, and what the anomaly sweep found.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowDiagnostics {
    /// Whether the engine's tables are present at all — the projection answers `false` on a database that predates them.
    pub configured: bool,
    pub summary: PortalWorkflowDiagnosticsSummary,
    pub definitions: Vec<PortalWorkflowDefinition>,
    pub instances: Vec<PortalWorkflowInstance>,
    pub anomalies: Vec<PortalWorkflowAnomaly>,
    /// ONE INSTANCE'S DETAIL, loaded when a row is opened and absent until then.
    ///
    /// IT RIDES IN THE SAME PAYLOAD as the list because the screen is one payload: a second route for "the detail of the row
    /// the operator just opened" would be a second shape to keep in step with this one, and the list that arrives with it is
    /// the same list — which is what lets the screen re-render the row it opened without asking for anything else.
    pub detail: Option<PortalWorkflowInstanceDetail>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowDiagnosticsSummary {
    pub definition_count: i64,
    pub instance_total: i64,
    pub instance_active: i64,
    pub instance_completed: i64,
    pub instance_failed: i64,
    pub instance_other: i64,
    pub ready_engine_tasks: i64,
    pub correlated_open_canonical_tasks: i64,
    pub pending_jobs: i64,
    pub pending_receipts: i64,
    pub anomaly_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowDefinition {
    pub definition_id: String,
    pub key: String,
    pub version: i64,
    pub name: String,
    pub status: String,
    pub instance_count: i64,
    pub active_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowInstance {
    pub instance_id: String,
    pub definition_key: String,
    pub definition_version: i64,
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub active_token_count: i64,
    pub task_count: i64,
    pub event_count: i64,
    /// The property this instance is about, resolved by the projection when the subject names one.
    pub property_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowAnomaly {
    pub kind: String,
    /// `info`, `warning` or `critical` — the projection's own vocabulary, rendered as its own pill.
    pub severity: String,
    pub instance_id: Option<String>,
    pub subject_id: Option<String>,
    pub message: String,
}

/// One instance, opened: its tokens, the tasks waiting on them, its jobs, events, task correlations and command receipts.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowInstanceDetail {
    pub instance_id: String,
    pub definition_key: String,
    pub definition_version: i64,
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub active_token_count: i64,
    pub task_count: i64,
    pub event_count: i64,
    /// The instance's variables, as the engine stored them — arbitrary JSON, shown as it is.
    pub variables: Option<serde_json::Value>,
    /// Node ids to their labels, so a token can be named rather than addressed.
    pub node_labels: std::collections::BTreeMap<String, String>,
    pub tokens: Vec<PortalWorkflowToken>,
    pub tasks: Vec<PortalWorkflowTask>,
    pub jobs: Vec<PortalWorkflowJob>,
    pub events: Vec<PortalWorkflowDiagnosticEvent>,
    pub correlations: Vec<PortalWorkflowCorrelation>,
    pub commands: Vec<PortalWorkflowCommand>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowToken {
    pub id: String,
    pub parent_token_id: Option<String>,
    pub node_id: String,
    pub status: String,
    pub outcome: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowTask {
    pub id: String,
    pub token_id: Option<String>,
    pub name: String,
    pub status: String,
    pub candidates: Vec<String>,
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowJob {
    pub id: String,
    pub job_type: String,
    pub status: String,
    pub due_at: Option<String>,
}

/// One engine event on an instance, as the diagnostics detail shows it.
///
/// NAMED FOR ITS SCREEN: `PortalWorkflowEvent` is the workflows screen's own event shape, and the two are not the same thing
/// — one is a milestone of a definition-driven timeline, this is a row of the engine's log.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowDiagnosticEvent {
    pub id: String,
    pub event_type: String,
    pub node_id: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowCorrelation {
    pub workflow_task_id: String,
    pub application_task_id: Option<String>,
    pub application_task_status: Option<String>,
    pub application_task_title: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowCommand {
    pub command_id: String,
    pub command_type: String,
    pub node_id: String,
    pub outcome: String,
    pub message: Option<String>,
    pub receipt_outcome: Option<String>,
}

/// Which workflow instance is open on System Health, and what happened when we asked for it.
///
/// THE INTERACTION THE EARLIER CONVERSION DROPPED. Pre-cutover this was React state — `selectedId`, `loadingId`, `detail`,
/// `error` — mutated by `toggleInstance`. It is here because it is state: the same click that expands a row is a message, the
/// read it triggers is an effect, and what comes back is decided in `update` rather than in the component that drew it.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowDiagnosticsState {
    /// The instance whose row is open, if any. NOT the same as `loading_instance`: a row can be open with nothing to show yet.
    pub selected_instance: Option<String>,
    /// The instance we have asked for and not yet heard about — what the loading line is keyed on.
    pub loading_instance: Option<String>,
    /// What the read said, when it said something bad. A missing instance is the ordinary case: the row was open, the id no
    /// longer resolves, and the operator is told so in the row rather than in a panel that steals the page.
    pub error: Option<String>,
    /// The open instance's detail, once it has arrived. Held here and not in the payload so the screen reads one place.
    pub detail: Option<PortalWorkflowInstanceDetail>,
}

/// The WhatsApp diagnostic: the four outcomes the pre-cutover screen distinguished, and the phone fields it printed.
///
/// THERE IS NO TOKEN FIELD HERE, and there must never be one. The access token is used inside the bridge and is not part of
/// this shape, so it cannot be serialised into a payload even by a mistake elsewhere.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWhatsAppMeta {
    pub waba_id: String,
    /// Whether a token exists at all — a yes/no about configuration, which is all a diagnostic needs to say.
    pub token_configured: bool,
    /// Meta's own words when it refused, or the absent-token notice, or nothing when the call succeeded.
    pub error: Option<String>,
    pub phones: Vec<PortalWhatsAppPhone>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWhatsAppPhone {
    pub id: Option<String>,
    pub display_phone_number: Option<String>,
    pub verified_name: Option<String>,
    pub quality_rating: Option<String>,
    pub code_verification_status: Option<String>,
}

/// The Security screen: operational counts, and break-glass posture.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSecurity {
    pub status: PortalSecurityStatus,
    pub break_glass: PortalBreakGlassReadiness,
}

/// The nine counts, each named for what it counts. Counts of things, not values of anything.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSecurityStatus {
    pub active_internal_users: i64,
    pub external_users: i64,
    pub users_with_no_role: i64,
    pub users_with_multiple_roles: i64,
    pub mapped_auth_identities: i64,
    pub unmapped_app_users: i64,
    pub owner_role_assignments: i64,
    pub inactive_users_with_active_role_mappings: i64,
    pub account_type_mismatch_count: i64,
}

/// Break-glass posture as six booleans — and nothing else, ever.
///
/// THIS TYPE MUST NOT GROW. The configuration behind it holds the root user's id and secret hash; what crosses is whether
/// each condition holds. A field added here is a field that could carry a secret to a browser, so the type is deliberately
/// six booleans and the payload module builds it field by field.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalBreakGlassReadiness {
    pub configured: bool,
    pub enabled: bool,
    pub root_resolvable: bool,
    pub root_active: bool,
    pub owner_role_present: bool,
    pub audit_table_available: bool,
}

/// The DB Test screen's read: whether the database answered, how many clients it holds, and the identity columns of each.
///
/// SMALLER THAN THE READ BEHIND IT, deliberately. `getClients()` returns a client's budget, preferences, priorities and
/// interests; a diagnostic screen printed the whole record as JSON and had no business carrying any of it.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDbTest {
    /// True when the read answered at all — see the payload: the flag is the absence of a failure, not a hopeful constant.
    pub connected: bool,
    pub client_count: i64,
    pub clients: Vec<PortalDbTestClient>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDbTestClient {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingPage {
    /// `/portal/accounting` — the projections over the two tables.
    pub dashboard: Option<PortalAccountingDashboard>,
    /// `/portal/accounting/expenses`.
    pub expenses: Vec<PortalAccountingExpense>,
    /// `/portal/accounting/expenses` — every posted expense by category with its share, so the screen's ring is drawn from
    /// one aggregation instead of a second sum computed in the browser.
    pub expense_categories: Vec<PortalAccountingShare>,
    /// `/portal/accounting/receivables`.
    pub receivables: Vec<PortalAccountingReceivable>,
    /// `/portal/accounting/pnl` — the period the caller asked for, echoed back.
    pub pnl: Option<PortalAccountingPnl>,
    /// The database's idea of today, so a new record's date field starts on the book's day rather than the browser's.
    pub today: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingLine {
    pub label: String,
    pub amount: String,
}

/// A category's total and its share of the month, both computed by the database.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingShare {
    pub label: String,
    pub amount: String,
    pub percent: i64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingTrendPoint {
    pub month: String,
    pub income: String,
    pub expenses: String,
    pub net: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingDashboard {
    pub receivables_outstanding: String,
    pub expenses_this_month: String,
    pub net_income: String,
    pub open_count: i64,
    pub overdue_count: i64,
    pub pnl_trend: Vec<PortalAccountingTrendPoint>,
    pub trend_income: String,
    pub trend_expenses: String,
    pub trend_net: String,
    pub recent_expenses: Vec<PortalAccountingExpense>,
    pub recent_activity: Vec<PortalAccountingReceivable>,
    pub expense_categories: Vec<PortalAccountingShare>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingReceivable {
    pub id: String,
    pub reference: Option<String>,
    pub description: String,
    pub category: String,
    pub amount: String,
    pub issued_on: String,
    pub due_on: Option<String>,
    /// `OPEN`, `PAID` or `VOID`.
    pub status: String,
    pub paid_on: Option<String>,
    pub deal_id: Option<String>,
    pub deal_name: Option<String>,
    pub property_id: Option<String>,
    pub property_name: Option<String>,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingExpense {
    pub id: String,
    pub vendor: String,
    pub category: String,
    pub amount: String,
    pub expense_on: String,
    /// `DRAFT`, `POSTED` or `VOID`.
    pub status: String,
    pub memo: Option<String>,
    pub deal_id: Option<String>,
    pub deal_name: Option<String>,
    pub property_id: Option<String>,
    pub property_name: Option<String>,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalAccountingPnl {
    pub from: String,
    pub to: String,
    pub income: Vec<PortalAccountingLine>,
    pub total_income: String,
    pub expenses: Vec<PortalAccountingLine>,
    pub total_expenses: String,
    pub net_income: String,
}
