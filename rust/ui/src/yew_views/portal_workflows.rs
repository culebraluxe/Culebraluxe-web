//! /portal/workflows — the real workflow-card screen, not generic cells.

use yew::prelude::*;

use crate::model::{Msg, PortalWorkflowSummary};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct WorkflowsProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Workflows;

impl Component for Workflows {
    type Message = ();
    type Properties = WorkflowsProps;

    fn create(_ctx: &Context<Self>) -> Self { Self }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("workflows").expect("workflows screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { self.body(&props.model) }
            </PortalShell>
        }
    }
}

impl Workflows {
    fn body(&self, model: &crate::model::Model) -> Html {
        let payload = model.page.as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.workflows.as_ref());

        html! {
            <div>
                <header class="mb-8">
                    <h1 class="font-serif text-3xl font-light text-[var(--portal-navy)]">{"Workflows"}</h1>
                    <p class="mt-1 text-sm font-light text-black/55">
                        {"Transaction orchestration for deals in motion."}
                    </p>
                </header>
                if !model.loading {
                    {
                        match payload {
                            None => html! {
                                <section class="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-16 text-center">
                                    <h2 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"Workflow data unavailable"}</h2>
                                </section>
                            },
                            Some(payload) if !payload.configured => html! {
                                <section class="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-16 text-center">
                                    <h2 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"Workflow runtime not ready"}</h2>
                                    <p class="mx-auto mt-3 max-w-lg text-sm font-light leading-6 text-black/55">
                                        {"The workflow runtime shares the CulebraLuxe database. Apply the unified activation script to enable transaction workflows."}
                                    </p>
                                </section>
                            },
                            Some(payload) if payload.items.is_empty() => html! {
                                <section class="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-16 text-center">
                                    <h2 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"No transaction workflows yet"}</h2>
                                    <p class="mx-auto mt-3 max-w-md text-sm font-light text-black/55">
                                        {"A workflow instance is created when an offer is accepted and the transaction moves into contract preparation."}
                                    </p>
                                </section>
                            },
                            Some(payload) => html! {
                                <div class="grid gap-4 xl:grid-cols-2">
                                    { for payload.items.iter().map(workflow_card) }
                                </div>
                            },
                        }
                    }
                }
            </div>
        }
    }
}

fn pill(summary: &PortalWorkflowSummary) -> (&'static str, String) {
    if summary.outcome.as_deref() == Some("cancelled") {
        return ("bg-black/5 text-black/55", "Cancelled".into());
    }
    if matches!(summary.outcome.as_deref(), Some("failed") | Some("conflict")) || summary.status == "error" {
        return ("bg-red-50 text-red-700", summary.outcome.clone().unwrap_or_else(|| summary.status.clone()));
    }
    if summary.outcome.as_deref() == Some("completed") {
        return ("bg-emerald-50 text-emerald-700", "Closed".into());
    }
    ("bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]", summary.status.clone())
}

fn workflow_card(summary: &PortalWorkflowSummary) -> Html {
    let (pill_class, pill_label) = pill(summary);
    let milestone = if summary.active_milestones.is_empty() {
        summary.responsible_party.clone().unwrap_or_else(|| "No active milestone".into())
    } else {
        summary.active_milestones.join(", ")
    };
    html! {
        <a href={format!("/portal/workflows/{}", summary.instance_id)}
            class="block rounded-[var(--portal-panel-radius)] portal-glass-panel p-6 transition-colors hover:border-[var(--portal-blue-gray)]/50">
            <div class="flex items-start justify-between gap-4">
                <div>
                    <div class="text-[10px] font-light uppercase tracking-[0.22em] text-black/45">
                        { format!("{} · v{}", summary.workflow_name, summary.workflow_version) }
                    </div>
                    <h2 class="mt-2 font-serif text-xl font-light text-[var(--portal-navy)]">
                        { summary.property_name.clone().unwrap_or_else(|| "Transaction".into()) }
                    </h2>
                </div>
                <span class={format!("rounded-full px-3 py-1 text-xs font-light capitalize {pill_class}")}>
                    { pill_label }
                </span>
            </div>
            <div class="mt-5 flex flex-wrap items-center gap-x-5 gap-y-2 text-sm font-light text-black/60">
                <span>{ milestone }</span>
                if summary.blocker_count > 0 {
                    <span class="text-[var(--portal-blue-gray)]">
                        { format!("{} blocker{}", summary.blocker_count, if summary.blocker_count == 1 { "" } else { "s" }) }
                    </span>
                }
                if summary.open_task_count > 0 {
                    <span>
                        { format!("{} open task{}", summary.open_task_count, if summary.open_task_count == 1 { "" } else { "s" }) }
                    </span>
                }
            </div>
        </a>
    }
}
