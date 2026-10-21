//! The whole-file property photograph upload: ONE multipart request, no chunk staging.
//!
//! WHAT THIS REPLACES. The browser used to send a photograph as a manifest, N chunks and a completion call,
//! staging every piece in `media_upload_chunk` so no single request had to survive a dropped connection. This
//! path takes the file in one request, derives the servable copies, writes the original and the copies to
//! object storage FIRST (each one verified), then records the metadata rows and the property link in ONE
//! database transaction. Nothing is staged, so nothing in `media_upload` / `media_upload_chunk` is written —
//! which is what lets those tables be dropped once the chunk routes are retired.
//!
//! FAILURE CLEANUP. Object storage cannot join a database transaction, so the order matters: upload and
//! verify every object, then write the rows. If any upload fails, the objects already written are deleted
//! before the error leaves. If the transaction fails, every object in the set is deleted. Either way a retry
//! starts from no objects rather than accumulating orphans in the bucket.
//!
//! ROUTE. This module is NOT wired into the router: `web/src/api/routes.rs` is outside this lane's write scope.
//! Register the handler in `web/src/api/portal_bridge.rs`'s `router()`, beside the existing
//! `/api/property-media/chunked` route (the browser path the Yew catalogue names `PropertyMediaFullUpload`):
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
//! this route against `ApiState`, so a mistyped registration is caught before it is wired.

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
const SUPPORTED_IMAGE_MIME_TYPES: [&str; 4] =
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

/// `ZoniBluff_1.jpg` + `web` -> `ZoniBluff_1-web.jpg`, the same naming the chunked writer uses.
fn derivative_filename(original: &str, kind: &str) -> String {
    match original.rsplit_once('.') {
        Some((stem, _extension)) if !stem.is_empty() => format!("{stem}-{kind}.jpg"),
        _ => format!("{original}-{kind}.jpg"),
    }
}

fn object_store_not_configured() -> CoreServiceError {
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
async fn discard_objects(
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
    /// insert every metadata row and attach the property in one transaction. No chunk rows are touched.
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
        // runtime, exactly as the chunked finisher does.
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

        // The publication rule decides which bucket the whole set goes to; every row of one photograph shares
        // the original's visibility, so a copy can never leak more than the photograph it copies.
        let public = self
            .repository
            .property_is_public(&request.property_id)
            .await
            .unwrap_or(false);
        let (bucket, visibility) = if public {
            (ObjectBucket::PropertyMediaPublic, "public")
        } else {
            (ObjectBucket::PropertyMediaPrivate, "private")
        };

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
    use crate::security::CasbinAuthorizationPort;
    use async_trait::async_trait;
    use db::{
        BeginMediaUpload, DbFailure, DbResult, MediaDerivativeInput, MediaObjectLocation,
        MediaObjectRow, MediaUploadAssembly, MediaUploadStatus,
    };
    use model::{
        AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset,
        UploadPropertyMediaRequest, UploadPropertyMediaResult,
    };
    use services::{
        CapturingAuditPort, CapturingDomainEventPort, ObjectMetadata, ObjectStoreError,
        ServiceActor, ServiceActorKind, ServiceInfrastructure, ServicePrincipal,
    };
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    const PROPERTY: &str = "0b0e0f56-89a1-4c4f-9b2b-6a1f0a1e0001";

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// The one signed-in user these tests run as: ROOT, so the real Casbin policy admits `property.write`.
    fn root_context() -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some("root-user".into()),
                kind: ServiceActorKind::User,
            },
            correlation_id: "media-whole-file-test".into(),
            causation_id: None,
            principal: Some(ServicePrincipal {
                app_user_id: "root-user".into(),
                level: "ADMIN".into(),
                role_codes: vec!["root".into()],
                account_type: "internal".into(),
                entitlement_codes: Vec::new(),
            }),
        }
    }

    #[derive(Default)]
    struct FakeStore {
        objects: Mutex<BTreeMap<(String, String), (String, i64)>>,
        puts: Mutex<Vec<String>>,
        deletes: Mutex<Vec<String>>,
        verifies: AtomicUsize,
        /// 1-based verify call that should fail (an integrity failure mid-set).
        fail_verify_at: Mutex<Option<usize>>,
        fail_urls: AtomicBool,
    }

    impl FakeStore {
        fn put_count(&self) -> usize {
            self.puts.lock().expect("puts poisoned").len()
        }

        fn delete_count(&self) -> usize {
            self.deletes.lock().expect("deletes poisoned").len()
        }
    }

    #[async_trait]
    impl AbstractObjectStoreService for FakeStore {
        async fn put_object(
            &self,
            bucket: ObjectBucket,
            key: &str,
            bytes: &[u8],
            _content_type: &str,
        ) -> Result<ObjectMetadata, ObjectStoreError> {
            let sha = sha256_hex(bytes);
            self.objects.lock().expect("objects poisoned").insert(
                (bucket.as_str().to_owned(), key.to_owned()),
                (sha.clone(), bytes.len() as i64),
            );
            self.puts
                .lock()
                .expect("puts poisoned")
                .push(key.to_owned());
            Ok(ObjectMetadata {
                bucket,
                key: key.to_owned(),
                size: bytes.len() as i64,
                content_type: None,
                etag: Some("etag".into()),
                sha256: Some(sha),
            })
        }

        async fn head_object(
            &self,
            bucket: ObjectBucket,
            key: &str,
        ) -> Result<ObjectMetadata, ObjectStoreError> {
            let objects = self.objects.lock().expect("objects poisoned");
            let (sha, size) = objects
                .get(&(bucket.as_str().to_owned(), key.to_owned()))
                .ok_or_else(|| ObjectStoreError::not_found(key))?;
            Ok(ObjectMetadata {
                bucket,
                key: key.to_owned(),
                size: *size,
                content_type: None,
                etag: Some("etag".into()),
                sha256: Some(sha.clone()),
            })
        }

        async fn delete_object(
            &self,
            bucket: ObjectBucket,
            key: &str,
        ) -> Result<(), ObjectStoreError> {
            self.objects
                .lock()
                .expect("objects poisoned")
                .remove(&(bucket.as_str().to_owned(), key.to_owned()));
            self.deletes
                .lock()
                .expect("deletes poisoned")
                .push(key.to_owned());
            Ok(())
        }

        async fn public_read_url(
            &self,
            _bucket: ObjectBucket,
            key: &str,
        ) -> Result<String, ObjectStoreError> {
            if self.fail_urls.load(Ordering::SeqCst) {
                return Err(ObjectStoreError::infrastructure("no URL", true));
            }
            Ok(format!("https://cdn.example/{key}"))
        }

        async fn signed_read_url(
            &self,
            _bucket: ObjectBucket,
            key: &str,
            _ttl: std::time::Duration,
        ) -> Result<String, ObjectStoreError> {
            if self.fail_urls.load(Ordering::SeqCst) {
                return Err(ObjectStoreError::infrastructure("no URL", true));
            }
            Ok(format!("https://signed.example/{key}"))
        }

        async fn signed_write_url(
            &self,
            _bucket: ObjectBucket,
            key: &str,
            _content_type: &str,
            _ttl: std::time::Duration,
        ) -> Result<String, ObjectStoreError> {
            Ok(format!("https://write.example/{key}"))
        }

        async fn verify_object(
            &self,
            bucket: ObjectBucket,
            key: &str,
            expected_sha256: &str,
            expected_size: i64,
        ) -> Result<ObjectMetadata, ObjectStoreError> {
            let call = self.verifies.fetch_add(1, Ordering::SeqCst) + 1;
            if *self.fail_verify_at.lock().expect("verify poisoned") == Some(call) {
                return Err(ObjectStoreError::integrity(format!("verify {call} failed")));
            }
            let objects = self.objects.lock().expect("objects poisoned");
            let (sha, size) = objects
                .get(&(bucket.as_str().to_owned(), key.to_owned()))
                .ok_or_else(|| ObjectStoreError::not_found(key))?;
            if sha != expected_sha256 || *size != expected_size {
                return Err(ObjectStoreError::integrity("stored bytes do not match"));
            }
            Ok(ObjectMetadata {
                bucket,
                key: key.to_owned(),
                size: *size,
                content_type: None,
                etag: Some("etag".into()),
                sha256: Some(sha.clone()),
            })
        }
    }

    /// A repository that records what the whole-file path did and counts every touch of the chunk protocol.
    #[derive(Default)]
    struct FakeRepository {
        public: bool,
        fail_save: bool,
        existing_photo_id: Option<String>,
        location: Option<MediaObjectLocation>,
        blob: Option<(String, Vec<u8>)>,
        saved_sets: Mutex<Vec<(String, String, Vec<MediaObjectRow>)>>,
        chunk_calls: AtomicUsize,
        blob_reads: AtomicUsize,
        object_lookups: AtomicUsize,
    }

    impl FakeRepository {
        fn saved_sets(&self) -> Vec<(String, String, Vec<MediaObjectRow>)> {
            self.saved_sets.lock().expect("saved poisoned").clone()
        }
    }

    #[async_trait]
    impl MediaRepository for FakeRepository {
        async fn media_bytes(&self, _id: &str) -> DbResult<Option<(String, Vec<u8>)>> {
            self.blob_reads.fetch_add(1, Ordering::SeqCst);
            Ok(self.blob.clone())
        }

        async fn upload_standalone(
            &self,
            _filename: &str,
            _mime_type: &str,
            _bytes: &[u8],
        ) -> DbResult<(String, String, String, i64)> {
            Err(unused())
        }

        async fn for_property(&self, _property_id: &str) -> DbResult<Vec<MediaAsset>> {
            Err(unused())
        }

        async fn set_property_hero(&self, _property_id: &str, _media_id: &str) -> DbResult<bool> {
            Err(unused())
        }

        async fn upload_property_media(
            &self,
            _request: &UploadPropertyMediaRequest,
        ) -> DbResult<UploadPropertyMediaResult> {
            Err(unused())
        }

        async fn attach_property_video(
            &self,
            _request: &AttachPropertyVideoRequest,
        ) -> DbResult<AttachPropertyVideoResult> {
            Err(unused())
        }

        async fn begin_media_upload(
            &self,
            _request: &BeginMediaUpload,
        ) -> DbResult<BeginMediaUploadResult> {
            self.chunk_calls.fetch_add(1, Ordering::SeqCst);
            Err(unused())
        }

        async fn stage_media_chunk(
            &self,
            _upload_id: &str,
            _chunk_index: i32,
            _bytes: &[u8],
        ) -> DbResult<MediaUploadStatus> {
            self.chunk_calls.fetch_add(1, Ordering::SeqCst);
            Err(unused())
        }

        async fn remove_property_media(
            &self,
            _property_id: &str,
            _media_id: &str,
        ) -> DbResult<bool> {
            Err(unused())
        }

        async fn property_has_photo(
            &self,
            _property_id: &str,
            _filename: &str,
            _byte_size: i64,
        ) -> DbResult<bool> {
            Ok(false)
        }

        async fn resumable_media_upload(
            &self,
            _property_id: &str,
            _filename: &str,
            _byte_size: i64,
            _chunk_size: i32,
        ) -> DbResult<Option<(String, String, Vec<i32>)>> {
            Ok(None)
        }

        async fn claim_media_upload(&self, _upload_id: &str) -> DbResult<bool> {
            self.chunk_calls.fetch_add(1, Ordering::SeqCst);
            Err(unused())
        }

        async fn fail_media_upload(&self, _upload_id: &str) -> DbResult<()> {
            self.chunk_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn media_upload_state(&self, _upload_id: &str) -> DbResult<Option<String>> {
            Ok(None)
        }

        async fn assemble_media_upload(&self, _upload_id: &str) -> DbResult<MediaUploadAssembly> {
            self.chunk_calls.fetch_add(1, Ordering::SeqCst);
            Err(unused())
        }

        async fn commit_media_upload(
            &self,
            _upload_id: &str,
            _assembly: &MediaUploadAssembly,
            _derivatives: &[MediaDerivativeInput],
        ) -> DbResult<UploadPropertyMediaResult> {
            self.chunk_calls.fetch_add(1, Ordering::SeqCst);
            Err(unused())
        }

        async fn media_object_location(
            &self,
            _media_id: &str,
        ) -> DbResult<Option<MediaObjectLocation>> {
            self.object_lookups.fetch_add(1, Ordering::SeqCst);
            Ok(self.location.clone())
        }

        async fn property_photo_id(
            &self,
            _property_id: &str,
            _filename: &str,
            _byte_size: i64,
        ) -> DbResult<Option<String>> {
            Ok(self.existing_photo_id.clone())
        }

        async fn property_is_public(&self, _property_id: &str) -> DbResult<bool> {
            Ok(self.public)
        }

        async fn save_property_media_object_set(
            &self,
            property_id: &str,
            role: &str,
            rows: &[MediaObjectRow],
        ) -> DbResult<UploadPropertyMediaResult> {
            if self.fail_save {
                return Err(DbFailure::configuration(
                    "test.save_property_media_object_set",
                    "the transaction failed",
                ));
            }
            self.saved_sets.lock().expect("saved poisoned").push((
                property_id.to_owned(),
                role.to_owned(),
                rows.to_vec(),
            ));
            Ok(UploadPropertyMediaResult {
                ok: true,
                media_id: rows[0].id.clone(),
                property_id: property_id.to_owned(),
                role: role.to_owned(),
            })
        }
    }

    fn unused() -> DbFailure {
        DbFailure::configuration(
            "test.unused",
            "this path is not part of the whole-file upload",
        )
    }

    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let mut image = image::RgbImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8]);
        }
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80)
            .encode_image(&image::DynamicImage::ImageRgb8(image))
            .expect("the fixture encodes");
        out
    }

    async fn service(
        repository: FakeRepository,
        store: Option<Arc<FakeStore>>,
        object_reads: bool,
    ) -> MediaService<FakeRepository> {
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(
                CasbinAuthorizationPort::new()
                    .await
                    .expect("the policy loads"),
            ),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        let service = MediaService::new(repository, infrastructure);
        let service = match store {
            Some(store) => service.with_object_store(store),
            None => service,
        };
        service.with_object_reads(object_reads)
    }

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

    /// The registration the Lead is asked to add must typecheck: this builds the exact route — handler plus the
    /// body limit that makes a whole photograph fit — against the real router state. It does not serve anything.
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
    async fn a_whole_photograph_is_stored_object_first_with_no_chunk_rows() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let service = service(repository, Some(store.clone()), false).await;

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

        // NO CHUNK STAGING: the whole-file path never opens a manifest, stages a piece or assembles one.
        assert_eq!(
            service.repository.chunk_calls.load(Ordering::SeqCst),
            0,
            "the whole-file path must not touch media_upload/media_upload_chunk"
        );
        assert_eq!(service.repository.blob_reads.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_failed_metadata_write_deletes_every_object_and_leaves_no_rows() {
        let repository = FakeRepository {
            fail_save: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let service = service(repository, Some(store.clone()), false).await;

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
        assert_eq!(service.repository.chunk_calls.load(Ordering::SeqCst), 0);
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
        let service = service(repository, Some(store.clone()), false).await;

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
        assert_eq!(service.repository.chunk_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_file_that_is_not_an_image_uploads_nothing() {
        let repository = FakeRepository::default();
        let store = Arc::new(FakeStore::default());
        let service = service(repository, Some(store.clone()), false).await;

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
        let media = service(FakeRepository::default(), Some(store.clone()), false).await;
        let error = media
            .upload_property_media_full(bad_type, &root_context())
            .await
            .expect_err("an image row that is not an image");
        assert_eq!(error.code(), "MEDIA_TYPE_INVALID");

        let mut empty = request(Vec::new());
        empty.mime_type = "image/jpeg".into();
        let media = service(FakeRepository::default(), Some(store.clone()), false).await;
        let error = media
            .upload_property_media_full(empty, &root_context())
            .await
            .expect_err("an empty file");
        assert_eq!(error.code(), "MEDIA_EMPTY");

        let mut bad_role = request(jpeg(100, 100));
        bad_role.role = "video".into();
        let media = service(FakeRepository::default(), Some(store.clone()), false).await;
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
        let service = service(repository, Some(store.clone()), false).await;

        let stored = service
            .upload_property_media_full(request(jpeg(400, 300)), &root_context())
            .await
            .expect("the retry is answered");

        assert!(stored.skipped);
        assert_eq!(stored.media_id, "11111111-1111-4111-8111-111111111111");
        assert_eq!(stored.derivative_kinds, Vec::<&str>::new());
        assert_eq!(store.put_count(), 0);
        assert!(service.repository.saved_sets().is_empty());
        assert_eq!(service.repository.chunk_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_upload_without_an_object_store_is_a_typed_failure() {
        let service = service(FakeRepository::default(), None, false).await;

        let error = service
            .upload_property_media_full(request(jpeg(100, 100)), &root_context())
            .await
            .expect_err("no object store");

        assert_eq!(error.code(), "MEDIA_OBJECT_STORE_NOT_CONFIGURED");
    }

    #[tokio::test]
    async fn object_reads_never_fall_back_to_the_legacy_blob() {
        let repository = FakeRepository {
            blob: Some(("image/jpeg".into(), vec![1, 2, 3])),
            location: None,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let service = service(repository, Some(store), true).await;

        let error = service
            .media_read("missing-media", &root_context())
            .await
            .expect_err("a row with no object must be a typed failure, not a blob read");

        assert_eq!(error.code(), "MEDIA_OBJECT_NOT_FOUND");
        assert_eq!(
            service.repository.blob_reads.load(Ordering::SeqCst),
            0,
            "the legacy blob must not be read while the object switch is on"
        );
    }

    #[tokio::test]
    async fn an_unverified_object_is_refused_instead_of_served() {
        let repository = FakeRepository {
            location: Some(location("MIGRATION_PENDING", "private")),
            blob: Some(("image/jpeg".into(), vec![1, 2, 3])),
            ..Default::default()
        };
        let service = service(repository, Some(Arc::new(FakeStore::default())), true).await;

        let error = service
            .media_read("half-migrated", &root_context())
            .await
            .expect_err("an unverified object is not servable");

        assert_eq!(error.code(), "MEDIA_OBJECT_NOT_VERIFIED");
        assert_eq!(service.repository.blob_reads.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_verified_public_object_redirects_to_its_public_url() {
        let repository = FakeRepository {
            location: Some(location("VERIFIED", "public")),
            blob: Some(("image/jpeg".into(), vec![1, 2, 3])),
            ..Default::default()
        };
        let service = service(repository, Some(Arc::new(FakeStore::default())), true).await;

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
        assert_eq!(service.repository.blob_reads.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_url_failure_is_typed_not_a_blob_fallback() {
        let repository = FakeRepository {
            location: Some(location("VERIFIED", "private")),
            blob: Some(("image/jpeg".into(), vec![1, 2, 3])),
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        store.fail_urls.store(true, Ordering::SeqCst);
        let service = service(repository, Some(store), true).await;

        let error = service
            .media_read("private-photo", &root_context())
            .await
            .expect_err("the URL could not be generated");

        assert_eq!(error.code(), "MEDIA_OBJECT_READ_URL_FAILED");
        assert_eq!(service.repository.blob_reads.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn the_legacy_blob_is_still_read_while_the_switch_is_off() {
        let repository = FakeRepository {
            blob: Some(("image/jpeg".into(), vec![1, 2, 3])),
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let service = service(repository, Some(store), false).await;

        let found = service
            .media_read("legacy-photo", &root_context())
            .await
            .expect("the legacy read is unchanged")
            .expect("the row exists");

        match found {
            MediaRead::Bytes { mime_type, bytes } => {
                assert_eq!(mime_type, "image/jpeg");
                assert_eq!(bytes, vec![1, 2, 3]);
            }
            MediaRead::Redirect { .. } => panic!("the switch is off; the blob is canonical"),
        }
        assert_eq!(service.repository.blob_reads.load(Ordering::SeqCst), 1);
    }
}
