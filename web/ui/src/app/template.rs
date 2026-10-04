//! The master template: the standard pieces every screen draws with, so the same state looks the same everywhere.
//!
//! A screen does not write its own loading line, its own error box or its own page heading. It calls these. When the
//! design of "loading" or "failed" changes, it changes here, for every screen at once.

use yew::prelude::*;

use crate::app::cmd::{ApiError, Remote};

/// The light portal panel surface.
pub const PANEL: &str =
    "portal-glass-panel portal-glass-panel-soft rounded-[var(--portal-panel-radius)]";

/// A portal screen's heading: eyebrow (the surface), title, and one line on what the screen is for.
pub fn portal_heading(eyebrow: &str, title: &str, purpose: &str) -> Html {
    html! {
        <div>
            <p class="text-xs font-light uppercase tracking-[0.28em] text-black/40">{ eyebrow.to_owned() }</p>
            <h1 class="mt-3 font-serif text-4xl font-light leading-[1.1]">{ title.to_owned() }</h1>
            if !purpose.is_empty() {
                <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">{ purpose.to_owned() }</p>
            }
        </div>
    }
}

/// Draw data that arrives over the network: nothing yet, loading, the data, or the failure — the same four ways on
/// every screen. `loading` names what is being read ("the database"), so the line says what it is waiting for.
pub fn remote<T>(remote: &Remote<T>, loading: &str, loaded: impl FnOnce(&T) -> Html) -> Html {
    match remote {
        Remote::NotAsked => Html::default(),
        Remote::Loading => loading_panel(loading),
        Remote::Loaded(value) => loaded(value),
        Remote::Failed(error) => failure(error),
    }
}

/// The ground a screen is drawn on: the portal's light glass, the dark Accounting shell, or the public site.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Portal,
    Dark,
    Site,
}

/// `remote`, on a given ground. The failure panel is the same everywhere; only the loading line takes the tone.
pub fn remote_toned<T>(
    remote: &Remote<T>,
    tone: Tone,
    loading: &str,
    loaded: impl FnOnce(&T) -> Html,
) -> Html {
    match (remote, tone) {
        (Remote::Loading, Tone::Portal) => loading_panel(loading),
        (Remote::Loading, tone) => loading_toned(tone, loading),
        (other, _) => self::remote(other, loading, loaded),
    }
}

/// A loading line inside something already drawn (a panel, a detail pane): the same words as `loading_panel`.
pub fn loading_line(what: &str) -> Html {
    loading_toned(Tone::Portal, what)
}

pub fn loading_toned(tone: Tone, what: &str) -> Html {
    let class = match tone {
        Tone::Portal => "text-sm font-light text-black/40",
        Tone::Dark => "text-sm font-light text-white/40",
        Tone::Site => "py-10 text-sm font-light text-muted-foreground",
    };
    html! { <p class={class} data-screen-state="loading">{ loading_words(what) }</p> }
}

/// The loading words, for a status line that says it in passing ("Reading visible dates…").
pub fn loading_words(what: &str) -> String {
    format!("Reading {what}\u{2026}")
}

pub fn loading_panel(what: &str) -> Html {
    html! {
        <section class={classes!(PANEL, "p-10", "text-center")} data-screen-state="loading">
            <p class="text-sm font-light text-black/40">{ format!("Reading {what}\u{2026}") }</p>
        </section>
    }
}

/// A failed read or command, in the server's own words, with its code for whoever has to look it up.
pub fn failure(error: &ApiError) -> Html {
    html! {
        <div class="rounded-md border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm" role="alert" data-screen-state="failed">
            <p>{ error.message.clone() }</p>
            <p class="mt-1 text-[11px] uppercase tracking-[0.14em] text-black/40">{ error.code.clone() }</p>
        </div>
    }
}

/// `failure` with a way back: when the read never got an answer or the
/// server failed (not a refusal), offer to run it again. The caller re-issues
/// its own read on retry — this only draws the button.
pub fn failure_retry(error: &ApiError, retry: Callback<MouseEvent>) -> Html {
    let transient = error.status == 0 || error.status >= 500;
    html! {
        <div class="rounded-md border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm" role="alert" data-screen-state="failed">
            <p>{ error.message.clone() }</p>
            <p class="mt-1 text-[11px] uppercase tracking-[0.14em] text-black/40">{ error.code.clone() }</p>
            if transient {
                <button
                    type="button"
                    onclick={retry}
                    class="mt-3 min-h-12 rounded-md border border-[var(--portal-border)] px-4 text-xs font-medium uppercase tracking-[0.14em]">
                    { "Try again" }
                </button>
            }
        </div>
    }
}

/// `remote` with a way back from failure: the retry callback re-issues the
/// caller's read. Screens with their own failure handling keep `remote`.
pub fn remote_retry<T>(
    remote: &Remote<T>,
    loading: &str,
    retry: Callback<MouseEvent>,
    loaded: impl FnOnce(&T) -> Html,
) -> Html {
    match remote {
        Remote::Failed(error) => failure_retry(error, retry),
        _ => self::remote(remote, loading, loaded),
    }
}

/// An answered read with nothing in it. Different from loading, and said so.
pub fn empty_panel(message: &str) -> Html {
    html! {
        <section class={classes!(PANEL, "p-10", "text-center")} data-screen-state="empty">
            <p class="text-sm font-light text-black/40">{ message.to_owned() }</p>
        </section>
    }
}

/// One headline number with its label and what it counts.
/// Where a vendor widget (Gantt, calendar, sorter, player, gallery) used to be. The widgets were deleted with the last
/// of the TypeScript (owner decision, 2026-09-26): a feature that needs one comes back only as a Rust port, never as a
/// JavaScript island.
pub fn widget_removed(what: &str) -> Html {
    empty_panel(&format!(
        "{what} is not available yet — it is being rebuilt in Rust."
    ))
}

pub fn metric(label: &str, value: &str, hint: &str) -> Html {
    html! {
        <div class={classes!(PANEL, "p-6")}>
            <p class="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">{ label.to_owned() }</p>
            <p class="mt-2 font-serif text-3xl font-light text-[var(--portal-navy)]">{ value.to_owned() }</p>
            <p class="mt-1 text-[11px] font-light text-black/40">{ hint.to_owned() }</p>
        </div>
    }
}

/// The value of the input an event came from. Screens read typed text through this, never through `web_sys`, so no
/// screen reaches into the DOM.
pub fn input_value(event: &InputEvent) -> String {
    event
        .target_dyn_into::<web_sys::HtmlInputElement>()
        .map(|input| input.value())
        .unwrap_or_default()
}

/// `ctx.path` with one query parameter set (or removed with `None`), every other parameter kept. How a tab or a row
/// selection becomes a link: the URL is where sub-navigation lives, so it can be reloaded, shared and gone back to.
pub fn with_query(ctx: &crate::app::screen::ScreenCtx, key: &str, value: Option<&str>) -> String {
    let mut query = ctx.query.clone();
    match value {
        Some(value) => {
            query.insert(key.to_owned(), value.to_owned());
        }
        None => {
            query.remove(key);
        }
    }
    let encode = crate::app::api::encode;
    let pairs: Vec<String> = query
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect();
    if pairs.is_empty() {
        ctx.path.clone()
    } else {
        format!("{}?{}", ctx.path, pairs.join("&"))
    }
}

/// The active tab: the URL's `?tab=` if it names one of `tabs`, else the first.
pub fn active_tab<'a>(ctx: &crate::app::screen::ScreenCtx, tabs: &[(&'a str, &'a str)]) -> &'a str {
    let wanted = ctx.query("tab");
    tabs.iter()
        .find(|(key, _)| Some(*key) == wanted)
        .or(tabs.first())
        .map(|(key, _)| *key)
        .unwrap_or_default()
}

/// A screen's sub-navigation: tabs addressed by `?tab=`. Choosing one is an in-app move; the screen's
/// `url_changed` sees it, and its view reads `active_tab`.
pub fn tabs(ctx: &crate::app::screen::ScreenCtx, tabs: &[(&str, &str)]) -> Html {
    let active = active_tab(ctx, tabs);
    html! {
        <nav class="flex flex-wrap gap-1" aria-label="Sections" data-sub-nav="">
            { for tabs.iter().map(|(key, label)| html! {
                <crate::app::chrome::AppLink href={with_query(ctx, "tab", Some(key))}
                    classes={classes!("portal-glass-tab")} current={*key == active}>
                    { *label }
                </crate::app::chrome::AppLink>
            }) }
        </nav>
    }
}

/// "← Clients" on a drill-in: back to the parent the registry names. Nothing on a screen with no parent.
pub fn back_link(ctx: &crate::app::screen::ScreenCtx) -> Html {
    match crate::app::registry::parent_of(&ctx.path) {
        Some(parent) => html! {
            <crate::app::chrome::AppLink href={parent.path}
                classes={classes!("text-[10px]", "font-medium", "uppercase", "tracking-[0.14em]", "text-[var(--portal-navy-soft)]")}>
                { format!("\u{2190} {}", parent.title) }
            </crate::app::chrome::AppLink>
        },
        None => Html::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::screen::{parse_query, ScreenCtx};

    fn ctx(path: &str, query: &str) -> ScreenCtx {
        ScreenCtx {
            path: path.into(),
            query: parse_query(query),
            ..ScreenCtx::default()
        }
    }

    #[test]
    fn a_link_changes_one_parameter_and_keeps_the_rest() {
        let here = ctx("/portal/projects", "?tab=calendar&selected=p 1");
        assert_eq!(
            with_query(&here, "tab", Some("timeline")),
            "/portal/projects?selected=p%201&tab=timeline"
        );
        assert_eq!(
            with_query(&here, "selected", None),
            "/portal/projects?tab=calendar"
        );
        assert_eq!(
            with_query(&ctx("/portal/deals", ""), "tab", None),
            "/portal/deals"
        );
    }

    #[test]
    fn the_active_tab_is_the_urls_when_it_names_a_tab_and_the_first_otherwise() {
        let tabs = [("workplan", "Workplan"), ("timeline", "Timeline")];
        assert_eq!(active_tab(&ctx("/p", "?tab=timeline"), &tabs), "timeline");
        assert_eq!(active_tab(&ctx("/p", "?tab=nonsense"), &tabs), "workplan");
        assert_eq!(active_tab(&ctx("/p", ""), &tabs), "workplan");
    }
}
