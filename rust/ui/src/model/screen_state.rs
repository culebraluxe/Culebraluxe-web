//! Shared screen state: page content, rows, list controls, and the per-screen state structs.

#[allow(unused_imports)]
use super::*;

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
    /// The published property an enquiry on the contact page is about, by name (screen `site-contact`, scoped).
    pub enquiry_property: Option<String>,
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
    pub new_property_type: String,
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
    /// A batch of photos (several files or a folder): how many, how many done, which failed.
    pub media_batch_total: usize,
    pub media_batch_done: usize,
    pub media_batch_failed: Vec<String>,
    /// The title of the batch's first photo (`VillaDelMar_7`); the rest count on from it.
    pub media_batch_first: Option<String>,
    /// The property the running batch uploads to.
    pub media_batch_property: Option<String>,
    /// The photo whose Delete was pressed once: a second press deletes it.
    pub media_confirm_delete: Option<String>,
    /// The film being uploaded: its name, and how far it has got.
    pub video_file_name: Option<String>,
    pub video_progress: Option<(f64, f64, String)>,
    /// `video` (a property film) or `short`.
    pub video_role: String,
    pub video_caption: String,
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
            new_property_type: String::new(),
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
            media_batch_total: 0,
            media_batch_done: 0,
            media_batch_failed: Vec::new(),
            media_batch_first: None,
            media_batch_property: None,
            media_confirm_delete: None,
            video_file_name: None,
            video_progress: None,
            video_role: "video".into(),
            video_caption: String::new(),
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
    /// The Kanban card being dragged (its card id), until it is dropped.
    pub dragging: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DealWorkspaceState {
    pub task_title: String,
    pub task_detail: String,
    pub task_due_at: String,
    pub offer_amounts: BTreeMap<String, String>,
    pub offer_financing: BTreeMap<String, String>,
    pub offer_deposits: BTreeMap<String, String>,
    pub offer_inspection_days: BTreeMap<String, String>,
    pub offer_seller_credits: BTreeMap<String, String>,
    pub offer_closing_dates: BTreeMap<String, String>,
    pub offer_contingencies: BTreeMap<String, String>,
    pub offer_expirations: BTreeMap<String, String>,
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

/// Where a contact form submission is. `Sent` replaces the form with the thank-you; `Failed` keeps the form and the
/// visitor's typing, and says so.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContactStatus {
    #[default]
    Idle,
    Sending,
    Sent,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContactFormState {
    /// "Buying", "Selling" or "Both" - the live form's chooser. Empty means the default, "Buying".
    pub interest: String,
    pub status: ContactStatus,
    /// One id per enquiry, reused on a retry so the intake pipeline can recognise the same submission twice.
    pub submission_id: Option<String>,
}

/// What the visitor typed, read from the form when it is submitted.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ContactSubmission {
    pub name: String,
    pub email: String,
    pub message: String,
    /// The honeypot. A person never sees it; a bot fills it.
    pub company: String,
    /// `private_viewing` or `property_information` for a property enquiry; empty for a general one.
    pub request_type: String,
    pub property_id: String,
    pub service: String,
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
