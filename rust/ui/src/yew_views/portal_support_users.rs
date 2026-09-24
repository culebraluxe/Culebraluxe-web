//! `/portal/settings/users` — internal security-role administration.
//!
//! The model owns every draft and command. This component only renders the current
//! Security-service projection and emits intents; it never writes a database or
//! decides authorization.

use yew::prelude::*;

use crate::model::{Msg, PortalSecurityUser};
use crate::yew_views::portal_shell::PortalShell;

const PANEL: &str = "portal-glass-panel rounded-[var(--portal-panel-radius)]";
const ROLES: [(&str, &str); 5] = [
    ("internal_guest", "Internal Guest"),
    ("user", "User"),
    ("business_power_user", "Business Power User"),
    ("owner", "Owner"),
    ("root", "Root"),
];

#[derive(Properties, PartialEq)]
pub struct SecurityUsersProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct SecurityUsers;

impl Component for SecurityUsers {
    type Message = ();
    type Properties = SecurityUsersProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("settings-users")
            .expect("the Settings Users screen is in the registry");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { self.body(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

impl SecurityUsers {
    fn body(&self, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let support = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.support.as_ref());
        let users = support.map(|support| support.security_users.as_slice());
        let can_manage = model.can("security.role.manage");

        html! {
            <div>
                <div class="mb-8">
                    <p class="text-xs font-light uppercase tracking-[0.28em] text-black/40">{"Security"}</p>
                    <h1 class="mt-3 font-serif text-4xl font-light leading-[1.1]">{"Application Users"}</h1>
                    <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
                        {"Internal application actors and their primary security roles. Role assignment is ROOT-only;                           changes take effect on the next identity resolution request."}
                    </p>
                </div>

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
        model: &crate::model::Model,
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
