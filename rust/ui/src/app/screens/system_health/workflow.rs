//! Workflow diagnostics: metrics, anomalies, the instance list and its rows.

#[allow(unused_imports)]
use super::*;

impl SystemHealth {
    /// WORKFLOW DIAGNOSTICS — the half the earlier conversion flattened into a list.
    ///
    /// The engine's own view, in its own section under the health snapshot: the seven summary figures, the anomaly sweep, and
    /// every instance with its detail one click away. Read-only, like everything else on this screen.
    pub(super) fn workflow_diagnostics(
        &self,
        model: &Model,
        diagnostics: &PortalWorkflowDiagnostics,
        on_msg: &Callback<Msg>,
    ) -> Html {
        html! {
            <div class="mt-10">
                <div class="mb-6">
                    <p class="text-xs font-light uppercase tracking-[0.28em] text-black/40">{"Workflow"}</p>
                    <h2 class="mt-3 font-serif text-3xl font-light leading-[1.1]">{"Workflow Engine"}</h2>
                    <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
                        {"Read-only diagnostics for the residential transaction engine — deployed definitions, instance lifecycle, correlations, jobs, receipts and support anomaly flags."}
                    </p>
                </div>
                // NOT CONFIGURED IS NOT AN ERROR. The projection answers `false` on a database that predates the engine's
                // tables, and the live screen said so in a panel rather than showing seven zeroes — which would read as an
                // engine that exists and has done nothing.
                if !diagnostics.configured {
                    <section class={classes!(PANEL, "px-10", "py-16", "text-center")}>
                        <p class="text-sm font-light text-black/50">
                            {"Workflow engine tables are not available in this environment."}
                        </p>
                    </section>
                } else {
                    { self.workflow_metrics(diagnostics) }
                    { self.workflow_anomalies(diagnostics) }
                    { self.workflow_instances(model, diagnostics, on_msg) }
                }
            </div>
        }
    }

    /// The engine's seven figures. The second line of each card is the breakdown the live screen put there — the counts
    /// that make a number actionable without opening anything.
    pub(super) fn workflow_metrics(&self, diagnostics: &PortalWorkflowDiagnostics) -> Html {
        let summary = &diagnostics.summary;
        html! {
            <section class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                { workflow_metric(
                    "Definitions",
                    &summary.definition_count.to_string(),
                    "Deployed process definitions",
                    false,
                ) }
                { workflow_metric(
                    "Instances",
                    &summary.instance_total.to_string(),
                    &format!(
                        "{} active · {} completed · {} failed · {} other",
                        summary.instance_active,
                        summary.instance_completed,
                        summary.instance_failed,
                        summary.instance_other
                    ),
                    false,
                ) }
                { workflow_metric(
                    "Ready Engine Tasks",
                    &summary.ready_engine_tasks.to_string(),
                    "ready / reserved / in progress",
                    false,
                ) }
                { workflow_metric(
                    "Correlated Open Tasks",
                    &summary.correlated_open_canonical_tasks.to_string(),
                    "Open canonical tasks with a correlation",
                    false,
                ) }
                { workflow_metric(
                    "Pending Jobs",
                    &summary.pending_jobs.to_string(),
                    "pending / locked jobs",
                    false,
                ) }
                { workflow_metric(
                    "Pending Receipts",
                    &summary.pending_receipts.to_string(),
                    "Stuck command receipts",
                    false,
                ) }
                // THE ONE CARD THAT SHOUTS. An anomaly count is the reason anyone opened this screen, so a non-zero one is
                // red and a zero is not — the same emphasis the live card applied.
                { workflow_metric(
                    "Anomalies",
                    &summary.anomaly_count.to_string(),
                    "Support flags to review",
                    summary.anomaly_count > 0,
                ) }
            </section>
        }
    }

    /// The anomaly sweep: what the engine's invariants noticed, in the order the projection returned it.
    pub(super) fn workflow_anomalies(&self, diagnostics: &PortalWorkflowDiagnostics) -> Html {
        html! {
            <section class={classes!(PANEL, "mt-6")}>
                <h3 class="font-serif text-xl font-light">{"Anomalies"}</h3>
                <p class="mt-1 text-xs font-light text-black/40">
                    {"Deterministic support flags derived from engine state and correlation invariants."}
                </p>
                <div class="mt-4 space-y-2">
                    if diagnostics.anomalies.is_empty() {
                        <p class="text-sm font-light text-black/40">{"No anomalies detected."}</p>
                    } else {
                        { for diagnostics.anomalies.iter().enumerate().map(|(index, anomaly)| {
                            anomaly_row(anomaly, index)
                        }) }
                    }
                </div>
            </section>
        }
    }

    /// Every instance the engine has run, each one openable.
    ///
    /// THE OPEN ROW IS THE MODEL'S, and the click is a message. What is rendered from `model.workflow` is the operator's
    /// selection rather than a component's memory: a row that is open stays open across a payload, and closing it skips the
    /// read entirely because `update` answers the close without one.
    pub(super) fn workflow_instances(
        &self,
        model: &Model,
        diagnostics: &PortalWorkflowDiagnostics,
        on_msg: &Callback<Msg>,
    ) -> Html {
        let workflow = &model.workflow;
        html! {
            <section class={classes!(PANEL, "mt-6")}>
                <div class="flex items-center justify-between">
                    <h3 class="font-serif text-xl font-light">{"Instances"}</h3>
                    <span class="text-xs font-light text-black/40">
                        { format!("{} total", diagnostics.instances.len()) }
                    </span>
                </div>
                <div class="mt-4 space-y-2">
                    if diagnostics.instances.is_empty() {
                        <p class="text-sm font-light text-black/40">{"No workflow instances recorded."}</p>
                    } else {
                        { for diagnostics.instances.iter().map(|instance| {
                            self.instance_row(diagnostics, workflow, instance, on_msg)
                        }) }
                    }
                </div>
            </section>
        }
    }

    /// One instance's row: what it is about, its status, when it started and how many tokens are live — with the detail
    /// under it when it is the open one.
    pub(super) fn instance_row(
        &self,
        diagnostics: &PortalWorkflowDiagnostics,
        workflow: &crate::model::WorkflowDiagnosticsState,
        instance: &PortalWorkflowInstance,
        on_msg: &Callback<Msg>,
    ) -> Html {
        let selected = workflow.selected_instance.as_deref() == Some(instance.instance_id.as_str());
        let toggle = {
            let on_msg = on_msg.clone();
            let instance_id = instance.instance_id.clone();
            Callback::from(move |_: MouseEvent| {
                on_msg.emit(Msg::WorkflowInstanceToggled {
                    instance_id: instance_id.clone(),
                })
            })
        };
        let definition_name = definition_name_for(diagnostics, instance);
        html! {
            <div class="rounded-sm border border-[var(--portal-border)] bg-white">
                <button
                    type="button"
                    onclick={toggle}
                    class="flex min-h-[48px] w-full flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3 text-left"
                >
                    <div class="min-w-0 flex-1">
                        <div class="truncate text-sm font-light text-black/80">
                            { instance_label(instance) }
                        </div>
                        <div class="font-mono text-[11px] text-black/40">
                            { format!("{} v{}", instance.definition_key, instance.definition_version) }
                        </div>
                    </div>
                    { status_pill(&instance.status, instance.outcome.as_deref()) }
                    <div class="hidden text-right sm:block">
                        <div class="text-[10px] font-light uppercase tracking-[0.14em] text-black/35">
                            {"Started"}
                        </div>
                        <div class="text-xs font-light text-black/60">
                            { format_instant(&instance.started_at) }
                        </div>
                    </div>
                    <div class="hidden text-right md:block">
                        <div class="text-[10px] font-light uppercase tracking-[0.14em] text-black/35">
                            {"Tokens"}
                        </div>
                        <div class="text-xs font-light text-black/60">
                            { format!("{} active", instance.active_token_count) }
                        </div>
                    </div>
                    <span class={classes!(
                        "text-xs",
                        "text-black/40",
                        "transition-transform",
                        selected.then_some("rotate-90"),
                    )}>
                        {"›"}
                    </span>
                </button>
                if selected {
                    <div class="border-t border-[var(--portal-border)] px-4 pb-6">
                        { self.open_row(workflow, instance, definition_name.as_deref()) }
                    </div>
                }
            </div>
        }
    }

    /// What an open row shows: a read in flight, a read that failed or found nothing, or the detail itself — the live
    /// component's three states, in its order.
    pub(super) fn open_row(
        &self,
        workflow: &crate::model::WorkflowDiagnosticsState,
        instance: &PortalWorkflowInstance,
        definition_name: Option<&str>,
    ) -> Html {
        if workflow.loading_instance.as_deref() == Some(instance.instance_id.as_str()) {
            return template::loading_line("technical detail");
        }
        if let Some(error) = workflow.error.as_deref() {
            return html! {
                <p class="py-6 text-sm font-light text-red-700">{ error }</p>
            };
        }
        match workflow.detail.as_ref() {
            Some(detail) => instance_detail(detail, definition_name),
            None => html! {
                <p class="py-6 text-sm font-light text-black/40">{"No detail available."}</p>
            },
        }
    }
}

/// A section heading inside an instance's detail, with its hint where the live component had one.
pub(super) fn section_title(title: &str, hint: Option<&str>) -> Html {
    html! {
        <div>
            <h3 class="font-serif text-lg font-light">{ title }</h3>
            if let Some(hint) = hint {
                <p class="mt-1 text-xs font-light text-black/40">{ hint }</p>
            }
        </div>
    }
}

/// One labelled signal whose value is markup rather than a string: a pill, a monospaced id, a formatted instant.
pub(super) fn detail_html(label: &str, value: Html) -> Html {
    html! {
        <div>
            <div class="text-[10px] font-light uppercase tracking-[0.18em] text-black/35">{ label }</div>
            <div class="mt-2 break-words text-sm font-light leading-6 text-black/70">{ value }</div>
        </div>
    }
}

/// A table's column heading, matching the other portal tables' small-caps treatment.
pub(super) fn table_head(label: &str) -> Html {
    html! {
        <th class="px-4 py-3 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
            { label }
        </th>
    }
}
