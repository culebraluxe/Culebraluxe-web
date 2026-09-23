//! `/portal/system-health` — the operational health, the environment posture, and the workflow diagnostics.
//!
//! PARITY WITH `app/portal/system-health/page.tsx` at `0acb29c5` (the parent of `cb5000a2`), which composed
//! `components/portal/system-health.tsx` and `components/portal/workflow-diagnostics.tsx` over three reads made together.
//!
//! WHAT THE EARLIER CONVERSION LOST, AND WHAT IS BACK: the Workflow Diagnostics half held React state — which instance is
//! open, its detail, which one is loading, and any error — and loaded that instance's detail when a row was clicked. The
//! generic rows cutover dropped the interaction entirely, so the screen became a list. Here the selection is
//! `Msg::WorkflowInstanceToggled` → an effect → the payload with that instance's detail, and the open row comes from the
//! model like every other piece of state on this portal.
//!
//! READ-ONLY. No workflow reset, repair or mutation control is on this screen, and none is added: it reports.

use yew::prelude::*;

use crate::model::{
    Msg, PortalEnvironmentReadiness, PortalSystemHealthSnapshot,
    PortalWorkflowAnomaly, PortalWorkflowCorrelation, PortalWorkflowDefinition,
    PortalWorkflowDiagnosticEvent, PortalWorkflowDiagnostics, PortalWorkflowInstance,
    PortalWorkflowInstanceDetail, PortalWorkflowJob, PortalWorkflowTask, PortalWorkflowToken,
};
use crate::yew_views::portal_shell::PortalShell;

/// The navy panel the live screen used for every section.
const PANEL: &str = "rounded-[var(--portal-panel-radius)] portal-glass-panel p-6";

/// The severity/status flag's shared shape: a small pill, so a flag never reads as a button.
const PILL: &str =
    "inline-flex items-center rounded-full border px-2.5 py-0.5 text-[10px] font-medium uppercase tracking-[0.14em]";

/// The receipt flag's shape: the same pill with the tighter padding the live receipt badge used.
const RECEIPT_PILL: &str =
    "inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.12em]";

#[derive(Properties, PartialEq)]
pub struct SystemHealthProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct SystemHealth;

impl Component for SystemHealth {
    type Message = ();
    type Properties = SystemHealthProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen =
            crate::model::screen("system-health").expect("the system health screen is in the registry");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { self.body(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

impl SystemHealth {
    fn body(&self, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let read = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.support.as_ref())
            .and_then(|support| support.system_health.clone());
        html! {
            <div>
                { self.heading() }
                if let Some(read) = read {
                    { self.metrics(&read.health) }
                    { self.recent_activity(&read.health) }
                    { self.data_quality(&read.health) }
                    { self.transaction_quality(&read.health) }
                    { self.write_invariants(&read.health) }
                    { self.security_model(&read.health) }
                    { self.environment(&read.environment) }
                    { self.workflow_diagnostics(model, &read.diagnostics, on_msg) }
                } else {
                    <section class={PANEL}>
                        <p class="text-sm font-light text-black/40">{"Reading the health snapshot…"}</p>
                    </section>
                }
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

impl SystemHealth {
    /// The four data-quality signals, all four of them the live screen's.
    fn data_quality(&self, health: &PortalSystemHealthSnapshot) -> Html {
        html! {
            <section class={classes!(PANEL, "mt-6")}>
                <h2 class="font-serif text-2xl font-light">{"Data Quality"}</h2>
                <p class="mt-1 text-xs font-light text-black/40">
                    {"Signals worth reviewing, derived directly from the schema."}
                </p>
                <div class="mt-6 grid gap-6 md:grid-cols-2 xl:grid-cols-4">
                    { detail("Clients without email", &health.persons_without_email_identity.to_string()) }
                    { detail("Clients without phone", &health.persons_without_phone_identity.to_string()) }
                    { detail("Open tasks no due date", &health.open_tasks_without_due_date.to_string()) }
                    { detail("Properties no hero image", &health.active_properties_without_hero_media.to_string()) }
                </div>
            </section>
        }
    }

    /// The six transaction signals: what showings, offers and participants can say about each other.
    fn transaction_quality(&self, health: &PortalSystemHealthSnapshot) -> Html {
        html! {
            <section class={classes!(PANEL, "mt-6")}>
                <h2 class="font-serif text-2xl font-light">{"Transaction Data Quality"}</h2>
                <p class="mt-1 text-xs font-light text-black/40">
                    {"Signals derivable from showings, offers, and participants."}
                </p>
                <div class="mt-6 grid gap-6 md:grid-cols-2 xl:grid-cols-3">
                    { detail("Completed showings missing completed_at", &health.completed_showings_missing_completed_at.to_string()) }
                    { detail("Scheduled showings missing scheduled_at", &health.scheduled_showings_missing_scheduled_at.to_string()) }
                    { detail("Active participants with ended_at", &health.active_participants_with_ended_at.to_string()) }
                    { detail("Other participants missing role label", &health.other_participants_missing_role_label.to_string()) }
                    { detail("Offers with cross-deal parent", &health.offers_with_cross_deal_parent.to_string()) }
                    { detail("Showings with deal/property mismatch", &health.showings_with_deal_property_mismatch.to_string()) }
                </div>
            </section>
        }
    }

    /// The write-side invariants: checks the listing, showing and participant write services are expected to maintain.
    fn write_invariants(&self, health: &PortalSystemHealthSnapshot) -> Html {
        html! {
            <section class={classes!(PANEL, "mt-6")}>
                <h2 class="font-serif text-2xl font-light">{"Write-Side Invariants"}</h2>
                <p class="mt-1 text-xs font-light text-black/40">
                    {"Deterministic checks on invariants the listing/showing write services are expected to maintain."}
                </p>
                <div class="mt-6 grid gap-6 md:grid-cols-2 xl:grid-cols-4">
                    { detail("Completed showings no showing interaction", &health.completed_showings_missing_showing_interaction.to_string()) }
                    { detail("Inactive participants without ended_at", &health.inactive_participants_without_ended_at.to_string()) }
                    { detail("Public properties with multiple heroes", &health.public_properties_with_multiple_heroes.to_string()) }
                    { detail("Hero media not an image", &health.hero_media_not_image.to_string()) }
                    { detail("Role / account-type mismatches", &health.account_type_mismatch_count.to_string()) }
                </div>
            </section>
        }
    }

    /// The application security invariants. The panel's own words say 0 is healthy, which is the only interpretation a
    /// reader needs: these are counts of things that should not exist.
    fn security_model(&self, health: &PortalSystemHealthSnapshot) -> Html {
        html! {
            <section class={classes!(PANEL, "mt-6")}>
                <h2 class="font-serif text-2xl font-light">{"Security Model"}</h2>
                <p class="mt-1 text-xs font-light text-black/40">
                    {"Application security invariants (guarded pre-migration; 0 is healthy)."}
                </p>
                <div class="mt-6 grid gap-6 md:grid-cols-2 xl:grid-cols-3">
                    { detail("Active users with no role", &health.active_app_users_without_role.to_string()) }
                    { detail("Auth identities → inactive user", &health.auth_identity_inactive_app_user.to_string()) }
                    { detail("Owner assignments", &health.owner_assignments.to_string()) }
                    { detail("Multiple owners (informational)", &health.multiple_owners.to_string()) }
                    { detail("Auth identities without usable user", &health.auth_identity_without_usable_app_user.to_string()) }
                </div>
            </section>
        }
    }
}


impl SystemHealth {
    /// The environment and secrets posture.
    ///
    /// EVERY ITEM IS A BOOLEAN ABOUT CONFIGURATION and nothing else is available to it: the projection this reads is built
    /// that way, the DTO carries it through, and this panel prints "Ready" or "Not configured". The paragraph under the
    /// heading is the live screen's, and it says the important part: a missing production secret reports itself as missing
    /// rather than falling back to a DEV or demo value.
    ///
    /// The production badge appears only IN production, as the live panel's did — on a development machine "Production
    /// secrets incomplete" is not a finding, it is the wrong question.
    fn environment(&self, readiness: &PortalEnvironmentReadiness) -> Html {
        let items: [(&str, bool); 10] = [
            ("Database configured", readiness.database_configured),
            ("DEV / PROD database separated", readiness.database_dev_prod_separated),
            ("Auth secret configured", readiness.auth_secret_configured),
            ("Auth provider configured", readiness.auth_provider_configured),
            ("Break-glass configured", readiness.break_glass_configured),
            ("Break-glass enabled", readiness.break_glass_enabled),
            ("Google Maps key configured", readiness.google_maps_key_configured),
            ("Demo key absent (production)", readiness.google_maps_demo_key_absent_in_production),
            ("Mux tokens configured", readiness.mux_configured),
            (
                "Broker signature configured",
                // The live panel required BOTH: a signature is only usable when it is configured AND switched on.
                readiness.broker_signature_configured && readiness.broker_signature_enabled,
            ),
        ];
        let all_ready = readiness.all_production_required_configured;
        html! {
            <section class={classes!(PANEL, "mt-6")}>
                <div class="flex flex-wrap items-center justify-between gap-3">
                    <div>
                        <h2 class="font-serif text-2xl font-light">{"Environment & Secrets Readiness"}</h2>
                        <p class="mt-1 text-xs font-light text-black/40">
                            {"Non-secret configuration posture — configured booleans only, never values. A missing production \
                              secret reports \"Not configured\" (fail closed) instead of falling back to a DEV/demo value."}
                        </p>
                    </div>
                    if readiness.is_production {
                        <span class={classes!("rounded-sm", "px-3", "py-1.5", "text-[10px]", "font-light",
                            "uppercase", "tracking-[0.14em]",
                            if all_ready {
                                "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]"
                            } else {
                                "bg-[var(--portal-archive)]/10 text-[var(--portal-archive)]"
                            })}>
                            { if all_ready { "Production secrets ready" } else { "Production secrets incomplete" } }
                        </span>
                    }
                </div>
                <div class="mt-6 grid gap-4 md:grid-cols-3">
                    { for items.iter().map(|(label, ready)| html! {
                        <div class="rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/30 p-4">
                            <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/45">{ *label }</div>
                            <div class={classes!("mt-2", "text-sm", "font-light",
                                if *ready { "text-[var(--portal-success)]" } else { "text-[var(--portal-archive)]" })}>
                                { if *ready { "Ready" } else { "Not configured" } }
                            </div>
                        </div>
                    }) }
                </div>
            </section>
        }
    }
}

/// One figure card: the label, the number, and the line under it that explains which number it is.
fn metric(label: &str, value: &str, detail_text: &str) -> Html {
    html! {
        <div class="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
            <div class="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                { label }
            </div>
            <div class="mt-4 font-serif text-3xl font-light text-[var(--portal-navy)]">{ value }</div>
            <div class="mt-2 text-xs font-light text-black/40">{ detail_text }</div>
        </div>
    }
}

/// One labelled signal inside a section: the small caps label and the value under it.
fn detail(label: &str, value: &str) -> Html {
    html! {
        <div>
            <div class="text-[10px] font-light uppercase tracking-[0.18em] text-black/35">{ label }</div>
            <div class="mt-2 text-sm font-light leading-6 text-black/70">{ value }</div>
        </div>
    }
}

impl SystemHealth {
    /// WORKFLOW DIAGNOSTICS — the half the earlier conversion flattened into a list.
    ///
    /// The engine's own view, in its own section under the health snapshot: the seven summary figures, the anomaly sweep, and
    /// every instance with its detail one click away. Read-only, like everything else on this screen.
    fn workflow_diagnostics(
        &self,
        model: &crate::model::Model,
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
    fn workflow_metrics(&self, diagnostics: &PortalWorkflowDiagnostics) -> Html {
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
    fn workflow_anomalies(&self, diagnostics: &PortalWorkflowDiagnostics) -> Html {
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
    fn workflow_instances(
        &self,
        model: &crate::model::Model,
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
    fn instance_row(
        &self,
        diagnostics: &PortalWorkflowDiagnostics,
        workflow: &crate::model::WorkflowDiagnosticsState,
        instance: &PortalWorkflowInstance,
        on_msg: &Callback<Msg>,
    ) -> Html {
        let selected =
            workflow.selected_instance.as_deref() == Some(instance.instance_id.as_str());
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
    fn open_row(
        &self,
        workflow: &crate::model::WorkflowDiagnosticsState,
        instance: &PortalWorkflowInstance,
        definition_name: Option<&str>,
    ) -> Html {
        if workflow.loading_instance.as_deref() == Some(instance.instance_id.as_str()) {
            return html! {
                <p class="py-6 text-sm font-light text-black/40">{"Loading technical detail…"}</p>
            };
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
fn section_title(title: &str, hint: Option<&str>) -> Html {
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
fn detail_html(label: &str, value: Html) -> Html {
    html! {
        <div>
            <div class="text-[10px] font-light uppercase tracking-[0.18em] text-black/35">{ label }</div>
            <div class="mt-2 break-words text-sm font-light leading-6 text-black/70">{ value }</div>
        </div>
    }
}

/// A table's column heading, matching the other portal tables' small-caps treatment.
fn table_head(label: &str) -> Html {
    html! {
        <th class="px-4 py-3 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
            { label }
        </th>
    }
}

/// A table row: the same hairline separator and hover the portal's other tables use.
const ROW: &str = "border-b border-[var(--portal-border)] transition-colors last:border-b-0 hover:bg-[var(--portal-blue-pale)]/40";

/// One metric card, with the emphasis the live card's red value used.
fn workflow_metric(label: &str, value: &str, detail_text: &str, emphasize: bool) -> Html {
    html! {
        <div class="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
            <div class="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                { label }
            </div>
            <div class={classes!(
                "mt-4",
                "font-serif",
                "text-3xl",
                "font-light",
                if emphasize { "text-red-700" } else { "text-[var(--portal-navy)]" },
            )}>
                { value }
            </div>
            <div class="mt-2 text-xs font-light text-black/40">{ detail_text }</div>
        </div>
    }
}

/// One anomaly: its severity, what it says, and the instance it is about when it names one.
fn anomaly_row(anomaly: &PortalWorkflowAnomaly, index: usize) -> Html {
    html! {
        <div
            key={format!("{}-{}", anomaly.kind, index)}
            class="flex flex-wrap items-start gap-x-3 gap-y-2 rounded-sm border border-[var(--portal-border)]/70 bg-white p-3"
        >
            { severity_pill(&anomaly.severity) }
            <div class="min-w-0 flex-1 text-sm font-light leading-6 text-black/70">
                { &anomaly.message }
            </div>
            if let Some(instance_id) = anomaly.instance_id.as_deref() {
                <span class="font-mono text-[11px] text-black/40">{ instance_id }</span>
            }
        </div>
    }
}

/// What an instance row is about: the property when the projection resolved one, else the subject, else nothing.
fn instance_label(instance: &PortalWorkflowInstance) -> String {
    if let Some(property_name) = instance.property_name.as_deref() {
        return property_name.to_string();
    }
    match instance.subject_type.as_deref() {
        Some(subject_type) => format!(
            "{}:{}",
            subject_type,
            instance.subject_id.as_deref().unwrap_or("—")
        ),
        None => "—".to_string(),
    }
}

/// The definition's own name for an instance, when the deployed definitions were read alongside it.
fn definition_name_for(
    diagnostics: &PortalWorkflowDiagnostics,
    instance: &PortalWorkflowInstance,
) -> Option<String> {
    diagnostics
        .definitions
        .iter()
        .find(|definition: &&PortalWorkflowDefinition| {
            definition.key == instance.definition_key
                && definition.version == instance.definition_version
        })
        .map(|definition| definition.name.clone())
}

/// A severity flag. `critical` is red, `warning` is the softer navy and everything else is the plain one — the live
/// component's three tones, which is why this is not a two-way choice.
fn severity_pill(severity: &str) -> Html {
    let tone = match severity {
        "critical" => "bg-red-50 text-red-700 border-red-200",
        "warning" => {
            "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)] border-[var(--portal-border)]"
        }
        _ => "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)] border-[var(--portal-border)]",
    };
    html! {
        <span class={classes!(PILL, tone)}>
            { severity }
        </span>
    }
}

/// An instance's status, which says the outcome when there is one that matters.
///
/// THE LABEL IS NOT THE STATUS ALONE: an instance whose status is still `active` shows the status, and one that finished
/// shows its outcome — `failed`, `cancelled`, `conflict` — because "completed" is the least interesting thing about a run
/// that ended badly. That rule, and the three tones, are the live pill's.
fn status_pill(status: &str, outcome: Option<&str>) -> Html {
    let mut tone =
        "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)] border-[var(--portal-border)]";
    if status == "completed" {
        tone = "bg-emerald-50 text-emerald-700 border-emerald-200";
    } else if status == "error" || outcome == Some("failed") || outcome == Some("conflict") {
        tone = "bg-red-50 text-red-700 border-red-200";
    } else if outcome == Some("cancelled") || status == "suspended" {
        tone =
            "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)] border-[var(--portal-border)]";
    }
    let label = match outcome {
        Some(outcome) if status != "active" => outcome.to_string(),
        _ => status.to_string(),
    };
    html! {
        <span class={classes!(PILL, tone)}>
            { label }
        </span>
    }
}

/// An instant as the reader's browser prints it, and the raw value when it is not a date at all.
///
/// THE BROWSER FORMATS IT, NOT THE SERVER: the live screen called `toLocaleString`, so a timestamp reads in the operator's
/// own zone. `js_sys::Date` is that formatter — reaching for it here is parity, not convenience.
fn format_instant(value: &str) -> String {
    if value.is_empty() {
        return "—".to_string();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(value));
    if date.get_time().is_nan() {
        return value.to_string();
    }
    date.to_locale_string("en-GB", &wasm_bindgen::JsValue::UNDEFINED)
        .as_string()
        .unwrap_or_else(|| value.to_string())
}

/// ONE INSTANCE'S TECHNICAL DETAIL: the live component's eight sections, in its order.
///
/// This is the part of the screen that answers "what actually happened in this run" — the tokens the engine is holding, the
/// tasks it created, the correlations to the canonical tasks, the timers, the facts, the commands and their receipts, and the
/// engine's own event log. Nothing here is aggregated: it is the run, as the engine recorded it.
fn instance_detail(detail: &PortalWorkflowInstanceDetail, definition_name: Option<&str>) -> Html {
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
fn detail_definition(
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
fn detail_subject(detail: &PortalWorkflowInstanceDetail) -> String {
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
fn detail_ended(detail: &PortalWorkflowInstanceDetail) -> String {
    detail
        .ended_at
        .as_deref()
        .map(format_instant)
        .unwrap_or_else(|| "—".to_string())
}

/// The canonical correlations: which of the application's tasks this run's engine tasks are attached to, and how those stand.
fn correlations_section(detail: &PortalWorkflowInstanceDetail) -> Html {
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

fn correlation_row(correlation: &PortalWorkflowCorrelation) -> Html {
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
fn jobs_section(detail: &PortalWorkflowInstanceDetail) -> Html {
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

fn job_row(job: &PortalWorkflowJob) -> Html {
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
fn facts_section(detail: &PortalWorkflowInstanceDetail) -> Html {
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
fn commands_section(detail: &PortalWorkflowInstanceDetail) -> Html {
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

fn command_row(command: &crate::model::PortalWorkflowCommand) -> Html {
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
fn receipt_pill(receipt_outcome: Option<&str>) -> Html {
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
fn events_section(detail: &PortalWorkflowInstanceDetail) -> Html {
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

fn event_row(event: &PortalWorkflowDiagnosticEvent) -> Html {
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
fn task_row(task: &PortalWorkflowTask) -> Html {
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
fn token_tree(
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
fn token_branch<'a>(
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
fn facts_block(variables: Option<&serde_json::Value>) -> Html {
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

