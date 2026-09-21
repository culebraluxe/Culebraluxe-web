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
    Screen { key: "decision-analysis", title: "Decision Analysis", path: "/portal/decision-analysis", surface: Surface::Ops, nav: Nav::Unlisted, deferred: None, detail_of: None },

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
    Screen { key: "design-lab", title: "UI Lab", path: "/portal/design-lab", surface: Surface::Tech, nav: Nav::Listed, deferred: None , detail_of: None },
    Screen { key: "media-test", title: "Media Test", path: "/portal/media-test", surface: Surface::Tech, nav: Nav::Listed, deferred: None , detail_of: None },
    // A screen the captain asked for: what Rust does with layout that the TypeScript pages do not, rendering on the same
    // design tokens. Its route is the only one this port adds on purpose, and it serves the port itself.
    Screen { key: "tech-lab", title: "Tech Lab", path: "/portal/tech/lab", surface: Surface::Tech, nav: Nav::Listed, deferred: None, detail_of: None },
    // RETIRED FROM THE NAV (2026-09-13, the registry's own note): "the code stays, the links go". Command Center,
    // Command Console, GROK and the Flight Recorder LIST were each an attempt at the same problem that never got
    // used, and their names were close enough to "the cockpit" that one conversation could mean five. Ported like
    // everything else, and not listed — which is exactly what the registry says about them.
    Screen { key: "command-center", title: "Command Center", path: "/portal/command-center", surface: Surface::Tech, nav: Nav::Retired, deferred: None, detail_of: None },
    Screen { key: "command-console", title: "Command Console", path: "/portal/command-console", surface: Surface::Tech, nav: Nav::Retired, deferred: None, detail_of: None },
    Screen { key: "tech-grok", title: "GROK", path: "/portal/tech/grok", surface: Surface::Tech, nav: Nav::Retired, deferred: None, detail_of: None },
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
    Screen { key: "site-properties", title: "Properties", path: "", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-property-detail", title: "Property", path: "/properties/[slug]", surface: Surface::Site, nav: Nav::Record, deferred: None, detail_of: Some("site-properties") },
    Screen { key: "site-about", title: "About", path: "/about", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-buyers", title: "Buyers", path: "/buyers", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-sellers", title: "Sellers", path: "/sellers", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-faq", title: "FAQ", path: "/faq", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-contact", title: "Contact", path: "/contact", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-guide", title: "Guide", path: "/guide", surface: Surface::Site, nav: Nav::Listed, deferred: None, detail_of: None },
    Screen { key: "site-services", title: "Services", path: "/services", surface: Surface::Site, nav: Nav::Listed, deferred: None , detail_of: None },
    Screen { key: "site-privacy", title: "Privacy", path: "/privacy", surface: Surface::Site, nav: Nav::Listed, deferred: None , detail_of: None },
    Screen { key: "site-video", title: "Video", path: "/video", surface: Surface::Site, nav: Nav::Listed, deferred: None , detail_of: None },
    Screen { key: "site-whatsapp", title: "WhatsApp", path: "/whatsapp", surface: Surface::Site, nav: Nav::Listed, deferred: None , detail_of: None },
    Screen { key: "site-favorites", title: "Favorites", path: "/favorites", surface: Surface::Site, nav: Nav::Unlisted, deferred: None , detail_of: None },

    // ---- ROUTES THE TABLE WAS MISSING. It is now checked against `find app -name page.tsx` (80 routes, minus the two
    // preview hosts this port adds), rather than against a registry that says of itself "Only EXISTING routes are
    // listed" — a statement about NAVIGATION, not about which routes exist. Titles are the route's own name, not a name
    // I invented for it.
    Screen { key: "login", title: "Login", path: "/login", surface: Surface::Site, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "login-recovery", title: "Login recovery", path: "/login/recovery", surface: Surface::Site, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "login-unauthorized", title: "Login unauthorized", path: "/login/unauthorized", surface: Surface::Site, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "auth-error", title: "Auth error", path: "/auth/error", surface: Surface::Site, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "review", title: "Review", path: "/review/[token]/[page]", surface: Surface::Site, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "portal-root", title: "Portal", path: "/portal", surface: Surface::Core, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "portal-auth-proof", title: "Portal auth proof", path: "/portal-auth-proof", surface: Surface::Support, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "dev-apple-map-test", title: "Apple map test", path: "/dev/apple-map-test", surface: Surface::Support, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "dev-google-map-test", title: "Google map test", path: "/dev/google-map-test", surface: Surface::Support, nav: Nav::Unlisted, deferred: None, detail_of: None },
    Screen { key: "console-story", title: "Command Console story", path: "/portal/command-console/[storyId]", surface: Surface::Tech, nav: Nav::Record, deferred: None, detail_of: Some("command-console") },
];

/// A row of any list screen.
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
}

/// What the host must do next. Requests, never decisions.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "effect")]
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
}
