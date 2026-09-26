//! CORE — Contracts: the deal portfolio (`/portal/deals`) and one deal's workspace (`/portal/deals/:dealId`).
//!
//! The navigation says Contracts; the canonical record is the Deal. The portfolio opens deals; the workspace runs a deal's
//! tasks, offers, showings and participants. Every command answers with the id it touched, and the workspace is then
//! read again, so what is on screen is always the service's state, never a guess made here.
//!
//! ONE COMMAND AT A TIME. `busy_action` names the one in flight (`task:create`, `offer:submit:root`, ...): every button
//! waits on it, and when it answers, the form that sent it is cleared — only that form.

mod view;

use yew::prelude::*;

use crate::app::api::{DealCreate, DealPeopleSearch, DealWorkspaceCommand, DealsRead, RecordId};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{
    DealCreateState, DealWorkspaceState, PortalDealCommand, PortalDealPeopleSearch,
    PortalDealsPage, PortalPage,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Controls {
    /// The portfolio's stage filter (`None` is all). Applied to the rows on the page, not a new read.
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalDealsPage>,
    /// A re-read after a command is in flight; the page stays on screen.
    pub refreshing: bool,
    pub deal_create: DealCreateState,
    pub deal_workspace: DealWorkspaceState,
    pub controls: Controls,
    /// What stopped the last intent: a missing field, or the service's own words.
    pub error: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    FilterChanged(String),

    DealCreateToggled,
    DealCreatePropertyChanged(String),
    DealCreateClientQueryChanged(String),
    DealCreateClientSelected {
        id: String,
        label: String,
    },
    DealCreateOwnerChanged(String),
    DealCreateNotesChanged(String),
    DealCreateRequested,
    DealCreated(Result<RecordId, ApiError>),

    /// A person search answered, for the search box `purpose` names (`client`, `participant`, `structural`) and the
    /// text it was asked for — an answer for text the box no longer holds is dropped.
    PeopleLoaded {
        purpose: &'static str,
        query: String,
        answer: Result<PortalDealPeopleSearch, ApiError>,
    },

    DealWorkspaceTaskTitleChanged(String),
    DealWorkspaceTaskDetailChanged(String),
    DealWorkspaceTaskDueChanged(String),
    DealWorkspaceCreateTaskRequested,
    DealWorkspaceCompleteTaskRequested {
        task_id: String,
    },
    DealWorkspaceOfferAmountChanged {
        key: String,
        value: String,
    },
    DealWorkspaceSubmitOfferRequested {
        parent_offer_id: Option<String>,
    },
    DealWorkspaceWithdrawOfferRequested {
        offer_id: String,
    },
    DealWorkspaceRejectOfferRequested {
        offer_id: String,
    },
    DealWorkspaceCreateShowingRequested,
    DealWorkspaceShowingTimeChanged {
        showing_id: String,
        value: String,
    },
    DealWorkspaceScheduleShowingRequested {
        showing_id: String,
    },
    DealWorkspaceCancelShowingRequested {
        showing_id: String,
    },
    DealWorkspaceCompleteShowingRequested {
        showing_id: String,
    },
    DealWorkspaceParticipantQueryChanged(String),
    DealWorkspaceParticipantSelected {
        id: String,
        label: String,
    },
    DealWorkspaceParticipantRoleChanged(String),
    DealWorkspaceAddParticipantRequested,
    DealWorkspaceOtherRoleChanged {
        participant_id: String,
        value: String,
    },
    DealWorkspaceUpdateOtherRequested {
        participant_id: String,
    },
    DealWorkspaceEndOtherRequested {
        participant_id: String,
    },
    DealWorkspaceStructuralRoleChanged(String),
    DealWorkspaceStructuralQueryChanged(String),
    DealWorkspaceStructuralPersonSelected {
        id: String,
        label: String,
    },
    DealWorkspaceStructuralOwnerChanged(String),
    DealWorkspaceSetStructuralRequested,
    DealWorkspaceEndStructuralRequested {
        participant_id: String,
    },
    DealWorkspaceCommandAnswered(Result<RecordId, ApiError>),
}

fn read(ctx: &ScreenCtx) -> Cmd<Msg> {
    Cmd::request(
        DealsRead {
            deal_id: ctx.id.clone(),
        },
        Msg::Loaded,
    )
}

fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
    (
        Model {
            read: Remote::Loading,
            ..Model::default()
        },
        read(ctx),
    )
}

/// A search for people once two characters are typed; fewer clears the list.
fn search(
    people: &mut Vec<crate::model::PortalDealPersonCandidate>,
    searching: &mut bool,
    purpose: &'static str,
    query: &str,
) -> Cmd<Msg> {
    let query = query.trim().to_string();
    people.clear();
    if query.chars().count() < 2 {
        *searching = false;
        return Cmd::none();
    }
    *searching = true;
    Cmd::request(
        DealPeopleSearch {
            query: query.clone(),
        },
        move |answer| Msg::PeopleLoaded {
            purpose,
            query,
            answer,
        },
    )
}

/// Send one workspace command, if the user may and none is in flight.
fn command(
    model: &mut Model,
    ctx: &ScreenCtx,
    busy_action: impl Into<String>,
    command: PortalDealCommand,
) -> Cmd<Msg> {
    let action = match &command {
        PortalDealCommand::CreateShowing { .. }
        | PortalDealCommand::ScheduleShowing { .. }
        | PortalDealCommand::CancelShowing { .. }
        | PortalDealCommand::CompleteShowing { .. } => "showing.write",
        _ => "deal.write",
    };
    if !ctx.can(action) || model.deal_workspace.busy_action.is_some() {
        return Cmd::none();
    }
    let Some(deal_id) = ctx.id.clone() else {
        model.error = Some("This workspace is missing its deal identifier.".into());
        return Cmd::none();
    };
    model.deal_workspace.busy_action = Some(busy_action.into());
    model.error = None;
    Cmd::request(
        DealWorkspaceCommand { deal_id, command },
        Msg::DealWorkspaceCommandAnswered,
    )
}

fn workspace(model: &Model) -> Option<&crate::model::PortalDealWorkspace> {
    model
        .read
        .loaded()
        .and_then(|deals| deals.workspace.as_ref())
}

fn trimmed(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
    let create = &mut model.deal_create;
    let ws = &mut model.deal_workspace;
    match msg {
        Msg::Loaded(answer) => {
            model.refreshing = false;
            match answer.and_then(|page| {
                page.deals
                    .ok_or_else(|| ApiError::decode("The answer had no contracts in it."))
            }) {
                Ok(deals) => model.read = Remote::Loaded(deals),
                // A failed refresh keeps the workspace the user was reading and says why.
                Err(error) if model.read.loaded().is_some() => model.error = Some(error.message),
                Err(error) => model.read = Remote::Failed(error),
            }
        }
        Msg::FilterChanged(filter) => model.controls.filter = Some(filter),

        // ---- the portfolio's create panel ---------------------------------------------------------------------------
        Msg::DealCreateToggled => {
            create.open = !create.open;
            model.error = None;
        }
        Msg::DealCreatePropertyChanged(value) => {
            create.property_id = value;
            model.error = None;
        }
        Msg::DealCreateClientQueryChanged(value) => {
            // Typing past a chosen client un-chooses it: the id and the text must name the same person.
            if value != create.client_label {
                create.client_person_id.clear();
                create.client_label.clear();
            }
            create.client_query = value;
            model.error = None;
            let query = create.client_query.clone();
            return search(&mut create.people, &mut create.searching, "client", &query);
        }
        Msg::DealCreateClientSelected { id, label } => {
            create.client_person_id = id;
            create.client_label = label.clone();
            create.client_query = label;
            create.people.clear();
            create.searching = false;
            model.error = None;
        }
        Msg::DealCreateOwnerChanged(value) => {
            create.owner_user_id = value;
            model.error = None;
        }
        Msg::DealCreateNotesChanged(value) => {
            create.notes = value;
            model.error = None;
        }
        Msg::DealCreateRequested => {
            if create.submitting || !ctx.can("deal.write") {
                return Cmd::none();
            }
            let Some(property_id) = trimmed(&create.property_id) else {
                model.error = Some("Choose a property first.".into());
                return Cmd::none();
            };
            let Some(client_person_id) = trimmed(&create.client_person_id) else {
                model.error = Some("Select an existing client person first.".into());
                return Cmd::none();
            };
            create.submitting = true;
            model.error = None;
            return Cmd::request(
                DealCreate {
                    property_id,
                    client_person_id,
                    owner_user_id: trimmed(&create.owner_user_id),
                    notes: trimmed(&create.notes),
                },
                Msg::DealCreated,
            );
        }
        Msg::DealCreated(answer) => {
            create.submitting = false;
            match answer {
                Ok(created) => {
                    *create = DealCreateState::default();
                    return Cmd::navigate(format!("/portal/deals/{}", created.id));
                }
                Err(error) => model.error = Some(error.message),
            }
        }

        Msg::PeopleLoaded {
            purpose,
            query,
            answer,
        } => {
            let (current, people, searching) = match purpose {
                "client" => (
                    &create.client_query,
                    &mut create.people,
                    &mut create.searching,
                ),
                "participant" => (
                    &ws.participant_query,
                    &mut ws.participant_people,
                    &mut ws.participant_searching,
                ),
                _ => (
                    &ws.structural_query,
                    &mut ws.structural_people,
                    &mut ws.structural_searching,
                ),
            };
            if current.trim() != query {
                return Cmd::none();
            }
            *searching = false;
            match answer {
                Ok(found) => *people = found.people,
                Err(error) => model.error = Some(error.message),
            }
        }

        // ---- the workspace: tasks -----------------------------------------------------------------------------------
        Msg::DealWorkspaceTaskTitleChanged(value) => {
            ws.task_title = value;
            model.error = None;
        }
        Msg::DealWorkspaceTaskDetailChanged(value) => {
            ws.task_detail = value;
            model.error = None;
        }
        Msg::DealWorkspaceTaskDueChanged(value) => {
            ws.task_due_at = value;
            model.error = None;
        }
        Msg::DealWorkspaceCreateTaskRequested => {
            let Some(title) = trimmed(&ws.task_title) else {
                model.error = Some("Task title is required.".into());
                return Cmd::none();
            };
            let detail = trimmed(&ws.task_detail);
            let due_at = trimmed(&ws.task_due_at);
            return command(
                model,
                ctx,
                "task:create",
                PortalDealCommand::CreateTask {
                    title,
                    detail,
                    due_at,
                },
            );
        }
        Msg::DealWorkspaceCompleteTaskRequested { task_id } => {
            return command(
                model,
                ctx,
                format!("task:complete:{task_id}"),
                PortalDealCommand::CompleteTask { task_id },
            )
        }

        // ---- offers -------------------------------------------------------------------------------------------------
        Msg::DealWorkspaceOfferAmountChanged { key, value } => {
            if value.is_empty() {
                ws.offer_amounts.remove(&key);
            } else {
                ws.offer_amounts.insert(key, value);
            }
            model.error = None;
        }
        Msg::DealWorkspaceSubmitOfferRequested { parent_offer_id } => {
            let key = parent_offer_id.as_deref().unwrap_or("root").to_string();
            let Some(amount) = ws.offer_amounts.get(&key).and_then(|value| trimmed(value)) else {
                model.error = Some("Offer amount is required.".into());
                return Cmd::none();
            };
            let Some(person_id) = workspace(model)
                .and_then(|workspace| workspace.client.as_ref())
                .map(|client| client.id.clone())
            else {
                model.error = Some("This deal does not have an active client.".into());
                return Cmd::none();
            };
            return command(
                model,
                ctx,
                format!("offer:submit:{key}"),
                PortalDealCommand::SubmitOffer {
                    person_id,
                    amount,
                    parent_offer_id,
                },
            );
        }
        Msg::DealWorkspaceWithdrawOfferRequested { offer_id } => {
            return command(
                model,
                ctx,
                format!("offer:withdraw:{offer_id}"),
                PortalDealCommand::WithdrawOffer { offer_id },
            )
        }
        Msg::DealWorkspaceRejectOfferRequested { offer_id } => {
            return command(
                model,
                ctx,
                format!("offer:reject:{offer_id}"),
                PortalDealCommand::RejectOffer { offer_id },
            )
        }

        // ---- showings -----------------------------------------------------------------------------------------------
        Msg::DealWorkspaceCreateShowingRequested => {
            let workspace = workspace(model);
            let person_id = workspace
                .and_then(|workspace| workspace.client.as_ref())
                .map(|client| client.id.clone());
            let property_id = workspace
                .and_then(|workspace| workspace.property.as_ref())
                .map(|property| property.id.clone());
            let Some(person_id) = person_id else {
                model.error = Some("This deal does not have an active client.".into());
                return Cmd::none();
            };
            return command(
                model,
                ctx,
                "showing:create",
                PortalDealCommand::CreateShowing {
                    person_id,
                    property_id,
                },
            );
        }
        Msg::DealWorkspaceShowingTimeChanged { showing_id, value } => {
            if value.is_empty() {
                ws.showing_times.remove(&showing_id);
            } else {
                ws.showing_times.insert(showing_id, value);
            }
            model.error = None;
        }
        Msg::DealWorkspaceScheduleShowingRequested { showing_id } => {
            let Some(scheduled_at) = ws
                .showing_times
                .get(&showing_id)
                .and_then(|value| trimmed(value))
            else {
                model.error = Some("Choose a showing date and time first.".into());
                return Cmd::none();
            };
            return command(
                model,
                ctx,
                format!("showing:schedule:{showing_id}"),
                PortalDealCommand::ScheduleShowing {
                    showing_id,
                    scheduled_at,
                },
            );
        }
        Msg::DealWorkspaceCancelShowingRequested { showing_id } => {
            return command(
                model,
                ctx,
                format!("showing:cancel:{showing_id}"),
                PortalDealCommand::CancelShowing { showing_id },
            )
        }
        Msg::DealWorkspaceCompleteShowingRequested { showing_id } => {
            return command(
                model,
                ctx,
                format!("showing:complete:{showing_id}"),
                PortalDealCommand::CompleteShowing { showing_id },
            )
        }

        // ---- other participants -------------------------------------------------------------------------------------
        Msg::DealWorkspaceParticipantQueryChanged(value) => {
            if value != ws.participant_label {
                ws.participant_person_id.clear();
                ws.participant_label.clear();
            }
            ws.participant_query = value.clone();
            model.error = None;
            return search(
                &mut ws.participant_people,
                &mut ws.participant_searching,
                "participant",
                &value,
            );
        }
        Msg::DealWorkspaceParticipantSelected { id, label } => {
            ws.participant_person_id = id;
            ws.participant_label = label.clone();
            ws.participant_query = label;
            ws.participant_people.clear();
            ws.participant_searching = false;
            model.error = None;
        }
        Msg::DealWorkspaceParticipantRoleChanged(value) => {
            ws.participant_role_label = value;
            model.error = None;
        }
        Msg::DealWorkspaceAddParticipantRequested => {
            let Some(person_id) = trimmed(&ws.participant_person_id) else {
                model.error = Some("Select an existing person first.".into());
                return Cmd::none();
            };
            let Some(role_label) = trimmed(&ws.participant_role_label) else {
                model.error = Some("Enter a participant role label.".into());
                return Cmd::none();
            };
            return command(
                model,
                ctx,
                "participant:add",
                PortalDealCommand::AddOtherParticipant {
                    person_id,
                    role_label,
                },
            );
        }
        Msg::DealWorkspaceOtherRoleChanged {
            participant_id,
            value,
        } => {
            if value.is_empty() {
                ws.other_role_labels.remove(&participant_id);
            } else {
                ws.other_role_labels.insert(participant_id, value);
            }
            model.error = None;
        }
        Msg::DealWorkspaceUpdateOtherRequested { participant_id } => {
            let Some(role_label) = ws
                .other_role_labels
                .get(&participant_id)
                .and_then(|value| trimmed(value))
            else {
                model.error = Some("Enter a new role label.".into());
                return Cmd::none();
            };
            return command(
                model,
                ctx,
                format!("participant:update:{participant_id}"),
                PortalDealCommand::UpdateOtherParticipant {
                    participant_id,
                    role_label,
                },
            );
        }
        Msg::DealWorkspaceEndOtherRequested { participant_id } => {
            return command(
                model,
                ctx,
                format!("participant:end:{participant_id}"),
                PortalDealCommand::EndOtherParticipant { participant_id },
            )
        }

        // ---- structural participants (client, owner, seller) --------------------------------------------------------
        Msg::DealWorkspaceStructuralRoleChanged(role) => {
            ws.structural_role = role;
            ws.structural_query.clear();
            ws.structural_person_id.clear();
            ws.structural_label.clear();
            ws.structural_owner_user_id.clear();
            ws.structural_people.clear();
            ws.structural_searching = false;
            model.error = None;
        }
        Msg::DealWorkspaceStructuralQueryChanged(value) => {
            if value != ws.structural_label {
                ws.structural_person_id.clear();
                ws.structural_label.clear();
            }
            ws.structural_query = value.clone();
            model.error = None;
            return search(
                &mut ws.structural_people,
                &mut ws.structural_searching,
                "structural",
                &value,
            );
        }
        Msg::DealWorkspaceStructuralPersonSelected { id, label } => {
            ws.structural_person_id = id;
            ws.structural_label = label.clone();
            ws.structural_query = label;
            ws.structural_people.clear();
            ws.structural_searching = false;
            model.error = None;
        }
        Msg::DealWorkspaceStructuralOwnerChanged(value) => {
            ws.structural_owner_user_id = value;
            model.error = None;
        }
        Msg::DealWorkspaceSetStructuralRequested => {
            let role = ws.structural_role.clone();
            if !matches!(role.as_str(), "client" | "owner" | "seller") {
                model.error = Some("Choose client, owner, or seller first.".into());
                return Cmd::none();
            }
            let (person_id, user_id) = if role == "owner" {
                let Some(id) = trimmed(&ws.structural_owner_user_id) else {
                    model.error = Some("Choose an owner user first.".into());
                    return Cmd::none();
                };
                (None, Some(id))
            } else {
                let Some(id) = trimmed(&ws.structural_person_id) else {
                    model.error = Some("Select an existing person first.".into());
                    return Cmd::none();
                };
                (Some(id), None)
            };
            return command(
                model,
                ctx,
                format!("structural:set:{role}"),
                PortalDealCommand::SetStructuralParticipant {
                    role,
                    person_id,
                    user_id,
                },
            );
        }
        Msg::DealWorkspaceEndStructuralRequested { participant_id } => {
            return command(
                model,
                ctx,
                format!("structural:end:{participant_id}"),
                PortalDealCommand::EndStructuralParticipant { participant_id },
            )
        }

        Msg::DealWorkspaceCommandAnswered(answer) => {
            let completed = ws.busy_action.take().unwrap_or_default();
            if let Err(error) = answer {
                // Refused: the form keeps what was typed, so it can be corrected and sent again.
                model.error = Some(error.message);
                return Cmd::none();
            }
            if completed == "task:create" {
                ws.task_title.clear();
                ws.task_detail.clear();
                ws.task_due_at.clear();
            } else if let Some(key) = completed.strip_prefix("offer:submit:") {
                ws.offer_amounts.remove(key);
            } else if completed == "participant:add" {
                ws.participant_query.clear();
                ws.participant_person_id.clear();
                ws.participant_label.clear();
                ws.participant_role_label.clear();
                ws.participant_people.clear();
            } else if completed.starts_with("structural:set:") {
                ws.structural_role.clear();
                ws.structural_query.clear();
                ws.structural_person_id.clear();
                ws.structural_label.clear();
                ws.structural_owner_user_id.clear();
                ws.structural_people.clear();
            } else if let Some(id) = completed.strip_prefix("participant:update:") {
                ws.other_role_labels.remove(id);
            } else if let Some(id) = completed.strip_prefix("showing:schedule:") {
                ws.showing_times.remove(id);
            }
            model.error = None;
            model.refreshing = true;
            return read(ctx);
        }
    }
    Cmd::none()
}

/// What the views read. Read-only.
pub struct Vm<'a> {
    data: &'a PortalDealsPage,
    pub deal_create: &'a DealCreateState,
    pub deal_workspace: &'a DealWorkspaceState,
    pub controls: &'a Controls,
    /// A refresh is in flight.
    pub loading: bool,
    ctx: &'a ScreenCtx,
}

impl<'a> Vm<'a> {
    pub fn can(&self, action: &str) -> bool {
        self.ctx.can(action)
    }
}

fn frame(
    model: &Model,
    ctx: &ScreenCtx,
    link: &Link<Msg>,
    body: fn(&Vm<'_>, &Callback<Msg>) -> Html,
) -> Html {
    let on_msg = link.callback(|msg: Msg| msg);
    html! {
        <div class="space-y-4">
            if let Some(error) = &model.error {
                <div class="rounded-md border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm" role="alert">
                    { error.clone() }
                </div>
            }
            { template::remote(&model.read, "the contracts", |data| body(
                &Vm {
                    data,
                    deal_create: &model.deal_create,
                    deal_workspace: &model.deal_workspace,
                    controls: &model.controls,
                    loading: model.refreshing,
                    ctx,
                },
                &on_msg,
            )) }
        </div>
    }
}

pub struct Deals;

impl Screen for Deals {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        init(ctx)
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        update(model, msg, ctx)
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        frame(model, ctx, link, view::portfolio)
    }
}

pub struct DealRecord;

impl Screen for DealRecord {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        init(ctx)
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        update(model, msg, ctx)
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        frame(model, ctx, link, view::deal_workspace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PortalEntitlements;
    use serde_json::json;

    fn ctx(id: Option<&str>) -> ScreenCtx {
        ScreenCtx {
            id: id.map(str::to_owned),
            grants: Some(PortalEntitlements {
                account_type: "internal".into(),
                security_level: "USER".into(),
                is_root: false,
                entitlement_codes: vec!["deal.write".into()],
            }),
            ..ScreenCtx::default()
        }
    }

    fn opened(ctx: &ScreenCtx, answer: serde_json::Value) -> Model {
        let (mut model, cmd) = init(ctx);
        let request = cmd.into_requests().remove(0);
        update(&mut model, request.respond(Ok(answer)), ctx);
        model
    }

    #[test]
    fn the_portfolio_and_a_workspace_read_their_pages() {
        let path = |ctx: &ScreenCtx| init(ctx).1.into_requests().remove(0).path;
        assert_eq!(path(&ctx(None)), "/api/portal/rust-ui/deals?screen=deals");
        assert_eq!(
            path(&ctx(Some("deal-7"))),
            "/api/portal/rust-ui/deals?screen=deal-record&scope=deal-7"
        );
    }

    #[test]
    fn a_deal_needs_a_property_and_a_chosen_client_and_opens_its_workspace() {
        let ctx = ctx(None);
        let mut model = opened(&ctx, json!({ "deals": {} }));
        assert!(update(&mut model, Msg::DealCreateRequested, &ctx)
            .into_requests()
            .is_empty());
        assert_eq!(model.error.as_deref(), Some("Choose a property first."));

        update(
            &mut model,
            Msg::DealCreatePropertyChanged("p-1".into()),
            &ctx,
        );
        let search = update(
            &mut model,
            Msg::DealCreateClientQueryChanged("Ali".into()),
            &ctx,
        )
        .into_requests()
        .remove(0);
        assert_eq!(search.path, "/api/portal/rust-ui/deals?peopleSearch=Ali");
        // Typing on makes the first answer stale: it is dropped, not shown under the new text.
        update(
            &mut model,
            Msg::DealCreateClientQueryChanged("Alic".into()),
            &ctx,
        );
        update(
            &mut model,
            search.respond(Ok(json!({ "people": [{ "id": "x" }] }))),
            &ctx,
        );
        assert!(model.deal_create.people.is_empty());

        update(
            &mut model,
            Msg::DealCreateClientSelected {
                id: "c-1".into(),
                label: "Alice".into(),
            },
            &ctx,
        );
        let create = update(&mut model, Msg::DealCreateRequested, &ctx)
            .into_requests()
            .remove(0);
        assert_eq!(create.body.clone().unwrap()["clientPersonId"], "c-1");
        match update(
            &mut model,
            create.respond(Ok(json!({ "id": "deal-9" }))),
            &ctx,
        ) {
            Cmd::Navigate(path) => assert_eq!(path, "/portal/deals/deal-9"),
            _ => panic!("a created deal opens its workspace"),
        }
    }

    #[test]
    fn a_workspace_command_runs_alone_clears_its_own_form_and_rereads() {
        let ctx = ctx(Some("deal-7"));
        let mut model = opened(&ctx, json!({ "deals": { "workspace": {} } }));
        update(
            &mut model,
            Msg::DealWorkspaceTaskTitleChanged("Call notario".into()),
            &ctx,
        );
        update(
            &mut model,
            Msg::DealWorkspaceParticipantRoleChanged("Lender".into()),
            &ctx,
        );
        let request = update(&mut model, Msg::DealWorkspaceCreateTaskRequested, &ctx)
            .into_requests()
            .remove(0);
        assert_eq!(request.path, "/api/portal/rust-ui/deals?scope=deal-7");
        assert_eq!(request.body.clone().unwrap()["action"], "createTask");
        assert!(
            update(
                &mut model,
                Msg::DealWorkspaceCompleteTaskRequested {
                    task_id: "t".into()
                },
                &ctx
            )
            .into_requests()
            .is_empty(),
            "one command at a time"
        );
        let reread = update(
            &mut model,
            request.respond(Ok(json!({ "id": "t-1" }))),
            &ctx,
        )
        .into_requests()
        .remove(0);
        assert_eq!(
            reread.path,
            "/api/portal/rust-ui/deals?screen=deal-record&scope=deal-7"
        );
        assert!(model.deal_workspace.task_title.is_empty());
        assert_eq!(
            model.deal_workspace.participant_role_label, "Lender",
            "only the sending form clears"
        );
        assert!(model.refreshing);

        update(
            &mut model,
            Msg::DealWorkspaceTaskTitleChanged("Again".into()),
            &ctx,
        );
        let request = update(&mut model, Msg::DealWorkspaceCreateTaskRequested, &ctx)
            .into_requests()
            .remove(0);
        update(
            &mut model,
            request.respond(Err(ApiError::network("Deal is closed."))),
            &ctx,
        );
        assert_eq!(model.error.as_deref(), Some("Deal is closed."));
        assert_eq!(
            model.deal_workspace.task_title, "Again",
            "a refusal keeps what was typed"
        );
        assert!(model.deal_workspace.busy_action.is_none());
    }

    #[test]
    fn showings_need_the_showing_entitlement() {
        let ctx = ctx(Some("deal-7"));
        let mut model = opened(
            &ctx,
            json!({ "deals": { "workspace": { "client": { "id": "c-1" } } } }),
        );
        assert!(
            update(&mut model, Msg::DealWorkspaceCreateShowingRequested, &ctx)
                .into_requests()
                .is_empty(),
            "deal.write alone does not book showings"
        );
    }
}
