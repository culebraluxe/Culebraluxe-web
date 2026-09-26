//! `/portal/settings/users` — internal application users and their primary security role. ROOT may replace a user's
//! primary role (`ctx.can`, visibility only; the Rust security service decides and refuses removing the last ROOT).
//! A change is a draft until applied; one apply at a time; its answer is the user table as it now stands.

use std::collections::BTreeMap;

use yew::prelude::*;

use crate::app::api::{PortalScreenPage, SetUserPrimaryRole};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{PortalPage, PortalSecurityUser};

pub struct SecurityUsers;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<Vec<PortalSecurityUser>>,
    /// The role chosen for a user and not yet applied.
    pub security_user_role_drafts: BTreeMap<String, String>,
    /// The user whose change is being applied.
    pub user_role_busy: Option<String>,
    /// Why the last change was refused.
    pub error: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    SecurityUserRoleDraftChanged {
        app_user_id: String,
        role_code: String,
    },
    SecurityUserRoleRequested {
        app_user_id: String,
        role_code: String,
    },
    RoleAnswered(Result<Vec<PortalSecurityUser>, ApiError>),
}

/// The five canonical internal roles a primary role may be.
fn is_canonical_internal_role(code: &str) -> bool {
    matches!(
        code,
        "internal_guest" | "user" | "business_power_user" | "owner" | "root"
    )
}

impl Screen for SecurityUsers {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
                ..Model::default()
            },
            Cmd::request(PortalScreenPage::of("settings-users"), Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded(answer) => {
                model.read = Remote::from_result(answer.map(|page| {
                    page.support
                        .map(|support| support.security_users)
                        .unwrap_or_default()
                }));
            }
            Msg::SecurityUserRoleDraftChanged {
                app_user_id,
                role_code,
            } => {
                if is_canonical_internal_role(&role_code) && !app_user_id.trim().is_empty() {
                    model
                        .security_user_role_drafts
                        .insert(app_user_id, role_code);
                    model.error = None;
                }
            }
            Msg::SecurityUserRoleRequested {
                app_user_id,
                role_code,
            } => {
                if model.user_role_busy.is_some()
                    || !ctx.can("security.role.manage")
                    || !is_canonical_internal_role(&role_code)
                    || app_user_id.trim().is_empty()
                {
                    return Cmd::none();
                }
                model.user_role_busy = Some(app_user_id.clone());
                model.error = None;
                return Cmd::request(
                    SetUserPrimaryRole {
                        app_user_id,
                        role_code,
                    },
                    |answer| Msg::RoleAnswered(answer.map(|answer| answer.users)),
                );
            }
            Msg::RoleAnswered(answer) => {
                model.user_role_busy = None;
                match answer {
                    Ok(users) => {
                        model.security_user_role_drafts.clear();
                        model.read = Remote::Loaded(users);
                    }
                    Err(error) => model.error = Some(error.message),
                }
            }
        }
        Cmd::none()
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        SecurityUsers.body(model, ctx, &link.callback(|msg: Msg| msg))
    }
}

const PANEL: &str = "portal-glass-panel rounded-[var(--portal-panel-radius)]";
const ROLES: [(&str, &str); 5] = [
    ("internal_guest", "Internal Guest"),
    ("user", "User"),
    ("business_power_user", "Business Power User"),
    ("owner", "Owner"),
    ("root", "Root"),
];

impl SecurityUsers {
    fn body(&self, model: &Model, ctx: &ScreenCtx, on_msg: &Callback<Msg>) -> Html {
        if let Remote::Failed(error) = &model.read {
            return template::failure(error);
        }
        let users = model.read.loaded().map(Vec::as_slice);
        let can_manage = ctx.can("security.role.manage");

        html! {
            <div>
                <div class="mb-8">
                    <p class="text-xs font-light uppercase tracking-[0.28em] text-black/40">{"Security"}</p>
                    <h1 class="mt-3 font-serif text-4xl font-light leading-[1.1]">{"Application Users"}</h1>
                    <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
                        {"Internal application actors and their primary security roles. Role assignment is ROOT-only;                           changes take effect on the next identity resolution request."}
                    </p>
                </div>

                if let Some(error) = &model.error {
                    <div class="mb-4 rounded-md border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm" role="alert">{ error.clone() }</div>
                }
                <section class={classes!(PANEL, "p-6")}>
                    if let Some(users) = users {
                        if users.is_empty() {
                            <p class="text-sm font-light text-black/45">{"No internal application users."}</p>
                        } else {
                            <div class="overflow-x-auto">
                                <table class="w-full text-left text-sm">
                                    <thead>
                                        <tr class="text-xs uppercase tracking-[0.12em] text-black/45">
                                            <th class="pb-3 pr-5">{"User"}</th>
                                            <th class="pb-3 pr-5">{"Email"}</th>
                                            <th class="pb-3 pr-5">{"Status"}</th>
                                            <th class="pb-3 pr-5">{"Effective role"}</th>
                                            if can_manage {
                                                <th class="pb-3">{"Assignment"}</th>
                                            }
                                        </tr>
                                    </thead>
                                    <tbody>
                                        { for users.iter().map(|user| self.user_row(model, user, on_msg, can_manage)) }
                                    </tbody>
                                </table>
                            </div>
                        }
                    } else {
                        <p class="text-sm font-light text-black/40">{"Reading application users…"}</p>
                    }
                </section>

                if can_manage {
                    <p class="mt-4 text-xs font-light leading-5 text-black/45">
                        {"ROOT may replace the primary role. Specialist additive roles are preserved.                           The service refuses any change that would remove the final active ROOT."}
                    </p>
                }
            </div>
        }
    }

    fn user_row(
        &self,
        model: &Model,
        user: &PortalSecurityUser,
        on_msg: &Callback<Msg>,
        can_manage: bool,
    ) -> Html {
        let current = user
            .primary_role_code
            .clone()
            .unwrap_or_else(|| "internal_guest".to_string());
        let draft = model
            .security_user_role_drafts
            .get(&user.app_user_id)
            .cloned()
            .unwrap_or_else(|| current.clone());
        let busy = model.user_role_busy.as_deref() == Some(user.app_user_id.as_str());
        let changed = draft != current;

        let select_role = {
            let on_msg = on_msg.clone();
            let app_user_id = user.app_user_id.clone();
            Callback::from(move |event: Event| {
                let role_code = event
                    .target_unchecked_into::<web_sys::HtmlSelectElement>()
                    .value();
                on_msg.emit(Msg::SecurityUserRoleDraftChanged {
                    app_user_id: app_user_id.clone(),
                    role_code,
                });
            })
        };
        let save_role = {
            let on_msg = on_msg.clone();
            let app_user_id = user.app_user_id.clone();
            let role_code = draft.clone();
            Callback::from(move |_: MouseEvent| {
                on_msg.emit(Msg::SecurityUserRoleRequested {
                    app_user_id: app_user_id.clone(),
                    role_code: role_code.clone(),
                });
            })
        };

        html! {
            <tr key={user.app_user_id.clone()} class="border-t border-black/10 align-middle">
                <td class="py-4 pr-5">
                    <div class="font-medium">{user.display_name.clone()}</div>
                    if user.role_codes.len() > 1 {
                        <div class="mt-1 text-xs font-light text-black/40">
                            {format!("Roles: {}", user.role_codes.join(", "))}
                        </div>
                    }
                </td>
                <td class="py-4 pr-5 font-light">{user.email.clone().unwrap_or_else(|| "—".into())}</td>
                <td class="py-4 pr-5">
                    <span class="text-xs font-light uppercase tracking-[0.1em]">
                        {if user.active {"Active"} else {"Inactive"}}
                    </span>
                </td>
                <td class="py-4 pr-5">{role_label(&current)}</td>
                if can_manage {
                    <td class="py-4">
                        <div class="flex min-w-[23rem] items-center gap-3">
                            <select
                                value={draft.clone()}
                                onchange={select_role}
                                disabled={!user.active || model.user_role_busy.is_some()}
                                class="min-h-10 flex-1 rounded border border-black/20 bg-white px-3 py-2 text-sm disabled:opacity-40"
                            >
                                { for ROLES.iter().map(|(code, label)| html! {
                                    <option key={*code} value={*code}>{*label}</option>
                                }) }
                            </select>
                            <button
                                type="button"
                                onclick={save_role}
                                disabled={!user.active || !changed || model.user_role_busy.is_some()}
                                class="min-h-10 rounded border border-black/20 px-4 text-xs uppercase tracking-[0.12em] disabled:opacity-35"
                            >
                                {if busy {"Saving…"} else {"Save"}}
                            </button>
                        </div>
                    </td>
                }
            </tr>
        }
    }
}

fn role_label(code: &str) -> &'static str {
    ROLES
        .iter()
        .find_map(|(candidate, label)| (*candidate == code).then_some(*label))
        .unwrap_or("Unmapped")
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
    fn the_real_answer_decodes_and_a_role_change_is_a_draft_until_applied() {
        let (mut model, cmd) = SecurityUsers::init(&root());
        let answer: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/portal-page-settings-users.json"
        ))
        .unwrap();
        SecurityUsers::update(
            &mut model,
            cmd.into_requests().remove(0).respond(Ok(answer)),
            &root(),
        );
        let user = model.read.loaded().expect("the real payload decodes")[0]
            .app_user_id
            .clone();

        SecurityUsers::update(
            &mut model,
            Msg::SecurityUserRoleDraftChanged {
                app_user_id: user.clone(),
                role_code: "owner".into(),
            },
            &root(),
        );
        SecurityUsers::update(
            &mut model,
            Msg::SecurityUserRoleDraftChanged {
                app_user_id: user.clone(),
                role_code: "emperor".into(),
            },
            &root(),
        );
        assert_eq!(
            model
                .security_user_role_drafts
                .get(&user)
                .map(String::as_str),
            Some("owner"),
            "only canonical roles"
        );

        let apply = Msg::SecurityUserRoleRequested {
            app_user_id: user.clone(),
            role_code: "owner".into(),
        };
        let request = SecurityUsers::update(&mut model, apply, &root())
            .into_requests()
            .remove(0);
        assert_eq!(
            request.body,
            Some(serde_json::json!({ "appUserId": user, "roleCode": "owner" }))
        );
        SecurityUsers::update(
            &mut model,
            request.respond(Ok(serde_json::json!({ "users": [] }))),
            &root(),
        );
        assert!(model.user_role_busy.is_none() && model.security_user_role_drafts.is_empty());
        assert_eq!(
            model.read.loaded().map(Vec::len),
            Some(0),
            "the answer is the table as it now stands"
        );
    }
}
