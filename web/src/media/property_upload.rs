//! The whole-file property photograph upload: ONE multipart request, no staging.
//!
//! WHAT THIS REPLACES. The browser used to send a photograph as a manifest, N chunks and a completion call,
//! staging every piece in `media_upload_chunk` so no single request had to survive a dropped connection. The
//! staged chunk protocol is now RETIRED (`media_upload` / `media_upload_chunk` are dropped by
//! `db/migrations/300`): photographs arrive either here (one request, whole file) or direct to object storage
//! (`direct_upload.rs`, for files over the hosting edge's body cap). This path takes the file in one request,
//! derives the servable copies, writes the original and the copies to object storage FIRST (each one verified),
//! then records the metadata rows and the property link in ONE database transaction.
//!
//! FAILURE CLEANUP. Object storage cannot join a database transaction, so the order matters: upload and
//! verify every object, then write the rows. If any upload fails, the objects already written are deleted
//! before the error leaves. If the transaction fails, every object in the set is deleted. Either way a retry
//! starts from no objects rather than accumulating orphans in the bucket.
//!
//! ROUTE. Registered in `web/src/api/portal_bridge.rs`:
//!
//! ```ignore
//! .route(
//!     "/api/property-media/full",
//!     axum::routing::post(crate::media::property_media_full_upload).layer(
//!         axum::extract::DefaultBodyLimit::max(model::MAX_MEDIA_UPLOAD_BYTES + 1024 * 1024),
//!     ),
//! )
//! ```
//!
//! The `DefaultBodyLimit` is REQUIRED: axum's default is 2 MiB, which is smaller than the photographs this
//! path exists for. `property_media_full_upload` also has an `#[cfg(test)]` compile test that builds exactly
//! this route against `ApiState`.

#[allow(unused_imports)]
use super::*;
use crate::api::ui_auth::resolve_portal_context;
use crate::api::{ApiError, ApiState};
use axum::{
    extract::{Multipart, State},
    http::HeaderMap,
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use services::{ServiceFailureClass, ServiceRuntimeError};

/// The image types this path can actually turn into copies. The `image` crate is built with JPEG, PNG and
/// WebP only, so a declared GIF/AVIF/HEIC would be accepted here and then refused by the decoder — better to
/// refuse the declaration with the sentence that says what is supported.
///
/// The list is also a safety rule, not just a capability list: an `image/svg+xml` row would be served back
/// to browsers under its own content type, so "any image/*" is not a safe admission rule for public media.
pub(super) const SUPPORTED_IMAGE_MIME_TYPES: [&str; 4] =
    ["image/jpeg", "image/jpg", "image/png", "image/webp"];

/// One whole-file property photograph, exactly as the service needs it.
pub struct FullPropertyMediaUploadRequest {
    pub property_id: String,
    pub role: String,
    pub filename: String,
    pub mime_type: String,
    pub alt_text: Option<String>,
    pub bytes: Vec<u8>,
}

/// What the whole-file upload stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullPropertyMediaUploadResult {
    pub media_id: String,
    pub property_id: String,
    pub role: String,
    /// Always `object`: this path never writes a `file_data` blob.
    pub storage: &'static str,
    /// True when the property already showed this exact file (a retried request), so nothing was written.
    pub skipped: bool,
    /// The copy kinds stored beside the original. Empty for a skipped upload.
    pub derivative_kinds: Vec<&'static str>,
}

/// `ZoniBluff_1.jpg` + `web` -> `ZoniBluff_1-web.jpg`, the naming every object-first writer uses.
pub(super) fn derivative_filename(original: &str, kind: &str) -> String {
    match original.rsplit_once('.') {
        Some((stem, _extension)) if !stem.is_empty() => format!("{stem}-{kind}.jpg"),
        _ => format!("{original}-{kind}.jpg"),
    }
}

pub(super) fn object_store_not_configured() -> CoreServiceError {
    CoreServiceError::Runtime(ServiceRuntimeError::Router {
        code: "MEDIA_OBJECT_STORE_NOT_CONFIGURED".into(),
        message:
            "the whole-file upload path requires media object storage, which is not configured"
                .into(),
        retryable: false,
        class: ServiceFailureClass::Infrastructure,
    })
}

/// Delete objects this request wrote but the database refused to record. Best effort: a delete that fails is
/// logged and the whole set still fails, because a stray object is recoverable and a media row with no
/// object is not.
pub(super) async fn discard_objects(
    store: &Arc<dyn AbstractObjectStoreService>,
    objects: &[(ObjectBucket, String)],
) {
    for (bucket, key) in objects {
        if let Err(error) = store.delete_object(*bucket, key).await {
            tracing::warn!(
                key,
                %error,
                "whole-file upload: could not delete an object after a failed upload"
            );
        }
    }
}

impl<R: MediaRepository> MediaService<R> {
    /// Store one whole property photograph: derive the copies, upload original and copies object-first, then
    /// insert every metadata row and attach the property in one transaction. Nothing is staged.
    pub async fn upload_property_media_full(
        &self,
        request: FullPropertyMediaUploadRequest,
        context: &ServiceContext,
    ) -> Result<FullPropertyMediaUploadResult, CoreServiceError> {
        const OP: &str = "media.uploadPropertyMediaFull";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self.upload_property_media_full_inner(request).await;
        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    async fn upload_property_media_full_inner(
        &self,
        mut request: FullPropertyMediaUploadRequest,
    ) -> Result<FullPropertyMediaUploadResult, CoreServiceError> {
        // Normalise before validating: the dedupe guard compares against the stored (sanitized) filename.
        request.property_id = request.property_id.trim().to_owned();
        request.role = request.role.trim().to_owned();
        request.filename = sanitize_media_filename(&request.filename);
        request.mime_type = request.mime_type.trim().to_ascii_lowercase();
        request.alt_text = request
            .alt_text
            .take()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        if request.property_id.is_empty() {
            return Err(CoreServiceError::business(
                "PROPERTY_REQUIRED",
                "Property is required.",
            ));
        }
        if !matches!(request.role.as_str(), "hero" | "gallery") {
            return Err(CoreServiceError::business(
                "MEDIA_ROLE_INVALID",
                "Invalid media role.",
            ));
        }
        if request.bytes.is_empty() {
            return Err(CoreServiceError::business(
                "MEDIA_EMPTY",
                "Image file is required.",
            ));
        }
        if request.bytes.len() > MAX_MEDIA_UPLOAD_BYTES {
            return Err(CoreServiceError::business(
                "MEDIA_TOO_LARGE",
                "Image is too large (max 50 MB).",
            ));
        }
        if !SUPPORTED_IMAGE_MIME_TYPES.contains(&request.mime_type.as_str()) {
            return Err(CoreServiceError::business(
                "MEDIA_TYPE_INVALID",
                "Only JPEG, PNG and WebP photographs are supported.",
            ));
        }

        // A request resent after a lost response must answer with the row that already exists rather than
        // attach the same file twice. Checked before any byte leaves the process.
        let byte_size = request.bytes.len() as i64;
        if let Some(media_id) = self
            .repository
            .property_photo_id(&request.property_id, &request.filename, byte_size)
            .await?
        {
            return Ok(FullPropertyMediaUploadResult {
                media_id,
                property_id: request.property_id,
                role: request.role,
                storage: "object",
                skipped: true,
                derivative_kinds: Vec::new(),
            });
        }

        let store = self
            .object_store
            .as_ref()
            .ok_or_else(object_store_not_configured)?;

        // Re-encoding a 13 MB photograph is hundreds of milliseconds of straight CPU; keep it off the async
        // runtime and on the blocking pool, as the direct-to-object finalize does.
        let original_bytes = std::mem::take(&mut request.bytes);
        let (original_bytes, derived) = tokio::task::spawn_blocking(move || {
            let derived = imaging::derive_web_and_thumb(&original_bytes);
            (original_bytes, derived)
        })
        .await
        .map_err(|error| {
            CoreServiceError::business(
                "MEDIA_DERIVATIVE_FAILED",
                format!("the photograph could not be processed ({error})"),
            )
        })?;
        let derived = derived
            .map_err(|message| CoreServiceError::business("MEDIA_DERIVATIVE_FAILED", message))?;

        // Property photography is public. There is no per-property private photo state: every copy of a
        // photograph uses the public media bucket, whatever the listing's publication state. PDFs and Vault
        // documents remain private through their own paths.
        let (bucket, visibility) = (ObjectBucket::PropertyMediaPublic, "public");

        let mut rows: Vec<db::MediaObjectRow> = Vec::with_capacity(1 + derived.len());
        let mut uploaded: Vec<(ObjectBucket, String)> = Vec::with_capacity(1 + derived.len());
        let original_id = uuid::Uuid::new_v4().to_string();

        let original = match self
            .object_row(
                store,
                bucket,
                visibility,
                &original_id,
                &request.filename,
                &request.mime_type,
                &original_bytes,
                request.alt_text.clone(),
                "image",
                None,
                None,
            )
            .await
        {
            Ok(row) => row,
            Err(error) => {
                discard_objects(store, &uploaded).await;
                return Err(CoreServiceError::infrastructure(
                    "MEDIA_OBJECT_UPLOAD_FAILED",
                    error,
                ));
            }
        };
        uploaded.push((bucket, original.object.key.clone()));
        rows.push(original);

        let mut derivative_kinds = Vec::with_capacity(derived.len());
        for derivative in &derived {
            let id = uuid::Uuid::new_v4().to_string();
            let filename = derivative_filename(&request.filename, derivative.kind);
            match self
                .object_row(
                    store,
                    bucket,
                    visibility,
                    &id,
                    &filename,
                    "image/jpeg",
                    &derivative.bytes,
                    None,
                    "image",
                    Some(original_id.clone()),
                    Some(derivative.kind.to_owned()),
                )
                .await
            {
                Ok(row) => {
                    uploaded.push((bucket, row.object.key.clone()));
                    rows.push(row);
                    derivative_kinds.push(derivative.kind);
                }
                Err(error) => {
                    discard_objects(store, &uploaded).await;
                    return Err(CoreServiceError::infrastructure(
                        "MEDIA_OBJECT_UPLOAD_FAILED",
                        error,
                    ));
                }
            }
        }

        match self
            .repository
            .save_property_media_object_set(&request.property_id, &request.role, &rows)
            .await
        {
            Ok(stored) => Ok(FullPropertyMediaUploadResult {
                media_id: stored.media_id,
                property_id: stored.property_id,
                role: stored.role,
                storage: "object",
                skipped: false,
                derivative_kinds,
            }),
            Err(error) => {
                // The rows rolled back; the object set has no row pointing at it, so it goes too.
                discard_objects(store, &uploaded).await;
                Err(error.into())
            }
        }
    }
}

/// `POST /api/property-media/full` (multipart/form-data).
///
/// Fields: `propertyId` (required), `role` (`hero` or `gallery`, default `gallery`), `altText` (optional),
/// and the photograph itself in `file`.
///
/// Answers the standard envelope: `{ ok, value: { mediaId, propertyId, role, storage, skipped,
/// derivativeKinds }, correlationId }`. A skipped answer means the property already shows that exact file, so
/// a retried request is safe.
pub async fn property_media_full_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();
    let bad = |code: &'static str, message: String| {
        ApiError::bad_request(code, message).with_correlation(correlation.clone())
    };

    let mut property_id: Option<String> = None;
    let mut role = String::from("gallery");
    let mut alt_text: Option<String> = None;
    let mut file: Option<(String, String, Vec<u8>)> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        bad(
            "MEDIA_MULTIPART_INVALID",
            format!("Invalid media upload: {error}"),
        )
    })? {
        let name = field.name().unwrap_or_default().to_owned();
        match name.as_str() {
            "propertyId" => {
                property_id = Some(field.text().await.map_err(|error| {
                    bad(
                        "MEDIA_MULTIPART_INVALID",
                        format!("Invalid property: {error}"),
                    )
                })?);
            }
            "role" => {
                role = field.text().await.map_err(|error| {
                    bad("MEDIA_ROLE_INVALID", format!("Invalid media role: {error}"))
                })?;
            }
            "altText" => {
                alt_text = Some(field.text().await.map_err(|error| {
                    bad(
                        "MEDIA_ALT_TEXT_INVALID",
                        format!("Invalid media alt text: {error}"),
                    )
                })?);
            }
            "file" => {
                let filename = field.file_name().unwrap_or("upload").to_owned();
                let mime_type = field.content_type().unwrap_or_default().to_owned();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|error| {
                        bad("MEDIA_FILE_INVALID", format!("Invalid media file: {error}"))
                    })?
                    .to_vec();
                file = Some((filename, mime_type, bytes));
            }
            _ => {}
        }
    }

    let property_id = property_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| bad("PROPERTY_REQUIRED", "Property is required.".to_owned()))?;
    let (filename, mime_type, bytes) =
        file.ok_or_else(|| bad("MEDIA_FILE_REQUIRED", "Image file is required.".to_owned()))?;

    let value = state
        .services()
        .media()
        .upload_property_media_full(
            FullPropertyMediaUploadRequest {
                property_id,
                role,
                filename,
                mime_type,
                alt_text,
                bytes,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| {
            ApiError::from(error).with_correlation(resolved.service.correlation_id.clone())
        })?;

    Ok(Json(json!({
        "ok": true,
        "value": value,
        "correlationId": resolved.service.correlation_id,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::test_support::*;
    use db::MediaObjectLocation;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    fn request(bytes: Vec<u8>) -> FullPropertyMediaUploadRequest {
        FullPropertyMediaUploadRequest {
            property_id: PROPERTY.into(),
            role: "gallery".into(),
            filename: "ZoniBluff_1.jpg".into(),
            mime_type: "image/jpeg".into(),
            alt_text: Some("Zoni bluff at dusk".into()),
            bytes,
        }
    }

    fn location(status: &str, visibility: &str) -> MediaObjectLocation {
        MediaObjectLocation {
            bucket: if visibility == "public" {
                "property_media_public".into()
            } else {
                "property_media_private".into()
            },
            key: "media/abc/original.jpg".into(),
            status: status.into(),
            visibility: visibility.into(),
        }
    }

    /// The registration in `web/src/api/portal_bridge.rs` must typecheck: this builds the exact route — handler
    /// plus the body limit that makes a whole photograph fit — against the real router state. It does not serve anything.
    #[test]
    fn the_handler_registers_on_the_api_router() {
        let _router: axum::Router<ApiState> = axum::Router::new().route(
            "/api/property-media/full",
            axum::routing::post(property_media_full_upload).layer(
                axum::extract::DefaultBodyLimit::max(MAX_MEDIA_UPLOAD_BYTES + 1024 * 1024),
            ),
        );
    }

    #[tokio::test]
    async fn a_whole_photograph_is_stored_object_first_without_staging() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let service = service(repository, Some(store.clone())).await;

        let stored = service
            .upload_property_media_full(request(jpeg(800, 600)), &root_context())
            .await
            .expect("the photograph is stored");

        assert!(!stored.skipped);
        assert_eq!(stored.storage, "object");
        assert_eq!(stored.role, "gallery");
        assert_eq!(stored.derivative_kinds, vec!["web", "card", "thumb"]);

        // The original and its three copies: four verified objects, four metadata rows, no deletes.
        assert_eq!(store.put_count(), 4);
        assert_eq!(store.delete_count(), 0);

        let saved = service.repository.saved_sets();
        assert_eq!(saved.len(), 1, "one transaction, one row set");
        let (property_id, role, rows) = &saved[0];
        assert_eq!(property_id, PROPERTY);
        assert_eq!(role, "gallery");
        assert_eq!(rows.len(), 4);

        let original = &rows[0];
        assert_eq!(original.id, stored.media_id);
        assert!(original.derivative_of.is_none());
        assert_eq!(original.object.bucket, "property_media_public");
        assert_eq!(original.object.visibility.as_deref(), Some("public"));
        assert_eq!(original.object.status, "VERIFIED");
        assert!(original.object.size > 0, "the original's size is recorded");
        let mut kinds = Vec::new();
        for row in &rows[1..] {
            assert_eq!(row.derivative_of.as_deref(), Some(original.id.as_str()));
            assert_eq!(row.media_type, "image");
            assert_eq!(row.mime_type, "image/jpeg");
            assert_eq!(row.object.status, "VERIFIED");
            kinds.push(row.derivative_kind.clone().unwrap_or_default());
        }
        assert_eq!(kinds, vec!["web", "card", "thumb"]);
    }

    #[tokio::test]
    async fn a_failed_metadata_write_deletes_every_object_and_leaves_no_rows() {
        let repository = FakeRepository {
            fail_save: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let service = service(repository, Some(store.clone())).await;

        let error = service
            .upload_property_media_full(request(jpeg(400, 300)), &root_context())
            .await
            .expect_err("the transaction failed");

        assert_eq!(store.put_count(), 4);
        assert_eq!(
            store.delete_count(),
            4,
            "the object set must be deleted when no row points at it"
        );
        assert!(service.repository.saved_sets().is_empty());
        assert!(
            error.to_string().contains("transaction failed"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn a_verify_failure_mid_set_deletes_the_objects_already_written() {
        let repository = FakeRepository::default();
        let store = Arc::new(FakeStore::default());
        *store.fail_verify_at.lock().expect("verify poisoned") = Some(3);
        let service = service(repository, Some(store.clone())).await;

        let error = service
            .upload_property_media_full(request(jpeg(400, 300)), &root_context())
            .await
            .expect_err("the third object could not be verified");

        assert_eq!(error.code(), "MEDIA_OBJECT_UPLOAD_FAILED");
        assert_eq!(store.put_count(), 3, "three objects were written");
        assert_eq!(
            store.delete_count(),
            3,
            "the unverified object and every earlier one are deleted"
        );
        assert!(service.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn a_file_that_is_not_an_image_uploads_nothing() {
        let repository = FakeRepository::default();
        let store = Arc::new(FakeStore::default());
        let service = service(repository, Some(store.clone())).await;

        let error = service
            .upload_property_media_full(
                request(b"this is not a photograph".to_vec()),
                &root_context(),
            )
            .await
            .expect_err("the bytes are not an image");

        assert_eq!(error.code(), "MEDIA_DERIVATIVE_FAILED");
        assert_eq!(store.put_count(), 0);
        assert!(service.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn declarations_are_refused_before_any_byte_leaves_the_process() {
        let store = Arc::new(FakeStore::default());

        let mut bad_type = request(jpeg(100, 100));
        bad_type.mime_type = "text/html".into();
        let media = service(FakeRepository::default(), Some(store.clone())).await;
        let error = media
            .upload_property_media_full(bad_type, &root_context())
            .await
            .expect_err("an image row that is not an image");
        assert_eq!(error.code(), "MEDIA_TYPE_INVALID");

        let mut empty = request(Vec::new());
        empty.mime_type = "image/jpeg".into();
        let media = service(FakeRepository::default(), Some(store.clone())).await;
        let error = media
            .upload_property_media_full(empty, &root_context())
            .await
            .expect_err("an empty file");
        assert_eq!(error.code(), "MEDIA_EMPTY");

        let mut bad_role = request(jpeg(100, 100));
        bad_role.role = "video".into();
        let media = service(FakeRepository::default(), Some(store.clone())).await;
        let error = media
            .upload_property_media_full(bad_role, &root_context())
            .await
            .expect_err("a video role");
        assert_eq!(error.code(), "MEDIA_ROLE_INVALID");

        assert_eq!(store.put_count(), 0, "no object was written");
    }

    #[tokio::test]
    async fn a_retry_of_a_photograph_the_property_already_shows_is_skipped() {
        let repository = FakeRepository {
            existing_photo_id: Some("11111111-1111-4111-8111-111111111111".into()),
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let service = service(repository, Some(store.clone())).await;

        let stored = service
            .upload_property_media_full(request(jpeg(400, 300)), &root_context())
            .await
            .expect("the retry is answered");

        assert!(stored.skipped);
        assert_eq!(stored.media_id, "11111111-1111-4111-8111-111111111111");
        assert_eq!(stored.derivative_kinds, Vec::<&str>::new());
        assert_eq!(store.put_count(), 0);
        assert!(service.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn an_upload_without_an_object_store_is_a_typed_failure() {
        let service = service(FakeRepository::default(), None).await;

        let error = service
            .upload_property_media_full(request(jpeg(100, 100)), &root_context())
            .await
            .expect_err("no object store");

        assert_eq!(error.code(), "MEDIA_OBJECT_STORE_NOT_CONFIGURED");
    }

    #[tokio::test]
    async fn a_row_with_no_object_is_a_typed_failure() {
        let repository = FakeRepository {
            location: None,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let service = service(repository, Some(store)).await;

        let error = service
            .media_read("missing-media", &root_context())
            .await
            .expect_err("a row with no object must be a typed failure");

        assert_eq!(error.code(), "MEDIA_OBJECT_NOT_FOUND");
    }

    #[tokio::test]
    async fn an_unverified_object_is_refused_instead_of_served() {
        let repository = FakeRepository {
            location: Some(location("MIGRATION_PENDING", "private")),
            ..Default::default()
        };
        let service = service(repository, Some(Arc::new(FakeStore::default()))).await;

        let error = service
            .media_read("half-migrated", &root_context())
            .await
            .expect_err("an unverified object is not servable");

        assert_eq!(error.code(), "MEDIA_OBJECT_NOT_VERIFIED");
    }

    #[tokio::test]
    async fn a_verified_public_object_redirects_to_its_public_url() {
        let repository = FakeRepository {
            location: Some(location("VERIFIED", "public")),
            ..Default::default()
        };
        let service = service(repository, Some(Arc::new(FakeStore::default()))).await;

        let found = service
            .media_read("public-photo", &root_context())
            .await
            .expect("the object is servable")
            .expect("the row exists");

        match found {
            MediaRead::Redirect { url } => {
                assert_eq!(url, "https://cdn.example/media/abc/original.jpg");
            }
            MediaRead::Bytes { .. } => panic!("a public object must redirect, not stream the blob"),
        }
    }

    #[tokio::test]
    async fn a_url_failure_is_typed_not_a_blob_fallback() {
        let repository = FakeRepository {
            location: Some(location("VERIFIED", "private")),
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        store.fail_urls.store(true, Ordering::SeqCst);
        let service = service(repository, Some(store)).await;

        let error = service
            .media_read("private-photo", &root_context())
            .await
            .expect_err("the URL could not be generated");

        assert_eq!(error.code(), "MEDIA_OBJECT_READ_URL_FAILED");
    }
}
