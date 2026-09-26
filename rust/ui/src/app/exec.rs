//! The executor: the ONLY place a screen's `Cmd` touches the network, storage, or the browser's location.
//!
//! Every screen's HTTP goes through `request` below, so every screen gets the same decoding and the same failure shape.
//! That sameness is the point. A screen that "just calls fetch" is how a screen ends up with its own idea of errors.

use gloo_net::http::Request as HttpRequest;
use yew::platform::spawn_local;
use yew::Callback;
use yew_router::prelude::Navigator;
use yew_router::AnyRoute;

use crate::app::cmd::{ApiError, Cmd, Method, Request};

/// Perform a command. Its messages go to `deliver`. Without a navigator (outside the router) navigation is a document
/// load, which is always correct, only heavier.
pub fn run<Msg: 'static>(cmd: Cmd<Msg>, deliver: &Callback<Msg>, navigator: Option<&Navigator>) {
    match cmd {
        Cmd::None => {}
        Cmd::Batch(cmds) => {
            for cmd in cmds {
                run(cmd, deliver, navigator);
            }
        }
        Cmd::Request(request) => request_http(request, deliver.clone()),
        // The same rule as a link (`chrome::in_app`): in-app within one area, a document load otherwise.
        Cmd::Navigate(path) => {
            let here = web_sys::window()
                .and_then(|window| window.location().pathname().ok())
                .unwrap_or_default();
            match navigator {
                Some(navigator) if crate::app::chrome::in_app(&here, &path) => {
                    navigator.push(&AnyRoute::new(path))
                }
                _ => load(&path),
            }
        }
        Cmd::Load(href) => load(&href),
        Cmd::StorageRead { key, reply } => {
            let value = storage().and_then(|storage| storage.get_item(&key).ok().flatten());
            deliver.emit(reply(value));
        }
        Cmd::After { millis, msg } => {
            let deliver = deliver.clone();
            spawn_local(async move {
                yew::platform::time::sleep(std::time::Duration::from_millis(u64::from(millis)))
                    .await;
                deliver.emit(msg);
            });
        }
        Cmd::StorageWrite { key, value } => {
            if let Some(storage) = storage() {
                let _ = match value {
                    Some(value) => storage.set_item(&key, &value),
                    None => storage.remove_item(&key),
                };
            }
        }
    }
}

pub fn load(href: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_href(href);
    }
}

/// Device storage, or `None` where it is unavailable (private mode, blocked site data). Never a panic.
fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
}

fn request_http<Msg: 'static>(request: Request<Msg>, deliver: Callback<Msg>) {
    spawn_local(async move {
        let answer = send(request.method, &request.path, request.body.as_ref()).await;
        deliver.emit(request.respond(answer));
    });
}

async fn send(
    method: Method,
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    let request = match (method, body) {
        (Method::Get, _) => HttpRequest::get(path).build(),
        (Method::Post, Some(body)) => HttpRequest::post(path).json(body),
        (Method::Post, None) => HttpRequest::post(path).build(),
    }
    .map_err(|error| ApiError::network(error.to_string()))?;
    let response = request
        .send()
        .await
        .map_err(|error| ApiError::network(error.to_string()))?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    interpret(status, response.ok(), &text)
}

/// Turn an HTTP answer into the one result shape. Pure, so it is unit-tested below.
///
/// Two answer styles exist today and both are handled here, once: the Rust API's envelope (`{ ok, value }` /
/// `{ ok: false, error: { code, message } }`) and a relay's plain JSON. A non-2xx without an envelope is a failure
/// carrying whatever `code`/`message` the body offers.
pub(crate) fn interpret(status: u16, ok: bool, text: &str) -> Result<serde_json::Value, ApiError> {
    let value: Option<serde_json::Value> = serde_json::from_str(text).ok();
    let field = |value: &serde_json::Value, name: &str| {
        value
            .get(name)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    match value {
        Some(value)
            if value.get("ok").and_then(serde_json::Value::as_bool) == Some(true)
                && value.get("value").is_some() =>
        {
            Ok(value["value"].clone())
        }
        Some(value) if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) => {
            let error = value.get("error").cloned().unwrap_or_default();
            Err(ApiError {
                status,
                code: field(&error, "code").unwrap_or_else(|| "FAILED".into()),
                message: field(&error, "message").unwrap_or_else(|| "The request failed.".into()),
            })
        }
        Some(value) if ok => Ok(value),
        Some(value) => Err(ApiError {
            status,
            code: field(&value, "code").unwrap_or_else(|| "HTTP".into()),
            message: field(&value, "message")
                .unwrap_or_else(|| format!("The request failed ({status}).")),
        }),
        None if ok => Err(ApiError::decode("The answer was not JSON.")),
        None => Err(ApiError {
            status,
            code: "HTTP".into(),
            message: format!("The request failed ({status})."),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::interpret;
    use serde_json::json;

    #[test]
    fn the_rust_envelope_is_unwrapped_and_its_failure_carries_the_servers_words() {
        assert_eq!(
            interpret(200, true, r#"{"ok":true,"value":[1]}"#),
            Ok(json!([1]))
        );
        let error = interpret(
            400,
            false,
            r#"{"ok":false,"error":{"code":"GUEST_CODE_INVALID","message":"Wrong code."}}"#,
        )
        .unwrap_err();
        assert_eq!(
            (error.status, error.code.as_str(), error.message.as_str()),
            (400, "GUEST_CODE_INVALID", "Wrong code.")
        );
    }

    #[test]
    fn a_relays_plain_json_passes_through_and_its_refusal_keeps_its_message() {
        assert_eq!(
            interpret(200, true, r#"{"signedIn":false}"#),
            Ok(json!({ "signedIn": false }))
        );
        let error =
            interpret(422, false, r#"{"sent":false,"message":"Too many codes."}"#).unwrap_err();
        assert_eq!(
            (error.code.as_str(), error.message.as_str()),
            ("HTTP", "Too many codes.")
        );
        assert_eq!(interpret(502, false, "<html>").unwrap_err().code, "HTTP");
        assert_eq!(interpret(200, true, "<html>").unwrap_err().code, "DECODE");
    }
}
