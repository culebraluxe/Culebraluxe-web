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
            // Accounting V1 — the dashboard is the first of its five screens to have a component; the other four follow
            // one at a time, and each is added here and to the portal app's match together.
            | "accounting"
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
    model.deal_workspace = DealWorkspaceState::default();

    // Seller Strategy is a local deterministic calculator. Opening it resets the
    // same state the former React component created on mount and performs no fetch.
    if screen.key == "seller-strategy" {
        model.seller_strategy = crate::seller_strategy::SellerStrategyState::default();
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
        if screen.key == "dashboard" {
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
            model.deal_create.searching = false;
            model.deal_create.submitting = false;
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
        // The receipt scanner is the screen that is a placeholder BY DESIGN (its own header calls it FAKE V1), so it is
        // the honest example now that Projects is wired.
        assert!(update(
            &mut model,
            Msg::Navigate(target("accounting-receipt-scanner"))
        )
        .is_empty());
        assert_eq!(model.screen, target("accounting-receipt-scanner"));
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
}
