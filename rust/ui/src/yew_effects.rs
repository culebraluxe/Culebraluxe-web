//! The effects, run by Yew.
//!
//! WHAT THIS REPLACES: the TypeScript host, which owned the network on the old path and had to be told what to fetch by
//! a JSON effect list over a DOM event. Here an `Effect` is a value the app can act on directly, the request is an async
//! function, and the answer comes back as a `Msg` — which is the only way state changes, so a late response is subject
//! to the same ownership rule as everything else (`update::owns`).
//!
//! THE ENDPOINTS ARE UNCHANGED. `/api/rust-ui/public-page` and `/api/rust-ui/public-rows` are the application's own
//! routes: they hold the session and the database, and this module holds no credential. The one thing that moved is the
//! side of the boundary the fetch happens on.

use gloo_net::http::Request;
use yew::platform::spawn_local;
use yew::Callback;

use crate::model::{Effect, Msg};

/// Page content: a screen's blocks, hero, listings and records.
const PAGE_PATH: &str = "/api/rust-ui/public-page";
/// Rows: a list screen's data, and nothing else.
const ROWS_PATH: &str = "/api/rust-ui/public-rows";
/// A portal screen's payload, for the screens that have a real component.
const PORTAL_PATH: &str = "/api/portal/rust-ui/page";

/// Run one effect and dispatch what it produces.
///
/// A failed request becomes `Msg::EffectFailed`, which the model renders as a message rather than an empty screen:
/// "nothing to show" and "we could not ask" are different states, and the user deserves the difference.
pub fn run(effect: Effect, dispatch: &Callback<Msg>) {
    let (url, screen, generation, kind) = match effect {
        Effect::FetchPage {
            screen,
            scope,
            generation,
        } => (
            query(PAGE_PATH, screen, scope.as_deref()),
            screen,
            generation,
            Kind::Page,
        ),
        Effect::FetchPortal { screen, generation } => (
            query(PORTAL_PATH, screen, None),
            screen,
            generation,
            Kind::Portal,
        ),
        Effect::FetchRows {
            screen,
            scope,
            generation,
        } => (
            query(ROWS_PATH, screen, scope.as_deref()),
            screen,
            generation,
            Kind::Rows,
        ),
    };

    let dispatch = dispatch.clone();
    spawn_local(async move {
        let answer = Request::get(&url).send().await;
        // A FAILURE CARRIES ITS OWNER TOO. The message names the screen and the generation the request was made under,
        // so the reducer can refuse a failure that belongs to a screen the visitor has already left — otherwise a
        // rejected request for one page would put its error on another.
        let fail = |message: String| Msg::EffectFailed {
            screen: screen.to_string(),
            generation,
            message,
        };
        let msg = match answer {
            Ok(response) if response.ok() => match response.text().await {
                // The payload carries the screen and the generation it was fetched for, so the reducer can refuse an
                // answer whose owner has moved on. Order of arrival is not ownership.
                Ok(body) => match kind {
                    Kind::Page => Msg::page_loaded_json(screen, generation, &body),
                    Kind::Portal => Msg::portal_loaded_json(screen, generation, &body),
                    Kind::Rows => Msg::rows_loaded_json(screen, generation, &body),
                },
                Err(error) => fail(format!("the answer could not be read: {error}")),
            },
            Ok(response) => fail(format!(
                "the {} request failed with {}",
                match kind {
                    Kind::Page => "page",
                    Kind::Portal => "portal",
                    Kind::Rows => "rows",
                },
                response.status()
            )),
            Err(error) => fail(format!("the request could not be sent: {error}")),
        };
        dispatch.emit(msg);
    });
}

/// What a payload is for, so the right parser reads it. Three requests, three shapes — and a portal payload parsed as
/// rows would be an empty screen that looks like a successful answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Page,
    Portal,
    Rows,
}

/// The request URL: the screen, and the record key when the screen is about one record.
fn query(path: &str, screen: &str, scope: Option<&str>) -> String {
    match scope {
        Some(key) if !key.is_empty() => format!("{path}?screen={screen}&scope={key}"),
        _ => format!("{path}?screen={screen}"),
    }
}
