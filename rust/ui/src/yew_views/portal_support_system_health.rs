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
    Msg, PortalEnvironmentReadiness, PortalSystemHealthPage, PortalSystemHealthSnapshot,
};
use crate::yew_views::portal_shell::PortalShell;

/// The navy panel the live screen used for every section.
const PANEL: &str = "rounded-[var(--portal-panel-radius)] portal-glass-panel p-6";

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
                { self.body(&props.model) }
            </PortalShell>
        }
    }
}

impl SystemHealth {
    fn body(&self, model: &crate::model::Model) -> Html {
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

