//! The executor: the ONLY place a screen's `Cmd` touches the network, storage, or the browser's location.
//!
//! Every screen's HTTP goes through `request` below, so every screen gets the same decoding and the same failure shape.
//! That sameness is the point. A screen that "just calls fetch" is how a screen ends up with its own idea of errors.

use base64::Engine as _;
use gloo_net::http::Request as HttpRequest;
use yew::platform::spawn_local;
use yew::Callback;
use yew_router::prelude::Navigator;
use yew_router::AnyRoute;

use wasm_bindgen::JsCast;

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
        Cmd::ReplacePath(path) => replace_path(&path),
        Cmd::SharePdf {
            data_uri,
            filename,
            reply,
        } => {
            // The sheet opens inside the click (Safari requires it); its outcome arrives later and is reported —
            // shared, cancelled, or refused — rather than assumed.
            match share_pdf(&data_uri, &filename) {
                Ok(promise) => settle_share(promise, reply, deliver.clone()),
                Err(error) => deliver.emit(reply(Err(error))),
            }
        }
        Cmd::Listen { reply } => {
            listen(reply, deliver.clone());
        }
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
        Cmd::Upload(upload) => {
            let deliver = deliver.clone();
            spawn_local(async move {
                let reply = upload.reply;
                let result = upload_chunked(
                    upload.file,
                    &upload.path,
                    &upload.fields,
                    &upload.init_fields,
                )
                .await;
                deliver.emit(reply(result));
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

/// 3 MB per request: comfortably under the gateway's ~4.5 MB cap, so a 13 MB photograph uploads.
const CHUNK_BYTES: f64 = 3.0 * 1024.0 * 1024.0;

/// THE WAY OUT. No upload request may take longer than this: past it, the request is abandoned (aborted, so the
/// browser lets go of it) and answers as a network failure, which is retried. A request that neither answers nor
/// fails used to stall the whole batch silently — the failure was never reported because it never happened.
const REQUEST_SECONDS: u64 = 50;

/// Tries per request, waiting 4, 8, 16, 32 seconds between them: a dropped connection or a server restart is
/// ridden out instead of costing the photograph.
const ATTEMPTS: u32 = 5;

/// One request, with the way out: it answers, fails, or is aborted at `REQUEST_SECONDS` — never hangs.
async fn post_once(path: &str, form: web_sys::FormData) -> Result<serde_json::Value, ApiError> {
    let controller = web_sys::AbortController::new()
        .map_err(|_| ApiError::network("The browser could not start the upload."))?;
    let signal = controller.signal();
    yew::platform::spawn_local(async move {
        yew::platform::time::sleep(std::time::Duration::from_secs(REQUEST_SECONDS)).await;
        // Aborting a request that already finished does nothing.
        controller.abort();
    });
    let response = HttpRequest::post(path)
        .abort_signal(Some(&signal))
        .body(form)
        .map_err(|error| ApiError::network(error.to_string()))?
        .send()
        .await
        .map_err(|error| ApiError::network(format!("the request did not complete ({error})")))?;
    let status = response.status();
    // A body that could not be read is a failure to report, not an empty answer.
    let text = response
        .text()
        .await
        .map_err(|error| ApiError::network(format!("the answer did not arrive ({error})")))?;
    interpret(status, response.ok(), &text)
}

/// One request, retried: a network failure or a server error is tried again; a refusal is final.
async fn post_form(
    path: &str,
    build: impl Fn() -> Result<web_sys::FormData, ApiError>,
) -> Result<serde_json::Value, ApiError> {
    let mut attempt = 1;
    loop {
        match post_once(path, build()?).await {
            Err(error) if attempt < ATTEMPTS && (error.code == "NETWORK" || error.status >= 500) => {
                yew::platform::time::sleep(std::time::Duration::from_secs(2u64 << attempt)).await;
                attempt += 1;
            }
            answer => return answer,
        }
    }
}

/// A photograph, sent the way a torrent is: in pieces, each its own short request, and only the pieces the server
/// does not already have. Every request is safe to send again, so any of them can be retried, and choosing the same
/// file again later resumes it (or skips it, when the property already shows it).
async fn upload_chunked(
    file: web_sys::File,
    path: &str,
    fields: &[(String, String)],
    init_fields: &[(String, String)],
) -> Result<(), ApiError> {
    let size = file.size();
    let chunk_count = ((size / CHUNK_BYTES).ceil() as i32).max(1);
    let form = |step: &str, extra: &[(&str, String)]| -> Result<web_sys::FormData, ApiError> {
        let form = web_sys::FormData::new()
            .map_err(|_| ApiError::network("The browser could not create the upload form."))?;
        let append = |key: &str, value: &str| {
            form.append_with_str(key, value)
                .map_err(|_| ApiError::network("The browser could not prepare the upload."))
        };
        append("step", step)?;
        for (key, value) in fields {
            append(key, value)?;
        }
        for (key, value) in extra {
            append(key, value)?;
        }
        Ok(form)
    };

    // One: declare the file. The server answers what it already has: `done` (the property shows this file already),
    // `processing` (an earlier try is being finished), or an upload id with the pieces it holds.
    let mut declared: Vec<(&str, String)> = vec![
        ("filename", file.name()),
        ("mimeType", file.type_()),
        ("byteSize", format!("{size}")),
        ("chunkCount", chunk_count.to_string()),
        ("chunkSize", format!("{CHUNK_BYTES}")),
    ];
    declared.extend(
        init_fields
            .iter()
            .map(|(key, value)| (key.as_str(), value.clone())),
    );
    let opened = post_form(path, || form("init", &declared)).await?;
    let state = opened.get("state").and_then(|state| state.as_str()).unwrap_or("uploading");
    if state == "done" {
        return Ok(());
    }
    let upload_id = opened
        .get("uploadId")
        .and_then(|id| id.as_str())
        .map(str::to_owned)
        .ok_or_else(|| ApiError::decode("The upload was opened but no upload id came back."))?;

    if state != "processing" {
        let received: std::collections::HashSet<i64> = opened
            .get("received")
            .and_then(|received| received.as_array())
            .map(|indexes| indexes.iter().filter_map(serde_json::Value::as_i64).collect())
            .unwrap_or_default();

        // Two: the pieces the server lacks, in order — the receiver refuses an index beyond the declared count.
        for index in 0..chunk_count {
            if received.contains(&i64::from(index)) {
                continue;
            }
            let start = f64::from(index) * CHUNK_BYTES;
            let end = (start + CHUNK_BYTES).min(size);
            let chunk = || -> Result<web_sys::FormData, ApiError> {
                let piece = file
                    .unchecked_ref::<web_sys::Blob>()
                    .slice_with_f64_and_f64(start, end)
                    .map_err(|_| ApiError::network(format!("Part {} of {chunk_count} could not be read.", index + 1)))?;
                let chunk = form("chunk", &[("uploadId", upload_id.clone()), ("chunkIndex", index.to_string())])?;
                chunk
                    .append_with_blob_and_filename("chunk", &piece, &file.name())
                    .map_err(|_| ApiError::network("The browser could not prepare the upload."))?;
                Ok(chunk)
            };
            post_form(path, chunk).await.map_err(|error| ApiError {
                message: format!("Part {} of {chunk_count}: {}", index + 1, error.message),
                ..error
            })?;
        }

        // Three: finish. The server claims the upload and finishes it in the background (the image is re-encoded,
        // which takes a while); this answers at once.
        post_form(path, || form("complete", &[("uploadId", upload_id.clone())])).await?;
    }

    // Four: ask, in short requests, until the photograph is stored — or has failed, and says why.
    // Twenty minutes of asking: past that, something is wrong that waiting will not fix.
    for _ in 0..400 {
        yew::platform::time::sleep(std::time::Duration::from_secs(3)).await;
        let answer = post_form(path, || form("status", &[("uploadId", upload_id.clone())])).await?;
        match answer.get("state").and_then(|state| state.as_str()) {
            Some("done") => return Ok(()),
            Some("failed") => {
                let message = answer.get("message").and_then(|m| m.as_str()).unwrap_or("The photo could not be saved.");
                return Err(ApiError::network(message.to_owned()));
            }
            // Pieces all there but nobody finishing it (the server restarted mid-way): finish it again.
            Some("uploading") => {
                post_form(path, || form("complete", &[("uploadId", upload_id.clone())])).await?;
            }
            _ => {}
        }
    }
    Err(ApiError::network("The photo is taking too long to finish."))
}

pub fn load(href: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_href(href);
    }
}

fn replace_path(path: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(history) = window.history() else {
        return;
    };
    let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(path));
}

fn share_pdf(data_uri: &str, filename: &str) -> Result<wasm_bindgen::JsValue, ApiError> {
    let (_, encoded) = data_uri
        .split_once(',')
        .ok_or_else(|| ApiError::decode("The PDF preview is not a data URI."))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| ApiError::decode(format!("The PDF preview could not be decoded: {error}")))?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(ApiError::decode("The generated file was not a PDF."));
    }

    let window =
        web_sys::window().ok_or_else(|| ApiError::network("The browser window is unavailable."))?;
    let window_value: &wasm_bindgen::JsValue = window.as_ref();

    // Construct File([Uint8Array(bytes)], filename, { type: "application/pdf" }) in Rust/WASM.
    // There is deliberately no JavaScript bridge: the executor owns this browser capability just like navigation.
    let file_ctor = js_sys::Reflect::get(
        window_value,
        &wasm_bindgen::JsValue::from_str("File"),
    )
    .map_err(|_| ApiError::network("This browser cannot create a PDF attachment."))?
    .dyn_into::<js_sys::Function>()
    .map_err(|_| ApiError::network("This browser cannot create a PDF attachment."))?;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes.as_slice()));
    let options = js_sys::Object::new();
    js_sys::Reflect::set(
        &options,
        &wasm_bindgen::JsValue::from_str("type"),
        &wasm_bindgen::JsValue::from_str("application/pdf"),
    )
    .map_err(|_| ApiError::network("The PDF attachment could not be prepared."))?;
    let args = js_sys::Array::new();
    args.push(&parts);
    args.push(&wasm_bindgen::JsValue::from_str(filename));
    args.push(&options);
    let file = js_sys::Reflect::construct(&file_ctor, &args)
        .map_err(|_| ApiError::network("The PDF attachment could not be prepared."))?;

    let navigator = js_sys::Reflect::get(
        window_value,
        &wasm_bindgen::JsValue::from_str("navigator"),
    )
    .map_err(|_| ApiError::network("Native sharing is unavailable in this browser."))?;
    let share_data = js_sys::Object::new();
    let files = js_sys::Array::new();
    files.push(&file);
    js_sys::Reflect::set(
        &share_data,
        &wasm_bindgen::JsValue::from_str("title"),
        &wasm_bindgen::JsValue::from_str("CulebraLuxe Document"),
    )
    .map_err(|_| ApiError::network("The share sheet could not be prepared."))?;
    js_sys::Reflect::set(
        &share_data,
        &wasm_bindgen::JsValue::from_str("text"),
        &wasm_bindgen::JsValue::from_str("CulebraLuxe transaction document"),
    )
    .map_err(|_| ApiError::network("The share sheet could not be prepared."))?;
    js_sys::Reflect::set(
        &share_data,
        &wasm_bindgen::JsValue::from_str("files"),
        &files,
    )
    .map_err(|_| ApiError::network("The share sheet could not be prepared."))?;

    if let Ok(can_share) = js_sys::Reflect::get(
        &navigator,
        &wasm_bindgen::JsValue::from_str("canShare"),
    )
    .and_then(|value| value.dyn_into::<js_sys::Function>())
    {
        let supported = can_share
            .call1(&navigator, &share_data)
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if !supported {
            return Err(ApiError::network(
                "This browser cannot attach the PDF to its native share sheet.",
            ));
        }
    }

    let share = js_sys::Reflect::get(
        &navigator,
        &wasm_bindgen::JsValue::from_str("share"),
    )
    .map_err(|_| ApiError::network("Native sharing is unavailable in this browser."))?
    .dyn_into::<js_sys::Function>()
    .map_err(|_| ApiError::network("Native sharing is unavailable in this browser."))?;

    // Call synchronously while the click still owns transient user activation. Waiting for another HTTP request first
    // is what made Safari refuse the legacy Share button. The returned Promise represents the user's share-sheet
    // interaction; opening the sheet successfully is the executor's completed effect.
    share
        .call1(&navigator, &share_data)
        .map_err(|_| ApiError::network("Share needs a direct click. Try Share again."))
}

/// Reports how the share sheet ended: shared, cancelled by the person (code `CANCELLED`), or refused by the browser.
fn settle_share<Msg: 'static>(
    promise: wasm_bindgen::JsValue,
    reply: Box<dyn FnOnce(Result<(), ApiError>) -> Msg>,
    deliver: Callback<Msg>,
) {
    use wasm_bindgen::closure::Closure;
    let Ok(promise) = promise.dyn_into::<js_sys::Promise>() else {
        deliver.emit(reply(Ok(())));
        return;
    };
    let reply = std::rc::Rc::new(std::cell::RefCell::new(Some(reply)));
    let (ok_reply, ok_deliver) = (reply.clone(), deliver.clone());
    let on_ok = Closure::once(move |_: wasm_bindgen::JsValue| {
        if let Some(reply) = ok_reply.borrow_mut().take() {
            ok_deliver.emit(reply(Ok(())));
        }
    });
    let on_err = Closure::once(move |error: wasm_bindgen::JsValue| {
        let name = js_sys::Reflect::get(&error, &wasm_bindgen::JsValue::from_str("name"))
            .ok()
            .and_then(|name| name.as_string())
            .unwrap_or_default();
        let error = match name.as_str() {
            "AbortError" => ApiError { status: 0, code: "CANCELLED".into(), message: "Share was cancelled.".into() },
            "NotAllowedError" => ApiError::network("Safari did not allow the share sheet. Tap Share again."),
            other => ApiError::network(format!("The share did not complete ({other}).")),
        };
        if let Some(reply) = reply.borrow_mut().take() {
            deliver.emit(reply(Err(error)));
        }
    });
    let _ = promise.then2(&on_ok, &on_err);
    on_ok.forget();
    on_err.forget();
}

/// One utterance through the browser's speech recognition (`SpeechRecognition`, or Safari's
/// `webkitSpeechRecognition`), answered exactly once: the words heard, or why nothing was.
fn listen<Msg: 'static>(reply: Box<dyn FnOnce(Result<String, ApiError>) -> Msg>, deliver: Callback<Msg>) {
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsValue;

    let reply = Rc::new(RefCell::new(Some(reply)));
    let finish: Rc<dyn Fn(Result<String, ApiError>)> = Rc::new(move |result| {
        if let Some(reply) = reply.borrow_mut().take() {
            deliver.emit(reply(result));
        }
    });
    let key = |name: &str| JsValue::from_str(name);
    let started = (|| -> Result<(), ApiError> {
        let window = web_sys::window().ok_or_else(|| ApiError::network("The browser window is unavailable."))?;
        let window: &JsValue = window.as_ref();
        let recognizer = ["SpeechRecognition", "webkitSpeechRecognition"]
            .iter()
            .filter_map(|name| js_sys::Reflect::get(window, &key(name)).ok())
            .find_map(|value| value.dyn_into::<js_sys::Function>().ok())
            .ok_or_else(|| ApiError::network("This browser has no speech recognition. Type what happened instead."))?;
        let recognition = js_sys::Reflect::construct(&recognizer, &js_sys::Array::new())
            .map_err(|_| ApiError::network("Speech recognition could not start."))?;
        let set = |name: &str, value: &JsValue| {
            let _ = js_sys::Reflect::set(&recognition, &key(name), value);
        };
        set("lang", &key("en-US"));
        set("interimResults", &JsValue::FALSE);
        set("continuous", &JsValue::FALSE);

        let heard = Rc::new(RefCell::new(String::new()));
        let on_result = {
            let heard = heard.clone();
            Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                let results = js_sys::Reflect::get(&event, &key("results")).unwrap_or(JsValue::UNDEFINED);
                let count = js_sys::Reflect::get(&results, &key("length")).ok().and_then(|n| n.as_f64()).unwrap_or(0.0);
                let mut text = String::new();
                for index in 0..count as u32 {
                    let result = js_sys::Reflect::get_u32(&results, index).unwrap_or(JsValue::UNDEFINED);
                    let best = js_sys::Reflect::get_u32(&result, 0).unwrap_or(JsValue::UNDEFINED);
                    if let Some(words) = js_sys::Reflect::get(&best, &key("transcript")).ok().and_then(|w| w.as_string()) {
                        text.push_str(&words);
                    }
                }
                *heard.borrow_mut() = text.trim().to_owned();
            })
        };
        let on_error = {
            let finish = finish.clone();
            Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                let code = js_sys::Reflect::get(&event, &key("error")).ok().and_then(|c| c.as_string()).unwrap_or_default();
                let message = match code.as_str() {
                    "not-allowed" | "service-not-allowed" => {
                        "Microphone access was not allowed. Allow it for this site in Safari, then tap the mic again.".to_owned()
                    }
                    "no-speech" => "No speech was heard. Tap the mic and speak.".to_owned(),
                    "audio-capture" => "No microphone was found.".to_owned(),
                    other => format!("Speech recognition stopped ({other})."),
                };
                finish(Err(ApiError::network(message)));
            })
        };
        let on_end = {
            let finish = finish.clone();
            Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
                let text = heard.borrow().clone();
                finish(if text.is_empty() {
                    Err(ApiError::network("No speech was heard. Tap the mic and speak."))
                } else {
                    Ok(text)
                });
            })
        };
        set("onresult", on_result.as_ref());
        set("onerror", on_error.as_ref());
        set("onend", on_end.as_ref());
        on_result.forget();
        on_error.forget();
        on_end.forget();

        js_sys::Reflect::get(&recognition, &key("start"))
            .ok()
            .and_then(|start| start.dyn_into::<js_sys::Function>().ok())
            .ok_or_else(|| ApiError::network("Speech recognition could not start."))?
            .call0(&recognition)
            .map_err(|_| ApiError::network("Speech recognition could not start."))?;
        Ok(())
    })();
    if let Err(error) = started {
        finish(Err(error));
    }
}

/// Device storage, or `None` where it is unavailable (private mode, blocked site data). Never a panic.
fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
}

/// Read an endpoint from the shell's own infrastructure (not a screen): the entitlements for a portal visit.
pub async fn fetch<E: crate::app::cmd::Endpoint>(endpoint: E) -> Result<E::Response, ApiError> {
    let text = send(E::METHOD, &endpoint.path(), endpoint.body().as_ref())
        .await?
        .to_string();
    serde_json::from_str(&text).map_err(|error| ApiError::decode(error.to_string()))
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
        (Method::Put, Some(body)) => HttpRequest::put(path).json(body),
        (Method::Put, None) => HttpRequest::put(path).build(),
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
            // Two refusal spellings exist: `error: { code, message }` (the Rust API) and `error: "CODE", message`
            // (a relay). Both become the same ApiError.
            let error = value.get("error").cloned().unwrap_or_default();
            let code = error
                .as_str()
                .map(str::to_owned)
                .or_else(|| field(&error, "code"));
            let message = field(&error, "message").or_else(|| field(&value, "message"));
            Err(ApiError {
                status,
                code: code.unwrap_or_else(|| "FAILED".into()),
                message: message.unwrap_or_else(|| "The request failed.".into()),
            })
        }
        Some(value) if ok => Ok(value),
        Some(value) => Err(ApiError {
            status,
            code: field(&value, "code").unwrap_or_else(|| "HTTP".into()),
            message: field(&value, "message")
                .or_else(|| field(&value, "error"))
                // The recorder's relay speaks problem+json: `{ title, detail }`.
                .or_else(|| field(&value, "detail"))
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
        let relay = interpret(
            403,
            false,
            r#"{"ok":false,"error":"FORBIDDEN","message":"no matching entitlement"}"#,
        )
        .unwrap_err();
        assert_eq!(
            (relay.code.as_str(), relay.message.as_str()),
            ("FORBIDDEN", "no matching entitlement")
        );
        let text = interpret(409, false, r#"{"error":"The role is in use."}"#).unwrap_err();
        assert_eq!(text.message, "The role is in use.");
        assert_eq!(interpret(200, true, "<html>").unwrap_err().code, "DECODE");
    }
}
