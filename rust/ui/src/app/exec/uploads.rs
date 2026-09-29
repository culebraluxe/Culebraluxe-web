//! Files in pieces: the chunked upload, the one multipart form, and a film straight to Mux — each with timeouts and retries.

#[allow(unused_imports)]
use super::*;

/// 3 MB per request: comfortably under the gateway's ~4.5 MB cap, so a 13 MB photograph uploads.
pub(super) const CHUNK_BYTES: f64 = 3.0 * 1024.0 * 1024.0;

/// THE WAY OUT. No upload request may take longer than this: past it, the request is abandoned (aborted, so the
/// browser lets go of it) and answers as a network failure, which is retried. A request that neither answers nor
/// fails used to stall the whole batch silently — the failure was never reported because it never happened.
pub(super) const REQUEST_SECONDS: u64 = 50;

/// Tries per request, waiting 4, 8, 16, 32 seconds between them: a dropped connection or a server restart is
/// ridden out instead of costing the photograph.
pub(super) const ATTEMPTS: u32 = 5;

/// One request, with the way out: it answers, fails, or is aborted at `REQUEST_SECONDS` — never hangs.
pub(super) async fn post_once(path: &str, form: web_sys::FormData) -> Result<serde_json::Value, ApiError> {
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
pub(super) async fn post_form(
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
pub(super) async fn upload_chunked(
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
pub(super) async fn post_json(path: &str, body: &serde_json::Value) -> Result<serde_json::Value, ApiError> {
    let mut attempt = 1;
    loop {
        let answer = async {
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
        .await;
        match answer {
            Err(error) if attempt < ATTEMPTS && (error.code == "NETWORK" || error.status >= 500) => {
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
                .and_then(|range| range.rsplit('-').next().and_then(|end| end.trim().parse::<f64>().ok()))
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
        .filter(|saved| saved.get("at").and_then(serde_json::Value::as_f64).is_some_and(|at| now - at < 55.0 * 60_000.0))
        .and_then(|saved| {
            Some((saved.get("uploadId")?.as_str()?.to_owned(), saved.get("uploadUrl")?.as_str()?.to_owned()))
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
            let session = post_json("/api/portal/property-video/upload", &serde_json::json!({})).await?;
            let text = |key: &str| session.get(key).and_then(serde_json::Value::as_str).map(str::to_owned);
            let (Some(upload_id), Some(url)) = (text("uploadId"), text("uploadUrl")) else {
                return Err(ApiError::decode("Mux did not give an upload address."));
            };
            if let Some(storage) = storage() {
                let saved = serde_json::json!({ "uploadId": upload_id, "uploadUrl": url, "at": now });
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
                    .header("Content-Range", &format!("bytes {start}-{}/{total}", end - 1.0))
                    .body(piece)
                    .map_err(|error| ApiError::network(error.to_string()))?
                    .send()
                    .await
                    .map_err(|error| ApiError::network(format!("the piece did not arrive ({error})")))?;
                match response.status() {
                    200 | 201 => Ok(None),
                    308 => Ok(Some(
                        response
                            .headers()
                            .get("range")
                            .and_then(|range| range.rsplit('-').next().and_then(|e| e.trim().parse::<f64>().ok()))
                            .map(|last| last + 1.0)
                            .unwrap_or(end),
                    )),
                    status => Err(ApiError { status, code: "NETWORK".into(), message: format!("Mux answered {status}.") }),
                }
            }
            .await;
            match sent {
                Ok(next) => break next,
                Err(error) if attempt < ATTEMPTS => {
                    yew::platform::time::sleep(std::time::Duration::from_secs(2u64 << attempt)).await;
                    attempt += 1;
                    // Mux may hold part of the failed piece: continue from what it has, if it says.
                    if let Ok(position) = video_offset(&url, total).await {
                        if position != Some(start) {
                            break position;
                        }
                    }
                    let _ = error;
                }
                Err(error) => return Err(ApiError { message: format!("The video stopped uploading: {}", error.message), ..error }),
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
    Err(ApiError::network("Mux is taking too long to prepare the video. Look again on the Video tab later."))
}
