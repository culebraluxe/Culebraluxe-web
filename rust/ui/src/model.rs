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
    Screen { key: "accounting-receipt-scanner", title: "Receipt Scanner", path: "/portal/accounting/receipt-scanner", surface: Surface::Accounting, nav: Nav::Listed, detail_of: None, deferred: Some("This screen is a demo placeholder by design: its own header calls it FAKE V1, with deterministic demo extraction and no OCR vendor. Real OCR is deferred to a separate story, so there is nothing to read yet — the polish is the deliverable.") },

    // ---- MARKETING ----
    Screen { key: "marketing", title: "Dashboard", path: "/portal/marketing", surface: Surface::Marketing, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "marketing-syndication", title: "Syndication", path: "/portal/marketing/syndication", surface: Surface::Marketing, nav: Nav::Listed, deferred: None, detail_of: None },

    // ---- OPPS (the registry's own spelling) ----
    Screen { key: "issues", title: "Issue Queue", path: "/portal/issues", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "needs-review", title: "Needs Review", path: "/portal/needs-review", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "property-admin", title: "Property Admin", path: "/portal/property-admin", surface: Surface::Ops, nav: Nav::Listed, deferred: None, detail_of: None },
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
    Screen { key: "framer-ui-lab", title: "Framer UI Lab", path: "/portal/tech/framer-ui-lab", surface: Surface::Tech, nav: Nav::Listed, deferred: Some("A client-rendered experiment lab: its model and controller live in ui/framer-ui-lab and its view is a React component with its own CSS module, so there is no read model to return. Promoting an experiment into the shared Rust view language is a separate, deliberate step."), detail_of: None },
    Screen { key: "media-test", title: "Media Test", path: "/portal/media-test", surface: Surface::Tech, nav: Nav::Listed, deferred: Some("A manual harness for the media pipeline: it uploads, transforms and inspects. There is no read model to render, and a list of rows would not be the screen that was here.") , detail_of: None },
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
    pub hero_url: Option<String>,
    pub gallery: Vec<MediaItem>,
    pub videos: Vec<MediaItem>,
    pub documents: Vec<MediaItem>,
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
    /// The user picked a screen from the nav.
    Navigate(Screen),
    RowsLoaded(Vec<Row>),
    RowSelected(String),
    /// A row was opened as a record rather than merely selected: on a listing, this navigates to that record's own
    /// screen. A screen with no record treats it as a selection instead, so the same click is never ambiguous.
    RecordOpened(String),
    /// The host reports a failed request. The model keeps what it had; the message is the record.
    EffectFailed(String),

    /// A public page's content arrived: the blocks an editorial page is built from.
    PageLoaded(PageContent),

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
}

impl Msg {
    /// Parse the JSON the host fetched. A bad payload becomes a visible error rather than a panic, because a panic in
    /// WASM takes the whole screen down and explains nothing.
    pub fn rows_loaded_json(payload: &str) -> Msg {
        match serde_json::from_str::<Vec<Row>>(payload) {
            Ok(rows) => Msg::RowsLoaded(rows),
            Err(error) => Msg::EffectFailed(format!("could not read the screen payload: {error}")),
        }
    }

    /// The same contract for a page: the host fetched blocks, and a payload that does not parse is an error the user
    /// can see rather than a page that silently renders empty.
    pub fn page_loaded_json(payload: &str) -> Msg {
        match serde_json::from_str::<PageContent>(payload) {
            Ok(page) => Msg::PageLoaded(page),
            Err(error) => Msg::EffectFailed(format!("could not read the page payload: {error}")),
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
    /// Fetch rows for this screen, optionally about one record.
    ///
    /// The screen travels with the effect rather than being scraped back out of the DOM, and `scope` is the record key
    /// when the screen is about one record. Turning that into a request — which path, which query parameter — stays
    /// the host's business, because the host is what owns the network.
    FetchRows {
        screen: &'static str,
        scope: Option<String>,
    },
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
    },
}
