//! The site's plain-text pages: Privacy (`/privacy`, which Meta requires for WhatsApp Business), the unauthorized
//! notice (`/login/unauthorized`), and emergency administrative access (`/login/recovery`).
//!
//! EMERGENCY ACCESS IS A FORM AGAIN. The page had lost its form in an earlier conversion and showed only its two
//! sentences, so the outage-recovery path (Auth.js's `break-glass` provider) could not be used. It posts the recovery
//! credential to Auth.js with Auth.js's form token, like every other sign-in form in this app (see `account.rs`).

use yew::prelude::*;

use crate::app::api::{AuthCsrf, CsrfToken};
use crate::app::cmd::{ApiError, Cmd};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::site::{StaticPage, StaticPageSpec};
use crate::view::{
    LOGIN_RECOVERY_VIEW_CONTENT, LOGIN_UNAUTHORIZED_VIEW_CONTENT, PRIVACY_VIEW_CONTENT,
};

/// A page of headings, paragraphs and list items, in order.
fn article(content: &[(&str, &str)], extra: Html) -> Html {
    html! {
        <article class="px-6 pb-20 pt-32 md:px-12 md:pb-28 md:pt-40">
            <div class="mx-auto max-w-4xl space-y-6">
                { for content.iter().map(|(kind, text)| match *kind {
                    "h1" => html! { <h1 class="font-serif text-3xl font-light text-foreground md:text-4xl">{ *text }</h1> },
                    "h2" | "h3" => html! { <h2 class="font-serif text-2xl font-light text-foreground">{ *text }</h2> },
                    "li" => html! { <li class="ml-6 list-disc text-sm font-light leading-7 text-muted-foreground">{ *text }</li> },
                    _ => html! { <p class="mt-4 text-sm font-light leading-7 text-muted-foreground">{ *text }</p> },
                }) }
                { extra }
            </div>
        </article>
    }
}

pub struct Privacy;

impl StaticPageSpec for Privacy {
    fn view(_ctx: &ScreenCtx) -> Html {
        article(&PRIVACY_VIEW_CONTENT, Html::default())
    }
}

pub type PrivacyPage = StaticPage<Privacy>;

pub struct Unauthorized;

impl StaticPageSpec for Unauthorized {
    fn view(_ctx: &ScreenCtx) -> Html {
        article(
            &LOGIN_UNAUTHORIZED_VIEW_CONTENT,
            html! {
                <div class="mt-10 flex flex-wrap gap-6 text-xs font-light uppercase tracking-[0.22em]">
                    <a href="/login" class="text-foreground underline-offset-4 hover:underline">{"Sign in with another account"}</a>
                    <a href="/" class="text-muted-foreground underline-offset-4 hover:underline">{"Return home"}</a>
                </div>
            },
        )
    }
}

pub type UnauthorizedPage = StaticPage<Unauthorized>;

/// `/login/recovery` — the break-glass sign-in.
pub struct Recovery;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RecoveryModel {
    pub csrf_token: String,
    /// Auth.js sends a refused credential back with `?error=`.
    pub refused: bool,
}

#[derive(Debug, PartialEq)]
pub enum RecoveryMsg {
    CsrfLoaded(Result<CsrfToken, ApiError>),
}

impl Screen for Recovery {
    type Model = RecoveryModel;
    type Msg = RecoveryMsg;

    fn init(ctx: &ScreenCtx) -> (RecoveryModel, Cmd<RecoveryMsg>) {
        (
            RecoveryModel {
                refused: ctx.query("error").is_some(),
                ..RecoveryModel::default()
            },
            Cmd::request(AuthCsrf, RecoveryMsg::CsrfLoaded),
        )
    }

    fn update(model: &mut RecoveryModel, msg: RecoveryMsg, _ctx: &ScreenCtx) -> Cmd<RecoveryMsg> {
        let RecoveryMsg::CsrfLoaded(answer) = msg;
        model.csrf_token = answer.map(|token| token.csrf_token).unwrap_or_default();
        Cmd::none()
    }

    fn view(model: &RecoveryModel, _ctx: &ScreenCtx, _link: &Link<RecoveryMsg>) -> Html {
        let ready = !model.csrf_token.is_empty();
        article(
            &LOGIN_RECOVERY_VIEW_CONTENT,
            html! {
                <form method="post" action="/api/auth/callback/break-glass" class="mt-10 max-w-md space-y-4">
                    <input type="hidden" name="csrfToken" value={model.csrf_token.clone()} />
                    <input type="hidden" name="callbackUrl" value="/portal" />
                    <label class="block text-xs font-light uppercase tracking-[0.22em] text-muted-foreground">
                        {"Recovery credential"}
                        <input type="password" name="secret" required={true} autocomplete="off"
                            class="mt-2 block min-h-12 w-full rounded-sm border border-border bg-background px-4 text-sm text-foreground outline-none focus:border-foreground" />
                    </label>
                    if model.refused {
                        <p class="text-sm font-light text-destructive" role="alert">
                            {"That credential was not accepted."}
                        </p>
                    }
                    <button type="submit" disabled={!ready}
                        class="flex min-h-12 w-full items-center justify-center rounded-sm border border-foreground/20 px-4 text-sm font-light text-foreground transition hover:border-foreground disabled:opacity-40">
                        { if ready { "Sign in" } else { "Preparing…" } }
                    </button>
                </form>
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_reads_the_form_token_and_says_when_a_credential_was_refused() {
        let ctx = ScreenCtx {
            query: crate::app::screen::parse_query("?error=CredentialsSignin"),
            ..ScreenCtx::default()
        };
        let (mut model, cmd) = Recovery::init(&ctx);
        assert!(model.refused);
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/api/auth/csrf");
        Recovery::update(
            &mut model,
            request.respond(Ok(serde_json::json!({ "csrfToken": "t" }))),
            &ctx,
        );
        assert_eq!(model.csrf_token, "t");
    }
}
