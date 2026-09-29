//! One workflow instance, opened: correlations, jobs, facts, commands, events, tasks and the token tree.

#[allow(unused_imports)]
use super::*;

/// ONE INSTANCE'S TECHNICAL DETAIL: the live component's eight sections, in its order.
///
/// This is the part of the screen that answers "what actually happened in this run" — the tokens the engine is holding, the
/// tasks it created, the correlations to the canonical tasks, the timers, the facts, the commands and their receipts, and the
/// engine's own event log. Nothing here is aggregated: it is the run, as the engine recorded it.
pub(super) fn instance_detail(detail: &PortalWorkflowInstanceDetail, definition_name: Option<&str>) -> Html {
    html! {
        <div class="mt-4 space-y-6">
            <section class={PANEL}>
                { section_title("Process / Definition", None) }
                <div class="mt-4 grid gap-6 md:grid-cols-2 xl:grid-cols-4">
                    { detail_html(
                        "Definition",
                        html! { { detail_definition(detail, definition_name) } },
                    ) }
                    { detail_html(
                        "Instance",
                        html! { <span class="font-mono text-xs">{ &detail.instance_id }</span> },
                    ) }
                    { detail_html(
                        "Status",
                        status_pill(&detail.status, detail.outcome.as_deref()),
                    ) }
                    { detail_html("Subject", html! { { detail_subject(detail) } }) }
                    { detail_html("Started", html! { { format_instant(&detail.started_at) } }) }
                    { detail_html("Ended", html! { { detail_ended(detail) } }) }
                    { detail_html(
                        "Counts",
                        html! {
                            { format!(
                                "{} active tokens · {} tasks · {} events",
                                detail.active_token_count, detail.task_count, detail.event_count,
                            ) }
                        },
                    ) }
                </div>
            </section>

            <section class={PANEL}>
                { section_title("Tokens", Some("Nested token tree (technical id + human label).")) }
                <div class="mt-4">{ token_tree(&detail.tokens, &detail.node_labels) }</div>
            </section>

            <section class={PANEL}>
                { section_title("Engine Tasks", None) }
                <div class="mt-4 overflow-x-auto">
                    if detail.tasks.is_empty() {
                        <p class="text-sm font-light text-black/40">{"No engine tasks."}</p>
                    } else {
                        <table class="w-full text-left text-sm">
                            <thead>
                                <tr>
                                    { table_head("Task") }
                                    { table_head("Status") }
                                    { table_head("Token") }
                                    { table_head("Assignee") }
                                    { table_head("Candidates") }
                                </tr>
                            </thead>
                            <tbody>
                                { for detail.tasks.iter().map(task_row) }
                            </tbody>
                        </table>
                    }
                </div>
            </section>

            { correlations_section(detail) }
            { jobs_section(detail) }
            { facts_section(detail) }
            { commands_section(detail) }
            { events_section(detail) }
        </div>
    }
}

/// The definition line: its name when the deployed definitions were read with it, else the key and version.
pub(super) fn detail_definition(
    detail: &PortalWorkflowInstanceDetail,
    definition_name: Option<&str>,
) -> String {
    match definition_name {
        Some(name) => format!(
            "{} ({} v{})",
            name, detail.definition_key, detail.definition_version
        ),
        None => format!("{} v{}", detail.definition_key, detail.definition_version),
    }
}

/// The subject line: what the run is about, or nothing when the engine recorded no subject.
pub(super) fn detail_subject(detail: &PortalWorkflowInstanceDetail) -> String {
    match detail.subject_type.as_deref() {
        Some(subject_type) => format!(
            "{}:{}",
            subject_type,
            detail.subject_id.as_deref().unwrap_or("—")
        ),
        None => "—".to_string(),
    }
}

/// When the run ended, or nothing when it has not — a run still going has no end, which is not an error.
pub(super) fn detail_ended(detail: &PortalWorkflowInstanceDetail) -> String {
    detail
        .ended_at
        .as_deref()
        .map(format_instant)
        .unwrap_or_else(|| "—".to_string())
}

/// The canonical correlations: which of the application's tasks this run's engine tasks are attached to, and how those stand.
pub(super) fn correlations_section(detail: &PortalWorkflowInstanceDetail) -> Html {
    html! {
        <section class={PANEL}>
            { section_title("Canonical Correlations", None) }
            <div class="mt-4 overflow-x-auto">
                if detail.correlations.is_empty() {
                    <p class="text-sm font-light text-black/40">{"No correlations."}</p>
                } else {
                    <table class="w-full text-left text-sm">
                        <thead>
                            <tr>
                                { table_head("Engine Task") }
                                { table_head("Canonical Task") }
                                { table_head("Title") }
                                { table_head("Status") }
                            </tr>
                        </thead>
                        <tbody>
                            { for detail.correlations.iter().map(correlation_row) }
                        </tbody>
                    </table>
                }
            </div>
        </section>
    }
}

pub(super) fn correlation_row(correlation: &PortalWorkflowCorrelation) -> Html {
    html! {
        <tr key={correlation.workflow_task_id.clone()} class={ROW}>
            <td class="px-4 py-3 font-mono text-[11px] text-black/50">
                { &correlation.workflow_task_id }
            </td>
            <td class="px-4 py-3 font-mono text-[11px] text-black/50">
                { correlation.application_task_id.as_deref().unwrap_or("—") }
            </td>
            <td class="px-4 py-3 font-light text-black/70">
                { correlation.application_task_title.as_deref().unwrap_or("—") }
            </td>
            <td class="px-4 py-3 font-light text-black/60">
                { correlation.application_task_status.as_deref().unwrap_or("—") }
            </td>
        </tr>
    }
}

/// The timers: what the engine is waiting to do, and when it is due.
pub(super) fn jobs_section(detail: &PortalWorkflowInstanceDetail) -> Html {
    html! {
        <section class={PANEL}>
            { section_title("Jobs / Timers", None) }
            <div class="mt-4 overflow-x-auto">
                if detail.jobs.is_empty() {
                    <p class="text-sm font-light text-black/40">{"No jobs."}</p>
                } else {
                    <table class="w-full text-left text-sm">
                        <thead>
                            <tr>
                                { table_head("Job") }
                                { table_head("Type") }
                                { table_head("Status") }
                                { table_head("Due") }
                            </tr>
                        </thead>
                        <tbody>
                            { for detail.jobs.iter().map(job_row) }
                        </tbody>
                    </table>
                }
            </div>
        </section>
    }
}

pub(super) fn job_row(job: &PortalWorkflowJob) -> Html {
    html! {
        <tr key={job.id.clone()} class={ROW}>
            <td class="px-4 py-3 font-mono text-[11px] text-black/50">{ &job.id }</td>
            <td class="px-4 py-3 font-light text-black/70">{ &job.job_type }</td>
            <td class="px-4 py-3 font-light text-black/60">{ &job.status }</td>
            <td class="px-4 py-3 font-light text-black/60">
                { job.due_at.as_deref().map(format_instant).unwrap_or_else(|| "—".to_string()) }
            </td>
        </tr>
    }
}

/// The facts: the run's variables, exactly as the engine stored them.
pub(super) fn facts_section(detail: &PortalWorkflowInstanceDetail) -> Html {
    html! {
        <section class={PANEL}>
            { section_title("Facts", Some("Process variables captured at runtime.")) }
            <div class="mt-4">{ facts_block(detail.variables.as_ref()) }</div>
        </section>
    }
}

/// The commands: what the run asked the application to do, and what came back.
///
/// THE RECEIPT IS THE INTERESTING COLUMN: an engine command that succeeded and an application receipt that is still pending
/// is a run that will never move on its own, which is exactly the state this screen exists to make visible.
pub(super) fn commands_section(detail: &PortalWorkflowInstanceDetail) -> Html {
    html! {
        <section class={PANEL}>
            { section_title(
                "Command Receipts",
                Some("Engine commands and their application-side receipt outcome."),
            ) }
            <div class="mt-4 overflow-x-auto">
                if detail.commands.is_empty() {
                    <p class="text-sm font-light text-black/40">{"No commands executed."}</p>
                } else {
                    <table class="w-full text-left text-sm">
                        <thead>
                            <tr>
                                { table_head("Command") }
                                { table_head("Node") }
                                { table_head("Outcome") }
                                { table_head("Receipt") }
                                { table_head("Message") }
                            </tr>
                        </thead>
                        <tbody>
                            { for detail.commands.iter().map(command_row) }
                        </tbody>
                    </table>
                }
            </div>
        </section>
    }
}

pub(super) fn command_row(command: &crate::model::PortalWorkflowCommand) -> Html {
    html! {
        <tr key={command.command_id.clone()} class={ROW}>
            <td class="px-4 py-3">
                <div class="font-light text-black/70">{ &command.command_type }</div>
                <div class="font-mono text-[11px] text-black/40">{ &command.command_id }</div>
            </td>
            <td class="px-4 py-3 font-light text-black/60">{ &command.node_id }</td>
            <td class="px-4 py-3 font-light text-black/60">{ &command.outcome }</td>
            <td class="px-4 py-3">{ receipt_pill(command.receipt_outcome.as_deref()) }</td>
            <td class="px-4 py-3 font-light text-black/60">
                { command.message.as_deref().unwrap_or("—") }
            </td>
        </tr>
    }
}

/// A receipt outcome: pending is the red one, because it is the one that means a run is stuck.
pub(super) fn receipt_pill(receipt_outcome: Option<&str>) -> Html {
    let Some(receipt_outcome) = receipt_outcome else {
        return html! { <span class="text-black/40">{"—"}</span> };
    };
    let tone = if receipt_outcome == "pending" {
        "bg-red-50 text-red-700 border-red-200"
    } else {
        "bg-emerald-50 text-emerald-700 border-emerald-200"
    };
    html! {
        <span class={classes!(RECEIPT_PILL, tone)}>
            { receipt_outcome }
        </span>
    }
}

/// The engine's own event log for the run, most recent first as the projection returns it.
pub(super) fn events_section(detail: &PortalWorkflowInstanceDetail) -> Html {
    html! {
        <section class={PANEL}>
            { section_title("Events", Some("Engine process events (most recent first).")) }
            <div class="mt-4 overflow-x-auto">
                if detail.events.is_empty() {
                    <p class="text-sm font-light text-black/40">{"No events."}</p>
                } else {
                    <table class="w-full text-left text-sm">
                        <thead>
                            <tr>
                                { table_head("Event") }
                                { table_head("Node") }
                                { table_head("Actor") }
                            </tr>
                        </thead>
                        <tbody>
                            { for detail.events.iter().map(event_row) }
                        </tbody>
                    </table>
                }
            </div>
        </section>
    }
}

pub(super) fn event_row(event: &PortalWorkflowDiagnosticEvent) -> Html {
    html! {
        <tr key={event.id.clone()} class={ROW}>
            <td class="px-4 py-3 font-light text-black/70">{ &event.event_type }</td>
            <td class="px-4 py-3 font-light text-black/60">
                { event.node_id.as_deref().unwrap_or("—") }
            </td>
            <td class="px-4 py-3 font-light text-black/60">
                { event.actor.as_deref().unwrap_or("—") }
            </td>
        </tr>
    }
}

/// One engine task, with its candidates rendered as the engine stores them — a list of roles or users, joined.
pub(super) fn task_row(task: &PortalWorkflowTask) -> Html {
    html! {
        <tr key={task.id.clone()} class={ROW}>
            <td class="px-4 py-3">
                <div class="font-light text-black/70">{ &task.name }</div>
                <div class="font-mono text-[11px] text-black/40">{ &task.id }</div>
            </td>
            <td class="px-4 py-3 font-light text-black/60">{ &task.status }</td>
            <td class="px-4 py-3 font-mono text-[11px] text-black/50">
                { task.token_id.as_deref().unwrap_or("—") }
            </td>
            <td class="px-4 py-3 font-light text-black/60">
                { task.assignee.as_deref().unwrap_or("—") }
            </td>
            <td class="px-4 py-3 font-light text-black/60">
                { if task.candidates.is_empty() { "—".to_string() } else { task.candidates.join(", ") } }
            </td>
        </tr>
    }
}

/// The token tree, nested by parent, with each node named by its label where the definition gave one.
///
/// THE TREE IS THE POINT: a token's parent is what makes a run's position legible — a parallel gateway's branches, and which
/// of them is still holding the run. Rendering it flat would show the same tokens and say nothing, so the depth is carried
/// as indentation and each level is a child of the level above, exactly as the live tree did.
pub(super) fn token_tree(
    tokens: &[PortalWorkflowToken],
    node_labels: &std::collections::BTreeMap<String, String>,
) -> Html {
    let mut children: std::collections::BTreeMap<&str, Vec<&PortalWorkflowToken>> =
        std::collections::BTreeMap::new();
    let mut roots: Vec<&PortalWorkflowToken> = Vec::new();
    for token in tokens {
        match token.parent_token_id.as_deref() {
            Some(parent) => children.entry(parent).or_default().push(token),
            None => roots.push(token),
        }
    }
    if roots.is_empty() {
        return html! { <p class="text-sm font-light text-black/40">{"No tokens."}</p> };
    }
    html! {
        <div>
            { for roots.into_iter().map(|token| token_branch(token, &children, node_labels, 0)) }
        </div>
    }
}

/// One token and its children. Recursive by design: a token tree is a tree, and flattening it is how the shape is lost.
pub(super) fn token_branch<'a>(
    token: &'a PortalWorkflowToken,
    children: &std::collections::BTreeMap<&'a str, Vec<&'a PortalWorkflowToken>>,
    node_labels: &std::collections::BTreeMap<String, String>,
    depth: usize,
) -> Html {
    let kids = children.get(token.id.as_str());
    html! {
        <div key={token.id.clone()}>
            <div
                class="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-[var(--portal-border)]/60 py-2"
                style={format!("padding-left: {}px", depth * 18)}
            >
                <span class="font-mono text-[11px] text-black/45">{ &token.id }</span>
                <span class="text-sm font-light text-black/70">
                    { node_labels.get(&token.node_id).cloned().unwrap_or_else(|| token.node_id.clone()) }
                </span>
                <span class="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
                    { &token.status }
                </span>
                if let Some(outcome) = token.outcome.as_deref() {
                    <span class="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
                        { outcome }
                    </span>
                }
                <span class="text-[10px] font-light text-black/35">
                    { if token.required { "required" } else { "optional" } }
                </span>
            </div>
            if let Some(kids) = kids {
                { for kids.iter().map(|kid| token_branch(kid, children, node_labels, depth + 1)) }
            }
        </div>
    }
}

/// The run's variables, printed as they are stored.
///
/// PRINTED, NOT INTERPRETED. The engine's facts are application-shaped JSON the portal has no schema for, and the live
/// component printed them verbatim rather than guessing at a table — the reader of this screen is a diagnostician reading a
/// run, and a pretty-printed object is the least lossy thing to hand them.
pub(super) fn facts_block(variables: Option<&serde_json::Value>) -> Html {
    let Some(variables) = variables else {
        return html! { <p class="text-sm font-light text-black/40">{"No facts recorded."}</p> };
    };
    let empty = variables
        .as_object()
        .map(|fields| fields.is_empty())
        .unwrap_or(false);
    if empty {
        return html! { <p class="text-sm font-light text-black/40">{"No facts recorded."}</p> };
    }
    let printed = serde_json::to_string_pretty(variables).unwrap_or_else(|_| variables.to_string());
    html! {
        <pre class="overflow-x-auto rounded-sm bg-[var(--portal-blue-pale)]/50 p-4 font-mono text-[11px] leading-5 text-black/70">
            { printed }
        </pre>
    }
}
