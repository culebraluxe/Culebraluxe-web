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
        }
    }

    /// The live route this port replaces.
    pub fn portal_path(self) -> &'static str {
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
}

impl Default for Model {
    fn default() -> Self {
        Self {
            screen: Screen::Dashboard,
            loading: false,
            error: None,
            rows: Vec::new(),
            selected_row_id: None,
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
pub enum Effect {
    /// Fetch rows for this screen and hand them back through `rows_loaded`.
    FetchRows,
}
