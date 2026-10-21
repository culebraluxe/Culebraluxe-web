//! Uploads: a photograph whole or straight to object storage, and a film straight to Mux — each with a way out
//! and retries. The staged chunk protocol is retired; there is no piecewise photograph path here any more.

#[allow(unused_imports)]
use super::*;

/// THE WAY OUT. No upload request may take longer than this: past it, the request is abandoned (aborted, so the
/// browser lets go of it) and answers as a network failure, which is retried. A request that neither answers nor
/// fails used to stall the whole batch silently — the failure was never reported because it never happened.
pub(super) const REQUEST_SECONDS: u64 = 50;

/// Tries per request, waiting 4, 8, 16, 32 seconds between them: a dropped connection or a server restart is
/// ridden out instead of costing the photograph.
pub(super) const ATTEMPTS: u32 = 5;

/// One request, with the way out: it answers, fails, or is aborted at `seconds` — never hangs.
pub(super) async fn post_once_within(
    path: &str,
    form: web_sys::FormData,
    seconds: u64,
) -> Result<serde_json::Value, ApiError> {
    let controller = web_sys::AbortController::new()
        .map_err(|_| ApiError::network("The browser could not start the upload."))?;
    let signal = controller.signal();
    yew::platform::spawn_local(async move {
        yew::platform::time::sleep(std::time::Duration::from_secs(seconds)).await;
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

/// One request, with the way out: a short request (a chunk, a status poll) at `REQUEST_SECONDS`.
pub(super) async fn post_once(
    path: &str,
    form: web_sys::FormData,
) -> Result<serde_json::Value, ApiError> {
    post_once_within(path, form, REQUEST_SECONDS).await
}

/// The same retry rule with an explicit per-attempt ceiling: a WHOLE photograph is one long request, not a
/// short chunk, so it needs more time than a piece does.
pub(super) async fn post_form_within(
    path: &str,
    build: impl Fn() -> Result<web_sys::FormData, ApiError>,
    seconds: u64,
) -> Result<serde_json::Value, ApiError> {
    let mut attempt = 1;
    loop {
        match post_once_within(path, build()?, seconds).await {
            Err(error)
                if attempt < ATTEMPTS && (error.code == "NETWORK" || error.status >= 500) =>
            {
                yew::platform::time::sleep(std::time::Duration::from_secs(2u64 << attempt)).await;
                attempt += 1;
            }
            answer => return answer,
        }
    }
}

/// A whole photograph, in ONE request. The server derives the web/card/thumb copies, writes them and the
/// original to object storage (verified), then records the rows and attaches the property in one transaction.
/// Nothing is staged, so no chunk manifest exists to resume — and none is needed: the server answers a
/// resent upload of a file the property already shows with `skipped: true`, so the whole request is as safe
/// to retry as a chunk was.
///
/// `fields` and `init_fields` all travel with the single request; "first step only" has no meaning here.
pub(super) async fn upload_whole_file(
    file: web_sys::File,
    path: &str,
    fields: &[(String, String)],
    init_fields: &[(String, String)],
) -> Result<(), ApiError> {
    let form = || -> Result<web_sys::FormData, ApiError> {
        let form = web_sys::FormData::new()
            .map_err(|_| ApiError::network("The browser could not create the upload form."))?;
        for (key, value) in fields.iter().chain(init_fields.iter()) {
            form.append_with_str(key, value)
                .map_err(|_| ApiError::network("The browser could not prepare the upload."))?;
        }
        form.append_with_blob_and_filename("file", &file, &file.name())
            .map_err(|_| ApiError::network("The browser could not prepare the upload."))?;
        Ok(form)
    };
    // One long request, not a series of short ones: the ceiling has to fit the whole photograph.
    post_form_within(path, form, WHOLE_FILE_REQUEST_SECONDS).await?;
    Ok(())
}

/// A whole photograph is one request, and may take far longer than a 3 MB chunk: five minutes is the way out.
pub(super) const WHOLE_FILE_REQUEST_SECONDS: u64 = 300;

/// A long-lived file PUT to object storage has the same ceiling as a whole-file request.
pub(super) const DIRECT_PUT_SECONDS: u64 = WHOLE_FILE_REQUEST_SECONDS;

fn field<'a>(fields: &'a [(String, String)], name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Read a file's bytes in the browser. Needed only for the direct upload's SHA-256 declaration; the PUT itself
/// sends the browser's own `File` handle, so the bytes are hashed once, not copied into a form.
async fn file_bytes(file: &web_sys::File) -> Result<Vec<u8>, ApiError> {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};
    let blob: &web_sys::Blob = file.unchecked_ref();
    let promise = blob.array_buffer();
    type Outcome = Rc<RefCell<Option<Result<Vec<u8>, ()>>>>;
    let outcome: Outcome = Rc::new(RefCell::new(None));
    let waker: Rc<RefCell<Option<Waker>>> = Rc::new(RefCell::new(None));
    // The callbacks are owned by this future and live until the promise settles (or this function is
    // dropped); JS holds a reference to them while the promise is pending.
    let (fulfilled_outcome, fulfilled_waker) = (outcome.clone(), waker.clone());
    let on_fulfilled: wasm_bindgen::closure::Closure<dyn FnMut(wasm_bindgen::JsValue)> =
        wasm_bindgen::closure::Closure::new(move |value| {
            let buffer = js_sys::ArrayBuffer::from(value);
            *fulfilled_outcome.borrow_mut() = Some(Ok(js_sys::Uint8Array::new(&buffer).to_vec()));
            if let Some(waker) = fulfilled_waker.borrow_mut().take() {
                waker.wake();
            }
        });
    let (rejected_outcome, rejected_waker) = (outcome.clone(), waker.clone());
    let on_rejected: wasm_bindgen::closure::Closure<dyn FnMut(wasm_bindgen::JsValue)> =
        wasm_bindgen::closure::Closure::new(move |_| {
            *rejected_outcome.borrow_mut() = Some(Err(()));
            if let Some(waker) = rejected_waker.borrow_mut().take() {
                waker.wake();
            }
        });
    let _ = promise.then2(&on_fulfilled, &on_rejected);
    let bytes = std::future::poll_fn(move |context: &mut Context<'_>| {
        if let Some(outcome) = outcome.borrow_mut().take() {
            return Poll::Ready(
                outcome.map_err(|_| ApiError::network("The file could not be read.")),
            );
        }
        *waker.borrow_mut() = Some(context.waker().clone());
        Poll::Pending
    })
    .await;
    drop((on_fulfilled, on_rejected));
    bytes
}

/// Tell the server an upload was abandoned, so it deletes the one object the browser stored. Best effort: a
/// failed abort is reported nowhere and the bucket's lifecycle policy is the backstop.
async fn abort_direct_upload(upload_id: &str) {
    let body = serde_json::json!({ "uploadId": upload_id });
    let _ = post_json_once(crate::app::api::direct_upload::ABORT, &body).await;
}

/// One JSON POST, no retries: used for a cleanup call where waiting is not worth it.
async fn post_json_once(
    path: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let signal = way_out(REQUEST_SECONDS)?;
    let response = HttpRequest::post(path)
        .abort_signal(Some(&signal))
        .json(body)
        .map_err(|error| ApiError::network(error.to_string()))?
        .send()
        .await
        .map_err(|error| ApiError::network(format!("the request did not complete ({error})")))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| ApiError::network(format!("the answer did not arrive ({error})")))?;
    interpret(status, response.ok(), &text)
}

/// A property photograph sent DIRECT TO OBJECT STORAGE: initiate, PUT, finalize.
///
/// The file never reaches this application: the server signs a short-lived PUT URL for the property-media
/// bucket, the browser PUTs the bytes there, and the server then HEADs/SHA-256-verifies the object, derives the
/// servable copies and records the rows in one transaction. Nothing is staged, so a large photograph no longer
/// depends on the hosting edge's request-body cap — and no chunk row is written.
///
/// There is deliberately NO fallback: the staged chunk protocol is retired, so a deployment that cannot serve the
/// direct flow reports the failure instead of silently restarting an unsafe path.
pub(super) async fn upload_direct(
    file: web_sys::File,
    fields: &[(String, String)],
    init_fields: &[(String, String)],
) -> Result<(), ApiError> {
    direct_upload_once(&file, fields, init_fields).await
}

async fn direct_upload_once(
    file: &web_sys::File,
    fields: &[(String, String)],
    init_fields: &[(String, String)],
) -> Result<(), ApiError> {
    let property_id = field(fields, "propertyId")
        .ok_or_else(|| ApiError::decode("The upload is missing its property."))?;
    let role = field(init_fields, "role").unwrap_or("gallery");
    let alt_text = field(init_fields, "altText").unwrap_or("");
    let mime_type = file.type_();
    let byte_size = file.size();
    if byte_size <= 0.0 {
        return Err(ApiError::network("That file is empty."));
    }

    // The declared SHA-256 is what finalize proves the stored object against; a transport error or a tampered
    // object is refused there rather than recorded as a photograph.
    let sha256 = sha256_hex(&file_bytes(file).await?);

    let opened = post_json(
        crate::app::api::direct_upload::INITIATE,
        &serde_json::json!({
            "propertyId": property_id,
            "role": role,
            "filename": file.name(),
            "mimeType": mime_type,
            "byteSize": byte_size as i64,
            "sha256": sha256,
            "altText": alt_text,
        }),
    )
    .await?;
    if opened.get("state").and_then(serde_json::Value::as_str) == Some("done") {
        // The property already shows this file: nothing to send, exactly like the whole-file route's retry.
        return Ok(());
    }
    let text = |key: &str| {
        opened
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let (Some(upload_id), Some(upload_url)) = (text("uploadId"), text("uploadUrl")) else {
        return Err(ApiError::decode(
            "The direct upload was opened but no upload address came back.",
        ));
    };
    let content_type = text("contentType").unwrap_or_else(|| mime_type.clone());

    // Two: the bytes, straight to object storage. A PUT is idempotent for this key, so a retry is safe.
    let uploaded = put_file_once(&upload_url, &content_type, file).await;
    if let Err(error) = uploaded {
        abort_direct_upload(&upload_id).await;
        return Err(error);
    }

    // Three: the server verifies the object, derives the copies, and records them and the property link.
    match post_json_within(
        crate::app::api::direct_upload::FINALIZE,
        &serde_json::json!({ "uploadId": upload_id }),
        WHOLE_FILE_REQUEST_SECONDS,
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(error) => {
            abort_direct_upload(&upload_id).await;
            Err(error)
        }
    }
}

/// PUT one file to a signed object-storage URL, with the way out and the same retry rule as every other request.
async fn put_file_once(
    url: &str,
    content_type: &str,
    file: &web_sys::File,
) -> Result<(), ApiError> {
    let mut attempt = 1;
    loop {
        let sent = async {
            let signal = way_out(DIRECT_PUT_SECONDS)?;
            let response = HttpRequest::put(url)
                .abort_signal(Some(&signal))
                .header("Content-Type", content_type)
                .body(file.clone())
                .map_err(|error| ApiError::network(error.to_string()))?
                .send()
                .await
                .map_err(|error| {
                    ApiError::network(format!(
                        "the photograph did not reach object storage ({error})"
                    ))
                })?;
            let status = response.status();
            if response.ok() {
                Ok(())
            } else {
                Err(ApiError {
                    status,
                    code: "HTTP".into(),
                    message: format!("Object storage answered {status}."),
                })
            }
        }
        .await;
        match sent {
            Err(error)
                if attempt < ATTEMPTS && (error.code == "NETWORK" || error.status >= 500) =>
            {
                yew::platform::time::sleep(std::time::Duration::from_secs(2u64 << attempt)).await;
                attempt += 1;
            }
            answer => return answer,
        }
    }
}

/// 8 MiB per request to Mux: a multiple of 256 KiB, as its upload store requires, and short enough on a slow line.
pub(super) const VIDEO_CHUNK_BYTES: f64 = 8.0 * 1024.0 * 1024.0;

/// A video piece may take longer than a photo request: the way out is further away, but it is there.
pub(super) const VIDEO_REQUEST_SECONDS: u64 = 120;

/// A signal that aborts its request after `seconds` — every request in an upload has a way out.
pub(super) fn way_out(seconds: u64) -> Result<web_sys::AbortSignal, ApiError> {
    let controller = web_sys::AbortController::new()
        .map_err(|_| ApiError::network("The browser could not start the upload."))?;
    let signal = controller.signal();
    yew::platform::spawn_local(async move {
        yew::platform::time::sleep(std::time::Duration::from_secs(seconds)).await;
        controller.abort();
    });
    Ok(signal)
}

/// A JSON POST to this server, with the way out and retries.
pub(super) async fn post_json(
    path: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    post_json_within(path, body, REQUEST_SECONDS).await
}

/// The same retry rule with an explicit per-attempt ceiling: finalizing a large photograph happens after the
/// bytes are already stored, but it still downloads, decodes and re-encodes them, so it needs the long ceiling.
pub(super) async fn post_json_within(
    path: &str,
    body: &serde_json::Value,
    seconds: u64,
) -> Result<serde_json::Value, ApiError> {
    let mut attempt = 1;
    loop {
        let answer = async {
            let signal = way_out(seconds)?;
            let response = HttpRequest::post(path)
                .abort_signal(Some(&signal))
                .json(body)
                .map_err(|error| ApiError::network(error.to_string()))?
                .send()
                .await
                .map_err(|error| {
                    ApiError::network(format!("the request did not complete ({error})"))
                })?;
            let status = response.status();
            let text = response.text().await.map_err(|error| {
                ApiError::network(format!("the answer did not arrive ({error})"))
            })?;
            interpret(status, response.ok(), &text)
        }
        .await;
        match answer {
            Err(error)
                if attempt < ATTEMPTS && (error.code == "NETWORK" || error.status >= 500) =>
            {
                yew::platform::time::sleep(std::time::Duration::from_secs(2u64 << attempt)).await;
                attempt += 1;
            }
            answer => return answer,
        }
    }
}

/// Where Mux's upload store stands: `Ok(Some(next byte))` while incomplete, `Ok(None)` once it has the whole file.
/// `Err` when the answer says nothing usable (the session expired, or its progress is not readable here).
pub(super) async fn video_offset(url: &str, total: f64) -> Result<Option<f64>, ApiError> {
    let signal = way_out(30)?;
    let response = HttpRequest::put(url)
        .abort_signal(Some(&signal))
        .header("Content-Range", &format!("bytes */{total}"))
        .send()
        .await
        .map_err(|error| ApiError::network(format!("Mux did not answer ({error})")))?;
    match response.status() {
        200 | 201 => Ok(None),
        308 => Ok(Some(
            response
                .headers()
                .get("range")
                .and_then(|range| {
                    range
                        .rsplit('-')
                        .next()
                        .and_then(|end| end.trim().parse::<f64>().ok())
                })
                .map(|end| end + 1.0)
                .unwrap_or(0.0),
        )),
        status => Err(ApiError::network(format!("Mux answered {status}."))),
    }
}

/// A PROPERTY FILM, SENT THE WAY THE PHOTOS ARE — in pieces, each its own short request with a way out, retried when
/// it fails, and resumed when the same file is chosen again within the hour. Unlike the photos, the pieces go straight
/// from the browser to Mux (a direct upload URL this server asks Mux for), so a gigabyte never passes through here.
/// Then Mux encodes it; this asks until it is ready, and the server attaches it to the property.
pub(super) async fn upload_video(
    file: &web_sys::File,
    property_id: &str,
    role: &str,
    caption: &str,
    progress: &dyn Fn(crate::app::cmd::VideoProgress),
) -> Result<(), ApiError> {
    use crate::app::cmd::VideoProgress;
    let total = file.size();
    if total <= 0.0 {
        return Err(ApiError::network("That file is empty."));
    }
    let step = |sent: f64, stage: &'static str| progress(VideoProgress { sent, total, stage });
    let resume_key = format!("culebra-video:{property_id}:{}:{total}", file.name());
    let now = js_sys::Date::now();

    // An upload of this same file started within the hour resumes where Mux says it stands.
    let saved: Option<(String, String)> = storage()
        .and_then(|storage| storage.get_item(&resume_key).ok().flatten())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .filter(|saved| {
            saved
                .get("at")
                .and_then(serde_json::Value::as_f64)
                .is_some_and(|at| now - at < 55.0 * 60_000.0)
        })
        .and_then(|saved| {
            Some((
                saved.get("uploadId")?.as_str()?.to_owned(),
                saved.get("uploadUrl")?.as_str()?.to_owned(),
            ))
        });
    let mut resumed: Option<(String, String, Option<f64>)> = None;
    if let Some((upload_id, url)) = saved {
        if let Ok(offset) = video_offset(&url, total).await {
            resumed = Some((upload_id, url, offset));
        }
    }
    let (upload_id, url, mut offset) = match resumed {
        Some((upload_id, url, offset)) => (upload_id, url, offset),
        None => {
            let session =
                post_json("/api/portal/property-video/upload", &serde_json::json!({})).await?;
            let text = |key: &str| {
                session
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            };
            let (Some(upload_id), Some(url)) = (text("uploadId"), text("uploadUrl")) else {
                return Err(ApiError::decode("Mux did not give an upload address."));
            };
            if let Some(storage) = storage() {
                let saved =
                    serde_json::json!({ "uploadId": upload_id, "uploadUrl": url, "at": now });
                let _ = storage.set_item(&resume_key, &saved.to_string());
            }
            (upload_id, url, Some(0.0))
        }
    };

    // The pieces. A piece that fails is retried from where Mux says it stands (or from its own start).
    while let Some(start) = offset {
        step(start, "uploading");
        let end = (start + VIDEO_CHUNK_BYTES).min(total);
        let mut attempt = 1;
        let next = loop {
            let sent = async {
                let piece = file
                    .slice_with_f64_and_f64(start, end)
                    .map_err(|_| ApiError::network("Part of the video could not be read."))?;
                let signal = way_out(VIDEO_REQUEST_SECONDS)?;
                let response = HttpRequest::put(&url)
                    .abort_signal(Some(&signal))
                    .header(
                        "Content-Range",
                        &format!("bytes {start}-{}/{total}", end - 1.0),
                    )
                    .body(piece)
                    .map_err(|error| ApiError::network(error.to_string()))?
                    .send()
                    .await
                    .map_err(|error| {
                        ApiError::network(format!("the piece did not arrive ({error})"))
                    })?;
                match response.status() {
                    200 | 201 => Ok(None),
                    308 => Ok(Some(
                        response
                            .headers()
                            .get("range")
                            .and_then(|range| {
                                range
                                    .rsplit('-')
                                    .next()
                                    .and_then(|e| e.trim().parse::<f64>().ok())
                            })
                            .map(|last| last + 1.0)
                            .unwrap_or(end),
                    )),
                    status => Err(ApiError {
                        status,
                        code: "NETWORK".into(),
                        message: format!("Mux answered {status}."),
                    }),
                }
            }
            .await;
            match sent {
                Ok(next) => break next,
                Err(error) if attempt < ATTEMPTS => {
                    yew::platform::time::sleep(std::time::Duration::from_secs(2u64 << attempt))
                        .await;
                    attempt += 1;
                    // Mux may hold part of the failed piece: continue from what it has, if it says.
                    if let Ok(position) = video_offset(&url, total).await {
                        if position != Some(start) {
                            break position;
                        }
                    }
                    let _ = error;
                }
                Err(error) => {
                    return Err(ApiError {
                        message: format!("The video stopped uploading: {}", error.message),
                        ..error
                    })
                }
            }
        };
        offset = next;
    }
    step(total, "preparing");
    if let Some(storage) = storage() {
        let _ = storage.remove_item(&resume_key);
    }

    // Mux encodes the film; ask until it is ready (the server attaches it then). An hour is the ceiling.
    for _ in 0..720 {
        let answer = post_json(
            "/api/portal/property-video/finalize",
            &serde_json::json!({ "propertyId": property_id, "uploadId": upload_id, "role": role, "caption": caption }),
        )
        .await?;
        if answer.get("attached").and_then(serde_json::Value::as_bool) == Some(true) {
            return Ok(());
        }
        yew::platform::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    Err(ApiError::network(
        "Mux is taking too long to prepare the video. Look again on the Video tab later.",
    ))
}
