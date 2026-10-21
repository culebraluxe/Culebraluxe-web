//! Media: private and standalone media, property media and films. (The chunked upload protocol is retired.)

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
        .media_read(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;

    let Some(found) = found else {
        return Err(correlate(
            ApiError::not_found("MEDIA_NOT_FOUND", "Not found."),
            &resolved,
        ));
    };

    match found {
        crate::media::MediaRead::Bytes { mime_type, bytes } => Response::builder()
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
            }),
        crate::media::MediaRead::Redirect { url } => Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, url)
            // A signed URL is short-lived; do not let a browser cache the redirect as the bytes.
            .header(header::CACHE_CONTROL, "private, no-store")
            .body(Body::empty())
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
            }),
    }
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
