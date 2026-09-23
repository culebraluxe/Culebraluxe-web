//! `/portal/settings` — the Security landing screen.
//!
//! PARITY WITH `app/portal/settings/page.tsx` at `141df386` (the parent of `34d2fa87`): the eyebrow and heading, the three
//! navigation cards to Users, Roles and Authorities, then the Security Status panel with its nine counts and the
//! Break-glass readiness panel with its six conditions.
//!
//! READ-ONLY, AND THE CARDS ARE LINKS. Users, Roles and Authorities are their own routes that still render what they render;
//! these three cards are navigation, drawn as anchors because the portal's URLs belong to Next rather than to this app's
//! router. Nothing on this screen creates, edits or assigns anything.
//!
//! WHAT IS NOT ON THIS SCREEN, AND MUST NOT BE: the break-glass secret, its hash, the root user's id, any token, and any
//! credential of any kind. The panel says whether each condition holds — "Ready" or "Not configured" — which is exactly what
//! the pre-cutover panel said. The six booleans are the whole vocabulary.

use yew::prelude::*;

use crate::model::{Msg, PortalBreakGlassReadiness, PortalSecurity, PortalSecurityStatus};
use crate::yew_views::portal_shell::PortalShell;

/// The light surface the portal's read-only pages use.
const PANEL: &str = "portal-glass-panel rounded-[var(--portal-panel-radius)]";
const SOFT_PANEL: &str = "portal-glass-panel portal-glass-panel-soft rounded-[var(--portal-panel-radius)]";

/// The three destinations, with the words the pre-cutover page used for each.
const SECTIONS: [(&str, &str, &str); 3] = [
    (
        "/portal/settings/users",
        "Users",
        "Application actors and their role assignments. CRM people are not authentication principals.",
    ),
    (
        "/portal/settings/roles",
        "Roles",
        "Named bundles of authorities, split between internal and external account types.",
    ),
    (
        "/portal/settings/authorities",
        "Authorities",
        "Coarse application capabilities that roles are built from.",
    ),
];

#[derive(Properties, PartialEq)]
pub struct SecurityProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Security;

impl Component for Security {
    type Message = ();
    type Properties = SecurityProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("security").expect("the Security screen is in the registry");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { self.body(&props.model) }
            </PortalShell>
        }
    }
}

impl Security {
    fn body(&self, model: &crate::model::Model) -> Html {
        let read = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.support.as_ref())
            .and_then(|support| support.security.clone());
        html! {
            <div>
                <div class="mb-8">
                    <p class="text-xs font-light uppercase tracking-[0.28em] text-black/40">{"Security"}</p>
                    <h1 class="mt-3 font-serif text-4xl font-light leading-[1.1]">{"Application Security Model"}</h1>
                    <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
                        {"Canonical users, roles, and authorities. Read-only views for now — assignment and management arrive \
                          with authentication and authorization."}
                    </p>
                </div>
                { self.sections() }
                { self.status_panel(read.as_ref().map(|read| &read.status)) }
                { self.break_glass_panel(read.as_ref().map(|read| &read.break_glass)) }
            </div>
        }
    }

    /// The three destinations, exactly as the live page drew them.
    fn sections(&self) -> Html {
        html! {
            <section class="grid gap-4 md:grid-cols-3">
                { for SECTIONS.iter().map(|(href, title, intro)| html! {
                    <a href={*href}
                        class={classes!(SOFT_PANEL, "group", "block", "p-6", "transition",
                            "hover:border-[var(--portal-navy-soft)]")}>
                        <h2 class="font-serif text-2xl font-light">{ *title }</h2>
                        <p class="mt-2 text-sm font-light leading-6 text-black/50">{ *intro }</p>
                        <div class="mt-4 text-xs font-light uppercase tracking-[0.16em] text-[var(--portal-navy-soft)]">
                            {"Open →"}
                        </div>
                    </a>
                }) }
            </section>
        }
    }
}

impl Security {
    /// The nine counts, or the statement that they have not arrived.
    ///
    /// The owner-assignment badge is the live panel's: amber when nobody holds the owner role, navy when somebody does. That
    /// is the one judgement the panel made, and it is a judgement about a count rather than about a person.
    fn status_panel(&self, status: Option<&PortalSecurityStatus>) -> Html {
        let Some(status) = status else {
            return html! {
                <section class={classes!(PANEL, "mt-6", "p-6")}>
                    <h2 class="font-serif text-2xl font-light">{"Security Status"}</h2>
                    <p class="mt-3 text-sm font-light text-black/40">{"Reading the security projection…"}</p>
                </section>
            };
        };
        let items: [(&str, i64, &str); 9] = [
            ("Active internal users", status.active_internal_users, "Internal accounts currently enabled"),
            ("External users", status.external_users, "Customer/client accounts"),
            ("Users with no role", status.users_with_no_role, "Active actors that would lack authority"),
            ("Users with multiple roles", status.users_with_multiple_roles, "Actors holding more than one role"),
            ("Mapped auth identities", status.mapped_auth_identities, "Provider subjects linked to app_users"),
            ("Unmapped app_users", status.unmapped_app_users, "Application users with no provider identity"),
            ("Owner assignments", status.owner_role_assignments, "Current owner-role holders"),
            ("Inactive users w/ active role", status.inactive_users_with_active_role_mappings, "Disabled actors still mapped to an active role"),
            ("Account-type mismatches", status.account_type_mismatch_count, "Should always be 0 (invariant)"),
        ];
        let owner_assigned = status.owner_role_assignments != 0;
        html! {
            <section class={classes!(PANEL, "mt-6", "p-6")}>
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <div>
                        <h2 class="font-serif text-2xl font-light">{"Security Status"}</h2>
                        <p class="mt-1 text-xs font-light text-black/40">
                            {"Operational security facts — no tokens or credentials."}
                        </p>
                    </div>
                    <span class={classes!("rounded-sm", "px-3", "py-1.5", "text-[10px]", "font-light",
                        "uppercase", "tracking-[0.14em]",
                        if owner_assigned {
                            "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]"
                        } else {
                            "bg-[var(--portal-archive)]/10 text-[var(--portal-archive)]"
                        })}>
                        { if owner_assigned { "Owner assigned" } else { "No owner assigned" } }
                    </span>
                </div>
                <div class="mt-6 grid gap-4 md:grid-cols-3">
                    { for items.iter().map(|(label, value, detail)| html! {
                        <div class="rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/30 p-4">
                            <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/45">{ *label }</div>
                            <div class="mt-2 font-serif text-2xl font-light">{ value.to_string() }</div>
                            <div class="mt-1 text-xs font-light text-black/40">{ *detail }</div>
                        </div>
                    }) }
                </div>
            </section>
        }
    }
}


impl Security {
    /// Break-glass posture: six conditions, each ready or not.
    ///
    /// THE PANEL PRINTS NO CONFIGURATION. Not the root user's id, not the secret's hash, not whether a secret is set beyond
    /// the single "configured" boolean the readiness projection already exposed. "Not configured" covers a wrong value and a
    /// missing one, and that is deliberate: a diagnostic that distinguishes them tells a reader something about a secret
    /// they do not need to know.
    ///
    /// THE DASH IS NOT AN ERROR STATE. A configuration that is not set up is not a failure to load — it is a fact about the
    /// deployment, and the panel states it in the same calm register as "Ready".
    fn break_glass_panel(&self, readiness: Option<&PortalBreakGlassReadiness>) -> Html {
        let Some(readiness) = readiness else {
            return html! {
                <section class={classes!(PANEL, "mt-6", "p-6")}>
                    <h2 class="font-serif text-2xl font-light">{"Break-glass readiness"}</h2>
                    <p class="mt-3 text-sm font-light text-black/40">{"Reading the readiness projection…"}</p>
                </section>
            };
        };
        let items: [(&str, bool); 6] = [
            ("Break-glass configured", readiness.configured),
            ("Break-glass enabled", readiness.enabled),
            ("Root user resolvable", readiness.root_resolvable),
            ("Root user active", readiness.root_active),
            ("Root holds owner role", readiness.owner_role_present),
            ("Security audit table", readiness.audit_table_available),
        ];
        html! {
            <section class={classes!(PANEL, "mt-6", "p-6")}>
                <h2 class="font-serif text-2xl font-light">{"Break-glass readiness"}</h2>
                <p class="mt-1 text-xs font-light text-black/40">
                    {"Emergency root access posture. No secrets, hashes, or tokens shown."}
                </p>
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

