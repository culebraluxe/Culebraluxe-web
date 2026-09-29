//! `/portal/system-health` — the platform's health snapshot and the workflow engine's diagnostics.
//!
//! One read on open (the snapshot, with the instance list). Opening an instance's row reads that instance's detail —
//! one row at a time, and a click that closes the open row reads nothing. An answer for a row that is no longer the one
//! open is dropped: a row must never name one instance and describe another. A failed instance read is told inside its
//! row, not over the page, because the page's own snapshot is still good.

use yew::prelude::*;

use crate::app::api::PortalScreenPage;
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{
    PortalEnvironmentReadiness, PortalPage, PortalSystemHealthPage, PortalSystemHealthSnapshot,
    PortalWorkflowAnomaly, PortalWorkflowCorrelation, PortalWorkflowDefinition,
    PortalWorkflowDiagnosticEvent, PortalWorkflowDiagnostics, PortalWorkflowInstance,
    PortalWorkflowInstanceDetail, PortalWorkflowJob, PortalWorkflowTask, PortalWorkflowToken,
    WorkflowDiagnosticsState,
};
mod snapshot;
mod workflow;
mod workflow_parts;
mod instance_detail;
#[allow(unused_imports)]
pub(super) use snapshot::*;
#[allow(unused_imports)]
pub(super) use workflow::*;
#[allow(unused_imports)]
pub(super) use workflow_parts::*;
#[allow(unused_imports)]
pub(super) use instance_detail::*;


pub struct SystemHealth;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalSystemHealthPage>,
    /// The open instance row: which one, whether its detail is on its way, the detail, or why it could not be read.
    pub workflow: WorkflowDiagnosticsState,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    /// A row was clicked: open it (and read its detail), or close it if it is the open one.
    WorkflowInstanceToggled {
        instance_id: String,
    },
    DetailLoaded {
        instance_id: String,
        answer: Result<PortalPage, ApiError>,
    },
}

fn health(page: PortalPage) -> Option<PortalSystemHealthPage> {
    page.support.and_then(|support| support.system_health)
}

impl Screen for SystemHealth {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
                ..Model::default()
            },
            Cmd::request(PortalScreenPage::of("system-health"), Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded(answer) => {
                model.read = Remote::from_result(answer.and_then(|page| {
                    health(page)
                        .ok_or_else(|| ApiError::decode("The answer had no health snapshot in it."))
                }));
                Cmd::none()
            }
            Msg::WorkflowInstanceToggled { instance_id } => {
                if model.workflow.selected_instance.as_deref() == Some(instance_id.as_str()) {
                    model.workflow = WorkflowDiagnosticsState::default();
                    return Cmd::none();
                }
                // Read again on every open: the engine moves, and a detail from the last look is a stale answer.
                model.workflow = WorkflowDiagnosticsState {
                    selected_instance: Some(instance_id.clone()),
                    loading_instance: Some(instance_id.clone()),
                    ..WorkflowDiagnosticsState::default()
                };
                Cmd::request(
                    PortalScreenPage::scoped("system-health", instance_id.clone()),
                    move |answer| Msg::DetailLoaded {
                        instance_id,
                        answer,
                    },
                )
            }
            Msg::DetailLoaded {
                instance_id,
                answer,
            } => {
                if model.workflow.loading_instance.as_deref() != Some(instance_id.as_str()) {
                    return Cmd::none();
                }
                model.workflow.loading_instance = None;
                let detail = answer.map(|page| {
                    health(page)
                        .and_then(|health| health.diagnostics.detail)
                        .filter(|detail| detail.instance_id == instance_id)
                });
                match detail {
                    Ok(Some(detail)) => {
                        model.workflow.detail = Some(detail);
                        model.workflow.error = None;
                    }
                    Ok(None) => {
                        model.workflow.error =
                            Some(format!("No detail found for instance {instance_id}."))
                    }
                    Err(error) => model.workflow.error = Some(error.message),
                }
                Cmd::none()
            }
        }
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        SystemHealth.body(model, &link.callback(|msg: Msg| msg))
    }
}

/// The navy panel the live screen used for every section.
const PANEL: &str = "rounded-[var(--portal-panel-radius)] portal-glass-panel p-6";

/// The severity/status flag's shared shape: a small pill, so a flag never reads as a button.
const PILL: &str =
    "inline-flex items-center rounded-full border px-2.5 py-0.5 text-[10px] font-medium uppercase tracking-[0.14em]";

/// The receipt flag's shape: the same pill with the tighter padding the live receipt badge used.
const RECEIPT_PILL: &str =
    "inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.12em]";

impl SystemHealth {
    fn body(&self, model: &Model, on_msg: &Callback<Msg>) -> Html {
        html! {
            <div>
                { self.heading() }
                { template::remote(&model.read, "the health snapshot", |read| html! {
                    <>
                        { self.metrics(&read.health) }
                        { self.recent_activity(&read.health) }
                        { self.data_quality(&read.health) }
                        { self.transaction_quality(&read.health) }
                        { self.write_invariants(&read.health) }
                        { self.security_model(&read.health) }
                        { self.environment(&read.environment) }
                        { self.workflow_diagnostics(model, &read.diagnostics, on_msg) }
                    </>
                }) }
            </div>
        }
    }

    fn heading(&self) -> Html {
        html! {
            <div class="mb-8">
                <p class="text-xs font-light uppercase tracking-[0.28em] text-black/40">{"Portal"}</p>
                <h1 class="mt-3 font-serif text-4xl font-light leading-[1.1]">{"System Health"}</h1>
                <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
                    {"Operational signals across intake, tasks, deals, properties and relationship data — read-only."}
                </p>
            </div>
        }
    }

    /// The four headline counts. The live screen folded the second figure of each into the detail line ("3 overdue",
    /// "2 under contract"), which is where they stay.
    fn metrics(&self, health: &PortalSystemHealthSnapshot) -> Html {
        html! {
            <section class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                { metric(
                    "Needs Review",
                    &health.unresolved_intake_count.to_string(),
                    "Unresolved intake submissions",
                ) }
                { metric(
                    "Open Tasks",
                    &health.open_task_count.to_string(),
                    &format!("{} overdue", health.overdue_task_count),
                ) }
                { metric(
                    "Active Deals",
                    &health.active_deal_count.to_string(),
                    &format!("{} under contract", health.under_contract_count),
                ) }
                { metric(
                    "Active Properties",
                    &health.active_property_count.to_string(),
                    "Active inventory",
                ) }
            </section>
        }
    }

    fn recent_activity(&self, health: &PortalSystemHealthSnapshot) -> Html {
        html! {
            <section class={classes!(PANEL, "mt-6")}>
                <h2 class="font-serif text-2xl font-light">{"Recent Activity"}</h2>
                <div class="mt-4 grid gap-6 md:grid-cols-2">
                    { detail(
                        "Last Interaction",
                        &health
                            .recent_interaction_at_label
                            .clone()
                            .unwrap_or_else(|| "None recorded".to_string()),
                    ) }
                    { detail("Interactions (7 days)", &health.interactions_last7_days.to_string()) }
                </div>
            </section>
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_real_answer_decodes_and_one_row_opens_at_a_time() {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = SystemHealth::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(
            request.path,
            "/api/portal/rust-ui/page?screen=system-health"
        );
        let answer: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/portal-page-system-health.json"
        ))
        .unwrap();
        SystemHealth::update(&mut model, request.respond(Ok(answer)), &ctx);
        assert!(model.read.loaded().is_some(), "the real payload decodes");

        let open = SystemHealth::update(
            &mut model,
            Msg::WorkflowInstanceToggled {
                instance_id: "i-1".into(),
            },
            &ctx,
        );
        assert_eq!(
            open.into_requests().remove(0).path,
            "/api/portal/rust-ui/page?screen=system-health&scope=i-1"
        );
        // The operator opens another row before the first answer lands: the first answer is dropped.
        let second = SystemHealth::update(
            &mut model,
            Msg::WorkflowInstanceToggled {
                instance_id: "i-2".into(),
            },
            &ctx,
        );
        let stale = json!({ "support": { "systemHealth": { "diagnostics": { "detail": { "instanceId": "i-1" } } } } });
        SystemHealth::update(
            &mut model,
            Msg::DetailLoaded {
                instance_id: "i-1".into(),
                answer: Ok(serde_json::from_value(stale).unwrap()),
            },
            &ctx,
        );
        assert!(
            model.workflow.detail.is_none()
                && model.workflow.loading_instance.as_deref() == Some("i-2")
        );
        let fresh = json!({ "support": { "systemHealth": { "diagnostics": { "detail": { "instanceId": "i-2" } } } } });
        SystemHealth::update(
            &mut model,
            second.into_requests().remove(0).respond(Ok(fresh)),
            &ctx,
        );
        assert_eq!(
            model
                .workflow
                .detail
                .as_ref()
                .map(|detail| detail.instance_id.as_str()),
            Some("i-2")
        );

        // Closing the open row reads nothing.
        assert!(SystemHealth::update(
            &mut model,
            Msg::WorkflowInstanceToggled {
                instance_id: "i-2".into()
            },
            &ctx
        )
        .into_requests()
        .is_empty());
        assert_eq!(model.workflow, WorkflowDiagnosticsState::default());
    }

    #[test]
    fn a_failed_row_read_is_told_in_the_row_and_the_page_stays() {
        let ctx = ScreenCtx::default();
        let mut model = Model {
            read: Remote::Loaded(PortalSystemHealthPage::default()),
            ..Model::default()
        };
        SystemHealth::update(
            &mut model,
            Msg::WorkflowInstanceToggled {
                instance_id: "i-9".into(),
            },
            &ctx,
        );
        SystemHealth::update(
            &mut model,
            Msg::DetailLoaded {
                instance_id: "i-9".into(),
                answer: Err(ApiError::network("offline")),
            },
            &ctx,
        );
        assert_eq!(model.workflow.error.as_deref(), Some("offline"));
        assert!(model.read.loaded().is_some());
    }
}
