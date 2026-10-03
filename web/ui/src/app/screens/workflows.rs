//! CORE — Workflows: the transaction workflow cards (`/portal/workflows`) and one instance's timeline
//! (`/portal/workflows/:instanceId`). Both are reads; the runtime changes workflows, never this screen.

use yew::prelude::*;

use crate::app::page::{PageScreen, PageSpec};
use crate::app::screen::ScreenCtx;
use crate::model::{
    PortalPage, PortalWorkflowDetail, PortalWorkflowList, PortalWorkflowSummary,
    PortalWorkflowTimelineItem,
};

pub type Workflows = PageScreen<WorkflowList>;
pub type WorkflowRecord = PageScreen<WorkflowInstance>;

pub struct WorkflowList;

impl PageSpec for WorkflowList {
    type Data = PortalWorkflowList;
    const SCREEN: &'static str = "workflows";
    const NOUN: &'static str = "the workflows";
    fn pick(page: PortalPage) -> Option<PortalWorkflowList> {
        page.workflows
    }
    fn view(payload: &PortalWorkflowList, _ctx: &ScreenCtx) -> Html {
        html! {
            <div>
                <header class="mb-8">
                    <h1 class="font-serif text-3xl font-light text-[var(--portal-navy)]">{"Workflows"}</h1>
                    <p class="mt-1 text-sm font-light text-black/55">
                        {"Transaction orchestration for deals in motion."}
                    </p>
                </header>
                if !payload.configured {
                    <section class="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-16 text-center">
                        <h2 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"Workflow runtime not ready"}</h2>
                        <p class="mx-auto mt-3 max-w-lg text-sm font-light leading-6 text-black/55">
                            {"The workflow runtime shares the CulebraLuxe database. Apply the unified activation script to enable transaction workflows."}
                        </p>
                    </section>
                } else if payload.items.is_empty() {
                    <section class="rounded-[var(--portal-panel-radius)] portal-glass-panel px-10 py-16 text-center">
                        <h2 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"No transaction workflows yet"}</h2>
                        <p class="mx-auto mt-3 max-w-md text-sm font-light text-black/55">
                            {"A workflow instance is created when an offer is accepted and the transaction moves into contract preparation."}
                        </p>
                    </section>
                } else {
                    <div class="grid gap-4 xl:grid-cols-2">
                        { for payload.items.iter().map(workflow_card) }
                    </div>
                }
            </div>
        }
    }
}

pub struct WorkflowInstance;

impl PageSpec for WorkflowInstance {
    type Data = PortalWorkflowDetail;
    const SCREEN: &'static str = "workflow-record";
    const NOUN: &'static str = "the workflow";
    const SCOPED: bool = true;
    fn pick(page: PortalPage) -> Option<PortalWorkflowDetail> {
        page.workflow
    }
    fn view(detail: &PortalWorkflowDetail, _ctx: &ScreenCtx) -> Html {
        html! {
            <div>
                { header(detail) }
                <div class="mt-6 grid gap-6 2xl:grid-cols-[minmax(0,1.2fr)_minmax(320px,0.8fr)]">
                    { timeline(detail) }
                    { operational(detail) }
                </div>
            </div>
        }
    }
}

fn pill(summary: &PortalWorkflowSummary) -> (&'static str, String) {
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
        return ("bg-emerald-50 text-emerald-700", "Closed".into());
    }
    (
        "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]",
        summary.status.clone(),
    )
}

fn workflow_card(summary: &PortalWorkflowSummary) -> Html {
    let (pill_class, pill_label) = pill(summary);
    let milestone = if summary.active_milestones.is_empty() {
        summary
            .responsible_party
            .clone()
            .unwrap_or_else(|| "No active milestone".into())
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

fn status_pill(detail: &PortalWorkflowDetail) -> (&'static str, String) {
    if detail.outcome.as_deref() == Some("cancelled") {
        return ("bg-black/5 text-black/55", "Cancelled".into());
    }
    if matches!(detail.outcome.as_deref(), Some("failed") | Some("conflict")) {
        return (
            "bg-red-50 text-red-700",
            detail.outcome.clone().unwrap_or_default(),
        );
    }
    if detail.outcome.as_deref() == Some("completed") {
        return ("bg-emerald-50 text-emerald-700", "Closed".into());
    }
    (
        "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]",
        detail.status.clone(),
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::screen::Screen;
    use serde_json::json;

    #[test]
    fn the_list_and_the_instance_read_their_pages() {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = Workflows::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/api/portal/rust-ui/page?screen=workflows");
        Workflows::update(
            &mut model,
            request.respond(Ok(
                json!({ "workflows": { "configured": true, "items": [] } }),
            )),
            &ctx,
        );
        assert!(model.read.loaded().unwrap().configured);

        let ctx = ScreenCtx {
            id: Some("wf 1".into()),
            ..ScreenCtx::default()
        };
        let (mut model, cmd) = WorkflowRecord::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(
            request.path,
            "/api/portal/rust-ui/page?screen=workflow-record&scope=wf%201"
        );
        WorkflowRecord::update(&mut model, request.respond(Ok(json!({}))), &ctx);
        assert!(
            matches!(model.read, crate::app::cmd::Remote::Failed(ref e) if e.code == "DECODE"),
            "an instance that is not there is said so"
        );
    }
}
