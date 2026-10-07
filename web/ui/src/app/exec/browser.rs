//! The browser's own services a command can ask for: a full load, swapping the URL, the share sheet, speech, storage.

#[allow(unused_imports)]
use super::*;

pub fn load(href: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_href(href);
    }
}

pub(super) fn replace_path(path: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(history) = window.history() else {
        return;
    };
    let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(path));
}

pub(super) fn share_pdf(data_uri: &str, filename: &str) -> Result<wasm_bindgen::JsValue, ApiError> {
    let (_, encoded) = data_uri
        .split_once(',')
        .ok_or_else(|| ApiError::decode("The PDF preview is not a data URI."))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| {
            ApiError::decode(format!("The PDF preview could not be decoded: {error}"))
        })?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(ApiError::decode("The generated file was not a PDF."));
    }

    let window =
        web_sys::window().ok_or_else(|| ApiError::network("The browser window is unavailable."))?;
    let window_value: &wasm_bindgen::JsValue = window.as_ref();

    // Construct File([Uint8Array(bytes)], filename, { type: "application/pdf" }) in Rust/WASM.
    // There is deliberately no JavaScript bridge: the executor owns this browser capability just like navigation.
    let file_ctor = js_sys::Reflect::get(window_value, &wasm_bindgen::JsValue::from_str("File"))
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

    let navigator =
        js_sys::Reflect::get(window_value, &wasm_bindgen::JsValue::from_str("navigator"))
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

    if let Ok(can_share) =
        js_sys::Reflect::get(&navigator, &wasm_bindgen::JsValue::from_str("canShare"))
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

    let share = js_sys::Reflect::get(&navigator, &wasm_bindgen::JsValue::from_str("share"))
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
pub(super) fn settle_share<Msg: 'static>(
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
            "AbortError" => ApiError {
                status: 0,
                code: "CANCELLED".into(),
                message: "Share was cancelled.".into(),
            },
            "NotAllowedError" => {
                ApiError::network("Safari did not allow the share sheet. Tap Share again.")
            }
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
pub(super) fn listen<Msg: 'static>(
    reply: Box<dyn FnOnce(Result<String, ApiError>) -> Msg>,
    deliver: Callback<Msg>,
) {
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
        let window = web_sys::window()
            .ok_or_else(|| ApiError::network("The browser window is unavailable."))?;
        let window: &JsValue = window.as_ref();
        let recognizer = ["SpeechRecognition", "webkitSpeechRecognition"]
            .iter()
            .filter_map(|name| js_sys::Reflect::get(window, &key(name)).ok())
            .find_map(|value| value.dyn_into::<js_sys::Function>().ok())
            .ok_or_else(|| {
                ApiError::network(
                    "This browser has no speech recognition. Type what happened instead.",
                )
            })?;
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
                let results =
                    js_sys::Reflect::get(&event, &key("results")).unwrap_or(JsValue::UNDEFINED);
                let count = js_sys::Reflect::get(&results, &key("length"))
                    .ok()
                    .and_then(|n| n.as_f64())
                    .unwrap_or(0.0);
                let mut text = String::new();
                for index in 0..count as u32 {
                    let result =
                        js_sys::Reflect::get_u32(&results, index).unwrap_or(JsValue::UNDEFINED);
                    let best = js_sys::Reflect::get_u32(&result, 0).unwrap_or(JsValue::UNDEFINED);
                    if let Some(words) = js_sys::Reflect::get(&best, &key("transcript"))
                        .ok()
                        .and_then(|w| w.as_string())
                    {
                        text.push_str(&words);
                    }
                }
                *heard.borrow_mut() = text.trim().to_owned();
            })
        };
        let on_error = {
            let finish = finish.clone();
            Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                let code = js_sys::Reflect::get(&event, &key("error"))
                    .ok()
                    .and_then(|c| c.as_string())
                    .unwrap_or_default();
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
                    Err(ApiError::network(
                        "No speech was heard. Tap the mic and speak.",
                    ))
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
pub(super) fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
}

/// Call `object.method(...args)` through reflection: the canvas and font APIs are used from this one place, so the
/// browser-facing surface stays small and no extra web-sys features are needed.
fn call_method(
    object: &wasm_bindgen::JsValue,
    method: &str,
    args: &[wasm_bindgen::JsValue],
) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    let function = js_sys::Reflect::get(object, &wasm_bindgen::JsValue::from_str(method))?
        .dyn_into::<js_sys::Function>()?;
    let arguments = js_sys::Array::new();
    for arg in args {
        arguments.push(arg);
    }
    function.apply(object, &arguments)
}

fn set_property(object: &wasm_bindgen::JsValue, key: &str, value: &wasm_bindgen::JsValue) {
    let _ = js_sys::Reflect::set(object, &wasm_bindgen::JsValue::from_str(key), value);
}

/// One piece of text on a transparent canvas, as large as fits, in navy: `Ok(data URL)`.
fn draw_text_png(
    document: &wasm_bindgen::JsValue,
    family: &str,
    text: &str,
    width: u32,
    height: u32,
    centered: bool,
) -> Result<String, wasm_bindgen::JsValue> {
    use wasm_bindgen::JsValue;
    let canvas = call_method(document, "createElement", &[JsValue::from_str("canvas")])?;
    set_property(&canvas, "width", &JsValue::from_f64(f64::from(width)));
    set_property(&canvas, "height", &JsValue::from_f64(f64::from(height)));
    let context = call_method(&canvas, "getContext", &[JsValue::from_str("2d")])?;
    // Fit by measuring at a reference size, then scaling: a long name shrinks rather than being clipped.
    let reference = 100.0_f64;
    set_property(
        &context,
        "font",
        &JsValue::from_str(&format!("{reference}px \"{family}\"")),
    );
    let measured = call_method(&context, "measureText", &[JsValue::from_str(text)])?;
    let measured_width = js_sys::Reflect::get(&measured, &JsValue::from_str("width"))?
        .as_f64()
        .unwrap_or(reference * 4.0)
        .max(1.0);
    let padding = f64::from(height) * 0.12;
    let available = f64::from(width) - padding * 2.0;
    let size = (reference * available / measured_width).min(f64::from(height) * 0.62);
    set_property(
        &context,
        "font",
        &JsValue::from_str(&format!("{size}px \"{family}\"")),
    );
    set_property(&context, "fillStyle", &JsValue::from_str("#041024"));
    set_property(&context, "textBaseline", &JsValue::from_str("alphabetic"));
    let x = if centered {
        set_property(&context, "textAlign", &JsValue::from_str("center"));
        f64::from(width) / 2.0
    } else {
        set_property(&context, "textAlign", &JsValue::from_str("left"));
        padding
    };
    call_method(
        &context,
        "fillText",
        &[
            JsValue::from_str(text),
            JsValue::from_f64(x),
            JsValue::from_f64(f64::from(height) * 0.7),
        ],
    )?;
    let url = call_method(&canvas, "toDataURL", &[JsValue::from_str("image/png")])?;
    url.as_string()
        .filter(|url| url.starts_with("data:image/png;base64,"))
        .ok_or_else(|| JsValue::from_str("the canvas produced no picture"))
}

/// Draw the signature and the initials, once the chosen face has loaded (a canvas drawn before its font arrives uses a
/// fallback and the picture would not match what the signer saw). Answered exactly once.
pub(super) fn render_signature<Msg: 'static>(
    name: String,
    style: usize,
    reply: Box<dyn FnOnce(Result<crate::app::cmd::SignatureArt, ApiError>) -> Msg>,
    deliver: Callback<Msg>,
) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsValue;

    let family = crate::app::cmd::SIGNATURE_FONTS
        .get(style)
        .map(|(family, _)| *family)
        .unwrap_or(crate::app::cmd::SIGNATURE_FONTS[0].0);
    let reply = std::rc::Rc::new(std::cell::RefCell::new(Some(reply)));
    let finish: std::rc::Rc<dyn Fn(Result<crate::app::cmd::SignatureArt, ApiError>)> = {
        let reply = reply.clone();
        std::rc::Rc::new(move |result| {
            if let Some(reply) = reply.borrow_mut().take() {
                deliver.emit(reply(result));
            }
        })
    };
    let initials = model::forms_applied_signature::format_broker_initials(&name);
    let draw = {
        let (finish, family, name, initials) = (
            finish.clone(),
            family.to_owned(),
            name.clone(),
            initials.clone(),
        );
        move || {
            let document = web_sys::window()
                .and_then(|window| window.document())
                .map(JsValue::from);
            let Some(document) = document else {
                finish(Err(ApiError::network("The browser page is unavailable.")));
                return;
            };
            let signature = draw_text_png(&document, &family, &name, 1000, 260, false);
            let initial_marks = draw_text_png(&document, &family, &initials, 420, 260, true);
            match (signature, initial_marks) {
                (Ok(signature), Ok(initials)) => finish(Ok(crate::app::cmd::SignatureArt {
                    signature,
                    initials,
                })),
                _ => finish(Err(ApiError::network(
                    "Your browser could not draw the signature. Try another browser.",
                ))),
            }
        }
    };
    // Ask the font loader for the face, with the text itself so the right glyph ranges come down.
    let loading = web_sys::window()
        .and_then(|window| window.document())
        .map(JsValue::from)
        .and_then(|document| js_sys::Reflect::get(&document, &JsValue::from_str("fonts")).ok())
        .filter(|fonts| !fonts.is_undefined() && !fonts.is_null())
        .and_then(|fonts| {
            call_method(
                &fonts,
                "load",
                &[
                    JsValue::from_str(&format!("100px \"{family}\"")),
                    JsValue::from_str(&format!("{name}{initials}")),
                ],
            )
            .ok()
        })
        .and_then(|promise| promise.dyn_into::<js_sys::Promise>().ok());
    let Some(promise) = loading else {
        draw();
        return;
    };
    let draw = std::rc::Rc::new(draw);
    let (on_loaded, on_failed) = (draw.clone(), draw);
    // A face that fails to load still gets drawn (in the fallback) rather than leaving the signer stuck.
    let ok = Closure::once(move |_: JsValue| on_loaded());
    let err = Closure::once(move |_: JsValue| on_failed());
    let _ = promise.then2(&ok, &err);
    ok.forget();
    err.forget();
}
