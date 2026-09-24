//! /portal/tech/flight-recorder — recent workflow executions that can be opened in the recorder.
//!
//! This is intentionally a thin list over the existing Rust workflow projection. The recorder itself is the
//! [instanceId] screen; this page is only the front door.

use yew::prelude::*;

use crate::model::{Msg, PortalWorkflowSummary};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct FlightRecorderListProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct FlightRecorderList;

impl Component for FlightRecorderList {
    type Message = ();
    type Properties = FlightRecorderListProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("tech-flight-recorder")
            .expect("flight recorder list screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { body(&props.model) }
            </PortalShell>
        }
    }
}

fn body(model: &crate::model::Model) -> Html {
    let payload = model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.workflows.as_ref());

    html! {
        <div>
            <header class="mb-6 flex flex-wrap items-end justify-between gap-3">
                <div>
                    <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">
                        {"TECH / INTROSPECTION"}
                    </p>
                    <h1 class="mt-1 font-serif text-3xl font-light text-[var(--portal-navy)]">
                        {"Flight Recorder"}
                    </h1>
                    <p class="mt-1 max-w-3xl text-sm font-light leading-6 text-black/55">
                        {"Open a real workflow execution to inspect its timeline, master graph, causality, system swimlanes and raw trace evidence."}
                    </p>
                </div>
                <a
                    href="/portal/tech"
                    class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]/60 hover:text-[var(--portal-navy)]"
                >
                    {"← Forge Cockpit"}
                </a>
            </header>

            if !model.loading {
                {
                    match payload {
                        None => html! {
                            <section class="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-14 text-center">
                                <h2 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"Flight Recorder data unavailable"}</h2>
                            </section>
                        },
                        Some(payload) if !payload.configured => html! {
                            <section class="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-14 text-center">
                                <h2 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"Workflow runtime not ready"}</h2>
                            </section>
                        },
                        Some(payload) if payload.items.is_empty() => html! {
                            <section class="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-14 text-center">
                                <h2 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"No workflow executions recorded yet"}</h2>
                                <p class="mx-auto mt-3 max-w-lg text-sm font-light text-black/55">
                                    {"Runs will appear here as process instances are created."}
                                </p>
                            </section>
                        },
                        Some(payload) => html! {
                            <div class="grid gap-3">
                                { for payload.items.iter().map(recorder_row) }
                            </div>
                        },
                    }
                }
            }
        </div>
    }
}

fn recorder_row(summary: &PortalWorkflowSummary) -> Html {
    let (tone, label) = status(summary);
    let milestone = if summary.active_milestones.is_empty() {
        summary
            .responsible_party
            .clone()
            .unwrap_or_else(|| "No active milestone".into())
    } else {
        summary.active_milestones.join(", ")
    };

    html! {
        <article class="rounded-[var(--portal-panel-radius)] portal-glass-panel p-5">
            <div class="flex flex-wrap items-start justify-between gap-4">
                <div class="min-w-0">
                    <div class="text-[10px] font-light uppercase tracking-[0.18em] text-black/45">
                        { format!("{} · v{}", summary.workflow_name, summary.workflow_version) }
                    </div>
                    <h2 class="mt-1.5 font-serif text-lg font-light text-[var(--portal-navy)]">
                        { summary.property_name.clone().unwrap_or_else(|| "Execution".into()) }
                    </h2>
                    <p class="mt-2 font-mono text-[9px] text-black/40">{ summary.instance_id.clone() }</p>
                    <div class="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-xs font-light text-black/50">
                        <span>{ milestone }</span>
                        if summary.open_task_count > 0 {
                            <span>{ format!("{} open task{}", summary.open_task_count, if summary.open_task_count == 1 { "" } else { "s" }) }</span>
                        }
                        if summary.blocker_count > 0 {
                            <span class="text-[var(--portal-blue-gray)]">
                                { format!("{} blocker{}", summary.blocker_count, if summary.blocker_count == 1 { "" } else { "s" }) }
                            </span>
                        }
                    </div>
                </div>
                <div class="flex items-center gap-2">
                    <span class={format!("rounded-full px-2.5 py-1 text-[10px] font-light capitalize {tone}")}>
                        { label }
                    </span>
                    <a
                        href={format!("/portal/tech/flight-recorder/{}", summary.instance_id)}
                        class="rounded-md bg-[var(--portal-navy)] px-3 py-1.5 text-[10px] font-medium uppercase tracking-[0.1em] text-white hover:bg-black/80"
                    >
                        {"Open Recorder →"}
                    </a>
                </div>
            </div>
        </article>
    }
}

fn status(summary: &PortalWorkflowSummary) -> (&'static str, String) {
    if summary.outcome.as_deref() == Some("cancelled") {
        return ("bg-black/5 text-black/55", "Cancelled".into());
    }
    if matches!(
        summary.outcome.as_deref(),
        Some("failed") | Some("conflict")
    ) || summary.status == "error"
    {
        return (
            "bg-red-50 text-red-700",
            summary
                .outcome
                .clone()
                .unwrap_or_else(|| summary.status.clone()),
        );
    }
    if summary.outcome.as_deref() == Some("completed") {
        return ("bg-emerald-50 text-emerald-700", "Completed".into());
    }
    (
        "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]",
        summary.status.clone(),
    )
}
