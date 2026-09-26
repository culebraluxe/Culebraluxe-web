//! `/portal/settings` — the application security model: the destinations (users, roles, authorities), the security
//! status, the entitlements each internal role holds (ROOT may grant or revoke), and break-glass readiness.
//!
//! A grant is a command: one at a time, offered only to ROOT (`ctx.can`, visibility only — the Rust security service
//! decides), and its answer is the role table as it now stands.

use yew::prelude::*;

use crate::app::api::{PortalScreenPage, SetRoleEntitlement};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{
    PortalBreakGlassReadiness, PortalPage, PortalRoleEntitlements, PortalSecurity,
    PortalSecurityStatus,
};

pub struct Security;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalSecurity>,
    pub selected_security_role: Option<String>,
    pub role_grant_busy: bool,
    /// Why the last grant was refused.
    pub error: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    SecurityRoleSelected(String),
    SecurityRoleGrantRequested {
        role_code: String,
        action: String,
        granted: bool,
    },
    GrantAnswered(Result<Vec<PortalRoleEntitlements>, ApiError>),
}

impl Screen for Security {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
                ..Model::default()
            },
            Cmd::request(PortalScreenPage::of("security"), Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded(answer) => {
                model.read = Remote::from_result(answer.and_then(|page| {
                    page.support
                        .and_then(|support| support.security)
                        .ok_or_else(|| ApiError::decode("The answer had no security model in it."))
                }));
            }
            Msg::SecurityRoleSelected(role) => model.selected_security_role = Some(role),
            Msg::SecurityRoleGrantRequested {
                role_code,
                action,
                granted,
            } => {
                if model.role_grant_busy || !ctx.can("security.entitlement.manage") {
                    return Cmd::none();
                }
                model.role_grant_busy = true;
                model.error = None;
                return Cmd::request(
                    SetRoleEntitlement {
                        role_code,
                        action,
                        granted,
                    },
                    |answer| Msg::GrantAnswered(answer.map(|answer| answer.roles)),
                );
            }
            Msg::GrantAnswered(answer) => {
                model.role_grant_busy = false;
                match answer {
                    Ok(roles) => {
                        if let Remote::Loaded(read) = &mut model.read {
                            read.role_entitlements = roles;
                        }
                    }
                    Err(error) => model.error = Some(error.message),
                }
            }
        }
        Cmd::none()
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        Security.body(model, ctx, &link.callback(|msg: Msg| msg))
    }
}

/// The light surface the portal's read-only pages use.
const PANEL: &str = "portal-glass-panel rounded-[var(--portal-panel-radius)]";
const SOFT_PANEL: &str =
    "portal-glass-panel portal-glass-panel-soft rounded-[var(--portal-panel-radius)]";

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

impl Security {
    fn body(&self, model: &Model, ctx: &ScreenCtx, on_msg: &Callback<Msg>) -> Html {
        let read = model.read.loaded().cloned();
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
                if let Remote::Failed(error) = &model.read { { template::failure(error) } }
                if let Some(error) = &model.error {
                    <div class="mb-4 rounded-md border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm" role="alert">{ error.clone() }</div>
                }
                { self.entitlement_panel(model, ctx, read.as_ref().map(|read| read.role_entitlements.as_slice()), on_msg) }
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
    fn entitlement_panel(
        &self,
        model: &Model,
        ctx: &ScreenCtx,
        roles: Option<&[crate::model::PortalRoleEntitlements]>,
        on_msg: &Callback<Msg>,
    ) -> Html {
        let selected = roles.and_then(|roles| {
            roles
                .iter()
                .filter(|role| role.account_type == "internal")
                .find(|role| {
                    model.selected_security_role.as_deref() == Some(role.role_code.as_str())
                })
                .or_else(|| {
                    roles
                        .iter()
                        .find(|role| role.role_code == "user" && role.account_type == "internal")
                })
                .or_else(|| roles.iter().find(|role| role.account_type == "internal"))
        });
        let actions = roles
            .map(|roles| {
                roles
                    .iter()
                    .flat_map(|role| role.entitlement_codes.iter().cloned())
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        let select_role = {
            let on_msg = on_msg.clone();
            Callback::from(move |event: Event| {
                on_msg.emit(Msg::SecurityRoleSelected(
                    event
                        .target_unchecked_into::<web_sys::HtmlSelectElement>()
                        .value(),
                ));
            })
        };
        html! {
            <section class={classes!(PANEL, "mt-6", "p-6")}>
                <h2 class="font-serif text-2xl font-light">{"Role entitlements"}</h2>
                <p class="mt-2 text-xs font-light text-black/45">
                    {"Effective service actions by active role. External guest has no portal grants."}
                </p>
                if let Some(roles) = roles {
                    <div class="mt-4 overflow-x-auto">
                        <table class="w-full text-left text-xs">
                            <thead><tr><th class="py-2">{"Role"}</th><th>{"Account"}</th><th>{"Granted actions"}</th></tr></thead>
                            <tbody>
                                { for roles.iter().map(|role| html! {
                                    <tr key={role.role_code.clone()} class="border-t border-black/10 align-top">
                                        <td class="py-3 pr-4 font-medium">{ role.role_code.clone() }</td>
                                        <td class="py-3 pr-4">{ role.account_type.clone() }</td>
                                        <td class="py-3 font-light">{ role.entitlement_codes.join(", ") }</td>
                                    </tr>
                                }) }
                            </tbody>
                        </table>
                    </div>
                    if ctx.can("security.entitlement.manage") {
                        if let Some(role) = selected {
                            <div class="mt-6 border-t border-black/10 pt-5">
                                <label for="role-grant-selector" class="text-xs font-medium uppercase tracking-[0.12em]">{"Edit role grants"}</label>
                                <select id="role-grant-selector" value={role.role_code.clone()} onchange={select_role}
                                    class="ml-3 rounded border border-black/20 bg-white px-3 py-2 text-sm">
                                    { for roles.iter().filter(|role| role.account_type == "internal").map(|role| html! {
                                        <option key={role.role_code.clone()} value={role.role_code.clone()}>{role.role_code.clone()}</option>
                                    }) }
                                </select>
                                <div class="mt-4 grid gap-2 md:grid-cols-2">
                                    { for actions.iter().filter(|code| !matches!(
                                        code.as_str(),
                                        "security.entitlement.manage" | "security.role.manage"
                                    )).map(|code| {
                                        let granted = role.entitlement_codes.contains(code);
                                        let on_msg = on_msg.clone();
                                        let role_code = role.role_code.clone();
                                        let action = code.clone();
                                        html! {
                                            <button key={code.clone()} type="button" disabled={model.role_grant_busy}
                                                onclick={Callback::from(move |_: MouseEvent| on_msg.emit(Msg::SecurityRoleGrantRequested {
                                                    role_code: role_code.clone(), action: action.clone(), granted: !granted,
                                                }))}
                                                class="flex min-h-10 items-center justify-between gap-3 rounded border border-black/10 px-3 py-2 text-left text-xs disabled:opacity-40">
                                                <span>{code.clone()}</span><span>{if granted {"Granted"} else {"Denied"}}</span>
                                            </button>
                                        }
                                    }) }
                                </div>
                            </div>
                        }
                    }
                } else {
                    <p class="mt-3 text-sm font-light text-black/40">{"Reading role entitlements…"}</p>
                }
            </section>
        }
    }

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
            (
                "Active internal users",
                status.active_internal_users,
                "Internal accounts currently enabled",
            ),
            (
                "External users",
                status.external_users,
                "Customer/client accounts",
            ),
            (
                "Users with no role",
                status.users_with_no_role,
                "Active actors that would lack authority",
            ),
            (
                "Users with multiple roles",
                status.users_with_multiple_roles,
                "Actors holding more than one role",
            ),
            (
                "Mapped auth identities",
                status.mapped_auth_identities,
                "Provider subjects linked to app_users",
            ),
            (
                "Unmapped app_users",
                status.unmapped_app_users,
                "Application users with no provider identity",
            ),
            (
                "Owner assignments",
                status.owner_role_assignments,
                "Current owner-role holders",
            ),
            (
                "Inactive users w/ active role",
                status.inactive_users_with_active_role_mappings,
                "Disabled actors still mapped to an active role",
            ),
            (
                "Account-type mismatches",
                status.account_type_mismatch_count,
                "Should always be 0 (invariant)",
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PortalEntitlements;

    fn root() -> ScreenCtx {
        ScreenCtx {
            grants: Some(PortalEntitlements {
                account_type: "internal".into(),
                security_level: "ROOT".into(),
                is_root: true,
                entitlement_codes: vec![],
            }),
            ..ScreenCtx::default()
        }
    }

    #[test]
    fn only_root_grants_one_at_a_time_and_the_answer_replaces_the_table() {
        let (mut model, cmd) = Security::init(&root());
        let answer: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/portal-page-security.json"))
                .unwrap();
        Security::update(
            &mut model,
            cmd.into_requests().remove(0).respond(Ok(answer)),
            &root(),
        );
        assert!(model.read.loaded().is_some());

        let grant = || Msg::SecurityRoleGrantRequested {
            role_code: "user".into(),
            action: "deal.read".into(),
            granted: true,
        };
        assert!(
            Security::update(&mut model, grant(), &ScreenCtx::default())
                .into_requests()
                .is_empty(),
            "not offered without ROOT"
        );
        let request = Security::update(&mut model, grant(), &root())
            .into_requests()
            .remove(0);
        assert_eq!(request.method, crate::app::cmd::Method::Put);
        assert_eq!(
            request.body,
            Some(serde_json::json!({ "roleCode": "user", "action": "deal.read", "granted": true }))
        );
        assert!(
            Security::update(&mut model, grant(), &root())
                .into_requests()
                .is_empty(),
            "one at a time"
        );

        Security::update(
            &mut model,
            request.respond(Err(ApiError {
                status: 403,
                code: "FORBIDDEN".into(),
                message: "Not ROOT.".into(),
            })),
            &root(),
        );
        assert_eq!(
            (model.role_grant_busy, model.error.as_deref()),
            (false, Some("Not ROOT."))
        );
    }
}
