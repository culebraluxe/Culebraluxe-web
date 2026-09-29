//! /login and /auth/error — portal sign-in with Google. The button is a plain link to the server's Google handshake
//! (rust/server/src/api/google_auth.rs); nothing here talks to Google, and there is no JavaScript.

use yew::prelude::*;

use crate::app::screen::ScreenCtx;

const PANEL: &str = "mx-auto mt-24 max-w-md rounded-2xl border border-black/10 bg-white/70 p-8 text-center shadow-sm";

/// The Google sign-in address, returning to `callbackUrl` (the portal page that sent the visitor here) afterwards.
pub fn sign_in_href(ctx: &ScreenCtx) -> String {
    let back = ctx.query("callbackUrl").unwrap_or("/portal/dashboard");
    crate::app::api::auth::sign_in_google(back)
}

pub fn login(ctx: ScreenCtx) -> Html {
    html! {
        <section class={PANEL} data-screen-key="login">
            <h1 class="font-serif text-3xl font-light text-[var(--portal-navy)]">{"Portal sign-in"}</h1>
            <p class="mt-3 text-sm font-light text-black/55">{"Sign in with the Google account CulebraLuxe knows you by."}</p>
            <a href={sign_in_href(&ctx)}
                class="mt-6 inline-flex items-center justify-center rounded-md bg-[var(--portal-navy)] px-5 py-2.5 text-[12px] font-medium uppercase tracking-[0.14em] text-white">
                {"Sign in with Google"}
            </a>
        </section>
    }
}

fn reason(code: Option<&str>) -> &'static str {
    match code {
        Some("AccessDenied") => "That Google account is not a CulebraLuxe portal user.",
        Some("Verification") => "The sign-in expired or was interrupted. Please try again.",
        Some("Configuration") => "Sign-in is not configured on this server.",
        _ => "Google sign-in did not complete. Please try again.",
    }
}

pub fn auth_error(ctx: ScreenCtx) -> Html {
    html! {
        <section class={PANEL} data-screen-key="auth-error">
            <h1 class="font-serif text-3xl font-light text-[var(--portal-navy)]">{"Sign-in failed"}</h1>
            <p class="mt-3 text-sm font-light text-black/55">{ reason(ctx.query("error")) }</p>
            <a href="/login" class="mt-6 inline-block text-[12px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy)] underline">
                {"Try again"}
            </a>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_button_returns_to_the_page_that_sent_the_visitor() {
        let mut ctx = ScreenCtx::default();
        assert_eq!(sign_in_href(&ctx), "/api/auth/signin/google?callbackUrl=/portal/dashboard");
        ctx.query.insert("callbackUrl".into(), "/portal/clients?selected=a b".into());
        assert_eq!(sign_in_href(&ctx), "/api/auth/signin/google?callbackUrl=/portal/clients%3Fselected%3Da%20b");
    }
}
