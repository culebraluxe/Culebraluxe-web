//! The reducer: the only place the model changes, and it is pure.
//!
//! Navigation is a message like everything else, which is what keeps the shell dumb: a nav click, a deep link and a
//! restored session all arrive as `Navigate` and produce the same state.

use crate::model::{
    record_for, Controls, DealCreateState, DealWorkspaceState, Effect, Model, Msg,
    PortalDealCommand, Screen, PAGE_SIZE,
};

/// THE PORTAL SCREENS THAT HAVE A REAL COMPONENT, which is the other half of the coupling `is_editorial` warns about.
///
/// A screen named here asks for the portal page DTO and is rendered by its own Yew component; a screen that is not keeps
/// asking for rows and keeps the generic list. The two lists — this one and the set of screens the portal Yew app can
/// render — must agree, and the test below pins that: a screen that asks for a payload nobody renders is a screen with
/// an empty body, which is the failure this project has already paid for once.
pub fn is_ported_portal_screen(key: &str) -> bool {
    matches!(
        key,
        "dashboard"
            | "tech"
            | "cabinet"
            | "activity"
            | "workflows"
            | "workflow-record"
            | "clients"
            | "client-record"
            | "forms"
            | "form-record"
            | "projects"
            | "deals"
            | "deal-record"
            | "design-lab"
            // Accounting V1 — the dashboard is the first of its five screens to have a component; the other four follow
            // one at a time, and each is added here and to the portal app's match together.
            | "accounting"
            | "accounting-expenses"
            | "accounting-receivables"
            | "accounting-pnl"
            | "accounting-receipt-scanner"
            | "seller-strategy"
    )
}

/// Whether a screen renders from a page payload rather than a list of rows.
///
/// This is the distinction the whole conversion turns on. A list screen answers "what rows are there" and renders them.
/// An editorial page answers "what are my blocks and cards" and lays them out — its hero has an image and an alt text,
/// its sections have eyebrows and calls to action. Asking for rows on a page like that is asking the wrong question,
/// and the answer is a page that renders as a list of strings with no design.
///
/// It is a small explicit list rather than a property of the surface, because "public" does not imply "editorial":
/// `/properties` is public and is a list.
pub fn is_editorial(key: &str) -> bool {
    // WHICH SCREENS ASK FOR A PAGE RATHER THAN ROWS. This list has to name every screen whose body renders blocks, and
    // on 2026-09-21 it did not name `site-services`: the Services page was ported, its route was wired, its payload was
    // served, and it still rendered an empty body, because opening it asked the host for ROWS. `model.page` stayed
    // `None`, the body function returned nothing, and the page was chrome over white. Nothing failed; nothing was empty
    // in a way anybody could see; the page was simply, silently, not the page.
    //
    // THE COUPLING IS THE HAZARD: a screen needs its page exactly when `custom_body` renders blocks for it, and the two
    // facts live in different files with nothing tying them together. If a screen renders blocks and is not named here,
    // it is blank; if it is named here and renders rows, it fetches a payload nobody reads. Both are invisible.
    matches!(
        key,
        "site-home"
            | "site-about"
            | "site-buyers"
            | "site-sellers"
            | "site-services"
            | "site-guide"
            | "site-contact"
            | "site-faq"
            // The property record is a page, not a list: it is a cockpit, a gallery, four tabs of documents and video, and
            // the neighbours. Serving it as rows was what flattened it into a fact table.
            | "site-property-detail"
    )
}

/// Whether a response belongs to the state the model is holding now.
///
/// THE INVARIANT: **a response issued for screen A can never mutate screen B.** The message carries the screen it was
/// fetched for and the generation it was fetched under — the two facts that were missing when a page payload for one
/// screen could land while another was mounted, which is the defect the browser showed. Arrival order is not ownership:
/// two requests can complete in either order, and a slow one is not evidence about what the visitor is looking at.
///
/// Both halves matter and neither is redundant. The screen stops a request for another screen, which is the common case
/// (click away while a page is loading); the generation stops a request for THIS screen that a previous mount issued —
/// navigating back to a screen the visitor just left, where the screen key matches but the mount does not.
fn owns(model: &Model, screen: &str, generation: u64) -> bool {
    model.screen.key == screen && model.generation == generation
}

fn client_effect(model: &Model) -> Effect {
    Effect::FetchClients {
        screen: model.screen.key,
        scope: model.scope.clone(),
        selected: model.selected_row_id.clone(),
        search: model.controls.query.clone(),
        page: model.controls.page,
        generation: model.generation,
    }
}

fn project_in_domain(
    project: &crate::model::PortalProject,
    items: &[crate::model::PortalProjectWorkItem],
    domain: &str,
) -> bool {
    let project_items = items
        .iter()
        .filter(|item| item.project_id.as_deref() == Some(project.id.as_str()));
    match domain {
        "properties" => {
            project.property_id.is_some()
                || project.areas.iter().any(|area| area == "properties" || area == "media")
                || project_items
                    .clone()
                    .any(|item| item.entity.as_ref().is_some_and(|entity| entity.entity_type == "property"))
        }
        "people" => {
            project.person_id.is_some()
                || project.areas.iter().any(|area| area == "clients")
                || project_items
                    .clone()
                    .any(|item| item.entity.as_ref().is_some_and(|entity| entity.entity_type == "person"))
        }
        "deals" => {
            project.contract_id.is_some()
                || project.areas.iter().any(|area| area == "contracts")
                || project_items.clone().any(|item| {
                    item.entity.as_ref().is_some_and(|entity| {
                        matches!(entity.entity_type.as_str(), "contract" | "deal")
                    })
                })
        }
        "marketing" => project.areas.iter().any(|area| area == "marketing"),
        "accounting" => project.areas.iter().any(|area| area == "accounting"),
        "firm" => {
            project.areas.iter().any(|area| area == "management")
                || (project.person_id.is_none()
                    && project.property_id.is_none()
                    && project.contract_id.is_none())
        }
        _ => false,
    }
}

fn initial_project_domain(projects: &crate::model::PortalProjectsPage) -> String {
    const DOMAINS: &[&str] = &[
        "properties",
        "people",
        "deals",
        "firm",
        "marketing",
        "accounting",
    ];
    DOMAINS
        .iter()
        .find(|domain| {
            projects
                .projects
                .iter()
                .any(|project| project_in_domain(project, &projects.items, domain))
        })
        .copied()
        .unwrap_or("properties")
        .to_string()
}

fn first_project_for_domain(
    projects: &crate::model::PortalProjectsPage,
    domain: &str,
) -> Option<String> {
    projects
        .projects
        .iter()
        .find(|project| project_in_domain(project, &projects.items, domain))
        .or_else(|| projects.projects.first())
        .map(|project| project.id.clone())
}

fn first_node_for_project(
    projects: &crate::model::PortalProjectsPage,
    project_id: Option<&str>,
) -> Option<String> {
    let project_id = project_id?;
    projects
        .items
        .iter()
        .filter(|item| item.project_id.as_deref() == Some(project_id))
        .find(|item| matches!(item.status.as_str(), "doing" | "open"))
        .or_else(|| {
            projects
                .items
                .iter()
                .find(|item| item.project_id.as_deref() == Some(project_id))
        })
        .map(|item| item.id.clone())
}

fn deal_workspace_command(
    model: &mut Model,
    busy_action: impl Into<String>,
    command: PortalDealCommand,
) -> Vec<Effect> {
    if model.screen.key != "deal-record" || model.deal_workspace.busy_action.is_some() {
        return Vec::new();
    }
    let Some(deal_id) = model.scope.clone() else {
        model.error = Some("This workspace is missing its deal identifier.".into());
        return Vec::new();
    };
    model.deal_workspace.busy_action = Some(busy_action.into());
    model.error = None;
    vec![Effect::RunDealWorkspaceCommand {
        screen: model.screen.key,
        generation: model.generation,
        deal_id,
        command,
    }]
}

fn deal_workspace_people_search(
    model: &mut Model,
    purpose: &str,
    query: String,
) -> Vec<Effect> {
    if model.screen.key != "deal-record" {
        return Vec::new();
    }
    let query = query.trim().to_string();
    match purpose {
        "participant" => {
            model.deal_workspace.participant_people.clear();
            if query.len() < 2 {
                model.deal_workspace.participant_searching = false;
                return Vec::new();
            }
            model.deal_workspace.participant_searching = true;
        }
        "structural" => {
            model.deal_workspace.structural_people.clear();
            if query.len() < 2 {
                model.deal_workspace.structural_searching = false;
                return Vec::new();
            }
            model.deal_workspace.structural_searching = true;
        }
        _ => return Vec::new(),
    }
    vec![Effect::SearchDealWorkspacePeople {
        screen: model.screen.key,
        generation: model.generation,
        purpose: purpose.to_string(),
        query,
    }]
}

/// Move to a screen and ask for its rows. The single place a screen change happens, so navigation and record-opening
/// cannot drift apart.
fn open(model: &mut Model, screen: Screen, scope: Option<String>) -> Vec<Effect> {
    model.screen = screen;
    model.scope = scope;
    // A deferred screen loads nothing: it is a placeholder, and pretending to fetch would put a spinner on a screen
    // that has no data to show.
    model.loading = !screen.is_deferred();
    model.error = None;
    // Rows belong to the screen that fetched them. Clearing on navigate is what stops a detail screen from briefly
    // rendering the previous screen's records.
    model.rows = Vec::new();
    model.selected_row_id = None;
    // The previous screen's blocks go with its rows: a page that has not loaded must not show the last one's hero.
    model.page = None;
    // Controls are the screen's own input and live their own life: a filter typed on Clients must not follow the user
    // to Deals and silently narrow a list they never filtered.
    model.controls = Controls::default();
    model.deal_create = DealCreateState::default();
    model.tech = crate::model::TechCockpitState::default();
    model.deal_workspace = DealWorkspaceState::default();

    // Local screens own deterministic browser-only state and do not ask the server for a payload.
    // Seller Strategy resets its calculator model; UI Lab owns its comparison/demo model inside its Yew component.
    if matches!(screen.key, "seller-strategy" | "design-lab") {
        if screen.key == "seller-strategy" {
            model.seller_strategy = crate::seller_strategy::SellerStrategyState::default();
        }
        model.loading = false;
        return Vec::new();
    }

    if model.loading {
        // A page asks for its blocks; a list asks for its rows. Two questions, two payloads, and the screen decides
        // which one it is asking — see `is_editorial`.
        //
        // EVERY EFFECT CARRIES THE GENERATION IT WAS ASKED UNDER, so the answer can be matched to the question. See
        // `Model::generation`: without it, a request issued for one screen can land while another is mounted.
        // A screen that has a real component asks for its DTO; every other portal screen still asks for rows, so the two
        // live side by side while the port goes screen by screen. See `is_ported_portal_screen`.
        if screen.key == "tech" {
            vec![Effect::FetchTech {
                screen: screen.key,
                selected: model.selected_row_id.clone(),
                generation: model.generation,
            }]
        } else if screen.key == "dashboard" {
            vec![Effect::FetchCockpit {
                screen: screen.key,
                generation: model.generation,
            }]
        } else if screen.key == "cabinet" {
            vec![Effect::FetchCabinet {
                screen: screen.key,
                generation: model.generation,
            }]
        } else if matches!(screen.key, "clients" | "client-record") {
            vec![client_effect(model)]
        } else if matches!(screen.key, "forms" | "form-record") {
            vec![Effect::FetchForms {
                screen: screen.key,
                scope: model.scope.clone(),
                generation: model.generation,
            }]
        } else if matches!(screen.key, "deals" | "deal-record") {
            vec![Effect::FetchDeals {
                screen: screen.key,
                scope: model.scope.clone(),
                generation: model.generation,
            }]
        } else if screen.key == "projects" {
            vec![Effect::FetchProjects {
                screen: screen.key,
                generation: model.generation,
            }]
        } else if screen.key == "accounting-pnl" {
            // The P&L is asked for a PERIOD, so its effect carries one: the draft the reducer holds, or empty on a first
            // open, which the bridge reads as "the current month" — the period the live page projected.
            vec![Effect::FetchAccountingPnl {
                screen: screen.key,
                generation: model.generation,
                from: model.accounting.pnl_from.clone(),
                to: model.accounting.pnl_to.clone(),
            }]
        } else if is_ported_portal_screen(screen.key) {
            vec![Effect::FetchPortal {
                screen: screen.key,
                scope: model.scope.clone(),
                generation: model.generation,
            }]
        } else if is_editorial(screen.key) {
            vec![Effect::FetchPage {
                screen: screen.key,
                scope: model.scope.clone(),
                generation: model.generation,
            }]
        } else {
            vec![Effect::FetchRows {
                screen: screen.key,
                scope: model.scope.clone(),
                generation: model.generation,
            }]
        }
    } else {
        Vec::new()
    }
}

/// The demonstration receipts, in the order the live component cycled them.
///
/// FIVE FIELDS EACH: vendor, amount, category, memo, and a date used only if the book's day is somehow unknown. The amounts
/// are the digits the seed carries, formatted the way the live component displayed them — `412.5`, `89`, `1200`, `235.75` —
/// and they go through Rust's validation like any other amount an operator types.
const SCANNER_SEEDS: [(&str, &str, &str, &str, &str); 4] = [
    (
        "Metro Maintenance Co.",
        "412.5",
        "Property / Deal Expense",
        "Walkthrough cleanup — demo receipt",
        "2026-01-01",
    ),
    (
        "Wells Fargo Merchant Services",
        "89",
        "Merchant / Bank Fees",
        "Monthly processing — demo receipt",
        "2026-01-01",
    ),
    (
        "State Insurance Group",
        "1200",
        "Insurance",
        "E&O premium — demo receipt",
        "2026-01-01",
    ),
    (
        "Luxe Signage & Print",
        "235.75",
        "Marketing & Advertising",
        "Listing collateral — demo receipt",
        "2026-01-01",
    ),
];

/// The nth demonstration receipt. The cycle is the point: pressing Scan again shows the next one, as the live component did.
fn scanner_seed(index: usize) -> (&'static str, &'static str, &'static str, &'static str, &'static str) {
    SCANNER_SEEDS[index % SCANNER_SEEDS.len()]
}

/// The database's idea of today, as the payload carries it.
///
/// The screen's date fields default to it rather than to a date the browser works out: an operator an hour from the server
/// is on a different day than the book, and a record dated by the browser would be filed on the wrong side of midnight.
pub(crate) fn accounting_today(model: &Model) -> String {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.accounting.as_ref())
        .map(|accounting| accounting.today.clone())
        .unwrap_or_default()
}

fn project_work_change<F>(model: &mut Model, change: F) -> Vec<Effect>
where
    F: FnOnce(&mut crate::model::PortalProjectWorkItem),
{
    let Some(projects) = model
        .page
        .as_mut()
        .and_then(|page| page.portal.as_mut())
        .and_then(|portal| portal.projects.as_mut())
    else {
        return Vec::new();
    };
    let Some(node_id) = projects.selected_node_id.as_deref() else {
        return Vec::new();
    };
    let Some(item) = projects.items.iter_mut().find(|item| item.id == node_id) else {
        return Vec::new();
    };
    change(item);
    projects.work_dirty = true;
    model.error = None;
    Vec::new()
}

/// Apply one intent. Returns the effects the host must run.
pub fn update(model: &mut Model, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::ScreenOpened(screen) => open(model, screen, None),
        Msg::Mount { screen, generation } => {
            // The generation is stamped BEFORE the screen opens, because opening is what asks for the data and the
            // request has to carry the number it was asked under.
            model.generation = generation;
            open(model, screen, None)
        }
        Msg::MountScoped {
            screen,
            scope,
            generation,
        } => {
            model.generation = generation;
            open(model, screen, scope)
        }
        Msg::Navigate(screen) => {
            // Already there, and not deep inside a record: nothing to do. Coming *back* from a record with the same
            // screen needs the scope cleared, which is what the second half of the condition allows.
            if model.screen == screen && model.scope.is_none() {
                return Vec::new();
            }
            open(model, screen, None)
        }
        Msg::RecordOpened(id) => match record_for(model.screen.key) {
            Some(detail) => open(model, detail, Some(id)),
            // No detail screen: the same click means "select this one". An id that is not in the list is refused
            // rather than half-applied.
            None => {
                if model.rows.iter().any(|row| row.id == id) {
                    model.selected_row_id = Some(id);
                }
                Vec::new()
            }
        },
        Msg::RowsLoaded {
            screen,
            generation,
            rows,
        } => {
            // WHOSE ANSWER IS THIS? A response the host fetched for another screen, or for a previous mount of this
            // one, is dropped rather than applied: the model belongs to the screen the visitor is looking at, and an
            // answer that arrives late is not a reason to change it. Dropped silently because it is not an error — the
            // request was correct when it was made, and the user is already somewhere else.
            if !owns(model, &screen, generation) {
                return Vec::new();
            }
            model.loading = false;
            model.error = None;
            if let Some(id) = model.selected_row_id.as_deref() {
                if !rows.iter().any(|row| row.id == id) {
                    model.selected_row_id = None;
                }
            }
            model.rows = rows;
            Vec::new()
        }
        Msg::TechStorySelected(id) => {
            if model.screen.key != "tech" {
                return Vec::new();
            }
            model.selected_row_id = Some(id.clone());
            model.loading = true;
            model.error = None;
            vec![Effect::FetchTech {
                screen: model.screen.key,
                selected: Some(id),
                generation: model.generation,
            }]
        }
        Msg::TechRefreshRequested => {
            if model.screen.key != "tech" {
                return Vec::new();
            }
            model.loading = true;
            model.error = None;
            vec![Effect::FetchTech {
                screen: model.screen.key,
                selected: model.selected_row_id.clone(),
                generation: model.generation,
            }]
        }
        Msg::TechScheduleChanged(value) => {
            if model.screen.key == "tech" {
                model.tech.schedule_at = value;
                model.tech.notice = None;
            }
            Vec::new()
        }
        Msg::TechClearWorkbenchRequested => {
            if model.screen.key != "tech" || model.tech.busy_action.is_some() {
                return Vec::new();
            }
            model.tech.busy_action = Some("clearWorkbench".into());
            model.tech.notice = None;
            model.error = None;
            vec![Effect::TechCommand {
                screen: model.screen.key,
                generation: model.generation,
                body: serde_json::json!({ "action": "clearWorkbench" }),
            }]
        }
        Msg::TechGoodToGoRequested => {
            if model.screen.key != "tech" || model.tech.busy_action.is_some() {
                return Vec::new();
            }
            let Some(story_id) = model.selected_row_id.clone() else {
                return Vec::new();
            };
            model.tech.busy_action = Some("goodToGo".into());
            model.tech.notice = None;
            model.error = None;
            vec![Effect::TechCommand {
                screen: model.screen.key,
                generation: model.generation,
                body: serde_json::json!({ "action": "goodToGo", "storyId": story_id }),
            }]
        }
        Msg::TechScopedRunRequested(stop_after) => {
            if model.screen.key != "tech" || model.tech.busy_action.is_some() {
                return Vec::new();
            }
            if !matches!(stop_after.as_str(), "scout" | "architect" | "lead") {
                return Vec::new();
            }
            let Some(story_id) = model.selected_row_id.clone() else {
                return Vec::new();
            };
            model.tech.busy_action = Some(format!("scoped:{stop_after}"));
            model.tech.notice = None;
            model.error = None;
            vec![Effect::TechCommand {
                screen: model.screen.key,
                generation: model.generation,
                body: serde_json::json!({
                    "action": "scopedRun",
                    "storyId": story_id,
                    "stopAfter": stop_after,
                }),
            }]
        }
        Msg::TechMoveWorkbenchRequested(target) => {
            if model.screen.key != "tech" || model.tech.busy_action.is_some() {
                return Vec::new();
            }
            if !matches!(target.as_str(), "backlog" | "closed" | "next") {
                return Vec::new();
            }
            let Some(story_id) = model.selected_row_id.clone() else {
                return Vec::new();
            };
            model.tech.busy_action = Some(format!("move:{target}"));
            model.tech.notice = None;
            model.error = None;
            vec![Effect::TechCommand {
                screen: model.screen.key,
                generation: model.generation,
                body: serde_json::json!({
                    "action": "moveWorkbench",
                    "storyId": story_id,
                    "target": target,
                }),
            }]
        }
        Msg::TechLaunchFlightRequested => {
            if model.screen.key != "tech" || model.tech.busy_action.is_some() {
                return Vec::new();
            }
            model.tech.busy_action = Some("launchFlight".into());
            model.tech.notice = None;
            model.error = None;
            vec![Effect::TechCommand {
                screen: model.screen.key,
                generation: model.generation,
                body: serde_json::json!({ "action": "launchFlight" }),
            }]
        }
        Msg::TechScheduleFlightRequested { scheduled_for } => {
            if model.screen.key != "tech" || model.tech.busy_action.is_some() || scheduled_for.trim().is_empty() {
                return Vec::new();
            }
            model.tech.busy_action = Some("scheduleFlight".into());
            model.tech.notice = None;
            model.error = None;
            vec![Effect::TechCommand {
                screen: model.screen.key,
                generation: model.generation,
                body: serde_json::json!({
                    "action": "scheduleFlight",
                    "scheduledFor": scheduled_for,
                }),
            }]
        }
        Msg::TechCancelFlightRequested(batch_id) => {
            if model.screen.key != "tech" || model.tech.busy_action.is_some() || batch_id.trim().is_empty() {
                return Vec::new();
            }
            model.tech.busy_action = Some("cancelFlight".into());
            model.tech.notice = None;
            model.error = None;
            vec![Effect::TechCommand {
                screen: model.screen.key,
                generation: model.generation,
                body: serde_json::json!({
                    "action": "cancelFlight",
                    "batchId": batch_id,
                }),
            }]
        }
        Msg::TechCommandCompleted {
            screen,
            generation,
            ok,
            message,
        } => {
            if !owns(model, &screen, generation) {
                return Vec::new();
            }
            model.tech.busy_action = None;
            model.tech.notice = Some(if ok {
                crate::model::CommandNotice::success(message)
            } else {
                crate::model::CommandNotice::failure(message)
            });
            model.error = None;
            model.loading = true;
            vec![Effect::FetchTech {
                screen: model.screen.key,
                selected: model.selected_row_id.clone(),
                generation: model.generation,
            }]
        }
        Msg::RowSelected(id) => {
            if model.screen.key == "clients" {
                let valid = model
                    .page
                    .as_ref()
                    .and_then(|page| page.portal.as_ref())
                    .and_then(|portal| portal.clients.as_ref())
                    .is_some_and(|clients| clients.rows.iter().any(|row| row.id == id));
                if valid {
                    model.selected_row_id = Some(id);
                    model.loading = true;
                    model.error = None;
                    return vec![client_effect(model)];
                }
                return Vec::new();
            }

            // Selecting never writes and never fetches on generic row screens.
            if model.rows.iter().any(|row| row.id == id) {
                model.selected_row_id = Some(id);
            }
            Vec::new()
        }
        Msg::PortalLoaded {
            screen,
            generation,
            mut page,
        } => {
            // The same ownership rule as every other response: a payload for a screen the user has left cannot become
            // the payload of the screen they are on.
            if !owns(model, &screen, generation) {
                return Vec::new();
            }
            model.loading = false;
            model.error = None;
            // AN ACCOUNTING PAYLOAD CAN BE A COMMAND'S ANSWER. A successful write comes back as the refreshed screen, so
            // the form's pending state ends here and the notice is the success the live forms printed — which also means
            // the screen cannot be left saying "Creating…" after the row it created is already on it.
            if matches!(
                model.screen.key,
                "accounting-expenses" | "accounting-receivables" | "accounting"
            ) {
                if model.accounting.submitting {
                    model.accounting.submitting = false;
                    model.accounting.notice = Some(crate::model::CommandNotice::success("Created."));
                }
                // A new record's date starts on the book's day: the payload carries what the database calls today, and the
                // draft takes it only while it has none of its own — so a date the operator chose is never overwritten by
                // a later refresh.
                let today = page
                    .accounting
                    .as_ref()
                    .map(|accounting| accounting.today.clone())
                    .unwrap_or_default();
                if !today.is_empty() && model.accounting.expense_on.is_empty() {
                    model.accounting.expense_on = today.clone();
                }
                // The receivable form's two dates have the same default, and its category starts where the live form's did:
                // `COMMISSION`, uppercased, which is what the seam stores anyway.
                if !today.is_empty() && model.accounting.receivable_issued_on.is_empty() {
                    model.accounting.receivable_issued_on = today;
                }
                if model.accounting.receivable_category.is_empty() {
                    model.accounting.receivable_category = "COMMISSION".to_owned();
                }
            }
            if model.screen.key == "accounting-pnl" {
                // The P&L's fields take the period that was projected: the screen shows what it asked for, and an operator
                // who then edits one end of it edits a range they can see rather than a blank pair of inputs.
                if let Some(pnl) = page
                    .accounting
                    .as_ref()
                    .and_then(|accounting| accounting.pnl.as_ref())
                {
                    if model.accounting.pnl_from.is_empty() {
                        model.accounting.pnl_from = pnl.from.clone();
                    }
                    if model.accounting.pnl_to.is_empty() {
                        model.accounting.pnl_to = pnl.to.clone();
                    }
                }
            }
            if model.screen.key == "tech" {
                model.selected_row_id = page
                    .tech
                    .as_ref()
                    .and_then(|tech| tech.selected_story.as_ref())
                    .map(|story| story.id.clone());
            }
            if matches!(model.screen.key, "clients" | "client-record") {
                model.selected_row_id = page
                    .clients
                    .as_ref()
                    .and_then(|clients| clients.selected_id.clone());
            }
            if model.screen.key == "projects" {
                let previous = model
                    .page
                    .as_ref()
                    .and_then(|content| content.portal.as_ref())
                    .and_then(|portal| portal.projects.as_ref())
                    .map(|projects| {
                        (
                            projects.active_domain.clone(),
                            projects.selected_project_id.clone(),
                            projects.selected_node_id.clone(),
                            projects.active_view.clone(),
                            projects.catch_up,
                            projects.work_collapsed,
                        )
                    });
                if let Some(projects) = page.projects.as_mut() {
                    if let Some((
                        domain,
                        project_id,
                        node_id,
                        view,
                        catch_up,
                        work_collapsed,
                    )) = previous
                    {
                        projects.active_domain = if domain.is_empty() {
                            initial_project_domain(projects)
                        } else {
                            domain
                        };
                        projects.selected_project_id = project_id
                            .filter(|id| projects.projects.iter().any(|project| &project.id == id))
                            .or_else(|| first_project_for_domain(projects, &projects.active_domain));
                        projects.selected_node_id = node_id
                            .filter(|id| {
                                projects.items.iter().any(|item| {
                                    &item.id == id
                                        && item.project_id.as_deref()
                                            == projects.selected_project_id.as_deref()
                                })
                            })
                            .or_else(|| {
                                first_node_for_project(
                                    projects,
                                    projects.selected_project_id.as_deref(),
                                )
                            });
                        projects.active_view = if view.is_empty() {
                            "work-plan".into()
                        } else {
                            view
                        };
                        projects.catch_up = catch_up;
                        projects.work_collapsed = work_collapsed;
                    } else {
                        projects.active_domain = initial_project_domain(projects);
                        projects.selected_project_id =
                            first_project_for_domain(projects, &projects.active_domain);
                        projects.selected_node_id = first_node_for_project(
                            projects,
                            projects.selected_project_id.as_deref(),
                        );
                        projects.active_view = "work-plan".into();
                        projects.work_collapsed = false;
                    }
                    projects.work_dirty = false;
                    projects.saving = false;
                }
            }
            // The portal payload rides in `page` as `portal`: one place on the model holds "the payload this screen
            // asked for", so a screen and its data cannot be out of step.
            model.page = Some(crate::model::PageContent {
                portal: Some(page),
                ..Default::default()
            });
            Vec::new()
        }
        Msg::EffectFailed {
            screen,
            generation,
            message,
        } => {
            // THE SAME RULE AS A PAYLOAD, and it matters more here. A failure that arrived for a screen the visitor has
            // left would put an error message on the page they are on now and — worse — clear its loading state while
            // its own request is still in flight, which reads as "loaded, nothing to show".
            if !owns(model, &screen, generation) {
                return Vec::new();
            }
            model.loading = false;
            if let Some(portal) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
            {
                if let Some(forms) = portal.forms.as_mut() {
                    forms.saving = false;
                }
                if let Some(projects) = portal.projects.as_mut() {
                    projects.saving = false;
                }
            }
            if model.tech.busy_action.is_some() {
                model.tech.busy_action = None;
                model.tech.notice =
                    Some(crate::model::CommandNotice::failure(message.clone()));
            }
            model.deal_create.searching = false;
            model.deal_create.submitting = false;
            // A failed Accounting command ends the form's pending state and says why, in the service's own words — this is
            // where "Vendor is required." or the 409 for a voided receivable reaches the operator.
            if model.accounting.submitting {
                model.accounting.submitting = false;
                model.accounting.notice =
                    Some(crate::model::CommandNotice::failure(message.clone()));
            }
            model.deal_workspace.participant_searching = false;
            model.deal_workspace.structural_searching = false;
            model.deal_workspace.busy_action = None;
            model.error = Some(message);
            Vec::new()
        }
        Msg::PageLoaded {
            screen,
            generation,
            page,
        } => {
            // The same ownership rule as rows: a page fetched for another screen, or for a previous mount, cannot
            // overwrite the page on screen. This is the bug the browser showed — a payload for one screen landing while
            // another was mounted.
            if !owns(model, &screen, generation) {
                return Vec::new();
            }
            model.loading = false;
            model.error = None;
            model.page = Some(page);
            Vec::new()
        }

        // ---- Contracts / Deal workspace -------------------------------------------------------------------------
        // ---- accounting: the expense form ------------------------------------------------------------------------
        //
        // EVERY ARM IS GATED BY THE SCREEN, as the Contracts form's are: a field message that arrives while another screen
        // is mounted must not write into this screen's draft. The view only renders these inputs on the Expenses screen,
        // but a message is not obliged to come from a view.
        Msg::ExpenseFormToggled => {
            if model.screen.key != "accounting-expenses" {
                return Vec::new();
            }
            model.accounting.expense_open = !model.accounting.expense_open;
            model.accounting.notice = None;
            Vec::new()
        }
        Msg::ExpenseVendorChanged(value) => {
            if model.screen.key == "accounting-expenses" {
                model.accounting.expense_vendor = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ExpenseCategoryChanged(value) => {
            if model.screen.key == "accounting-expenses" {
                model.accounting.expense_category = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ExpenseAmountChanged(value) => {
            if model.screen.key == "accounting-expenses" {
                model.accounting.expense_amount = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ExpenseDateChanged(value) => {
            if model.screen.key == "accounting-expenses" {
                model.accounting.expense_on = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ExpenseMemoChanged(value) => {
            if model.screen.key == "accounting-expenses" {
                model.accounting.expense_memo = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ExpenseSubmitted => {
            if model.screen.key != "accounting-expenses" || model.accounting.submitting {
                return Vec::new();
            }
            // A second click while the first is in flight would write a second row, so the reducer refuses to start
            // another command until the answer to this one arrives.
            model.accounting.submitting = true;
            model.accounting.notice = None;
            model.error = None;
            vec![Effect::AccountingCommand {
                screen: "accounting-expenses",
                generation: model.generation,
                body: serde_json::json!({
                    "action": "createExpense",
                    "screen": "accounting-expenses",
                    "vendor": model.accounting.expense_vendor.clone(),
                    "category": model.accounting.expense_category.clone(),
                    "amount": model.accounting.expense_amount.clone(),
                    "expenseOn": model.accounting.expense_on.clone(),
                    "memo": model.accounting.expense_memo.clone(),
                }),
            }]
        }

        // ---- accounting: the receivable form, and mark-paid ------------------------------------------------------
        //
        // Gated by the screen for the same reason as the expense form's: a draft is written only while its own screen is
        // mounted.
        Msg::ReceivableFormToggled => {
            if model.screen.key != "accounting-receivables" {
                return Vec::new();
            }
            model.accounting.receivable_open = !model.accounting.receivable_open;
            model.accounting.notice = None;
            Vec::new()
        }
        Msg::ReceivableReferenceChanged(value) => {
            if model.screen.key == "accounting-receivables" {
                model.accounting.receivable_reference = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ReceivableDescriptionChanged(value) => {
            if model.screen.key == "accounting-receivables" {
                model.accounting.receivable_description = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ReceivableCategoryChanged(value) => {
            if model.screen.key == "accounting-receivables" {
                model.accounting.receivable_category = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ReceivableAmountChanged(value) => {
            if model.screen.key == "accounting-receivables" {
                model.accounting.receivable_amount = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ReceivableIssuedOnChanged(value) => {
            if model.screen.key == "accounting-receivables" {
                model.accounting.receivable_issued_on = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ReceivableDueOnChanged(value) => {
            if model.screen.key == "accounting-receivables" {
                model.accounting.receivable_due_on = value;
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ReceivableSubmitted => {
            if model.screen.key != "accounting-receivables" || model.accounting.submitting {
                return Vec::new();
            }
            model.accounting.submitting = true;
            model.accounting.notice = None;
            model.error = None;
            vec![Effect::AccountingCommand {
                screen: "accounting-receivables",
                generation: model.generation,
                body: serde_json::json!({
                    "action": "createReceivable",
                    "screen": "accounting-receivables",
                    "reference": model.accounting.receivable_reference.clone(),
                    "description": model.accounting.receivable_description.clone(),
                    "category": model.accounting.receivable_category.clone(),
                    "amount": model.accounting.receivable_amount.clone(),
                    "issuedOn": model.accounting.receivable_issued_on.clone(),
                    "dueOn": model.accounting.receivable_due_on.clone(),
                }),
            }]
        }
        Msg::ReceivablePaidDateChanged { id, value } => {
            if model.screen.key == "accounting-receivables" {
                // One row's date, not the table's: the live screen gave every row its own input.
                model.accounting.paid_on.insert(id, value);
            }
            Vec::new()
        }
        Msg::ReceivablePaidSubmitted { id } => {
            if model.screen.key != "accounting-receivables" || model.accounting.submitting {
                return Vec::new();
            }
            // The date is the row's own, falling back to the book's today while the operator has not changed it — the same
            // default its input shows, so what is sent is what is on screen.
            let today = accounting_today(model);
            let paid_on = model
                .accounting
                .paid_on
                .get(&id)
                .cloned()
                .unwrap_or(today);
            model.accounting.submitting = true;
            model.accounting.notice = None;
            model.error = None;
            vec![Effect::AccountingCommand {
                screen: "accounting-receivables",
                generation: model.generation,
                body: serde_json::json!({
                    "action": "markReceivablePaid",
                    "screen": "accounting-receivables",
                    "receivableId": id,
                    "paidOn": paid_on,
                }),
            }]
        }

        // ---- accounting: the P&L's period ------------------------------------------------------------------------
        Msg::PnlFromChanged(value) => {
            if model.screen.key == "accounting-pnl" {
                model.accounting.pnl_from = value;
                model.error = None;
            }
            Vec::new()
        }
        Msg::PnlToChanged(value) => {
            if model.screen.key == "accounting-pnl" {
                model.accounting.pnl_to = value;
                model.error = None;
            }
            Vec::new()
        }
        Msg::PnlApplied => {
            if model.screen.key != "accounting-pnl" {
                return Vec::new();
            }
            // The screen is loading again, and the range is whatever the two fields hold — the same strings the operator
            // can see. Whether they are a valid period is Rust's answer to give.
            model.loading = true;
            model.error = None;
            vec![Effect::FetchAccountingPnl {
                screen: "accounting-pnl",
                generation: model.generation,
                from: model.accounting.pnl_from.clone(),
                to: model.accounting.pnl_to.clone(),
            }]
        }

        // ---- accounting: the receipt scanner's demonstration -----------------------------------------------------
        Msg::ScannerDragging(dragging) => {
            if model.screen.key == "accounting-receipt-scanner" {
                model.accounting.scanner.dragging = dragging;
            }
            Vec::new()
        }
        Msg::ScannerFileChosen(name) => {
            if model.screen.key == "accounting-receipt-scanner" {
                model.accounting.scanner.file_name = name;
            }
            Vec::new()
        }
        Msg::ScannerScanned => {
            if model.screen.key != "accounting-receipt-scanner" {
                return Vec::new();
            }
            // THE DEMONSTRATION'S EXTRACTION, deterministic and cyclical: this is the same seed sequence the live component
            // cycled, and it is deliberately not OCR. The date is the book's, like every other date on these screens.
            let seed = scanner_seed(model.accounting.scanner.demo_index);
            let today = accounting_today(model);
            model.accounting.scanner.demo_index += 1;
            if model.accounting.scanner.file_name.is_empty() {
                model.accounting.scanner.file_name = "demo-receipt.jpg".to_owned();
            }
            model.accounting.scanner.draft = Some(crate::model::ScannerDraft {
                vendor: seed.0.to_owned(),
                amount: seed.1.to_owned(),
                category: seed.2.to_owned(),
                memo: seed.3.to_owned(),
                expense_on: if today.is_empty() {
                    seed.4.to_owned()
                } else {
                    today
                },
            });
            Vec::new()
        }
        Msg::ScannerCategoryChanged(value) => {
            if model.screen.key == "accounting-receipt-scanner" {
                if let Some(draft) = model.accounting.scanner.draft.as_mut() {
                    draft.category = value;
                }
                model.accounting.notice = None;
            }
            Vec::new()
        }
        Msg::ScannerSubmitted => {
            if model.screen.key != "accounting-receipt-scanner" || model.accounting.submitting {
                return Vec::new();
            }
            let Some(draft) = model.accounting.scanner.draft.clone() else {
                // Nothing reviewed, nothing to save: the button is not rendered without a draft, and a message that
                // arrived anyway must not record an expense made of empty strings.
                return Vec::new();
            };
            model.accounting.submitting = true;
            model.accounting.notice = None;
            model.error = None;
            vec![Effect::AccountingCommand {
                screen: "accounting-receipt-scanner",
                generation: model.generation,
                body: serde_json::json!({
                    "action": "createExpense",
                    "screen": "accounting-receipt-scanner",
                    "vendor": draft.vendor,
                    "category": draft.category,
                    "amount": draft.amount,
                    "expenseOn": draft.expense_on,
                    // THE MEMO IS THE RECEIPT'S NOTE, AND IT IS SAVED. The live form displayed the seed's memo beside the
                    // button and never submitted it, so the recorded expense had none — the one field the review step shows
                    // and the one thing the row could not explain later. It is the note about the receipt, and the review
                    // step is where it is confirmed, so it travels with the rest of the draft. Nothing else about the
                    // demonstration changes: the seeds, the cycle and the extraction are as they were.
                    "memo": draft.memo,
                }),
            }]
        }

        Msg::DealCreateToggled => {
            if model.screen.key != "deals" {
                return Vec::new();
            }
            model.deal_create.open = !model.deal_create.open;
            model.error = None;
            Vec::new()
        }
        Msg::DealCreatePropertyChanged(value) => {
            if model.screen.key == "deals" {
                model.deal_create.property_id = value;
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealCreateClientQueryChanged(value) => {
            if model.screen.key != "deals" {
                return Vec::new();
            }
            if value != model.deal_create.client_label {
                model.deal_create.client_person_id.clear();
                model.deal_create.client_label.clear();
            }
            model.deal_create.client_query = value;
            let query = model.deal_create.client_query.trim().to_string();
            model.deal_create.people.clear();
            model.error = None;
            if query.len() < 2 {
                model.deal_create.searching = false;
                return Vec::new();
            }
            model.deal_create.searching = true;
            vec![Effect::SearchDealPeople {
                screen: model.screen.key,
                generation: model.generation,
                query,
            }]
        }
        Msg::DealCreateClientSelected { id, label } => {
            if model.screen.key != "deals" {
                return Vec::new();
            }
            model.deal_create.client_person_id = id;
            model.deal_create.client_label = label.clone();
            model.deal_create.client_query = label;
            model.deal_create.people.clear();
            model.deal_create.searching = false;
            model.error = None;
            Vec::new()
        }
        Msg::DealCreateOwnerChanged(value) => {
            if model.screen.key == "deals" {
                model.deal_create.owner_user_id = value;
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealCreateNotesChanged(value) => {
            if model.screen.key == "deals" {
                model.deal_create.notes = value;
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealPeopleLoaded {
            screen,
            generation,
            query,
            people,
        } => {
            if !owns(model, &screen, generation) || model.screen.key != "deals" {
                return Vec::new();
            }
            if model.deal_create.client_query.trim() != query {
                return Vec::new();
            }
            model.deal_create.people = people;
            model.deal_create.searching = false;
            Vec::new()
        }
        Msg::DealCreateRequested => {
            if model.screen.key != "deals" || model.deal_create.submitting {
                return Vec::new();
            }
            let property_id = model.deal_create.property_id.trim().to_string();
            let client_person_id = model.deal_create.client_person_id.trim().to_string();
            if property_id.is_empty() {
                model.error = Some("Choose a property first.".into());
                return Vec::new();
            }
            if client_person_id.is_empty() {
                model.error = Some("Select an existing client person first.".into());
                return Vec::new();
            }
            let owner_user_id = (!model.deal_create.owner_user_id.trim().is_empty())
                .then(|| model.deal_create.owner_user_id.trim().to_string());
            let notes = (!model.deal_create.notes.trim().is_empty())
                .then(|| model.deal_create.notes.trim().to_string());
            model.deal_create.submitting = true;
            model.error = None;
            vec![Effect::CreateDeal {
                screen: model.screen.key,
                generation: model.generation,
                property_id,
                client_person_id,
                owner_user_id,
                notes,
            }]
        }
        Msg::DealCreated {
            screen,
            generation,
            id,
        } => {
            if !owns(model, &screen, generation) {
                return Vec::new();
            }
            model.deal_create.submitting = false;
            model.error = None;
            vec![Effect::BrowserNavigate {
                href: format!("/portal/deals/{id}"),
            }]
        }

        Msg::DealWorkspaceTaskTitleChanged(value) => {
            if model.screen.key == "deal-record" {
                model.deal_workspace.task_title = value;
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealWorkspaceTaskDetailChanged(value) => {
            if model.screen.key == "deal-record" {
                model.deal_workspace.task_detail = value;
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealWorkspaceTaskDueChanged(value) => {
            if model.screen.key == "deal-record" {
                model.deal_workspace.task_due_at = value;
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealWorkspaceCreateTaskRequested => {
            let title = model.deal_workspace.task_title.trim().to_string();
            if title.is_empty() {
                model.error = Some("Task title is required.".into());
                return Vec::new();
            }
            let detail = (!model.deal_workspace.task_detail.trim().is_empty())
                .then(|| model.deal_workspace.task_detail.trim().to_string());
            let due_at = (!model.deal_workspace.task_due_at.trim().is_empty())
                .then(|| model.deal_workspace.task_due_at.trim().to_string());
            deal_workspace_command(
                model,
                "task:create",
                PortalDealCommand::CreateTask {
                    title,
                    detail,
                    due_at,
                },
            )
        }
        Msg::DealWorkspaceCompleteTaskRequested { task_id } => deal_workspace_command(
            model,
            format!("task:complete:{task_id}"),
            PortalDealCommand::CompleteTask { task_id },
        ),
        Msg::DealWorkspaceOfferAmountChanged { key, value } => {
            if model.screen.key == "deal-record" {
                if value.is_empty() {
                    model.deal_workspace.offer_amounts.remove(&key);
                } else {
                    model.deal_workspace.offer_amounts.insert(key, value);
                }
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealWorkspaceSubmitOfferRequested { parent_offer_id } => {
            let key = parent_offer_id.as_deref().unwrap_or("root").to_string();
            let amount = model
                .deal_workspace
                .offer_amounts
                .get(&key)
                .map(|value| value.trim().to_string())
                .unwrap_or_default();
            if amount.is_empty() {
                model.error = Some("Offer amount is required.".into());
                return Vec::new();
            }
            let client_id = model
                .page
                .as_ref()
                .and_then(|page| page.portal.as_ref())
                .and_then(|portal| portal.deals.as_ref())
                .and_then(|deals| deals.workspace.as_ref())
                .and_then(|workspace| workspace.client.as_ref())
                .map(|client| client.id.clone());
            let Some(person_id) = client_id else {
                model.error = Some("This deal does not have an active client.".into());
                return Vec::new();
            };
            deal_workspace_command(
                model,
                format!("offer:submit:{key}"),
                PortalDealCommand::SubmitOffer {
                    person_id,
                    amount,
                    parent_offer_id,
                },
            )
        }
        Msg::DealWorkspaceWithdrawOfferRequested { offer_id } => deal_workspace_command(
            model,
            format!("offer:withdraw:{offer_id}"),
            PortalDealCommand::WithdrawOffer { offer_id },
        ),
        Msg::DealWorkspaceRejectOfferRequested { offer_id } => deal_workspace_command(
            model,
            format!("offer:reject:{offer_id}"),
            PortalDealCommand::RejectOffer { offer_id },
        ),
        Msg::DealWorkspaceCreateShowingRequested => {
            let workspace = model
                .page
                .as_ref()
                .and_then(|page| page.portal.as_ref())
                .and_then(|portal| portal.deals.as_ref())
                .and_then(|deals| deals.workspace.as_ref());
            let person_id = workspace
                .and_then(|workspace| workspace.client.as_ref())
                .map(|client| client.id.clone());
            let property_id = workspace
                .and_then(|workspace| workspace.property.as_ref())
                .map(|property| property.id.clone());
            let Some(person_id) = person_id else {
                model.error = Some("This deal does not have an active client.".into());
                return Vec::new();
            };
            deal_workspace_command(
                model,
                "showing:create",
                PortalDealCommand::CreateShowing {
                    person_id,
                    property_id,
                },
            )
        }
        Msg::DealWorkspaceShowingTimeChanged { showing_id, value } => {
            if model.screen.key == "deal-record" {
                if value.is_empty() {
                    model.deal_workspace.showing_times.remove(&showing_id);
                } else {
                    model.deal_workspace.showing_times.insert(showing_id, value);
                }
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealWorkspaceScheduleShowingRequested { showing_id } => {
            let scheduled_at = model
                .deal_workspace
                .showing_times
                .get(&showing_id)
                .map(|value| value.trim().to_string())
                .unwrap_or_default();
            if scheduled_at.is_empty() {
                model.error = Some("Choose a showing date and time first.".into());
                return Vec::new();
            }
            deal_workspace_command(
                model,
                format!("showing:schedule:{showing_id}"),
                PortalDealCommand::ScheduleShowing {
                    showing_id,
                    scheduled_at,
                },
            )
        }
        Msg::DealWorkspaceCancelShowingRequested { showing_id } => deal_workspace_command(
            model,
            format!("showing:cancel:{showing_id}"),
            PortalDealCommand::CancelShowing { showing_id },
        ),
        Msg::DealWorkspaceCompleteShowingRequested { showing_id } => deal_workspace_command(
            model,
            format!("showing:complete:{showing_id}"),
            PortalDealCommand::CompleteShowing { showing_id },
        ),
        Msg::DealWorkspaceParticipantQueryChanged(value) => {
            if model.screen.key != "deal-record" {
                return Vec::new();
            }
            if value != model.deal_workspace.participant_label {
                model.deal_workspace.participant_person_id.clear();
                model.deal_workspace.participant_label.clear();
            }
            model.deal_workspace.participant_query = value.clone();
            model.error = None;
            deal_workspace_people_search(model, "participant", value)
        }
        Msg::DealWorkspaceParticipantSelected { id, label } => {
            if model.screen.key == "deal-record" {
                model.deal_workspace.participant_person_id = id;
                model.deal_workspace.participant_label = label.clone();
                model.deal_workspace.participant_query = label;
                model.deal_workspace.participant_people.clear();
                model.deal_workspace.participant_searching = false;
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealWorkspaceParticipantRoleChanged(value) => {
            if model.screen.key == "deal-record" {
                model.deal_workspace.participant_role_label = value;
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealWorkspaceAddParticipantRequested => {
            let person_id = model.deal_workspace.participant_person_id.trim().to_string();
            let role_label = model.deal_workspace.participant_role_label.trim().to_string();
            if person_id.is_empty() {
                model.error = Some("Select an existing person first.".into());
                return Vec::new();
            }
            if role_label.is_empty() {
                model.error = Some("Enter a participant role label.".into());
                return Vec::new();
            }
            deal_workspace_command(
                model,
                "participant:add",
                PortalDealCommand::AddOtherParticipant {
                    person_id,
                    role_label,
                },
            )
        }
        Msg::DealWorkspaceOtherRoleChanged {
            participant_id,
            value,
        } => {
            if model.screen.key == "deal-record" {
                if value.is_empty() {
                    model.deal_workspace.other_role_labels.remove(&participant_id);
                } else {
                    model
                        .deal_workspace
                        .other_role_labels
                        .insert(participant_id, value);
                }
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealWorkspaceUpdateOtherRequested { participant_id } => {
            let role_label = model
                .deal_workspace
                .other_role_labels
                .get(&participant_id)
                .map(|value| value.trim().to_string())
                .unwrap_or_default();
            if role_label.is_empty() {
                model.error = Some("Enter a new role label.".into());
                return Vec::new();
            }
            deal_workspace_command(
                model,
                format!("participant:update:{participant_id}"),
                PortalDealCommand::UpdateOtherParticipant {
                    participant_id,
                    role_label,
                },
            )
        }
        Msg::DealWorkspaceEndOtherRequested { participant_id } => deal_workspace_command(
            model,
            format!("participant:end:{participant_id}"),
            PortalDealCommand::EndOtherParticipant { participant_id },
        ),
        Msg::DealWorkspaceStructuralRoleChanged(role) => {
            if model.screen.key != "deal-record" {
                return Vec::new();
            }
            model.deal_workspace.structural_role = role;
            model.deal_workspace.structural_query.clear();
            model.deal_workspace.structural_person_id.clear();
            model.deal_workspace.structural_label.clear();
            model.deal_workspace.structural_owner_user_id.clear();
            model.deal_workspace.structural_people.clear();
            model.deal_workspace.structural_searching = false;
            model.error = None;
            Vec::new()
        }
        Msg::DealWorkspaceStructuralQueryChanged(value) => {
            if model.screen.key != "deal-record" {
                return Vec::new();
            }
            if value != model.deal_workspace.structural_label {
                model.deal_workspace.structural_person_id.clear();
                model.deal_workspace.structural_label.clear();
            }
            model.deal_workspace.structural_query = value.clone();
            model.error = None;
            deal_workspace_people_search(model, "structural", value)
        }
        Msg::DealWorkspaceStructuralPersonSelected { id, label } => {
            if model.screen.key == "deal-record" {
                model.deal_workspace.structural_person_id = id;
                model.deal_workspace.structural_label = label.clone();
                model.deal_workspace.structural_query = label;
                model.deal_workspace.structural_people.clear();
                model.deal_workspace.structural_searching = false;
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealWorkspaceStructuralOwnerChanged(value) => {
            if model.screen.key == "deal-record" {
                model.deal_workspace.structural_owner_user_id = value;
                model.error = None;
            }
            Vec::new()
        }
        Msg::DealWorkspaceSetStructuralRequested => {
            let role = model.deal_workspace.structural_role.clone();
            if !matches!(role.as_str(), "client" | "owner" | "seller") {
                model.error = Some("Choose client, owner, or seller first.".into());
                return Vec::new();
            }
            let (person_id, user_id) = if role == "owner" {
                let id = model
                    .deal_workspace
                    .structural_owner_user_id
                    .trim()
                    .to_string();
                if id.is_empty() {
                    model.error = Some("Choose an owner user first.".into());
                    return Vec::new();
                }
                (None, Some(id))
            } else {
                let id = model.deal_workspace.structural_person_id.trim().to_string();
                if id.is_empty() {
                    model.error = Some("Select an existing person first.".into());
                    return Vec::new();
                }
                (Some(id), None)
            };
            deal_workspace_command(
                model,
                format!("structural:set:{role}"),
                PortalDealCommand::SetStructuralParticipant {
                    role,
                    person_id,
                    user_id,
                },
            )
        }
        Msg::DealWorkspaceEndStructuralRequested { participant_id } => deal_workspace_command(
            model,
            format!("structural:end:{participant_id}"),
            PortalDealCommand::EndStructuralParticipant { participant_id },
        ),
        Msg::DealWorkspacePeopleLoaded {
            screen,
            generation,
            purpose,
            query,
            people,
        } => {
            if !owns(model, &screen, generation) || model.screen.key != "deal-record" {
                return Vec::new();
            }
            match purpose.as_str() {
                "participant"
                    if model.deal_workspace.participant_query.trim() == query =>
                {
                    model.deal_workspace.participant_people = people;
                    model.deal_workspace.participant_searching = false;
                }
                "structural"
                    if model.deal_workspace.structural_query.trim() == query =>
                {
                    model.deal_workspace.structural_people = people;
                    model.deal_workspace.structural_searching = false;
                }
                _ => {}
            }
            Vec::new()
        }
        Msg::DealWorkspaceCommandCompleted {
            screen,
            generation,
            id: _,
        } => {
            if !owns(model, &screen, generation) || model.screen.key != "deal-record" {
                return Vec::new();
            }
            let completed = model.deal_workspace.busy_action.take().unwrap_or_default();
            if completed == "task:create" {
                model.deal_workspace.task_title.clear();
                model.deal_workspace.task_detail.clear();
                model.deal_workspace.task_due_at.clear();
            } else if let Some(key) = completed.strip_prefix("offer:submit:") {
                model.deal_workspace.offer_amounts.remove(key);
            } else if completed == "participant:add" {
                model.deal_workspace.participant_query.clear();
                model.deal_workspace.participant_person_id.clear();
                model.deal_workspace.participant_label.clear();
                model.deal_workspace.participant_role_label.clear();
                model.deal_workspace.participant_people.clear();
            } else if completed.starts_with("structural:set:") {
                model.deal_workspace.structural_role.clear();
                model.deal_workspace.structural_query.clear();
                model.deal_workspace.structural_person_id.clear();
                model.deal_workspace.structural_label.clear();
                model.deal_workspace.structural_owner_user_id.clear();
                model.deal_workspace.structural_people.clear();
            }
            model.loading = true;
            model.error = None;
            vec![Effect::FetchDeals {
                screen: model.screen.key,
                scope: model.scope.clone(),
                generation: model.generation,
            }]
        }

        // ---- controls -------------------------------------------------------------------------------------------
        // None of these fetch. Filtering is applied to the rows already in the model, in the view, so it cannot be
        // mistaken for a server-side search that is not wired yet. When a filter does become a server round trip it
        // gains an effect here, and the host learns about it from the effect rather than from the message.
        Msg::QueryChanged(query) => {
            model.controls.query = query;
            // Page 4 of an unfiltered list means nothing once the list is not that list any more.
            model.controls.page = 0;
            if model.screen.key == "clients" {
                model.selected_row_id = None;
                model.loading = true;
                model.error = None;
                return vec![client_effect(model)];
            }
            Vec::new()
        }
        Msg::FilterChanged(filter) => {
            model.controls.filter = Some(filter);
            model.controls.page = 0;
            Vec::new()
        }
        Msg::TabSelected(tab) => {
            model.controls.tab = Some(tab);
            model.controls.page = 0;
            Vec::new()
        }
        Msg::FilterSelected { key, value } => {
            // An empty value is the "no filter" option, so it REMOVES the entry: "unset" is one state and not two, and
            // the view asks the same question of either.
            if value.is_empty() {
                model.controls.named.remove(&key);
            } else {
                model.controls.named.insert(key, value);
            }
            model.controls.page = 0;
            Vec::new()
        }
        Msg::Toggled(on) => {
            model.controls.toggled = on;
            Vec::new()
        }
        Msg::PageChanged(delta) => {
            if model.screen.key == "clients" {
                let pages = model
                    .page
                    .as_ref()
                    .and_then(|page| page.portal.as_ref())
                    .and_then(|portal| portal.clients.as_ref())
                    .map(|clients| {
                        let size = clients.page_size.max(1);
                        ((clients.total + size - 1) / size).max(1)
                    })
                    .unwrap_or(1);
                let next = (model.controls.page as i64)
                    .saturating_add(delta)
                    .clamp(0, pages - 1);
                model.controls.page = next as usize;
                model.selected_row_id = None;
                model.loading = true;
                model.error = None;
                return vec![client_effect(model)];
            }

            // The bounds live here rather than in the buttons, so a list that shrank while the user was reading it
            // cannot leave them on a page that no longer exists.
            let next = (model.controls.page as i64).saturating_add(delta).max(0);
            let pages = model.rows.len().div_ceil(PAGE_SIZE);
            model.controls.page = if pages == 0 {
                // No rows in the model means the screen renders its body from somewhere the reducer cannot see (the
                // Rust design lab builds its own list). It refuses to go below the first page and the body clamps the
                // rest, rather than inventing a last page it has no count for.
                next as usize
            } else {
                next.min(pages as i64 - 1) as usize
            };
            Vec::new()
        }
        Msg::SellerStrategyFieldChanged { key, raw, percent } => {
            if model.screen.key != "seller-strategy" {
                return Vec::new();
            }
            model.seller_strategy.set_field(&key, &raw, percent);
            model.error = None;
            Vec::new()
        }
        Msg::SellerStrategyOptionToggled { option, enabled } => {
            if model.screen.key != "seller-strategy" {
                return Vec::new();
            }
            model.seller_strategy.set_option(option, enabled);
            model.error = None;
            Vec::new()
        }
        Msg::SellerStrategyEditAllToggled => {
            if model.screen.key == "seller-strategy" {
                model.seller_strategy.edit_all = !model.seller_strategy.edit_all;
            }
            Vec::new()
        }
        Msg::SellerStrategyActiveEditChanged(option) => {
            if model.screen.key == "seller-strategy" {
                model.seller_strategy.active_edit = option;
            }
            Vec::new()
        }
        Msg::SellerStrategyDetailToggled => {
            if model.screen.key == "seller-strategy" {
                model.seller_strategy.show_detail = !model.seller_strategy.show_detail;
            }
            Vec::new()
        }
        Msg::SellerStrategyReset => {
            if model.screen.key == "seller-strategy" {
                model.seller_strategy.reset();
                model.error = None;
            }
            Vec::new()
        }

        Msg::FormFieldChanged { name, value } => {
            let Some(forms) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.forms.as_mut())
            else {
                return Vec::new();
            };
            let Some(form) = forms.selected.as_mut() else {
                return Vec::new();
            };
            form.field_values.insert(name, value);
            forms.dirty = true;
            model.error = None;
            Vec::new()
        }
        Msg::FormSectionChanged { name, value } => {
            let Some(forms) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.forms.as_mut())
            else {
                return Vec::new();
            };
            let Some(form) = forms.selected.as_mut() else {
                return Vec::new();
            };
            form.sections.insert(name, value);
            forms.dirty = true;
            model.error = None;
            Vec::new()
        }
        Msg::FormSaveRequested => {
            let Some(forms) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.forms.as_mut())
            else {
                return Vec::new();
            };
            if forms.saving || !forms.dirty {
                return Vec::new();
            }
            let Some(form) = forms.selected.as_ref() else {
                return Vec::new();
            };
            let effect = Effect::SaveForm {
                screen: model.screen.key,
                generation: model.generation,
                form_id: form.id.clone(),
                field_values: form.field_values.clone(),
                sections: form.sections.clone(),
            };
            forms.saving = true;
            model.error = None;
            vec![effect]
        }
        Msg::FormCreateRequested { template_id } => {
            let Some(forms) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.forms.as_mut())
            else {
                return Vec::new();
            };
            if forms.saving {
                return Vec::new();
            }
            let Some(form) = forms.selected.as_ref() else {
                return Vec::new();
            };
            let effect = Effect::CreateForm {
                screen: model.screen.key,
                generation: model.generation,
                template_id,
                deal_id: form.deal_id.clone(),
                person_id: form.person_id.clone(),
                property_id: form.property_id.clone(),
            };
            forms.saving = true;
            model.error = None;
            vec![effect]
        }
        Msg::FormCreated { form_id } => {
            if let Some(forms) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.forms.as_mut())
            {
                forms.saving = false;
            }
            vec![Effect::BrowserNavigate {
                href: format!("/portal/forms/{form_id}"),
            }]
        }

        Msg::CockpitTaskCompleteRequested { task_id } => {
            if model.screen.key != "dashboard" || model.loading || task_id.trim().is_empty() {
                return Vec::new();
            }
            model.loading = true;
            model.error = None;
            vec![Effect::CompleteCockpitTask {
                screen: model.screen.key,
                generation: model.generation,
                task_id,
            }]
        }
        Msg::ProjectDomainSelected(domain) => {
            let Some(projects) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.projects.as_mut())
            else {
                return Vec::new();
            };
            if !matches!(
                domain.as_str(),
                "properties" | "people" | "deals" | "firm" | "marketing" | "accounting"
            ) {
                return Vec::new();
            }
            projects.active_domain = domain.clone();
            projects.catch_up = false;
            projects.selected_project_id = first_project_for_domain(projects, &domain);
            projects.selected_node_id =
                first_node_for_project(projects, projects.selected_project_id.as_deref());
            projects.active_view = "work-plan".into();
            projects.work_collapsed = false;
            projects.work_dirty = false;
            Vec::new()
        }
        Msg::ProjectSelected(project_id) => {
            let Some(projects) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.projects.as_mut())
            else {
                return Vec::new();
            };
            if !projects
                .projects
                .iter()
                .any(|project| project.id == project_id)
            {
                return Vec::new();
            }
            projects.selected_project_id = Some(project_id);
            projects.selected_node_id =
                first_node_for_project(projects, projects.selected_project_id.as_deref());
            projects.catch_up = false;
            projects.active_view = "work-plan".into();
            projects.work_collapsed = false;
            projects.work_dirty = false;
            Vec::new()
        }
        Msg::ProjectNodeSelected(node_id) => {
            let Some(projects) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.projects.as_mut())
            else {
                return Vec::new();
            };
            if let Some(id) = node_id.as_deref() {
                let valid = projects.items.iter().any(|item| {
                    item.id == id
                        && item.project_id.as_deref()
                            == projects.selected_project_id.as_deref()
                });
                if !valid {
                    return Vec::new();
                }
            }
            if node_id.is_some() {
                projects.work_collapsed = false;
            }
            projects.selected_node_id = node_id;
            projects.work_dirty = false;
            Vec::new()
        }
        Msg::ProjectViewSelected(view) => {
            let Some(projects) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.projects.as_mut())
            else {
                return Vec::new();
            };
            if !matches!(
                view.as_str(),
                "work-plan" | "timeline" | "calendar" | "financials" | "documents" | "activity"
            ) {
                return Vec::new();
            }
            projects.active_view = view;
            projects.catch_up = false;
            Vec::new()
        }
        Msg::ProjectCatchUpToggled(on) => {
            let Some(projects) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.projects.as_mut())
            else {
                return Vec::new();
            };
            projects.catch_up = on;
            projects.work_dirty = false;
            Vec::new()
        }
        Msg::ProjectCatchUpItemSelected { project_id, node_id } => {
            let Some(projects) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.projects.as_mut())
            else {
                return Vec::new();
            };
            let valid = projects.items.iter().any(|item| {
                item.id == node_id && item.project_id.as_deref() == Some(project_id.as_str())
            });
            if !valid {
                return Vec::new();
            }
            projects.selected_project_id = Some(project_id);
            projects.selected_node_id = Some(node_id);
            projects.catch_up = true;
            projects.work_collapsed = false;
            projects.work_dirty = false;
            Vec::new()
        }
        Msg::ProjectCatchUpItemCompleteRequested { project_id, node_id } => {
            let Some(projects) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.projects.as_mut())
            else {
                return Vec::new();
            };
            if projects.saving {
                return Vec::new();
            }
            let Some(item) = projects.items.iter().find(|item| {
                item.id == node_id && item.project_id.as_deref() == Some(project_id.as_str())
            }) else {
                return Vec::new();
            };
            if matches!(item.status.as_str(), "done" | "dismissed") {
                return Vec::new();
            }
            let effect = Effect::SaveProjectWork {
                screen: model.screen.key,
                generation: model.generation,
                item_id: item.id.clone(),
                title: item.title.clone(),
                notes: item.notes.clone(),
                status: "done".into(),
                due_at: item.due_at.clone(),
                owner: item.owner.clone(),
            };
            projects.selected_project_id = Some(project_id);
            projects.selected_node_id = Some(node_id);
            projects.catch_up = true;
            projects.work_collapsed = false;
            projects.work_dirty = false;
            projects.saving = true;
            model.error = None;
            vec![effect]
        }
        Msg::ProjectStatusRequested(status) => {
            let Some(projects) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.projects.as_mut())
            else {
                return Vec::new();
            };
            if projects.saving
                || !matches!(status.as_str(), "open" | "doing" | "done" | "archived")
            {
                return Vec::new();
            }
            let Some(project_id) = projects.selected_project_id.clone() else {
                return Vec::new();
            };
            projects.saving = true;
            model.error = None;
            vec![Effect::UpdateProjectStatus {
                screen: model.screen.key,
                generation: model.generation,
                project_id,
                status,
            }]
        }
        Msg::ProjectWorkTitleChanged(value) => {
            project_work_change(model, |item| item.title = value)
        }
        Msg::ProjectWorkNotesChanged(value) => {
            project_work_change(model, |item| item.notes = value)
        }
        Msg::ProjectWorkOwnerChanged(value) => {
            project_work_change(model, |item| {
                item.owner = (!value.trim().is_empty()).then_some(value)
            })
        }
        Msg::ProjectWorkDueChanged(value) => {
            project_work_change(model, |item| {
                item.due_at = (!value.trim().is_empty()).then_some(value)
            })
        }
        Msg::ProjectWorkStatusChanged(value) => {
            if !matches!(value.as_str(), "open" | "doing" | "done" | "dismissed") {
                return Vec::new();
            }
            project_work_change(model, |item| item.status = value)
        }
        Msg::ProjectWorkCollapsedToggled => {
            let Some(projects) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.projects.as_mut())
            else {
                return Vec::new();
            };
            projects.work_collapsed = !projects.work_collapsed;
            Vec::new()
        }
        Msg::ProjectWorkSaveRequested => {
            let Some(projects) = model
                .page
                .as_mut()
                .and_then(|page| page.portal.as_mut())
                .and_then(|portal| portal.projects.as_mut())
            else {
                return Vec::new();
            };
            if projects.saving || !projects.work_dirty {
                return Vec::new();
            }
            let Some(node_id) = projects.selected_node_id.as_deref() else {
                return Vec::new();
            };
            let Some(item) = projects.items.iter().find(|item| item.id == node_id) else {
                return Vec::new();
            };
            let effect = Effect::SaveProjectWork {
                screen: model.screen.key,
                generation: model.generation,
                item_id: item.id.clone(),
                title: item.title.clone(),
                notes: item.notes.clone(),
                status: item.status.clone(),
                due_at: item.due_at.clone(),
                owner: item.owner.clone(),
            };
            projects.saving = true;
            model.error = None;
            vec![effect]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Row, Screen};

    /// Screens are addressed by KEY in these tests: the table is the source of truth, so a test that named a variant
    /// would be asserting a name that only exists in a previous version of this file.
    fn target(key: &str) -> Screen {
        crate::model::screen(key).expect("a screen the table defines")
    }

    fn row(id: &str) -> Row {
        Row {
            id: id.into(),
            cells: vec![format!("row {id}")],
            badge: None,
        }
    }

    /// A rows response that OWNS ITSELF: the screen the model is on, and the mount it holds.
    ///
    /// Every response carries who it is for now, so a test that built one by hand with the wrong screen would be testing
    /// the refusal path by accident — which is the sort of thing that reads as a passing test and asserts nothing. This
    /// is the shape the shell sends.
    fn rows_for(model: &Model, rows: Vec<Row>) -> Msg {
        Msg::RowsLoaded {
            screen: model.screen.key.to_string(),
            generation: model.generation,
            rows,
        }
    }

    /// A failure that OWNS ITSELF, exactly as `yew_effects` builds one and as the payload parsers build theirs: the
    /// screen the request was for and the mount that asked. A test that built one by hand with the wrong owner would be
    /// testing the refusal path by accident.
    fn failure_for(model: &Model, message: &str) -> Msg {
        Msg::EffectFailed {
            screen: model.screen.key.to_string(),
            generation: model.generation,
            message: message.to_string(),
        }
    }

    #[test]
    fn navigating_to_cabinet_fetches_the_typed_repository() {
        let mut model = Model::default();
        let effects = update(&mut model, Msg::Navigate(target("cabinet")));
        assert_eq!(model.screen, target("cabinet"));
        assert!(model.loading);
        assert_eq!(
            effects,
            vec![Effect::FetchCabinet {
                screen: "cabinet",
                generation: 0,
            }]
        );
    }

    #[test]
    fn navigating_to_dashboard_fetches_the_typed_cockpit() {
        let mut model = Model::default();
        let effects = update(&mut model, Msg::Navigate(target("dashboard")));
        assert_eq!(model.screen, target("dashboard"));
        assert!(model.loading);
        assert_eq!(
            effects,
            vec![Effect::FetchCockpit {
                screen: "dashboard",
                generation: 0,
            }]
        );
    }

    #[test]
    fn cockpit_task_completion_is_reducer_owned() {
        let mut model = Model {
            screen: target("dashboard"),
            ..Model::default()
        };
        assert_eq!(
            update(
                &mut model,
                Msg::CockpitTaskCompleteRequested {
                    task_id: "task-1".into(),
                },
            ),
            vec![Effect::CompleteCockpitTask {
                screen: "dashboard",
                generation: 0,
                task_id: "task-1".into(),
            }]
        );
        assert!(model.loading);
    }

    #[test]
    fn seller_strategy_is_local_and_reducer_owned() {
        let mut model = Model::default();
        let effects = update(&mut model, Msg::Navigate(target("seller-strategy")));
        assert!(effects.is_empty());
        assert!(!model.loading);

        update(
            &mut model,
            Msg::SellerStrategyFieldChanged {
                key: "appraisal".into(),
                raw: "500000".into(),
                percent: false,
            },
        );
        assert_eq!(model.seller_strategy.inputs.appraisal, 500000.0);

        update(
            &mut model,
            Msg::SellerStrategyOptionToggled {
                option: 3,
                enabled: false,
            },
        );
        assert!(!model.seller_strategy.inputs.o3_on);
    }

    #[test]
    fn navigating_to_deals_fetches_the_typed_portfolio() {
        let mut model = Model::default();
        let effects = update(&mut model, Msg::Navigate(target("deals")));
        assert_eq!(model.screen, target("deals"));
        assert!(model.loading);
        assert_eq!(
            effects,
            vec![Effect::FetchDeals {
                screen: "deals",
                scope: None,
                generation: 0,
            }]
        );
    }

    #[test]
    fn direct_deal_record_mount_preserves_scope_on_the_typed_fetch() {
        let mut model = Model::default();
        let effects = update(
            &mut model,
            Msg::MountScoped {
                screen: target("deal-record"),
                scope: Some("deal-7".into()),
                generation: 4,
            },
        );
        assert_eq!(
            effects,
            vec![Effect::FetchDeals {
                screen: "deal-record",
                scope: Some("deal-7".into()),
                generation: 4,
            }]
        );
    }

    #[test]
    fn deal_creation_and_people_search_are_reducer_owned() {
        let mut model = Model {
            screen: target("deals"),
            ..Model::default()
        };
        let search = update(
            &mut model,
            Msg::DealCreateClientQueryChanged("Ali".into()),
        );
        assert!(model.deal_create.searching);
        assert_eq!(
            search,
            vec![Effect::SearchDealPeople {
                screen: "deals",
                generation: 0,
                query: "Ali".into(),
            }]
        );

        update(
            &mut model,
            Msg::DealCreateClientSelected {
                id: "person-1".into(),
                label: "Alicia".into(),
            },
        );
        update(
            &mut model,
            Msg::DealCreatePropertyChanged("property-1".into()),
        );
        let create = update(&mut model, Msg::DealCreateRequested);
        assert!(model.deal_create.submitting);
        assert_eq!(
            create,
            vec![Effect::CreateDeal {
                screen: "deals",
                generation: 0,
                property_id: "property-1".into(),
                client_person_id: "person-1".into(),
                owner_user_id: None,
                notes: None,
            }]
        );
    }

    #[test]
    fn deal_workspace_commands_are_reducer_owned_and_scoped() {
        let mut model = Model {
            screen: target("deal-record"),
            scope: Some("deal-7".into()),
            ..Model::default()
        };

        update(
            &mut model,
            Msg::DealWorkspaceTaskTitleChanged("Call notario".into()),
        );
        update(
            &mut model,
            Msg::DealWorkspaceTaskDueChanged("2026-09-23T09:30".into()),
        );

        assert_eq!(
            update(&mut model, Msg::DealWorkspaceCreateTaskRequested),
            vec![Effect::RunDealWorkspaceCommand {
                screen: "deal-record",
                generation: 0,
                deal_id: "deal-7".into(),
                command: PortalDealCommand::CreateTask {
                    title: "Call notario".into(),
                    detail: None,
                    due_at: Some("2026-09-23T09:30".into()),
                },
            }]
        );
        assert_eq!(
            model.deal_workspace.busy_action.as_deref(),
            Some("task:create")
        );

        assert_eq!(
            update(
                &mut model,
                Msg::DealWorkspaceCommandCompleted {
                    screen: "deal-record".into(),
                    generation: 0,
                    id: "task-1".into(),
                },
            ),
            vec![Effect::FetchDeals {
                screen: "deal-record",
                scope: Some("deal-7".into()),
                generation: 0,
            }]
        );
        assert!(model.deal_workspace.task_title.is_empty());
        assert!(model.deal_workspace.busy_action.is_none());
    }

    #[test]
    fn navigating_to_clients_fetches_the_typed_workspace() {
        let mut model = Model::default();
        let effects = update(&mut model, Msg::Navigate(target("clients")));
        assert_eq!(model.screen, target("clients"));
        assert!(model.loading);
        assert_eq!(
            effects,
            vec![Effect::FetchClients {
                screen: "clients",
                scope: None,
                selected: None,
                search: String::new(),
                page: 0,
                generation: 0
            }]
        );
    }

    #[test]
    fn navigating_to_projects_fetches_the_typed_workspace() {
        let mut model = Model::default();
        let effects = update(&mut model, Msg::Navigate(target("projects")));
        assert_eq!(model.screen, target("projects"));
        assert!(model.loading);
        assert_eq!(
            effects,
            vec![Effect::FetchProjects {
                screen: "projects",
                generation: 0,
            }]
        );
    }

    #[test]
    fn project_work_changes_are_reducer_owned_and_save_as_one_effect() {
        let mut model = Model {
            screen: target("projects"),
            ..Model::default()
        };
        model.page = Some(crate::model::PageContent {
            portal: Some(crate::model::PortalPage {
                projects: Some(crate::model::PortalProjectsPage {
                    projects: vec![crate::model::PortalProject {
                        id: "p1".into(),
                        name: "Listing".into(),
                        ..Default::default()
                    }],
                    items: vec![crate::model::PortalProjectWorkItem {
                        id: "w1".into(),
                        title: "Agreement".into(),
                        status: "open".into(),
                        project_id: Some("p1".into()),
                        ..Default::default()
                    }],
                    selected_project_id: Some("p1".into()),
                    selected_node_id: Some("w1".into()),
                    active_domain: "properties".into(),
                    active_view: "work-plan".into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        });

        assert!(update(
            &mut model,
            Msg::ProjectWorkTitleChanged("Listing Agreement".into())
        )
        .is_empty());

        let projects = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.projects.as_ref())
            .expect("projects payload");
        assert!(projects.work_dirty);
        assert_eq!(projects.items[0].title, "Listing Agreement");

        assert_eq!(
            update(&mut model, Msg::ProjectWorkSaveRequested),
            vec![Effect::SaveProjectWork {
                screen: "projects",
                generation: 0,
                item_id: "w1".into(),
                title: "Listing Agreement".into(),
                notes: String::new(),
                status: "open".into(),
                due_at: None,
                owner: None,
            }]
        );
    }

    #[test]
    fn navigating_back_to_the_same_screen_does_nothing() {
        let mut model = Model {
            screen: target("clients"),
            ..Model::default()
        };
        assert!(update(&mut model, Msg::Navigate(target("clients"))).is_empty());
    }

    #[test]
    fn the_deferred_screen_navigates_without_fetching() {
        let mut model = Model::default();
        // Media Test is the screen that is a placeholder BY DESIGN: a manual harness with no read model, so it asks for
        // nothing and says why. (The receipt scanner was this example until it became a real screen; every Accounting screen
        // is ported now.)
        assert!(update(&mut model, Msg::Navigate(target("media-test"))).is_empty());
        assert_eq!(model.screen, target("media-test"));
        assert!(
            !model.loading,
            "a placeholder must not show a spinner for data it never asks for"
        );
    }

    #[test]
    fn opening_a_listing_row_opens_that_record_and_asks_about_it() {
        let mut model = Model {
            screen: target("site-properties"),
            ..Model::default()
        };
        let effects = update(&mut model, Msg::RecordOpened("villa-del-mar".into()));
        assert_eq!(model.screen, target("site-property-detail"));
        assert_eq!(
            effects,
            vec![Effect::FetchPage {
                screen: "site-property-detail",
                scope: Some("villa-del-mar".into()),
                generation: 0,
            }],
            "the record key must reach the host, and a record is a PAGE, not a row list: it is a cockpit, a gallery and \
             four tabs, so it asks for its blocks. This expected FetchRows until the record page was ported."
        );
    }

    #[test]
    fn opening_a_row_where_there_is_no_detail_view_selects_it() {
        // Activity is a feed: a row of it is history, not a record to open.
        let mut model = Model {
            screen: target("activity"),
            ..Model::default()
        };
        let rows = rows_for(&model, vec![row("a")]);
        update(&mut model, rows);
        assert!(update(&mut model, Msg::RecordOpened("a".into())).is_empty());
        assert_eq!(
            model.screen,
            target("activity"),
            "there is nowhere to navigate to"
        );
        assert_eq!(model.selected_row_id.as_deref(), Some("a"));
    }

    #[test]
    fn navigating_back_to_a_list_clears_the_record_it_was_about() {
        let mut model = Model::default();
        update(&mut model, Msg::RecordOpened("villa-del-mar".into()));
        update(&mut model, Msg::Navigate(target("site-properties")));
        assert_eq!(
            model.scope, None,
            "a stale slug would make the next detail fetch about the wrong record"
        );
    }

    #[test]
    fn rows_from_the_previous_screen_never_leak_into_the_next() {
        let mut model = Model::default();
        let rows = rows_for(&model, vec![row("a")]);
        update(&mut model, rows);
        update(&mut model, Msg::RowSelected("a".into()));
        update(&mut model, Msg::Navigate(target("deals")));
        assert!(model.rows.is_empty());
        assert_eq!(model.selected_row_id, None);
    }

    #[test]
    fn a_refresh_that_loses_the_selected_row_drops_the_selection() {
        let mut model = Model::default();
        let rows = rows_for(&model, vec![row("a")]);
        update(&mut model, rows);
        update(&mut model, Msg::RowSelected("a".into()));
        let rows = rows_for(&model, vec![row("b")]);
        update(&mut model, rows);
        assert_eq!(model.selected_row_id, None);
    }

    #[test]
    fn a_broken_payload_is_an_error_not_a_panic() {
        assert!(matches!(
            Msg::rows_loaded_json("clients", 0, "nope"),
            Msg::EffectFailed { .. }
        ));
    }

    #[test]
    fn a_failed_request_keeps_the_data_the_user_was_reading() {
        let mut model = Model {
            loading: true,
            ..Model::default()
        };
        let rows = rows_for(&model, vec![row("a")]);
        update(&mut model, rows);
        model.loading = true;
        let failure = failure_for(&model, "network");
        update(&mut model, failure);
        assert_eq!(model.error.as_deref(), Some("network"));
        assert_eq!(model.rows.len(), 1);
    }

    /// A FAILURE IS OWNED TOO, and an unowned one is the worse half of the problem the payload owner solves: a rejected
    /// request for a screen the visitor has left must not put its message on the screen they are on now, nor clear a
    /// loading state that belongs to a request still in flight.
    #[test]
    fn a_failure_for_another_screen_or_another_mount_is_discarded() {
        let mut model = Model::default();
        update(
            &mut model,
            Msg::Mount {
                screen: target("site-home"),
                generation: 7,
            },
        );
        model.loading = true;

        // A failure from the previous mount of this screen: same screen, superseded run.
        let stale_mount = Msg::EffectFailed {
            screen: "site-home".into(),
            generation: 6,
            message: "the previous run's request failed".into(),
        };
        update(&mut model, stale_mount);
        assert_eq!(
            model.error, None,
            "a superseded run's failure is not this one's"
        );
        assert!(
            model.loading,
            "and it must not clear the loading state of a live request"
        );

        // A failure for a screen the visitor has left.
        let other_screen = Msg::EffectFailed {
            screen: "site-buyers".into(),
            generation: 7,
            message: "Buyers could not be loaded".into(),
        };
        update(&mut model, other_screen);
        assert_eq!(model.error, None);
        assert!(model.loading);

        // And the current screen's OWN failure is recorded: the rule refuses the stale, not the real.
        let current = Msg::EffectFailed {
            screen: "site-home".into(),
            generation: 7,
            message: "Home could not be loaded".into(),
        };
        update(&mut model, current);
        assert_eq!(model.error.as_deref(), Some("Home could not be loaded"));
        assert!(!model.loading, "a failed request is finished, not loading");
    }

    // ---- controls ----------------------------------------------------------------------------------------------

    #[test]
    fn a_narrowing_control_returns_to_the_first_page() {
        let mut model = Model::default();
        let rows = rows_for(
            &model,
            (0..PAGE_SIZE + 5).map(|i| row(&i.to_string())).collect(),
        );
        update(&mut model, rows);
        update(&mut model, Msg::PageChanged(1));
        assert_eq!(model.controls.page, 1);
        update(&mut model, Msg::QueryChanged("ada".into()));
        assert_eq!(
            model.controls.page, 0,
            "page 2 of a list is not page 2 of the list the user is now filtering"
        );
    }

    #[test]
    fn paging_is_bounded_by_the_rows_the_model_holds() {
        let mut model = Model::default();
        let rows = rows_for(
            &model,
            (0..PAGE_SIZE + 1).map(|i| row(&i.to_string())).collect(),
        );
        update(&mut model, rows);
        update(&mut model, Msg::PageChanged(-1));
        assert_eq!(model.controls.page, 0, "there is no page before the first");
        update(&mut model, Msg::PageChanged(9));
        assert_eq!(
            model.controls.page, 1,
            "PAGE_SIZE+1 rows are two pages, so the last page is 1"
        );
    }

    #[test]
    fn filtering_does_not_follow_the_user_to_the_next_screen() {
        let mut model = Model::default();
        update(&mut model, Msg::QueryChanged("ada".into()));
        update(&mut model, Msg::TabSelected("open".into()));
        update(&mut model, Msg::Toggled(true));
        update(&mut model, Msg::Navigate(target("deals")));
        assert_eq!(
            model.controls,
            Controls::default(),
            "a filter typed on one screen must not narrow the next one"
        );
    }

    #[test]
    fn a_control_message_asks_the_host_for_nothing() {
        let mut model = Model::default();
        for msg in [
            Msg::QueryChanged("x".into()),
            Msg::FilterChanged("open".into()),
            Msg::FilterSelected {
                key: "price".into(),
                value: "2000000".into(),
            },
            Msg::TabSelected("all".into()),
            Msg::Toggled(true),
            Msg::PageChanged(1),
        ] {
            assert!(
                update(&mut model, msg).is_empty(),
                "filtering is local until a screen's filter becomes a server round trip, and then it earns an effect"
            );
        }
    }

    // ---- accounting: the expense form's reducer ownership, and what a command completion says ------------------------

    /// A model sitting on the Expenses screen with a filled-in draft, which is where the tests below start.
    fn expenses_screen() -> Model {
        let mut model = Model {
            screen: target("accounting-expenses"),
            ..Model::default()
        };
        model.accounting.expense_open = true;
        model.accounting.expense_vendor = "Sunrise Fuel".into();
        model.accounting.expense_category = "Office".into();
        model.accounting.expense_amount = "125.50".into();
        model.accounting.expense_on = "2026-03-04".into();
        model
    }

    #[test]
    fn the_expense_draft_belongs_to_the_expenses_screen() {
        // The same field message, on another screen: nothing is written. A draft that only the Expenses screen renders can
        // still be written by a message from anywhere, so the guard is the reducer's rather than the view's.
        let mut elsewhere = Model {
            screen: target("accounting"),
            ..Model::default()
        };
        update(&mut elsewhere, Msg::ExpenseVendorChanged("Sunrise Fuel".into()));
        assert_eq!(elsewhere.accounting.expense_vendor, "");

        let mut model = expenses_screen();
        update(&mut model, Msg::ExpenseVendorChanged("  Harbour Marine  ".into()));
        // The reducer stores what was typed, spaces and all: trimming is the domain's job, and doing it here as well would
        // mean two ideas of what the operator entered.
        assert_eq!(model.accounting.expense_vendor, "  Harbour Marine  ");
    }

    #[test]
    fn the_expense_form_asks_rust_to_create_the_row() {
        let mut model = expenses_screen();
        let effects = update(&mut model, Msg::ExpenseSubmitted);

        assert_eq!(effects.len(), 1);
        let Effect::AccountingCommand { screen, body, .. } = &effects[0] else {
            panic!("submitting the expense form must ask for an Accounting command");
        };
        assert_eq!(*screen, "accounting-expenses");
        assert_eq!(body["action"], "createExpense");
        // The amount travels as the digits the operator typed. A number here would have been through a float before Rust
        // could validate it.
        assert_eq!(body["amount"], "125.50");
        assert_eq!(body["vendor"], "Sunrise Fuel");
        assert_eq!(body["expenseOn"], "2026-03-04");
        assert!(model.accounting.submitting, "the form is pending until the answer arrives");
        assert!(model.accounting.notice.is_none());
    }

    #[test]
    fn a_second_click_cannot_write_a_second_row() {
        let mut model = expenses_screen();
        update(&mut model, Msg::ExpenseSubmitted);
        let again = update(&mut model, Msg::ExpenseSubmitted);
        assert!(
            again.is_empty(),
            "a form already waiting on its answer must not issue another command"
        );
    }

    #[test]
    fn a_command_completion_is_the_refresh_and_says_so() {
        let mut model = expenses_screen();
        update(&mut model, Msg::ExpenseSubmitted);

        // What the bridge answers with: the screen as it now is, including the row just written.
        let payload = r#"{"accounting":{"expenses":[{"id":"e1","vendor":"Sunrise Fuel","category":"Office",
            "amount":"125.50","expenseOn":"2026-03-04","status":"POSTED"}],"today":"2026-03-04"}}"#;
        update(&mut model, Msg::portal_loaded_json("accounting-expenses", 0, payload));

        assert!(!model.accounting.submitting, "the pending state ends with the answer");
        assert_eq!(
            model.accounting.notice,
            Some(crate::model::CommandNotice::success("Created."))
        );
        let rows = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.accounting.as_ref())
            .map(|accounting| accounting.expenses.clone())
            .unwrap_or_default();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vendor, "Sunrise Fuel");
    }

    #[test]
    fn a_failed_command_says_what_rust_said_and_leaves_the_draft_alone() {
        let mut model = expenses_screen();
        update(&mut model, Msg::ExpenseSubmitted);

        update(
            &mut model,
            Msg::EffectFailed {
                screen: "accounting-expenses".into(),
                generation: 0,
                message: "Vendor is required.".into(),
            },
        );

        assert!(!model.accounting.submitting);
        assert_eq!(
            model.accounting.notice,
            Some(crate::model::CommandNotice::failure("Vendor is required."))
        );
        // The draft survives, which is the point: an operator fixes one field rather than retyping four.
        assert_eq!(model.accounting.expense_vendor, "Sunrise Fuel");
        assert_eq!(model.accounting.expense_amount, "125.50");
    }

    #[test]
    fn the_book_s_day_fills_an_empty_date_and_never_overwrites_a_chosen_one() {
        let payload = r#"{"accounting":{"expenses":[],"today":"2026-03-04"}}"#;

        let mut fresh = Model {
            screen: target("accounting-expenses"),
            ..Model::default()
        };
        update(&mut fresh, Msg::portal_loaded_json("accounting-expenses", 0, payload));
        assert_eq!(fresh.accounting.expense_on, "2026-03-04");

        let mut chosen = expenses_screen();
        chosen.accounting.expense_on = "2026-03-01".into();
        update(&mut chosen, Msg::portal_loaded_json("accounting-expenses", 0, payload));
        assert_eq!(
            chosen.accounting.expense_on, "2026-03-01",
            "a date the operator chose must survive a refresh"
        );
    }

    // ---- accounting: receivables, and the mark-paid transition ------------------------------------------------------

    fn receivables_screen() -> Model {
        let mut model = Model {
            screen: target("accounting-receivables"),
            ..Model::default()
        };
        model.accounting.receivable_open = true;
        model.accounting.receivable_description = "Closing commission".into();
        model.accounting.receivable_amount = "12000".into();
        model.accounting.receivable_category = "COMMISSION".into();
        model.accounting.receivable_issued_on = "2026-03-01".into();
        model
    }

    #[test]
    fn the_receivable_form_asks_rust_to_create_the_row() {
        let mut model = receivables_screen();
        let effects = update(&mut model, Msg::ReceivableSubmitted);

        let Effect::AccountingCommand { screen, body, .. } = &effects[0] else {
            panic!("submitting the receivable form must ask for an Accounting command");
        };
        assert_eq!(*screen, "accounting-receivables");
        assert_eq!(body["action"], "createReceivable");
        assert_eq!(body["amount"], "12000");
        assert_eq!(body["issuedOn"], "2026-03-01");
        assert!(model.accounting.submitting);
    }

    #[test]
    fn mark_paid_names_the_receivable_and_the_date_it_was_paid_on() {
        let mut model = receivables_screen();
        update(
            &mut model,
            Msg::ReceivablePaidDateChanged {
                id: "r1".into(),
                value: "2026-03-10".into(),
            },
        );
        let effects = update(
            &mut model,
            Msg::ReceivablePaidSubmitted { id: "r1".into() },
        );

        let Effect::AccountingCommand { body, .. } = &effects[0] else {
            panic!("marking a receivable paid must ask for an Accounting command");
        };
        assert_eq!(body["action"], "markReceivablePaid");
        assert_eq!(body["receivableId"], "r1");
        // The date is the row's own, not one shared across the table.
        assert_eq!(body["paidOn"], "2026-03-10");
    }

    #[test]
    fn a_row_with_no_chosen_date_is_paid_on_the_books_day() {
        let mut model = receivables_screen();
        // What the payload brought with it, which is also what the row's input is showing.
        let payload = r#"{"accounting":{"receivables":[],"today":"2026-03-04"}}"#;
        update(
            &mut model,
            Msg::portal_loaded_json("accounting-receivables", 0, payload),
        );

        let effects = update(
            &mut model,
            Msg::ReceivablePaidSubmitted { id: "r1".into() },
        );
        let Effect::AccountingCommand { body, .. } = &effects[0] else {
            panic!("marking a receivable paid must ask for an Accounting command");
        };
        assert_eq!(body["paidOn"], "2026-03-04");
    }

    #[test]
    fn one_required_field_missing_is_the_service_s_decision_not_the_reducer_s() {
        let mut model = receivables_screen();
        // No description. The reducer does not refuse it — it asks, and Rust answers — which is what keeps one set of rules
        // in one place.
        model.accounting.receivable_description = String::new();
        let effects = update(&mut model, Msg::ReceivableSubmitted);
        assert_eq!(effects.len(), 1);

        update(
            &mut model,
            Msg::EffectFailed {
                screen: "accounting-receivables".into(),
                generation: 0,
                message: "Description is required.".into(),
            },
        );
        assert_eq!(
            model.accounting.notice,
            Some(crate::model::CommandNotice::failure("Description is required."))
        );
    }

    #[test]
    fn a_void_receivable_that_cannot_be_paid_keeps_its_row_and_reports_the_conflict() {
        let mut model = receivables_screen();
        update(
            &mut model,
            Msg::ReceivablePaidSubmitted { id: "r1".into() },
        );
        // The service's answer for a receivable that is missing or void: nothing was transitioned.
        update(
            &mut model,
            Msg::EffectFailed {
                screen: "accounting-receivables".into(),
                generation: 0,
                message: "Receivable not found or voided.".into(),
            },
        );
        assert!(!model.accounting.submitting);
        assert_eq!(
            model.accounting.notice,
            Some(crate::model::CommandNotice::failure(
                "Receivable not found or voided."
            ))
        );

    }

    // ---- accounting: the P&L's period ------------------------------------------------------------------------------
    // ---- accounting: the P&L's period ------------------------------------------------------------------------------

    #[test]
    fn applying_a_period_asks_for_exactly_that_period() {
        let mut model = Model {
            screen: target("accounting-pnl"),
            ..Model::default()
        };
        update(&mut model, Msg::PnlFromChanged("2026-03-01".into()));
        update(&mut model, Msg::PnlToChanged("2026-03-31".into()));

        let effects = update(&mut model, Msg::PnlApplied);

        let Effect::FetchAccountingPnl {
            screen, from, to, ..
        } = &effects[0]
        else {
            panic!("applying a period must ask for the P&L of that period");
        };
        assert_eq!(*screen, "accounting-pnl");
        assert_eq!(from, "2026-03-01");
        assert_eq!(to, "2026-03-31");
        assert!(model.loading, "the screen is loading the period it asked for");
    }

    #[test]
    fn the_period_fields_take_the_period_that_was_projected() {
        let mut model = Model {
            screen: target("accounting-pnl"),
            ..Model::default()
        };
        // The bridge's answer for a first visit: the current month, echoed back.
        let payload = r#"{"accounting":{"pnl":{"from":"2026-03-01","to":"2026-03-31","income":[],
            "totalIncome":"0","expenses":[],"totalExpenses":"0","netIncome":"0"}}}"#;
        update(&mut model, Msg::portal_loaded_json("accounting-pnl", 0, payload));

        assert_eq!(model.accounting.pnl_from, "2026-03-01");
        assert_eq!(model.accounting.pnl_to, "2026-03-31");

        // A second payload for a period the operator chose does not overwrite what they typed: the fields are theirs once
        // they hold anything.
        update(&mut model, Msg::PnlFromChanged("2026-01-01".into()));
        update(&mut model, Msg::portal_loaded_json("accounting-pnl", 0, payload));
        assert_eq!(model.accounting.pnl_from, "2026-01-01");
    }

    #[test]
    fn a_backwards_period_is_the_service_s_refusal_and_the_fields_survive_it() {
        let mut model = Model {
            screen: target("accounting-pnl"),
            ..Model::default()
        };
        update(&mut model, Msg::PnlFromChanged("2026-04-01".into()));
        update(&mut model, Msg::PnlToChanged("2026-03-31".into()));
        update(&mut model, Msg::PnlApplied);

        update(
            &mut model,
            Msg::EffectFailed {
                screen: "accounting-pnl".into(),
                generation: 0,
                message: "The period starts after it ends: 2026-04-01 to 2026-03-31.".into(),
            },
        );

        assert!(!model.loading);
        assert_eq!(
            model.error,
            Some("The period starts after it ends: 2026-04-01 to 2026-03-31.".to_string())
        );
        // Both ends are still there to be corrected, which is the point of the fields being the reducer's.
        assert_eq!(model.accounting.pnl_from, "2026-04-01");
        assert_eq!(model.accounting.pnl_to, "2026-03-31");
    }

    #[test]
    fn opening_the_pnl_carries_the_period_the_screen_is_showing() {
        // A screen that already holds a period asks for that one; a screen opened fresh asks with two empty ends, which the
        // bridge reads as the current month.
        let mut fresh = Model {
            screen: target("accounting"),
            ..Model::default()
        };
        let fresh = update(&mut fresh, Msg::Navigate(target("accounting-pnl")));
        let Effect::FetchAccountingPnl { from, to, .. } = &fresh[0] else {
            panic!("opening the P&L must ask for a period");
        };
        assert!(from.is_empty() && to.is_empty());

        let mut held = Model {
            screen: target("accounting-pnl"),
            ..Model::default()
        };
        held.accounting.pnl_from = "2026-02-01".into();
        held.accounting.pnl_to = "2026-02-28".into();
        let effects = update(&mut held, Msg::Navigate(target("accounting")));
        assert_eq!(effects.len(), 1);
        let effects = update(&mut held, Msg::Navigate(target("accounting-pnl")));
        let Effect::FetchAccountingPnl { from, to, .. } = &effects[0] else {
            panic!("returning to the P&L must ask for its period again");
        };
        assert_eq!(from, "2026-02-01");
        assert_eq!(to, "2026-02-28");
    }

    // ---- accounting: the receipt scanner's demonstration ------------------------------------------------------------

    fn scanner_screen(today: &str) -> Model {
        let mut model = Model {
            screen: target("accounting-receipt-scanner"),
            ..Model::default()
        };
        let payload = format!(r#"{{"accounting":{{"today":"{today}"}}}}"#);
        update(
            &mut model,
            Msg::portal_loaded_json("accounting-receipt-scanner", 0, &payload),
        );
        model
    }

    #[test]
    fn scanning_cycles_the_demonstrations_receipts() {
        let mut model = scanner_screen("2026-03-04");

        update(&mut model, Msg::ScannerScanned);
        let first = model.accounting.scanner.draft.clone().unwrap();
        assert_eq!(first.vendor, "Metro Maintenance Co.");
        assert_eq!(first.amount, "412.5");
        assert_eq!(first.category, "Property / Deal Expense");
        // The file name the live screen defaulted to when nothing was attached.
        assert_eq!(model.accounting.scanner.file_name, "demo-receipt.jpg");

        update(&mut model, Msg::ScannerScanned);
        let second = model.accounting.scanner.draft.clone().unwrap();
        assert_eq!(second.vendor, "Wells Fargo Merchant Services");
        // The cycle wraps rather than running out.
        update(&mut model, Msg::ScannerScanned);
        update(&mut model, Msg::ScannerScanned);
        update(&mut model, Msg::ScannerScanned);
        assert_eq!(
            model.accounting.scanner.draft.clone().unwrap().vendor,
            "Metro Maintenance Co."
        );
    }

    #[test]
    fn the_reviewed_draft_is_dated_by_the_book_and_not_by_the_browser() {
        let mut model = scanner_screen("2026-03-04");
        update(&mut model, Msg::ScannerScanned);
        assert_eq!(
            model.accounting.scanner.draft.clone().unwrap().expense_on,
            "2026-03-04"
        );
    }

    #[test]
    fn an_attached_file_is_named_by_the_reader_not_by_the_drop() {
        let mut model = scanner_screen("2026-03-04");
        update(
            &mut model,
            Msg::ScannerFileChosen("IMG_4821.HEIC".into()),
        );
        assert_eq!(model.accounting.scanner.file_name, "IMG_4821.HEIC");
        // Scanning does not rename an attachment that is already there.
        update(&mut model, Msg::ScannerScanned);
        assert_eq!(model.accounting.scanner.file_name, "IMG_4821.HEIC");
    }

    #[test]
    fn saving_the_draft_records_the_expense_through_rust() {
        let mut model = scanner_screen("2026-03-04");
        update(&mut model, Msg::ScannerScanned);
        // The reviewer corrects the extraction: the one field the live select appeared to offer but could not change.
        update(
            &mut model,
            Msg::ScannerCategoryChanged("Office".into()),
        );

        let effects = update(&mut model, Msg::ScannerSubmitted);
        let Effect::AccountingCommand { screen, body, .. } = &effects[0] else {
            panic!("saving a reviewed receipt must ask for an Accounting command");
        };
        assert_eq!(*screen, "accounting-receipt-scanner");
        assert_eq!(body["action"], "createExpense");
        assert_eq!(body["vendor"], "Metro Maintenance Co.");
        assert_eq!(body["category"], "Office");
        assert_eq!(body["amount"], "412.5");
        assert_eq!(body["expenseOn"], "2026-03-04");
        // THE MEMO TRAVELS, which it did not before: the live form showed the seed's note and never submitted it, so the
        // recorded expense could not explain itself later. It is the receipt's note, and this is where it is confirmed.
        assert_eq!(body["memo"], "Walkthrough cleanup — demo receipt");
    }

    #[test]
    fn there_is_nothing_to_save_before_a_receipt_is_scanned() {
        let mut model = scanner_screen("2026-03-04");
        let effects = update(&mut model, Msg::ScannerSubmitted);
        assert!(
            effects.is_empty(),
            "an expense must not be recorded out of an empty draft"
        );
    }

    #[test]
    fn a_save_that_rust_refuses_says_so_and_keeps_the_draft() {
        let mut model = scanner_screen("2026-03-04");
        update(&mut model, Msg::ScannerScanned);
        update(&mut model, Msg::ScannerSubmitted);
        update(
            &mut model,
            Msg::EffectFailed {
                screen: "accounting-receipt-scanner".into(),
                generation: 0,
                message: "Amount must be a non-negative number.".into(),
            },
        );
        assert!(!model.accounting.submitting);
        assert_eq!(
            model.accounting.notice,
            Some(crate::model::CommandNotice::failure(
                "Amount must be a non-negative number."
            ))
        );
        // The reviewed draft survives, so the reviewer can look again rather than start over.
        assert!(model.accounting.scanner.draft.is_some());
    }
}

