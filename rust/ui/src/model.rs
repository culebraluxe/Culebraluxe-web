//! The Model: every screen the portal menu can reach, and the state the shell renders.
//!
//! THE SCOPE IS THE MENU. Each variant here is a route that exists in `app/portal`, and `portal_path()` records which
//! one, so the port cannot quietly grow screens nobody can navigate to or drift from the menu it replaces.

/// One screen. Order follows the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Clients,
    Deals,
    Marketing,
    MarketingSyndication,
    PropertyAdmin,
    Showings,
    Storyboard,
    Attention,
    NeedsReview,
    Activity,
    AccountingExpenses,
    AccountingReceiptScanner,
    AccountingReceivables,
    CommandConsole,
    Tech,
    TechAppErrors,
    TechFlightRecorder,
    TechRuns,
    /// Project Management: deliberately not ported yet. The live TypeScript screen keeps its three third-party
    /// widgets (React Arborist tree, Gantt, FullCalendar) until the plan for Rust owning the container while each
    /// widget keeps its own subtree is settled. This variant exists so the navigation and state boundary are real.
    Projects,

    // ---- The public site ("the main front"), which is a different audience and a different set of routes ----
    /// The public home page.
    SiteHome,
    /// The public listing index.
    SiteProperties,
    /// One public property record, keyed by slug. The only screen that is *about* a single record rather than a list
    /// of them, which is why it is the first screen whose rows need an argument.
    SitePropertyDetail,
}

/// Which half of the application a screen belongs to. A host mounts ONE area: the public host shows the site nav and
/// the portal host shows the portal nav, so neither can offer a screen from the other — and the public host cannot be
/// handed a portal screen by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Site,
    Portal,
}

impl Area {
    pub fn label(self) -> &'static str {
        match self {
            Self::Site => "Site",
            Self::Portal => "Portal",
        }
    }
}

impl Screen {
    pub fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Clients => "Clients",
            Self::Deals => "Deals",
            Self::Marketing => "Marketing",
            Self::MarketingSyndication => "Syndication",
            Self::PropertyAdmin => "Property admin",
            Self::Showings => "Showings",
            Self::Storyboard => "Storyboard",
            Self::Attention => "Attention",
            Self::NeedsReview => "Needs review",
            Self::Activity => "Activity",
            Self::AccountingExpenses => "Expenses",
            Self::AccountingReceiptScanner => "Receipt scanner",
            Self::AccountingReceivables => "Receivables",
            Self::CommandConsole => "Command console",
            Self::Tech => "Tech",
            Self::TechAppErrors => "App errors",
            Self::TechFlightRecorder => "Flight recorder",
            Self::TechRuns => "Runs",
            Self::Projects => "Projects",
            Self::SiteHome => "Home",
            Self::SiteProperties => "Properties",
            Self::SitePropertyDetail => "Property",
        }
    }

    pub fn area(self) -> Area {
        match self {
            Self::SiteHome | Self::SiteProperties | Self::SitePropertyDetail => Area::Site,
            _ => Area::Portal,
        }
    }

    /// The screen a row opens into, if any. This is the one piece of navigation that is not a nav entry: a listing row
    /// opens its own record, and a screen without a detail view selects instead of navigating.
    pub fn detail(self) -> Option<Screen> {
        match self {
            Self::SiteProperties => Some(Self::SitePropertyDetail),
            _ => None,
        }
    }

    /// The live route this port replaces, in whichever area the screen belongs to.
    pub fn live_path(self) -> &'static str {
        match self {
            Self::Dashboard => "/portal/dashboard",
            Self::Clients => "/portal/clients",
            Self::Deals => "/portal/deals",
            Self::Marketing => "/portal/marketing",
            Self::MarketingSyndication => "/portal/marketing/syndication",
            Self::PropertyAdmin => "/portal/property-admin",
            Self::Showings => "/portal/showings",
            Self::Storyboard => "/portal/storyboard",
            Self::Attention => "/portal/attention",
            Self::NeedsReview => "/portal/needs-review",
            Self::Activity => "/portal/activity",
            Self::AccountingExpenses => "/portal/accounting/expenses",
            Self::AccountingReceiptScanner => "/portal/accounting/receipt-scanner",
            Self::AccountingReceivables => "/portal/accounting/receivables",
            Self::CommandConsole => "/portal/command-console",
            Self::Tech => "/portal/tech",
            Self::TechAppErrors => "/portal/tech/app-errors",
            Self::TechFlightRecorder => "/portal/tech/flight-recorder",
            Self::TechRuns => "/portal/tech/runs",
            Self::Projects => "/portal/projects",
            Self::SiteHome => "/",
            Self::SiteProperties => "/properties",
            // The live route interpolates the slug; the port passes it through the effect instead of baking it into
            // the path, which is why there is no `{}` here.
            Self::SitePropertyDetail => "/properties/[slug]",
        }
    }

    /// The stable name the host fetches rows by. Addressing screens by name means a new screen is one nav entry and
    /// no new endpoint shape.
    pub fn key(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Clients => "clients",
            Self::Deals => "deals",
            Self::Marketing => "marketing",
            Self::MarketingSyndication => "marketing-syndication",
            Self::PropertyAdmin => "property-admin",
            Self::Showings => "showings",
            Self::Storyboard => "storyboard",
            Self::Attention => "attention",
            Self::NeedsReview => "needs-review",
            Self::Activity => "activity",
            Self::AccountingExpenses => "accounting-expenses",
            Self::AccountingReceiptScanner => "accounting-receipt-scanner",
            Self::AccountingReceivables => "accounting-receivables",
            Self::CommandConsole => "command-console",
            Self::Tech => "tech",
            Self::TechAppErrors => "tech-app-errors",
            Self::TechFlightRecorder => "tech-flight-recorder",
            Self::TechRuns => "tech-runs",
            Self::Projects => "projects",
            Self::SiteHome => "site-home",
            Self::SiteProperties => "site-properties",
            Self::SitePropertyDetail => "site-property-detail",
        }
    }

    pub fn is_deferred(self) -> bool {
        matches!(self, Self::Projects)
    }

    /// The inverse of `key`, for the shell: a `data-nav` attribute in the DOM must become a screen again. Returning
    /// `Option` is deliberate — an unknown key is a stale page or a typo, and neither should navigate anywhere.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|screen| screen.key() == key)
    }

    /// Every screen, in menu order. The nav is built from this, so a screen cannot be added without appearing.
    pub const ALL: &'static [Screen] = &[
        // The public site first: a visitor's entry point is the home page, and a host renders the area it owns.
        Screen::SiteHome,
        Screen::SiteProperties,
        Screen::SitePropertyDetail,
        Screen::Dashboard,
        Screen::Clients,
        Screen::Deals,
        Screen::Marketing,
        Screen::MarketingSyndication,
        Screen::PropertyAdmin,
        Screen::Showings,
        Screen::Storyboard,
        Screen::Attention,
        Screen::NeedsReview,
        Screen::Activity,
        Screen::Projects,
        Screen::AccountingExpenses,
        Screen::AccountingReceiptScanner,
        Screen::AccountingReceivables,
        Screen::CommandConsole,
        Screen::Tech,
        Screen::TechAppErrors,
        Screen::TechFlightRecorder,
        Screen::TechRuns,
    ];

    pub fn group(self) -> &'static str {
        match self {
            Self::SiteHome | Self::SiteProperties | Self::SitePropertyDetail => "Site",
            Self::AccountingExpenses
            | Self::AccountingReceiptScanner
            | Self::AccountingReceivables => "Accounting",
            Self::CommandConsole
            | Self::Tech
            | Self::TechAppErrors
            | Self::TechFlightRecorder
            | Self::TechRuns => "Tech",
            _ => "Work",
        }
    }
}

/// A row of any list screen.
///
/// Deliberately generic. This sweep ports screen *structure* — route, heading, nav, state boundary — and per-screen
/// data contracts come after. A generic row means a new screen is a nav entry rather than a new module, and it keeps
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
    /// What the current screen is *about*, when it is about a single record — the property slug on the detail screen.
    /// Kept separate from `selected_row_id` because selecting a row and opening a record are different acts: one is
    /// browsing a list, the other is a different screen with its own fetch.
    pub scope: Option<String>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            // The first screen of the first area, i.e. what a host that says nothing gets. A host that cares which
            // screen opens says so (see `Program::open` / the shell's `mount`), because the URL is the host's business.
            screen: Screen::ALL[0],
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
    /// screen. A screen with no detail view treats it as a selection instead, so the same click is never ambiguous.
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
    /// (a property slug) when the screen is about one record. Turning that into a request — which path, which query
    /// parameter — stays the host's business, because the host is what owns the network.
    FetchRows {
        screen: &'static str,
        scope: Option<String>,
    },
}
