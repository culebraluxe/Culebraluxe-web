//! /portal/workflows/[instanceId] — one definition-driven workflow instance.

use yew::prelude::*;

use crate::model::{Msg, PortalWorkflowDetail, PortalWorkflowTimelineItem};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct WorkflowRecordProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct WorkflowRecord;

impl Component for WorkflowRecord {
    type Message = ();
    type Properties = WorkflowRecordProps;

    fn create(_ctx: &Context<Self>) -> Self { Self }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("workflow-record").expect("workflow record exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { self.body(&props.model) }
            </PortalShell>
        }
    }
}

impl WorkflowRecord {
    fn body(&self, model: &crate::model::Model) -> Html {
        let detail = model.page.as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.workflow.as_ref());

        if model.loading {
            return html! {};
        }
        let Some(detail) = detail else {
            return html! {
                <div>
                    <a href="/portal/workflows" class="mb-6 inline-flex text-sm font-light text-black/50 hover:text-[var(--portal-navy)]">{"← Workflows"}</a>
                    <section class="rounded-[var(--portal-panel-radius)] portal-glass-panel p-8">
                        <h1 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"Workflow not found"}</h1>
                    </section>
                </div>
            };
        };

        html! {
            <div>
                <a href="/portal/workflows" class="mb-6 inline-flex text-sm font-light text-black/50 hover:text-[var(--portal-navy)]">{"← Workflows"}</a>
                { header(detail) }
                <div class="mt-6 grid gap-6 2xl:grid-cols-[minmax(0,1.2fr)_minmax(320px,0.8fr)]">
                    { timeline(detail) }
                    { operational(detail) }
                </div>
            </div>
        }
    }
}

fn status_pill(detail: &PortalWorkflowDetail) -> (&'static str, String) {
    if detail.outcome.as_deref() == Some("cancelled") {
        return ("bg-black/5 text-black/55", "Cancelled".into());
    }
    if matches!(detail.outcome.as_deref(), Some("failed") | Some("conflict")) {
        return ("bg-red-50 text-red-700", detail.outcome.clone().unwrap_or_default());
    }
    if detail.outcome.as_deref() == Some("completed") {
        return ("bg-emerald-50 text-emerald-700", "Closed".into());
    }
    ("bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]", detail.status.clone())
}

fn header(detail: &PortalWorkflowDetail) -> Html {
    let (class, label) = status_pill(detail);
    html! {
        <header class="flex flex-wrap items-start justify-between gap-4 rounded-[var(--portal-panel-radius)] portal-glass-panel p-6 lg:p-8">
            <div>
                <div class="text-[10px] font-light uppercase tracking-[0.22em] text-black/45">
                    { format!("{} · v{}", detail.workflow_name, detail.workflow_version) }
                </div>
                <h1 class="mt-2 font-serif text-3xl font-light text-[var(--portal-navy)]">
                    { detail.property_name.clone().unwrap_or_else(|| "Transaction".into()) }
                </h1>
                <p class="mt-1 text-sm font-light text-black/55">
                    { format!("{} · started {}", detail.responsible_party.clone().unwrap_or_else(|| "Unassigned".into()), detail.started_at_label) }
                </p>
            </div>
            <span class={format!("rounded-full px-3 py-1 text-xs font-light capitalize {class}")}>{ label }</span>
        </header>
    }
}

fn timeline(detail: &PortalWorkflowDetail) -> Html {
    html! {
        <section class="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6 lg:p-8">
            <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Progress"}</h2>
            <ol class="mt-6 space-y-0">
                { for detail.timeline.iter().enumerate().map(|(index, item)| timeline_item(index, detail.timeline.len(), item)) }
            </ol>
        </section>
    }
}

fn timeline_item(index: usize, len: usize, item: &PortalWorkflowTimelineItem) -> Html {
    let marker = if item.completed {
        "bg-emerald-600 border-emerald-600"
    } else if item.active {
        "bg-[var(--portal-blue-gray)] border-[var(--portal-blue-gray)]"
    } else {
        "bg-transparent border-black/20"
    };
    html! {
        <li class="relative flex gap-4 pb-6 last:pb-0">
            if index + 1 < len {
                <span class="absolute left-[11px] top-6 h-full w-px bg-[var(--portal-border)]"></span>
            }
            <span class={format!("relative z-10 mt-1 h-6 w-6 shrink-0 rounded-full border-2 {marker}")}></span>
            <div class="min-w-0">
                <div class="text-sm font-normal text-[var(--portal-navy)]">
                    { item.label.clone() }
                    if item.optional {
                        <span class="ml-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">{"optional"}</span>
                    }
                </div>
                if let Some(deadline) = item.deadline.clone() {
                    <div class="text-xs font-light text-black/50">{ deadline }</div>
                }
                if let Some(description) = item.description.clone() {
                    <div class="mt-0.5 max-w-md text-xs font-light leading-5 text-black/45">{ description }</div>
                }
            </div>
        </li>
    }
}

fn operational(detail: &PortalWorkflowDetail) -> Html {
    html! {
        <section class="space-y-6">
            <div class="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Milestones"}</h2>
                <ul class="mt-4 space-y-2">
                    if detail.milestones.is_empty() && detail.outcome.is_none() {
                        <li class="text-sm font-light text-black/45">{"No active milestone."}</li>
                    } else {
                        { for detail.milestones.iter().map(|m| html! {
                            <li class="flex items-center justify-between text-sm font-light">
                                <span class="text-[var(--portal-navy)]">{ m.label.clone() }</span>
                                <span class="text-xs text-black/50">{ m.owner.clone() }</span>
                            </li>
                        }) }
                    }
                </ul>
            </div>
            <div class="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Operational"}</h2>
                <dl class="mt-4 space-y-2 text-sm font-light text-black/60">
                    <div class="flex justify-between"><dt>{"Open tasks"}</dt><dd>{ detail.open_task_count }</dd></div>
                    <div class="flex justify-between"><dt>{"Pending timers"}</dt><dd>{ detail.pending_timer_count }</dd></div>
                    <div class="flex justify-between">
                        <dt>{"Blockers"}</dt>
                        <dd class={if detail.blockers.is_empty() { "" } else { "text-[var(--portal-blue-gray)]" }}>
                            { if detail.blockers.is_empty() { "—".to_string() } else { detail.blockers.join(", ") } }
                        </dd>
                    </div>
                </dl>
            </div>
            <div class="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Recent activity"}</h2>
                <ol class="mt-4 space-y-3">
                    if detail.events.is_empty() {
                        <li class="text-sm font-light text-black/45">{"No events yet."}</li>
                    } else {
                        { for detail.events.iter().take(8).map(|event| html! {
                            <li class="text-sm font-light">
                                <div class="text-[var(--portal-navy)]">{ event.event_type.clone() }</div>
                                <div class="text-xs text-black/45">
                                    {
                                        [event.node_label.clone().map(|n| format!("Node: {n}")), event.actor.clone()]
                                            .into_iter().flatten().collect::<Vec<_>>().join(" · ")
                                    }
                                </div>
                            </li>
                        }) }
                    }
                </ol>
            </div>
        </section>
    }
}
