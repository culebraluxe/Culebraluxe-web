//! `/account` — guest sign-in for the public site, on Yew.
//!
//! A GUEST IS AN EXTERNAL ACCOUNT: signing in keeps a visitor's saves; it never opens the portal (the Rust policy
//! refuses an external account every grant). The host provisions the guest on a first sign-in (`/api/rust-ui/guest`).
//!
//! THE FORMS POST TO AUTH.JS ITSELF. "Continue with Google" is Auth.js's Google sign-in, and the code form is its
//! `email-code` provider, which asks Rust to check the code. Both carry Auth.js's form token and come back here. Only
//! "email me a code" goes through the reducer, because the screen has to show where the code went.

use yew::prelude::*;

use crate::model::{GuestSession, Model, Msg};
use crate::yew_router::query_param;
use crate::yew_views::chrome::PageProps;
use crate::yew_views::contact::field_value;

pub struct Account;

const LABEL: &str = "text-xs font-light uppercase tracking-[0.2em] text-muted-foreground";
const INPUT: &str = "border-b border-border bg-transparent py-3 text-base font-light text-foreground outline-none transition-colors focus:border-accent";
const BUTTON: &str = "inline-flex min-h-12 w-full items-center justify-center border border-foreground bg-foreground px-6 text-xs font-light uppercase tracking-[0.2em] text-background transition-colors hover:bg-transparent hover:text-foreground disabled:opacity-50";
const QUIET: &str = "inline-flex min-h-12 w-full items-center justify-center gap-3 border border-border px-6 text-sm font-light text-foreground transition-colors hover:border-foreground";
const LINK: &str = "text-xs font-light uppercase tracking-[0.2em] text-muted-foreground underline-offset-4 hover:text-accent hover:underline";

impl Component for Account {
    type Message = ();
    type Properties = PageProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let model = &ctx.props().model;
        let on_msg = &ctx.props().on_msg;
        let body = match &model.guest.session {
            GuestSession::Unknown => html! {
                <p class="mt-10 text-sm font-light text-muted-foreground">{"One moment\u{2026}"}</p>
            },
            GuestSession::SignedIn {
                display_name,
                email,
            } => signed_in(model, display_name, email.as_deref()),
            GuestSession::SignedOut => sign_in(model, on_msg),
        };
        html! {
            <section class="px-6 py-24 md:px-12 md:py-32">
                <div class="mx-auto max-w-md">
                    <p class="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">{"Your account"}</p>
                    <h1 class="font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl">
                        { if matches!(model.guest.session, GuestSession::SignedIn { .. }) { "Welcome back." } else { "Sign in" } }
                    </h1>
                    { body }
                </div>
            </section>
        }
    }
}

/// Auth.js's form token, as a hidden field.
fn csrf(model: &Model) -> Html {
    html! { <input type="hidden" name="csrfToken" value={model.guest.csrf_token.clone().unwrap_or_default()} /> }
}

fn signed_in(model: &Model, display_name: &str, email: Option<&str>) -> Html {
    html! {
        <>
            <p class="mt-6 text-sm font-light leading-relaxed text-muted-foreground">
                {"Signed in as "}<span class="text-foreground">{ display_name.to_owned() }</span>
                if let Some(email) = email { {" ("}{ email.to_owned() }{")"} }
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

fn sign_in(model: &Model, on_msg: &Callback<Msg>) -> Html {
    let guest = &model.guest;
    // Auth.js sends a refused code back here as `?error=CredentialsSignin`.
    let refused = query_param("error")
        .is_some()
        .then_some("That code is not right, or it has expired. Request a new code and try again.");
    let message = guest.message.as_deref().or(refused);
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
            if let Some(message) = message {
                <p class="mb-6 text-sm font-light text-destructive" role="alert">{ message.to_owned() }</p>
            }
            { match guest.code_sent_to.as_deref() {
                None => email_form(guest.sending, on_msg),
                Some(email) => code_form(model, email, on_msg),
            } }
        </>
    }
}

fn email_form(sending: bool, on_msg: &Callback<Msg>) -> Html {
    let onsubmit = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            on_msg.emit(Msg::GuestCodeRequested(field_value("guest-email")));
        })
    };
    html! {
        <form {onsubmit} class="flex flex-col gap-6">
            <div class="flex flex-col gap-3">
                <label for="guest-email" class={LABEL}>{"Email"}</label>
                <input id="guest-email" name="email" type="email" autocomplete="email" required=true class={INPUT} />
            </div>
            <button type="submit" class={BUTTON} disabled={sending}>
                { if sending { "Sending\u{2026}" } else { "Email me a code" } }
            </button>
        </form>
    }
}

/// The code form posts to Auth.js's `email-code` provider; Auth.js asks Rust to check it and sets the session.
fn code_form(model: &Model, email: &str, on_msg: &Callback<Msg>) -> Html {
    let resend = {
        let (on_msg, email) = (on_msg.clone(), email.to_owned());
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::GuestCodeRequested(email.clone())))
    };
    let reset = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::GuestCodeReset))
    };
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
                <button type="button" class={LINK} onclick={resend} disabled={model.guest.sending}>{"Send a new code"}</button>
                <button type="button" class={LINK} onclick={reset}>{"Use a different email"}</button>
            </div>
        </>
    }
}
