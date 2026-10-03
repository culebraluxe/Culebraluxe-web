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
mod update;
#[allow(unused_imports)]
pub(super) use update::*;

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
    DealWorkspaceOfferTermChanged {
        key: String,
        field: &'static str,
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
