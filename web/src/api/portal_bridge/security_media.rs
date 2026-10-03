//! Generic rows, the security grant and role writes, and the chunked property-media upload.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct RowsQuery {
    #[serde(default)]
    pub(super) screen: String,
}

/// The portal's generic rows: the Security settings tables (roles and authorities).
pub(super) async fn rows(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<RowsQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    match query.screen.as_str() {
        "settings-roles" | "settings-authorities" => {
            let roles = state
                .services()
                .security()
                .list_role_entitlements(&resolved.service)
                .await;
            Ok(Json(Value::Array(role_rows(&to_json(
                roles.map_err(failed(&resolved))?,
            )))))
        }
        other => Err(correlate(
            ApiError::new(
                StatusCode::NOT_IMPLEMENTED,
                "ROWS_NOT_IN_RUST_YET",
                format!("The '{other}' rows have no Rust service yet."),
                false,
            ),
            &resolved,
        )),
    }
}

pub(super) const CANONICAL_INTERNAL_ROLES: &[&str] = &[
    "internal_guest",
    "user",
    "business_power_user",
    "owner",
    "root",
];

pub(super) fn is_uuid(text: &str) -> bool {
    uuid::Uuid::parse_str(text).is_ok_and(|id| (1..=5).contains(&id.get_version_num()))
}

/// ROOT sets one internal user's canonical role; answers the users as they now stand.
pub(super) async fn security_users_put(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let (Some(user), Some(role)) = (str_at(&body, "appUserId"), str_at(&body, "roleCode")) else {
        return Err(ApiError::bad_request(
            "ROLE_ASSIGNMENT_INVALID",
            "Invalid user or canonical role.",
        ));
    };
    if !is_uuid(user) || !CANONICAL_INTERNAL_ROLES.contains(&role) {
        return Err(ApiError::bad_request(
            "ROLE_ASSIGNMENT_INVALID",
            "Invalid user or canonical role.",
        ));
    }
    let security = state.services().security();
    security
        .set_user_primary_role(user, role, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    let users = security
        .list_security_users(&resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(json!({ "users": to_json(users) })))
}

/// ROOT grants or revokes one entitlement for one internal role; answers the role table as it now stands.
pub(super) async fn role_entitlements_put(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let role = str_at(&body, "roleCode").unwrap_or("");
    let action = str_at(&body, "action").unwrap_or("");
    let granted = body.get("granted").and_then(Value::as_bool);
    let role_ok =
        (1..=64).contains(&role.len()) && role.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    let action_ok = (2..=101).contains(&action.len())
        && action.starts_with(|c: char| c.is_ascii_lowercase())
        && action.chars().all(|c| c.is_ascii_lowercase() || c == '.');
    let (true, true, Some(granted)) = (role_ok, action_ok, granted) else {
        return Err(ApiError::bad_request(
            "ROLE_GRANT_INVALID",
            "Invalid role or action.",
        ));
    };
    let security = state.services().security();
    security
        .set_role_entitlement(role, action, granted, &resolved.service)
        .await
        .map_err(failed(&resolved))?;
    let roles = security
        .list_role_entitlements(&resolved.service)
        .await
        .map_err(failed(&resolved))?;
    Ok(Json(json!({ "roles": to_json(roles) })))
}

/// Why a background finish failed, for the browser's next `status` question (this process only; elsewhere the
/// answer is the generic message).
pub(super) static UPLOAD_FAILURES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, String>>,
> = std::sync::LazyLock::new(Default::default);

/// A photograph uploaded in chunks, one step per request (`step`: init, chunk, complete, status), for photos larger than one
/// request may carry. The media service stages the chunks and assembles, stores and attaches the image.
pub(super) async fn property_media_chunked(
    State(state): State<ApiState>,
    headers: HeaderMap,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let bad =
        |code: &str, message: &str| correlate(ApiError::bad_request(code, message), &resolved);
    let mut fields = std::collections::HashMap::<String, String>::new();
    let mut chunk: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.map_err(|error| {
        bad(
            "MEDIA_MULTIPART_INVALID",
            &format!("Invalid media upload: {error}"),
        )
    })? {
        let name = field.name().unwrap_or_default().to_owned();
        if name == "chunk" {
            let bytes = field.bytes().await.map_err(|error| {
                bad(
                    "MEDIA_CHUNK_INVALID",
                    &format!("Invalid media chunk: {error}"),
                )
            })?;
            chunk = Some(bytes.to_vec());
        } else {
            let value = field.text().await.map_err(|error| {
                bad(
                    "MEDIA_MULTIPART_INVALID",
                    &format!("Invalid media upload: {error}"),
                )
            })?;
            fields.insert(name, value);
        }
    }
    let field = |key: &str| fields.get(key).map(String::as_str).unwrap_or("");
    let property_id = field("propertyId").to_owned();
    if property_id.is_empty() {
        return Err(bad("MEDIA_PROPERTY_REQUIRED", "Property is required."));
    }
    let media = state.services().media();
    let context = &resolved.service;
    let answer = if field("step") == "init" {
        let number = |key: &str| {
            field(key)
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && *n > 0.0)
        };
        let filename = field("filename").to_owned();
        if filename.is_empty() {
            return Err(bad(
                "MEDIA_FILENAME_REQUIRED",
                "An image filename is required.",
            ));
        }
        let (Some(byte_size), Some(chunk_count), Some(chunk_size)) = (
            number("byteSize"),
            number("chunkCount"),
            number("chunkSize"),
        ) else {
            return Err(bad(
                "MEDIA_UPLOAD_SHAPE_INVALID",
                "The upload must declare a positive size, chunk count and chunk size.",
            ));
        };
        if byte_size > model::MAX_MEDIA_UPLOAD_BYTES as f64 {
            return Err(bad(
                "MEDIA_UPLOAD_TOO_LARGE",
                "Image is too large (max 50 MB).",
            ));
        }
        let role = if field("role").is_empty() {
            "gallery"
        } else {
            field("role")
        }
        .to_owned();
        if role != "hero" && role != "gallery" {
            return Err(bad("MEDIA_ROLE_INVALID", "Invalid media role."));
        }
        let mime_type = field("mimeType").to_owned();
        if !mime_type.starts_with("image/") {
            return Err(bad(
                "MEDIA_TYPE_UNSUPPORTED",
                "Only image uploads are supported.",
            ));
        }
        // LIKE A TORRENT: what the server already has is not sent again. A file this property already shows is
        // skipped, and a file an earlier try left unfinished resumes with only the chunks it lacks — so choosing the
        // same folder again, after a failure or a closed tab, picks up where it stopped.
        let lookup = media
            .find_media_upload(
                &property_id,
                &filename,
                byte_size as i64,
                chunk_size as i32,
                context,
            )
            .await
            .map_err(failed(&resolved))?;
        match lookup {
            crate::media::UploadLookup::AlreadyStored => {
                return Ok(Json(
                    serde_json::json!({ "ok": true, "state": "done", "skipped": true }),
                ));
            }
            crate::media::UploadLookup::Unfinished {
                upload_id,
                status,
                received,
            } => {
                let state = match status.as_str() {
                    "complete" => "processing",
                    _ => "uploading",
                };
                return Ok(Json(
                    serde_json::json!({ "ok": true, "state": state, "uploadId": upload_id, "received": received }),
                ));
            }
            crate::media::UploadLookup::New => {}
        }
        let upload = db::BeginMediaUpload {
            upload_id: uuid::Uuid::new_v4().to_string(),
            property_id,
            filename,
            mime_type,
            byte_size: byte_size as i64,
            chunk_count: chunk_count as i32,
            chunk_size: chunk_size as i32,
            sha256: Some(field("sha256").to_owned()).filter(|v| !v.is_empty()),
            role,
            alt_text: Some(field("altText").trim().to_owned()).filter(|v| !v.is_empty()),
        };
        to_json(
            media
                .begin_media_upload(upload, context)
                .await
                .map_err(failed(&resolved))?,
        )
    } else {
        let upload_id = field("uploadId").to_owned();
        if upload_id.is_empty() {
            return Err(bad("MEDIA_UPLOAD_ID_REQUIRED", "An upload id is required."));
        }
        match field("step") {
            "chunk" => {
                let index = field("chunkIndex")
                    .trim()
                    .parse::<i32>()
                    .ok()
                    .filter(|i| *i >= 0);
                let (Some(bytes), Some(index)) = (chunk, index) else {
                    return Err(bad("MEDIA_CHUNK_REQUIRED", "An image chunk is required."));
                };
                to_json(
                    media
                        .stage_media_chunk(&upload_id, index, bytes, context)
                        .await
                        .map_err(failed(&resolved))?,
                )
            }
            // Claimed here, finished in the background: finishing a large photograph takes about a minute, longer
            // than Safari holds a request open, and a request the browser drops must not cancel the save with it.
            // The browser asks `status` until the answer is `done` or `failed`.
            "complete" => {
                // Sent twice (a retry after an answer that never arrived) is not an error: the upload is already
                // being finished, or already is.
                if let Err(error) = media.claim_media_upload(&upload_id, context).await {
                    let already = media
                        .media_upload_state(&upload_id, context)
                        .await
                        .map_err(failed(&resolved))?;
                    if already == "processing" || already == "done" {
                        return Ok(Json(
                            serde_json::json!({ "ok": true, "state": already, "uploadId": upload_id }),
                        ));
                    }
                    return Err(failed(&resolved)(error));
                }
                let (state, context, id) = (state.clone(), context.clone(), upload_id.clone());
                tokio::spawn(async move {
                    if let Err(error) = state
                        .services()
                        .media()
                        .finish_media_upload(&id, &context)
                        .await
                    {
                        // A business refusal is written for a person; anything else is not theirs to read.
                        let reason = match &error {
                            CoreServiceError::Business { message, .. } => message.clone(),
                            _ => "The photo could not be saved.".to_owned(),
                        };
                        UPLOAD_FAILURES
                            .lock()
                            .map(|mut failures| failures.insert(id.clone(), reason))
                            .ok();
                        crate::api::error_capture::record(
                            "rust:api",
                            "media.completeMediaUpload",
                            &error.to_string(),
                            "error",
                            None,
                            serde_json::json!({ "uploadId": id }),
                        );
                    }
                });
                serde_json::json!({ "ok": true, "state": "processing", "uploadId": upload_id })
            }
            "status" => {
                let upload_state = media
                    .media_upload_state(&upload_id, context)
                    .await
                    .map_err(failed(&resolved))?;
                let message = (upload_state == "failed").then(|| {
                    UPLOAD_FAILURES
                        .lock()
                        .ok()
                        .and_then(|failures| failures.get(&upload_id).cloned())
                        .unwrap_or_else(|| "The photo could not be saved.".to_owned())
                });
                serde_json::json!({ "ok": true, "state": upload_state, "message": message })
            }
            _ => return Err(bad("MEDIA_STEP_UNKNOWN", "Unknown upload step.")),
        }
    };
    Ok(Json(answer))
}
