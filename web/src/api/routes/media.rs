//! Media: private and standalone media, property media and films, and the chunked upload protocol.

#[allow(unused_imports)]
use super::*;

pub(super) async fn private_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let found = state
        .services()
        .media()
        .media_bytes(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    let Some((mime_type, bytes)) = found else {
        return Err(correlate(
            ApiError::not_found("MEDIA_NOT_FOUND", "Not found."),
            &resolved,
        ));
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(Body::from(bytes))
        .map_err(|error| {
            correlate(
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "MEDIA_RESPONSE_FAILED",
                    error.to_string(),
                    false,
                ),
                &resolved,
            )
        })
}

pub(super) async fn upload_standalone_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut file: Option<(String, String, Vec<u8>)> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        correlate(
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "MEDIA_MULTIPART_INVALID",
                format!("Invalid media upload: {error}"),
                false,
            ),
            &resolved,
        )
    })? {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().unwrap_or("upload").to_owned();
        let mime_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_owned();
        let bytes = field.bytes().await.map_err(|error| {
            correlate(
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "MEDIA_FILE_INVALID",
                    format!("Invalid media file: {error}"),
                    false,
                ),
                &resolved,
            )
        })?;
        file = Some((filename, mime_type, bytes.to_vec()));
    }

    let (filename, mime_type, bytes) = file.ok_or_else(|| {
        correlate(
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "MEDIA_FILE_REQUIRED",
                "Image file is required.",
                false,
            ),
            &resolved,
        )
    })?;

    let value = state
        .services()
        .media()
        .upload_standalone(&filename, &mime_type, bytes, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    Ok(success(
        json!({
            "id": value.0,
            "filename": value.1,
            "mime_type": value.2,
            "file_size": value.3,
        }),
        &resolved,
    ))
}

pub(super) async fn property_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<model::MediaAsset>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().media();
    let value = service
        .for_property(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn create_property_video_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(_property_id): Path<String>,
    Json(body): Json<CreatePropertyVideoUploadBody>,
) -> Result<Json<ApiSuccess<crate::media::PropertyVideoUploadSession>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mux = mux_video()?;
    let service = state.services().media();
    let value = service
        .create_property_video_upload(&mux, &body.cors_origin, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn finalize_property_video_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((property_id, upload_id)): Path<(String, String)>,
    Json(body): Json<FinalizePropertyVideoUploadBody>,
) -> Result<Json<ApiSuccess<crate::media::PropertyVideoFinalizeResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mux = mux_video()?;
    let service = state.services().media();
    let value = service
        .finalize_property_video_upload(
            &mux,
            &property_id,
            &upload_id,
            &body.role,
            body.caption,
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn attach_property_video(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(property_id): Path<String>,
    Json(body): Json<AttachPropertyVideoBody>,
) -> Result<Json<ApiSuccess<model::AttachPropertyVideoResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().media();
    let value = service
        .attach_property_video(
            AttachPropertyVideoRequest {
                property_id,
                role: body.role,
                mux_asset_id: body.mux_asset_id,
                mux_playback_id: body.mux_playback_id,
                duration_seconds: body.duration_seconds,
                aspect_ratio: body.aspect_ratio,
                caption: body.caption,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// Finishes a chunked upload. Everything expensive happens here: the chunks are checked, assembled, and turned into
/// the copies that make the photograph servable.
pub(super) async fn complete_media_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((_property_id, upload_id)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<model::UploadPropertyMediaResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().media();
    let value = service
        .complete_media_upload(&upload_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    Ok(success(value, &resolved))
}

/// One shape of failure for every step of a chunked upload, so the browser gets a message it can show.
pub(super) fn media_upload_error(
    correlation: &str,
    status: StatusCode,
    code: &str,
    message: String,
) -> ApiError {
    ApiError::new(status, code, message, false).with_correlation(correlation.to_owned())
}

/// Opens a chunked upload: declares the file, the destination Property and the role before any bytes are sent.
pub(super) async fn begin_media_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(property_id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<crate::media::BeginMediaUploadResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();

    let mut filename: Option<String> = None;
    let mut mime_type = String::new();
    let mut alt_text: Option<String> = None;
    let mut sha256: Option<String> = None;
    let mut role = String::from("gallery");
    let mut byte_size: i64 = 0;
    let mut chunk_count: i32 = 0;
    let mut chunk_size: i32 = 0;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        media_upload_error(
            &correlation,
            StatusCode::BAD_REQUEST,
            "MEDIA_MULTIPART_INVALID",
            format!("Invalid media upload: {error}"),
        )
    })? {
        let name = field.name().unwrap_or_default().to_owned();
        let value = field.text().await.map_err(|error| {
            media_upload_error(
                &correlation,
                StatusCode::BAD_REQUEST,
                "MEDIA_MULTIPART_INVALID",
                format!("Invalid media upload: {error}"),
            )
        })?;
        match name.as_str() {
            "filename" => filename = Some(value),
            "mimeType" => mime_type = value,
            "altText" => alt_text = Some(value),
            "sha256" => sha256 = Some(value),
            "role" => role = value,
            "byteSize" => byte_size = value.parse().unwrap_or(0),
            "chunkCount" => chunk_count = value.parse().unwrap_or(0),
            "chunkSize" => chunk_size = value.parse().unwrap_or(0),
            _ => {}
        }
    }

    let filename = filename
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            media_upload_error(
                &correlation,
                StatusCode::BAD_REQUEST,
                "MEDIA_FILENAME_REQUIRED",
                "An image filename is required.".to_owned(),
            )
        })?;

    // The declared shape is checked here as well as in the database, so an obviously wrong declaration is refused
    // with a sentence rather than a constraint violation.
    if byte_size <= 0 || chunk_count <= 0 || chunk_size <= 0 {
        return Err(media_upload_error(
            &correlation,
            StatusCode::BAD_REQUEST,
            "MEDIA_UPLOAD_SHAPE_INVALID",
            "The upload must declare a positive size, chunk count and chunk size.".to_owned(),
        ));
    }
    if byte_size > MAX_MEDIA_UPLOAD_BYTES as i64 {
        return Err(media_upload_error(
            &correlation,
            StatusCode::BAD_REQUEST,
            "MEDIA_UPLOAD_TOO_LARGE",
            "Image is too large (max 50 MB).".to_owned(),
        ));
    }

    let service = state.services().media();
    let value = service
        .begin_media_upload(
            db::BeginMediaUpload {
                upload_id: uuid::Uuid::new_v4().to_string(),
                property_id,
                filename,
                mime_type,
                byte_size,
                chunk_count,
                chunk_size,
                sha256,
                role,
                alt_text,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    Ok(success(value, &resolved))
}

/// Accepts one piece of the file. The bytes are opaque here: nothing inspects a partial upload.
pub(super) async fn stage_media_chunk(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((_property_id, upload_id, chunk_index)): Path<(String, String, i32)>,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<crate::media::MediaUploadProgress>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();

    let mut bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.map_err(|error| {
        media_upload_error(
            &correlation,
            StatusCode::BAD_REQUEST,
            "MEDIA_MULTIPART_INVALID",
            format!("Invalid media upload: {error}"),
        )
    })? {
        if field.name().unwrap_or_default() == "chunk" {
            bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|error| {
                        media_upload_error(
                            &correlation,
                            StatusCode::BAD_REQUEST,
                            "MEDIA_CHUNK_INVALID",
                            format!("Invalid media chunk: {error}"),
                        )
                    })?
                    .to_vec(),
            );
        }
    }

    let bytes = bytes.ok_or_else(|| {
        media_upload_error(
            &correlation,
            StatusCode::BAD_REQUEST,
            "MEDIA_CHUNK_REQUIRED",
            "An image chunk is required.".to_owned(),
        )
    })?;

    let service = state.services().media();
    let value = service
        .stage_media_chunk(&upload_id, chunk_index, bytes, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    Ok(success(value, &resolved))
}

pub(super) async fn upload_property_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(property_id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<model::UploadPropertyMediaResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let mut role: Option<String> = None;
    let mut alt_text: Option<String> = None;
    let mut file: Option<(String, String, Vec<u8>)> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "MEDIA_MULTIPART_INVALID",
            format!("Invalid media upload: {error}"),
            false,
        )
        .with_correlation(resolved.service.correlation_id.clone())
    })? {
        let name = field.name().unwrap_or_default().to_owned();
        match name.as_str() {
            "role" => {
                role = Some(field.text().await.map_err(|error| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "MEDIA_ROLE_INVALID",
                        format!("Invalid media role: {error}"),
                        false,
                    )
                    .with_correlation(resolved.service.correlation_id.clone())
                })?);
            }
            "altText" => {
                alt_text = Some(field.text().await.map_err(|error| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "MEDIA_ALT_TEXT_INVALID",
                        format!("Invalid media alt text: {error}"),
                        false,
                    )
                    .with_correlation(resolved.service.correlation_id.clone())
                })?);
            }
            "file" => {
                let filename = field.file_name().unwrap_or("upload").to_owned();
                let mime_type = field.content_type().unwrap_or_default().to_owned();
                let bytes = field.bytes().await.map_err(|error| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "MEDIA_FILE_INVALID",
                        format!("Invalid media file: {error}"),
                        false,
                    )
                    .with_correlation(resolved.service.correlation_id.clone())
                })?;
                file = Some((filename, mime_type, bytes.to_vec()));
            }
            _ => {}
        }
    }

    let (filename, mime_type, bytes) = file.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "MEDIA_FILE_REQUIRED",
            "Image file is required.",
            false,
        )
        .with_correlation(resolved.service.correlation_id.clone())
    })?;

    let service = state.services().media();
    let value = service
        .upload_property_media(
            UploadPropertyMediaRequest {
                property_id,
                role: role.unwrap_or_default(),
                filename,
                mime_type,
                alt_text,
                bytes,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    Ok(success(value, &resolved))
}
