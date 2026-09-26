//! `/account` — guest sign-in for the public site. A guest is an EXTERNAL account: it keeps a visitor's saves and never
//! opens the portal (the Rust policy refuses an external account every grant).
//!
//! THE REFERENCE SCREEN FOR COMMANDS AND FORMS. Two reads on open (who is signed in, Auth.js's form token), one command
//! (email me a code), and two forms that post to Auth.js itself (Google, the code) carrying that token. The typed email
//! lives in the model; nothing reads the DOM.

use yew::prelude::*;

use crate::app::api::{AuthCsrf, CodeSent, CsrfToken, GuestRequestCode, GuestSession, GuestWhoAmI};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;

pub struct Account;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub session: Remote<GuestSession>,
    pub csrf_token: String,
    pub email: String,
    /// The address a code went to: the screen then asks for the code.
    pub code_sent_to: Option<String>,
    pub sending: bool,
    /// Why the last request was refused, in words for the visitor.
    pub message: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    SessionLoaded(Result<GuestSession, ApiError>),
    CsrfLoaded(Result<CsrfToken, ApiError>),
    EmailTyped(String),
    CodeRequested,
    CodeAnswered(String, Result<CodeSent, ApiError>),
    UseAnotherEmail,
}

const REFUSED: &str =
    "That code is not right, or it has expired. Request a new code and try again.";

impl Screen for Account {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let model = Model {
            session: Remote::Loading,
            // Auth.js sends a refused code back here as `?error=CredentialsSignin`.
            message: ctx.query("error").map(|_| REFUSED.to_string()),
            ..Model::default()
        };
        (
            model,
            Cmd::batch([
                Cmd::request(GuestWhoAmI, Msg::SessionLoaded),
                Cmd::request(AuthCsrf, Msg::CsrfLoaded),
            ]),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::SessionLoaded(answer) => model.session = Remote::from_result(answer),
            // Without the token the forms cannot post; the failure is shown by the forms being unusable, and signing in
            // again after a reload is the remedy. It is not an error the visitor can act on here.
            Msg::CsrfLoaded(answer) => {
                model.csrf_token = answer.map(|token| token.csrf_token).unwrap_or_default()
            }
            Msg::EmailTyped(email) => model.email = email,
            Msg::CodeRequested => {
                let email = model
                    .code_sent_to
                    .clone()
                    .unwrap_or_else(|| model.email.trim().to_string());
                if model.sending || email.is_empty() {
                    return Cmd::none();
                }
                model.sending = true;
                model.message = None;
                return Cmd::request(
                    GuestRequestCode {
                        email: email.clone(),
                    },
                    move |answer| Msg::CodeAnswered(email, answer),
                );
            }
            Msg::CodeAnswered(email, answer) => {
                model.sending = false;
                match answer {
                    Ok(CodeSent { sent: true }) => model.code_sent_to = Some(email.to_lowercase()),
                    Ok(_) => {
                        model.message = Some("The code could not be sent. Please try again.".into())
                    }
                    Err(error) => model.message = Some(error.message),
                }
            }
            Msg::UseAnotherEmail => {
                model.code_sent_to = None;
                model.message = None;
            }
        }
        Cmd::none()
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let signed_in = model.session.loaded().filter(|session| session.signed_in);
        let body = match (&model.session, signed_in) {
            (Remote::Loading | Remote::NotAsked, _) => html! {
                <p class="mt-10 text-sm font-light text-muted-foreground">{"One moment\u{2026}"}</p>
            },
            (_, Some(session)) => signed_in_view(model, session),
            // A failed "who is signed in" shows the sign-in forms: signing in is the remedy.
            _ => sign_in_view(model, link),
        };
        html! {
            <section class="px-6 py-24 md:px-12 md:py-32">
                <div class="mx-auto max-w-md">
                    <p class="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">{"Your account"}</p>
                    <h1 class="font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl">
                        { if signed_in.is_some() { "Welcome back." } else { "Sign in" } }
                    </h1>
                    { body }
                </div>
            </section>
        }
    }
}

const LABEL: &str = "text-xs font-light uppercase tracking-[0.2em] text-muted-foreground";
const INPUT: &str = "border-b border-border bg-transparent py-3 text-base font-light text-foreground outline-none transition-colors focus:border-accent";
const BUTTON: &str = "inline-flex min-h-12 w-full items-center justify-center border border-foreground bg-foreground px-6 text-xs font-light uppercase tracking-[0.2em] text-background transition-colors hover:bg-transparent hover:text-foreground disabled:opacity-50";
const QUIET: &str = "inline-flex min-h-12 w-full items-center justify-center gap-3 border border-border px-6 text-sm font-light text-foreground transition-colors hover:border-foreground";
const LINK: &str = "text-xs font-light uppercase tracking-[0.2em] text-muted-foreground underline-offset-4 hover:text-accent hover:underline";

/// Auth.js's form token, as a hidden field. Every form posting to Auth.js carries it.
fn csrf(model: &Model) -> Html {
    html! { <input type="hidden" name="csrfToken" value={model.csrf_token.clone()} /> }
}

fn signed_in_view(model: &Model, session: &GuestSession) -> Html {
    html! {
        <>
            <p class="mt-6 text-sm font-light leading-relaxed text-muted-foreground">
                {"Signed in as "}<span class="text-foreground">{ session.display_name.clone() }</span>
                if let Some(email) = &session.email { {" ("}{ email.clone() }{")"} }
                {"."}
            </p>
            <div class="mt-10 flex flex-col gap-4">
                <a href="/favorites" class={QUIET}>{"Your saved properties"}</a>
                <a href="/buyers" class={QUIET}>{"Explore properties"}</a>
            </div>
            <form method="post" action="/api/auth/signout" class="mt-10">
                { csrf(model) }
                <input type="hidden" name="callbackUrl" value="/" />
                <button type="submit" class={LINK}>{"Sign out"}</button>
            </form>
        </>
    }
}

fn sign_in_view(model: &Model, link: &Link<Msg>) -> Html {
    html! {
        <>
            <p class="mt-6 text-sm font-light leading-relaxed text-muted-foreground">
                {"Sign in to keep your saved properties and searches. No password: use Google, or we will email you a code."}
            </p>
            <form method="post" action="/api/auth/signin/google" class="mt-10">
                { csrf(model) }
                <input type="hidden" name="callbackUrl" value="/account" />
                <button type="submit" class={QUIET}>{"Continue with Google"}</button>
            </form>
            <div class="my-8 flex items-center gap-4 text-xs font-light uppercase tracking-[0.2em] text-muted-foreground">
                <span class="h-px flex-1 bg-border"></span>{"or"}<span class="h-px flex-1 bg-border"></span>
            </div>
            if let Some(message) = &model.message {
                <p class="mb-6 text-sm font-light text-destructive" role="alert">{ message.clone() }</p>
            }
            { match &model.code_sent_to {
                None => email_form(model, link),
                Some(email) => code_form(model, email, link),
            } }
        </>
    }
}

fn email_form(model: &Model, link: &Link<Msg>) -> Html {
    let onsubmit = link.callback(|event: SubmitEvent| {
        event.prevent_default();
        Msg::CodeRequested
    });
    html! {
        <form {onsubmit} class="flex flex-col gap-6">
            <div class="flex flex-col gap-3">
                <label for="guest-email" class={LABEL}>{"Email"}</label>
                <input id="guest-email" name="email" type="email" autocomplete="email" required=true class={INPUT}
                    value={model.email.clone()}
                    oninput={link.callback(|event: InputEvent| Msg::EmailTyped(template::input_value(&event)))} />
            </div>
            <button type="submit" class={BUTTON} disabled={model.sending}>
                { if model.sending { "Sending\u{2026}" } else { "Email me a code" } }
            </button>
        </form>
    }
}

/// The code form posts to Auth.js's `email-code` provider; Auth.js asks Rust to check it and sets the session.
fn code_form(model: &Model, email: &str, link: &Link<Msg>) -> Html {
    html! {
        <>
            <p class="mb-6 text-sm font-light leading-relaxed text-muted-foreground">
                {"We sent a six-digit code to "}<span class="text-foreground">{ email.to_owned() }</span>
                {". It works for ten minutes."}
            </p>
            <form method="post" action="/api/auth/callback/email-code" class="flex flex-col gap-6">
                { csrf(model) }
                <input type="hidden" name="email" value={email.to_owned()} />
                <input type="hidden" name="callbackUrl" value="/account" />
                <div class="flex flex-col gap-3">
                    <label for="guest-code" class={LABEL}>{"Code"}</label>
                    <input id="guest-code" name="code" type="text" inputmode="numeric" autocomplete="one-time-code"
                        maxlength="6" pattern="[0-9]{6}" required=true class={INPUT} />
                </div>
                <button type="submit" class={BUTTON}>{"Sign in"}</button>
            </form>
            <div class="mt-6 flex justify-between">
                <button type="button" class={LINK} onclick={link.callback(|_: MouseEvent| Msg::CodeRequested)}
                    disabled={model.sending}>{"Send a new code"}</button>
                <button type="button" class={LINK} onclick={link.callback(|_: MouseEvent| Msg::UseAnotherEmail)}>
                    {"Use a different email"}</button>
            </div>
        </>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn it_reads_the_session_and_token_then_sends_one_code_at_a_time() {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = Account::init(&ctx);
        let paths: Vec<_> = cmd
            .into_requests()
            .into_iter()
            .map(|request| request.path)
            .collect();
        assert_eq!(paths, ["/api/rust-ui/guest", "/api/auth/csrf"]);

        Account::update(
            &mut model,
            Msg::EmailTyped(" ada@example.com ".into()),
            &ctx,
        );
        let cmd = Account::update(&mut model, Msg::CodeRequested, &ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.body, Some(json!({ "email": "ada@example.com" })));
        assert!(
            Account::update(&mut model, Msg::CodeRequested, &ctx)
                .into_requests()
                .is_empty(),
            "one at a time"
        );

        let refused = request.respond(Err(ApiError {
            status: 422,
            code: "HTTP".into(),
            message: "Too many codes.".into(),
        }));
        Account::update(&mut model, refused, &ctx);
        assert_eq!(
            (model.sending, model.message.as_deref()),
            (false, Some("Too many codes."))
        );

        Account::update(&mut model, Msg::CodeRequested, &ctx);
        Account::update(
            &mut model,
            Msg::CodeAnswered("Ada@Example.com".into(), Ok(CodeSent { sent: true })),
            &ctx,
        );
        assert_eq!(model.code_sent_to.as_deref(), Some("ada@example.com"));

        Account::update(&mut model, Msg::UseAnotherEmail, &ctx);
        assert_eq!(model.code_sent_to, None);
    }

    #[test]
    fn a_refused_code_returns_with_its_explanation() {
        let ctx = ScreenCtx {
            query: crate::app::screen::parse_query("?error=CredentialsSignin"),
            ..ScreenCtx::default()
        };
        assert_eq!(Account::init(&ctx).0.message.as_deref(), Some(REFUSED));
    }

    #[test]
    fn the_real_guest_session_answer_decodes() {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = Account::init(&ctx);
        let answer: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/guest-session.json")).unwrap();
        Account::update(
            &mut model,
            cmd.into_requests().remove(0).respond(Ok(answer)),
            &ctx,
        );
        assert!(model.session.loaded().is_some());
    }
}
