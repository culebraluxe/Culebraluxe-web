//! DIRECT-TO-OBJECT upload for large property photographs: the bytes never cross the gateway.
//!
//! WHY THIS EXISTS. The whole-file route (`/api/property-media/full`) carries one photograph in one
//! request, but the hosting edge caps a request body at about 4.5 MB. Everything larger used to travel
//! through the chunk protocol, which stage-wrote every piece into `media_upload_chunk`. That protocol is
//! retired now: a large photograph goes straight to the media bucket and only metadata crosses this app.
//!
//! THREE DOORWAYS, NO STAGING:
//! 1. `POST /api/property-media/direct/initiate` — authenticated, validates property/role/MIME/size and
//!    the publication bucket, then answers a short-lived S3 presigned PUT URL and a SIGNED pending token
//!    (`uploadId`). The token binds the property, the original media id, the sanitized filename, the MIME
//!    type, the exact byte size, the client's SHA-256, the bucket visibility, the acting user and an
//!    expiry: finalize accepts nothing else.
//! 2. The browser PUTs the bytes straight to the property-media bucket (CORS is configured there). No
//!    request to this application carries the photograph.
//! 3. `POST /api/property-media/direct/finalize` — HEADs the object (size + content type), GETs it once to
//!    prove the SHA-256 and size, derives the web/card/thumb copies, uploads those object-first (each
//!    verified), and records the original + copies + property link in ONE database transaction. On any
//!    failure every object this flow touched is deleted, so a retry starts clean.
//! 4. `POST /api/property-media/direct/abort` — an upload the browser gave up on deletes its one object.
//!
//! NO STAGING. The retired `media_upload` / `media_upload_chunk` tables have no code path left anywhere;
//! the only rows written here are the object-first `media` rows and the `property_media` link.
//!
//! CONFIGURATION. Token signing uses `MEDIA_DIRECT_UPLOAD_SECRET` when set (16+ characters), else
//! `AUTH_SECRET` — the session secret production already has. The object store itself is the media
//! store `composition.rs` already attaches to the media service; no new wiring is required.

#[allow(unused_imports)]
use super::*;
use crate::api::ui_auth::resolve_portal_context;
use crate::api::{ApiError, ApiState};
use axum::{extract::State, http::HeaderMap, Json};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use services::{ServiceFailureClass, ServiceRuntimeError};
use sha2::{Digest, Sha256};
use std::time::Duration;

/// How long the presigned PUT lives. Short enough that a leaked URL is worth little; long enough for a
/// large photograph on a slow line.
pub(super) const DIRECT_UPLOAD_URL_TTL_SECONDS: u64 = 10 * 60;

/// How long the pending token lives. Slightly longer than the URL, so a PUT that arrived just before the
/// URL expired can still be finalized.
pub(super) const DIRECT_UPLOAD_TOKEN_TTL_SECONDS: i64 = 15 * 60;

const DIRECT_UPLOAD_TOKEN_VERSION: u8 = 1;

const TOKEN_INVALID: &str = "MEDIA_DIRECT_UPLOAD_TOKEN_INVALID";
const TOKEN_EXPIRED: &str = "MEDIA_DIRECT_UPLOAD_TOKEN_EXPIRED";

/// What a browser asks to upload directly. Sizes are declared up front so nothing over the cap is ever
/// accepted, and the SHA-256 is what finalize proves the stored object against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectPropertyMediaUploadRequest {
    pub property_id: String,
    pub role: String,
    pub filename: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub sha256: String,
    pub alt_text: Option<String>,
}

/// The answer to `initiate`: either the property already shows this file (`state: "done"`, nothing to
/// send) or a signed PUT URL and the pending token to finalize with (`state: "upload"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectPropertyMediaUploadSession {
    /// `upload` or `done`.
    pub state: &'static str,
    /// The signed pending token. Absent when `state` is `done`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_id: Option<String>,
    /// The media row id the original will be recorded under (already known when `done`).
    pub media_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_url: Option<String>,
    pub method: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    pub byte_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Unix seconds after which the PUT URL no longer works.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

/// The answer to `abort`: whether an object was removed, and whether it was already finalized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectPropertyMediaAbortResult {
    pub deleted: bool,
    pub finalized: bool,
}

/// The signed claims of a pending upload. Everything finalize needs is here, so no staging row has to be
/// written before the file exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DirectUploadClaims {
    v: u8,
    /// The media row id the original is recorded under (also the object-key id).
    mid: String,
    /// The property the photograph will be attached to.
    pid: String,
    role: String,
    filename: String,
    mime: String,
    size: i64,
    sha256: String,
    /// Whether the property was publicly visible when the URL was issued.
    public: bool,
    /// The signed-in app user that initiated the upload; a different user cannot finalize it.
    actor: String,
    alt: Option<String>,
    exp: i64,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The object key the browser PUTs to. Matches the key layout `object_row` uses for every other
/// object-first image row, so a direct upload is indistinguishable downstream.
fn direct_object_key(media_id: &str, mime_type: &str) -> String {
    let extension = match mime_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => "bin",
    };
    format!("media/{media_id}/original.{extension}")
}

fn unix_now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn hmac_signature(secret: &[u8], payload: &str) -> Option<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).ok()?;
    mac.update(payload.as_bytes());
    Some(
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

fn constant_time_eq(expected: &str, given: &str) -> bool {
    expected.len() == given.len()
        && expected
            .bytes()
            .zip(given.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

fn encode_pending_token(
    secret: &[u8],
    claims: &DirectUploadClaims,
) -> Result<String, CoreServiceError> {
    use base64::Engine as _;
    let json = serde_json::to_vec(claims).map_err(|error| {
        CoreServiceError::infrastructure(
            "MEDIA_DIRECT_UPLOAD_TOKEN_FAILED",
            format!("the pending upload token could not be encoded ({error})"),
        )
    })?;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json);
    let signature = hmac_signature(secret, &payload).ok_or_else(|| {
        CoreServiceError::Runtime(ServiceRuntimeError::Router {
            code: "MEDIA_DIRECT_UPLOAD_NOT_CONFIGURED".into(),
            message: "direct property-media uploads are not configured".into(),
            retryable: false,
            class: ServiceFailureClass::Infrastructure,
        })
    })?;
    Ok(format!("{payload}.{signature}"))
}

fn invalid_token() -> CoreServiceError {
    CoreServiceError::business(
        TOKEN_INVALID,
        "This upload link is not valid; choose the photograph again.",
    )
}

/// Decode and signature-check a pending token. Expiry is checked by the caller, because `abort` may
/// still remove an object whose token has aged out.
fn decode_pending_token(
    secret: &[u8],
    token: &str,
) -> Result<DirectUploadClaims, CoreServiceError> {
    use base64::Engine as _;
    let (payload, given) = token.split_once('.').ok_or_else(invalid_token)?;
    let expected = hmac_signature(secret, payload).ok_or_else(invalid_token)?;
    if !constant_time_eq(&expected, given) {
        return Err(invalid_token());
    }
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| invalid_token())?;
    let claims: DirectUploadClaims = serde_json::from_slice(&json).map_err(|_| invalid_token())?;
    if claims.v != DIRECT_UPLOAD_TOKEN_VERSION || claims.mid.is_empty() || claims.pid.is_empty() {
        return Err(invalid_token());
    }
    Ok(claims)
}

/// The signed-in user a pending token was minted for. Finalize and abort refuse a different actor even
/// when that actor holds the same entitlement: the token is not a bearer capability.
fn acting_user(context: &ServiceContext) -> Result<String, CoreServiceError> {
    context
        .principal
        .as_ref()
        .map(|principal| principal.app_user_id.clone())
        .or_else(|| context.actor.id.clone())
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            CoreServiceError::business(
                "MEDIA_DIRECT_UPLOAD_ACTOR_REQUIRED",
                "A signed-in user is required to upload a photograph.",
            )
        })
}

fn validate_sha256(sha256: &str) -> Result<String, CoreServiceError> {
    let sha256 = sha256.trim().to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CoreServiceError::business(
            "MEDIA_SHA256_INVALID",
            "The upload must declare the file's SHA-256 hash.",
        ));
    }
    Ok(sha256)
}

/// Delete one object after a refusal. Best effort: a stray object is recoverable and is swept by
/// `abort`/the bucket's lifecycle policy; a media row pointing at a missing object is not.
async fn discard_object(
    store: &Arc<dyn AbstractObjectStoreService>,
    bucket: ObjectBucket,
    key: &str,
) {
    if let Err(error) = store.delete_object(bucket, key).await {
        tracing::warn!(
            key,
            %error,
            "direct property-media upload: could not delete an object during cleanup"
        );
    }
}

impl<R: MediaRepository> MediaService<R> {
    /// The key that signs pending tokens: an injected one (tests), else the deployment's dedicated
    /// secret, else the session secret production already carries.
    fn direct_upload_key(&self) -> Result<Vec<u8>, CoreServiceError> {
        if let Some(secret) = self.direct_upload_secret.as_ref() {
            if secret.len() >= 16 {
                return Ok(secret.clone());
            }
        }
        ["MEDIA_DIRECT_UPLOAD_SECRET", "AUTH_SECRET"]
            .iter()
            .find_map(|name| {
                std::env::var(name)
                    .ok()
                    .filter(|value| value.len() >= 16)
            })
            .map(String::into_bytes)
            .ok_or_else(|| {
                CoreServiceError::Runtime(ServiceRuntimeError::Router {
                    code: "MEDIA_DIRECT_UPLOAD_NOT_CONFIGURED".into(),
                    message: "direct property-media uploads need MEDIA_DIRECT_UPLOAD_SECRET or AUTH_SECRET"
                        .into(),
                    retryable: false,
                    class: ServiceFailureClass::Infrastructure,
                })
            })
    }

    /// Open a direct-to-object upload: validate the request, decide the publication bucket, and sign a
    /// short-lived PUT URL plus a pending token. No row is written and no chunk is staged.
    pub async fn initiate_property_media_direct_upload(
        &self,
        request: DirectPropertyMediaUploadRequest,
        context: &ServiceContext,
    ) -> Result<DirectPropertyMediaUploadSession, CoreServiceError> {
        const OP: &str = "media.initiatePropertyMediaDirectUpload";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self.initiate_direct_upload_inner(request, context).await;
        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    async fn initiate_direct_upload_inner(
        &self,
        mut request: DirectPropertyMediaUploadRequest,
        context: &ServiceContext,
    ) -> Result<DirectPropertyMediaUploadSession, CoreServiceError> {
        // Same normalisation as the whole-file path: the dedupe guard compares stored filenames.
        request.property_id = request.property_id.trim().to_owned();
        request.role = if request.role.trim().is_empty() {
            "gallery".to_owned()
        } else {
            request.role.trim().to_owned()
        };
        request.filename = sanitize_media_filename(&request.filename);
        request.mime_type = request.mime_type.trim().to_ascii_lowercase();
        request.sha256 = validate_sha256(&request.sha256)?;

        if request.property_id.is_empty() {
            return Err(CoreServiceError::business(
                "PROPERTY_REQUIRED",
                "Property is required.",
            ));
        }
        if uuid::Uuid::parse_str(&request.property_id).is_err() {
            return Err(CoreServiceError::business(
                "PROPERTY_INVALID",
                "Invalid property.",
            ));
        }
        if !matches!(request.role.as_str(), "hero" | "gallery") {
            return Err(CoreServiceError::business(
                "MEDIA_ROLE_INVALID",
                "Invalid media role.",
            ));
        }
        if request.byte_size <= 0 {
            return Err(CoreServiceError::business(
                "MEDIA_EMPTY",
                "Image file is required.",
            ));
        }
        if request.byte_size > MAX_MEDIA_UPLOAD_BYTES as i64 {
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

        // A retry of a photograph the property already shows answers `done` and sends nothing, exactly
        // as the whole-file path's retry guard does.
        if let Some(media_id) = self
            .repository
            .property_photo_id(&request.property_id, &request.filename, request.byte_size)
            .await?
        {
            return Ok(DirectPropertyMediaUploadSession {
                state: "done",
                upload_id: None,
                media_id,
                upload_url: None,
                method: "PUT",
                content_type: None,
                byte_size: request.byte_size,
                sha256: None,
                expires_at: None,
            });
        }

        let store = self
            .object_store
            .as_ref()
            .ok_or_else(object_store_not_configured)?;
        // The publication rule decides the bucket ONCE, here, and the choice is signed into the token:
        // a copy can never leak more than the photograph it copies.
        let public = self
            .repository
            .property_is_public(&request.property_id)
            .await
            .unwrap_or(false);
        let bucket = if public {
            ObjectBucket::PropertyMediaPublic
        } else {
            ObjectBucket::PropertyMediaPrivate
        };
        let media_id = uuid::Uuid::new_v4().to_string();
        let key = direct_object_key(&media_id, &request.mime_type);
        let upload_url = store
            .signed_write_url(
                bucket,
                &key,
                &request.mime_type,
                Duration::from_secs(DIRECT_UPLOAD_URL_TTL_SECONDS),
            )
            .await
            .map_err(|error| {
                CoreServiceError::infrastructure(
                    "MEDIA_DIRECT_UPLOAD_URL_FAILED",
                    format!("a direct upload URL could not be issued ({error})"),
                )
            })?;

        let secret = self.direct_upload_key()?;
        let expires_at = unix_now() + DIRECT_UPLOAD_TOKEN_TTL_SECONDS;
        let claims = DirectUploadClaims {
            v: DIRECT_UPLOAD_TOKEN_VERSION,
            mid: media_id.clone(),
            pid: request.property_id.clone(),
            role: request.role.clone(),
            filename: request.filename.clone(),
            mime: request.mime_type.clone(),
            size: request.byte_size,
            sha256: request.sha256.clone(),
            public,
            actor: acting_user(context)?,
            alt: request.alt_text.clone(),
            exp: expires_at,
        };
        let upload_id = encode_pending_token(&secret, &claims)?;

        Ok(DirectPropertyMediaUploadSession {
            state: "upload",
            upload_id: Some(upload_id),
            media_id,
            upload_url: Some(upload_url),
            method: "PUT",
            content_type: Some(request.mime_type),
            byte_size: request.byte_size,
            sha256: Some(request.sha256),
            expires_at: Some(expires_at),
        })
    }

    /// Close a direct-to-object upload: verify the object, derive the servable copies, upload them
    /// object-first, and record the rows and property link in one transaction.
    pub async fn finalize_property_media_direct_upload(
        &self,
        upload_id: &str,
        context: &ServiceContext,
    ) -> Result<FullPropertyMediaUploadResult, CoreServiceError> {
        const OP: &str = "media.finalizePropertyMediaDirectUpload";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self.finalize_direct_upload_inner(upload_id, context).await;
        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    async fn finalize_direct_upload_inner(
        &self,
        upload_id: &str,
        context: &ServiceContext,
    ) -> Result<FullPropertyMediaUploadResult, CoreServiceError> {
        let secret = self.direct_upload_key()?;
        let claims = decode_pending_token(&secret, upload_id)?;
        if claims.exp <= unix_now() {
            return Err(CoreServiceError::business(
                TOKEN_EXPIRED,
                "This upload link has expired; choose the photograph again.",
            ));
        }
        if claims.actor != acting_user(context)? {
            return Err(CoreServiceError::business(
                "MEDIA_DIRECT_UPLOAD_TOKEN_ACTOR_MISMATCH",
                "This upload link belongs to a different user.",
            ));
        }
        // Defence in depth: the token is authoritative, but re-check the rules that decide what may be
        // stored in case the token format ever changes.
        validate_sha256(&claims.sha256)?;
        if claims.size <= 0 || claims.size > MAX_MEDIA_UPLOAD_BYTES as i64 {
            return Err(CoreServiceError::business(
                "MEDIA_TOO_LARGE",
                "Image is too large (max 50 MB).",
            ));
        }
        if !SUPPORTED_IMAGE_MIME_TYPES.contains(&claims.mime.as_str()) {
            return Err(CoreServiceError::business(
                "MEDIA_TYPE_INVALID",
                "Only JPEG, PNG and WebP photographs are supported.",
            ));
        }
        if !matches!(claims.role.as_str(), "hero" | "gallery") {
            return Err(CoreServiceError::business(
                "MEDIA_ROLE_INVALID",
                "Invalid media role.",
            ));
        }

        let store = self
            .object_store
            .as_ref()
            .ok_or_else(object_store_not_configured)?;
        let bucket = if claims.public {
            ObjectBucket::PropertyMediaPublic
        } else {
            ObjectBucket::PropertyMediaPrivate
        };
        let visibility = if claims.public { "public" } else { "private" };
        let key = direct_object_key(&claims.mid, &claims.mime);

        // The publication rule is re-evaluated at finalize. If the listing's visibility changed while
        // the photograph was in flight, the object sits in the wrong bucket and the upload is refused;
        // the client gets a retryable sentence and initiates again. A database failure here propagates
        // WITHOUT deleting the object, so the same token can be finalized after the read recovers.
        let public_now = self.repository.property_is_public(&claims.pid).await?;
        if public_now != claims.public {
            discard_object(store, bucket, &key).await;
            return Err(CoreServiceError::business(
                "MEDIA_PUBLICATION_CHANGED",
                "This listing's visibility changed while the photograph uploaded; try again.",
            ));
        }

        // A resend after an answer that never arrived answers with the row that already exists and
        // removes the now-orphaned object.
        if let Some(existing) = self
            .repository
            .property_photo_id(&claims.pid, &claims.filename, claims.size)
            .await?
        {
            discard_object(store, bucket, &key).await;
            return Ok(FullPropertyMediaUploadResult {
                media_id: existing,
                property_id: claims.pid,
                role: claims.role,
                storage: "object",
                skipped: true,
                derivative_kinds: Vec::new(),
            });
        }

        // HEAD first: the cheapest proof that the PUT happened, with the size and content type the
        // gateway never saw.
        let head = match store.head_object(bucket, &key).await {
            Ok(head) => head,
            Err(error) if error.code == "OBJECT_NOT_FOUND" => {
                return Err(CoreServiceError::business(
                    "MEDIA_DIRECT_OBJECT_MISSING",
                    "The photograph was not found in storage; upload it again.",
                ))
            }
            Err(error) => {
                return Err(CoreServiceError::infrastructure(
                    "MEDIA_DIRECT_OBJECT_HEAD_FAILED",
                    error.to_string(),
                ))
            }
        };
        if head.size != claims.size {
            discard_object(store, bucket, &key).await;
            return Err(CoreServiceError::business(
                "MEDIA_DIRECT_SIZE_MISMATCH",
                format!(
                    "The uploaded file is {} bytes but {} was declared.",
                    head.size, claims.size
                ),
            ));
        }
        if let Some(content_type) = head.content_type.as_deref() {
            if content_type.to_ascii_lowercase() != claims.mime {
                discard_object(store, bucket, &key).await;
                return Err(CoreServiceError::business(
                    "MEDIA_DIRECT_CONTENT_TYPE_MISMATCH",
                    "The uploaded file's content type does not match the declaration.",
                ));
            }
        }

        // The bytes are needed anyway for the derivatives; hashing them here is the SHA-256 proof. The
        // provider is not trusted to have stored what it said it stored.
        let bytes = store.get_object(bucket, &key).await.map_err(|error| {
            CoreServiceError::infrastructure(
                "MEDIA_DIRECT_OBJECT_READ_FAILED",
                format!("the uploaded photograph could not be read back ({error})"),
            )
        })?;
        if bytes.len() as i64 != claims.size {
            discard_object(store, bucket, &key).await;
            return Err(CoreServiceError::business(
                "MEDIA_DIRECT_SIZE_MISMATCH",
                "The stored photograph is a different size than declared.",
            ));
        }
        let actual_sha256 = sha256_hex(&bytes);
        if actual_sha256 != claims.sha256 {
            discard_object(store, bucket, &key).await;
            return Err(CoreServiceError::business(
                "MEDIA_DIRECT_SHA_MISMATCH",
                "The uploaded bytes do not match the declared SHA-256; the upload was corrupted or tampered with.",
            ));
        }

        // Re-encoding a large photograph is straight CPU; keep it off the async runtime.
        let derived = tokio::task::spawn_blocking(move || imaging::derive_web_and_thumb(&bytes))
            .await
            .map_err(|error| {
                CoreServiceError::business(
                    "MEDIA_DERIVATIVE_FAILED",
                    format!("the photograph could not be processed ({error})"),
                )
            })?
            .map_err(|message| CoreServiceError::business("MEDIA_DERIVATIVE_FAILED", message))?;

        // The original's row reuses the object the browser already stored; its metadata comes from HEAD
        // plus the SHA-256 this server computed.
        let original = db::MediaObjectRow {
            id: claims.mid.clone(),
            filename: claims.filename.clone(),
            mime_type: claims.mime.clone(),
            alt_text: claims.alt.clone(),
            media_type: "image".to_owned(),
            derivative_of: None,
            derivative_kind: None,
            object: db::MediaObjectMetadata {
                bucket: bucket.as_str().to_owned(),
                key: key.clone(),
                etag: head.etag.clone(),
                sha256: Some(actual_sha256),
                size: claims.size,
                status: "VERIFIED".to_owned(),
                visibility: Some(visibility.to_owned()),
            },
        };
        let mut rows: Vec<db::MediaObjectRow> = Vec::with_capacity(1 + derived.len());
        let mut uploaded: Vec<(ObjectBucket, String)> = Vec::with_capacity(1 + derived.len());
        uploaded.push((bucket, key));
        rows.push(original);

        // Copies are stored object-first, each verified; a failure deletes the whole set including the
        // browser's original, so no row ever points at a missing object and no orphan is left behind.
        let mut derivative_kinds = Vec::with_capacity(derived.len());
        for derivative in &derived {
            let id = uuid::Uuid::new_v4().to_string();
            let filename = derivative_filename(&claims.filename, derivative.kind);
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
                    Some(claims.mid.clone()),
                    Some(derivative.kind.to_owned()),
                )
                .await
            {
                Ok(row) => {
                    uploaded.push((bucket, row.object.key.clone()));
                    rows.push(row);
                    derivative_kinds.push(derivative.kind);
                }
                Err(message) => {
                    discard_objects(store, &uploaded).await;
                    return Err(CoreServiceError::infrastructure(
                        "MEDIA_OBJECT_UPLOAD_FAILED",
                        message,
                    ));
                }
            }
        }

        match self
            .repository
            .save_property_media_object_set(&claims.pid, &claims.role, &rows)
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
                discard_objects(store, &uploaded).await;
                Err(error.into())
            }
        }
    }

    /// Remove the object of an upload the browser abandoned. A token that has already been finalized
    /// never deletes the stored photograph: the object a media row points at is not an orphan.
    pub async fn abort_property_media_direct_upload(
        &self,
        upload_id: &str,
        context: &ServiceContext,
    ) -> Result<DirectPropertyMediaAbortResult, CoreServiceError> {
        const OP: &str = "media.abortPropertyMediaDirectUpload";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self.abort_direct_upload_inner(upload_id, context).await;
        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    async fn abort_direct_upload_inner(
        &self,
        upload_id: &str,
        context: &ServiceContext,
    ) -> Result<DirectPropertyMediaAbortResult, CoreServiceError> {
        let secret = self.direct_upload_key()?;
        // An abandoned upload may be cleaned long after its PUT URL expired, so abort checks the
        // signature but not the expiry.
        let claims = decode_pending_token(&secret, upload_id)?;
        if claims.actor != acting_user(context)? {
            return Err(CoreServiceError::business(
                "MEDIA_DIRECT_UPLOAD_TOKEN_ACTOR_MISMATCH",
                "This upload link belongs to a different user.",
            ));
        }
        let store = self
            .object_store
            .as_ref()
            .ok_or_else(object_store_not_configured)?;
        let bucket = if claims.public {
            ObjectBucket::PropertyMediaPublic
        } else {
            ObjectBucket::PropertyMediaPrivate
        };
        if self
            .repository
            .media_object_location(&claims.mid)
            .await?
            .is_some()
        {
            return Ok(DirectPropertyMediaAbortResult {
                deleted: false,
                finalized: true,
            });
        }
        let key = direct_object_key(&claims.mid, &claims.mime);
        let exists = store.exists(bucket, &key).await.map_err(|error| {
            CoreServiceError::infrastructure(
                "MEDIA_DIRECT_CLEANUP_FAILED",
                format!("the abandoned upload could not be inspected ({error})"),
            )
        })?;
        if exists {
            store.delete_object(bucket, &key).await.map_err(|error| {
                CoreServiceError::infrastructure(
                    "MEDIA_DIRECT_CLEANUP_FAILED",
                    format!("the abandoned upload could not be removed ({error})"),
                )
            })?;
        }
        Ok(DirectPropertyMediaAbortResult {
            deleted: exists,
            finalized: false,
        })
    }
}

fn str_field<'a>(body: &'a Value, name: &str) -> &'a str {
    body.get(name).and_then(Value::as_str).unwrap_or_default()
}

/// `POST /api/property-media/direct/initiate` (JSON).
///
/// Body: `{ propertyId, role?, filename, mimeType, byteSize, sha256, altText? }`. Answers either
/// `{ state: "done", mediaId }` (the property already shows this file) or
/// `{ state: "upload", uploadId, mediaId, uploadUrl, method: "PUT", contentType, byteSize, sha256,
/// expiresAt }`. `uploadId` is a signed pending token, not a storage id: pass it back to finalize.
pub async fn property_media_direct_initiate(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let bad = |code: &'static str, message: &str| {
        ApiError::bad_request(code, message.to_owned())
            .with_correlation(resolved.service.correlation_id.clone())
    };
    let property_id = str_field(&body, "propertyId");
    if property_id.trim().is_empty() {
        return Err(bad("PROPERTY_REQUIRED", "Property is required."));
    }
    let fill_correlation =
        |error: ApiError| error.with_correlation(resolved.service.correlation_id.clone());
    let request = DirectPropertyMediaUploadRequest {
        property_id: property_id.to_owned(),
        role: str_field(&body, "role").to_owned(),
        filename: str_field(&body, "filename").to_owned(),
        mime_type: str_field(&body, "mimeType").to_owned(),
        byte_size: body.get("byteSize").and_then(Value::as_i64).unwrap_or(-1),
        sha256: str_field(&body, "sha256").to_owned(),
        alt_text: Some(str_field(&body, "altText").to_owned())
            .filter(|value| !value.trim().is_empty()),
    };
    let session = state
        .services()
        .media()
        .initiate_property_media_direct_upload(request, &resolved.service)
        .await
        .map_err(|error| fill_correlation(ApiError::from(error)))?;
    Ok(Json(json!({
        "ok": true,
        "value": session,
        "correlationId": resolved.service.correlation_id,
    })))
}

/// `POST /api/property-media/direct/finalize` (JSON): `{ uploadId }` where `uploadId` is the signed
/// token `initiate` answered. Answers the same value shape as the whole-file route.
pub async fn property_media_direct_finalize(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();
    let upload_id = str_field(&body, "uploadId").trim().to_owned();
    if upload_id.is_empty() {
        return Err(ApiError::bad_request(
            "MEDIA_UPLOAD_ID_REQUIRED",
            "An upload id is required.".to_owned(),
        )
        .with_correlation(correlation));
    }
    let value = state
        .services()
        .media()
        .finalize_property_media_direct_upload(&upload_id, &resolved.service)
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

/// `POST /api/property-media/direct/abort` (JSON): `{ uploadId }`. Removes the object of an upload the
/// browser gave up on; a finalized upload answers `{ deleted: false, finalized: true }`.
pub async fn property_media_direct_abort(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let correlation = resolved.service.correlation_id.clone();
    let upload_id = str_field(&body, "uploadId").trim().to_owned();
    if upload_id.is_empty() {
        return Err(ApiError::bad_request(
            "MEDIA_UPLOAD_ID_REQUIRED",
            "An upload id is required.".to_owned(),
        )
        .with_correlation(correlation));
    }
    let value = state
        .services()
        .media()
        .abort_property_media_direct_upload(&upload_id, &resolved.service)
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
    use super::sha256_hex;
    use super::*;
    use crate::media::test_support::*;
    use std::sync::atomic::Ordering;

    fn upload_request(bytes: &[u8]) -> DirectPropertyMediaUploadRequest {
        DirectPropertyMediaUploadRequest {
            property_id: PROPERTY.into(),
            role: "gallery".into(),
            filename: "ZoniBluff_1.jpg".into(),
            mime_type: "image/jpeg".into(),
            byte_size: bytes.len() as i64,
            sha256: sha256_hex(bytes),
            alt_text: Some("Zoni bluff at dusk".into()),
        }
    }

    /// A signed-in user with ROOT's grant who is NOT the user the token was minted for.
    fn another_root_context() -> ServiceContext {
        let mut context = root_context();
        context.actor.id = Some("another-root-user".into());
        context
            .principal
            .as_mut()
            .expect("the test user has a principal")
            .app_user_id = "another-root-user".into();
        context
    }

    /// A signed-in EXTERNAL account: authenticated, but never granted `property.write`.
    fn external_context() -> ServiceContext {
        let mut context = root_context();
        context
            .principal
            .as_mut()
            .expect("the test user has a principal")
            .account_type = "external".into();
        context
    }

    fn bucket(public: bool) -> ObjectBucket {
        if public {
            ObjectBucket::PropertyMediaPublic
        } else {
            ObjectBucket::PropertyMediaPrivate
        }
    }

    fn key_for(session: &DirectPropertyMediaUploadSession) -> String {
        direct_object_key(&session.media_id, "image/jpeg")
    }

    /// Initiate, then stand in for the browser's direct PUT of the same bytes.
    async fn initiate_and_put(
        media: &MediaService<FakeRepository>,
        store: &Arc<FakeStore>,
        bytes: &[u8],
        public: bool,
    ) -> (DirectPropertyMediaUploadSession, String) {
        let session = media
            .initiate_property_media_direct_upload(upload_request(bytes), &root_context())
            .await
            .expect("the upload is initiated");
        let key = key_for(&session);
        store.direct_put(bucket(public), &key, bytes, "image/jpeg");
        (session, key)
    }

    fn expired_claims(media_id: &str, bytes: &[u8], public: bool) -> DirectUploadClaims {
        DirectUploadClaims {
            v: DIRECT_UPLOAD_TOKEN_VERSION,
            mid: media_id.to_owned(),
            pid: PROPERTY.to_owned(),
            role: "gallery".to_owned(),
            filename: "ZoniBluff_1.jpg".to_owned(),
            mime: "image/jpeg".to_owned(),
            size: bytes.len() as i64,
            sha256: sha256_hex(bytes),
            public,
            actor: "root-user".to_owned(),
            alt: None,
            exp: unix_now() - 1,
        }
    }

    /// The registration in `web/src/api/portal_bridge.rs` must typecheck. This builds exactly the three
    /// routes against the real router state, so a mistyped handler or state type is caught here.
    #[test]
    fn the_direct_handlers_register_on_the_api_router() {
        let _router: axum::Router<ApiState> = axum::Router::new()
            .route(
                "/api/property-media/direct/initiate",
                axum::routing::post(property_media_direct_initiate),
            )
            .route(
                "/api/property-media/direct/finalize",
                axum::routing::post(property_media_direct_finalize),
            )
            .route(
                "/api/property-media/direct/abort",
                axum::routing::post(property_media_direct_abort),
            );
    }

    #[tokio::test]
    async fn initiate_signs_a_short_lived_put_url_and_a_pending_token() {
        let store = Arc::new(FakeStore::default());
        let media = service(
            FakeRepository {
                public: true,
                ..Default::default()
            },
            Some(store.clone()),
        )
        .await;
        let bytes = jpeg(800, 600);

        let session = media
            .initiate_property_media_direct_upload(upload_request(&bytes), &root_context())
            .await
            .expect("the upload is initiated");

        assert_eq!(session.state, "upload");
        assert_eq!(session.method, "PUT");
        assert_eq!(session.content_type.as_deref(), Some("image/jpeg"));
        assert_eq!(session.byte_size, bytes.len() as i64);
        assert_eq!(session.sha256.as_deref(), Some(sha256_hex(&bytes).as_str()));
        let upload_url = session.upload_url.as_deref().expect("a PUT URL is issued");
        assert_eq!(
            upload_url,
            format!("https://write.example/{}", key_for(&session))
        );
        let token = session
            .upload_id
            .as_deref()
            .expect("a pending token is issued");
        assert!(
            token.contains('.'),
            "the pending id is a signed token, not a bare uuid"
        );

        // Nothing was uploaded and nothing was recorded: initiate only signs.
        assert_eq!(store.put_count(), 0);
        assert!(media.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn initiate_refuses_a_caller_without_property_write() {
        let store = Arc::new(FakeStore::default());
        let media = service(FakeRepository::default(), Some(store.clone())).await;

        let error = media
            .initiate_property_media_direct_upload(
                upload_request(&jpeg(100, 100)),
                &external_context(),
            )
            .await
            .expect_err("an external account may not write property media");

        assert_eq!(error.code(), "FORBIDDEN");
        assert_eq!(store.put_count(), 0);
        assert!(media.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn initiate_validates_the_declaration_before_anything_is_signed() {
        let store = Arc::new(FakeStore::default());
        let media = service(FakeRepository::default(), Some(store.clone())).await;
        let bytes = jpeg(100, 100);

        let mut bad_type = upload_request(&bytes);
        bad_type.mime_type = "text/html".into();
        let error = media
            .initiate_property_media_direct_upload(bad_type, &root_context())
            .await
            .expect_err("an image row that is not an image");
        assert_eq!(error.code(), "MEDIA_TYPE_INVALID");

        let mut bad_role = upload_request(&bytes);
        bad_role.role = "video".into();
        let error = media
            .initiate_property_media_direct_upload(bad_role, &root_context())
            .await
            .expect_err("a video role");
        assert_eq!(error.code(), "MEDIA_ROLE_INVALID");

        let mut empty = upload_request(&bytes);
        empty.byte_size = 0;
        let error = media
            .initiate_property_media_direct_upload(empty, &root_context())
            .await
            .expect_err("an empty declaration");
        assert_eq!(error.code(), "MEDIA_EMPTY");

        let mut too_large = upload_request(&bytes);
        too_large.byte_size = MAX_MEDIA_UPLOAD_BYTES as i64 + 1;
        let error = media
            .initiate_property_media_direct_upload(too_large, &root_context())
            .await
            .expect_err("an oversized declaration");
        assert_eq!(error.code(), "MEDIA_TOO_LARGE");

        let mut bad_sha = upload_request(&bytes);
        bad_sha.sha256 = "not-a-hash".into();
        let error = media
            .initiate_property_media_direct_upload(bad_sha, &root_context())
            .await
            .expect_err("a malformed SHA-256");
        assert_eq!(error.code(), "MEDIA_SHA256_INVALID");

        let mut bad_property = upload_request(&bytes);
        bad_property.property_id = "not-a-uuid".into();
        let error = media
            .initiate_property_media_direct_upload(bad_property, &root_context())
            .await
            .expect_err("a malformed property id");
        assert_eq!(error.code(), "PROPERTY_INVALID");

        assert_eq!(store.put_count(), 0, "no object was written");
        assert!(media.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn initiate_answers_done_for_a_photograph_the_property_already_shows() {
        let store = Arc::new(FakeStore::default());
        let media = service(
            FakeRepository {
                existing_photo_id: Some("11111111-1111-4111-8111-111111111111".into()),
                ..Default::default()
            },
            Some(store.clone()),
        )
        .await;

        let session = media
            .initiate_property_media_direct_upload(upload_request(&jpeg(400, 300)), &root_context())
            .await
            .expect("the retry is answered");

        assert_eq!(session.state, "done");
        assert_eq!(session.media_id, "11111111-1111-4111-8111-111111111111");
        assert!(session.upload_id.is_none());
        assert!(session.upload_url.is_none());
        assert_eq!(store.put_count(), 0);
        assert!(media.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn initiate_without_an_object_store_is_a_typed_failure() {
        let media = service(FakeRepository::default(), None).await;

        let error = media
            .initiate_property_media_direct_upload(upload_request(&jpeg(100, 100)), &root_context())
            .await
            .expect_err("no object store");

        assert_eq!(error.code(), "MEDIA_OBJECT_STORE_NOT_CONFIGURED");
    }

    #[tokio::test]
    async fn finalize_stores_the_original_and_copies_object_first_without_staging() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;
        let bytes = jpeg(800, 600);
        let (session, key) = initiate_and_put(&media, &store, &bytes, true).await;

        let stored = media
            .finalize_property_media_direct_upload(
                session.upload_id.as_deref().expect("the token"),
                &root_context(),
            )
            .await
            .expect("the direct upload is stored");

        assert!(!stored.skipped);
        assert_eq!(stored.storage, "object");
        assert_eq!(stored.role, "gallery");
        assert_eq!(stored.derivative_kinds, vec!["web", "card", "thumb"]);
        assert_eq!(stored.media_id, session.media_id);

        // The original is the browser's object (not written by the app); the three copies were written
        // by the service and verified, and nothing was deleted.
        assert_eq!(store.put_count(), 3);
        assert_eq!(store.delete_count(), 0);
        let original = store
            .object(ObjectBucket::PropertyMediaPublic, &key)
            .expect("the browser's original is still in the bucket");
        assert_eq!(original.sha, sha256_hex(&bytes));

        let saved = media.repository.saved_sets();
        assert_eq!(saved.len(), 1, "one transaction, one row set");
        let (property_id, role, rows) = &saved[0];
        assert_eq!(property_id, PROPERTY);
        assert_eq!(role, "gallery");
        assert_eq!(rows.len(), 4);
        let original_row = &rows[0];
        assert_eq!(original_row.id, session.media_id);
        assert!(original_row.derivative_of.is_none());
        assert_eq!(original_row.object.bucket, "property_media_public");
        assert_eq!(original_row.object.key, key);
        assert_eq!(original_row.object.visibility.as_deref(), Some("public"));
        assert_eq!(original_row.object.status, "VERIFIED");
        assert_eq!(
            original_row.object.sha256.as_deref(),
            Some(sha256_hex(&bytes).as_str())
        );
        assert_eq!(original_row.object.size, bytes.len() as i64);
        for row in &rows[1..] {
            assert_eq!(row.derivative_of.as_deref(), Some(original_row.id.as_str()));
            assert_eq!(row.media_type, "image");
            assert_eq!(row.mime_type, "image/jpeg");
            assert_eq!(row.object.status, "VERIFIED");
        }
    }

    #[tokio::test]
    async fn finalize_refuses_an_object_whose_sha256_does_not_match_the_declaration() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;
        let declared = jpeg(800, 600);
        let session = media
            .initiate_property_media_direct_upload(upload_request(&declared), &root_context())
            .await
            .expect("initiate");
        let key = key_for(&session);
        // Same length, different bytes: only the SHA-256 proof can catch this.
        let mut tampered = declared.clone();
        let middle = tampered.len() / 2;
        tampered[middle] ^= 0xff;
        assert_eq!(tampered.len(), declared.len());
        store.direct_put(
            ObjectBucket::PropertyMediaPublic,
            &key,
            &tampered,
            "image/jpeg",
        );

        let error = media
            .finalize_property_media_direct_upload(
                session.upload_id.as_deref().expect("the token"),
                &root_context(),
            )
            .await
            .expect_err("a tampered object must be refused");

        assert_eq!(error.code(), "MEDIA_DIRECT_SHA_MISMATCH");
        assert!(
            store
                .object(ObjectBucket::PropertyMediaPublic, &key)
                .is_none(),
            "the refused object must be deleted, not left as an orphan"
        );
        assert!(media.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn finalize_refuses_an_oversize_object() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;
        let declared = jpeg(400, 300);
        let session = media
            .initiate_property_media_direct_upload(upload_request(&declared), &root_context())
            .await
            .expect("initiate");
        let key = key_for(&session);
        // The PUT stored more bytes than the token declared.
        let mut bigger = declared.clone();
        bigger.extend_from_slice(b"trailing bytes the declaration never saw");
        store.direct_put(
            ObjectBucket::PropertyMediaPublic,
            &key,
            &bigger,
            "image/jpeg",
        );

        let error = media
            .finalize_property_media_direct_upload(
                session.upload_id.as_deref().expect("the token"),
                &root_context(),
            )
            .await
            .expect_err("an oversize object must be refused");

        assert_eq!(error.code(), "MEDIA_DIRECT_SIZE_MISMATCH");
        assert!(store
            .object(ObjectBucket::PropertyMediaPublic, &key)
            .is_none());
        assert!(media.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn finalize_refuses_a_content_type_that_differs_from_the_declaration() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;
        let bytes = jpeg(400, 300);
        let session = media
            .initiate_property_media_direct_upload(upload_request(&bytes), &root_context())
            .await
            .expect("initiate");
        let key = key_for(&session);
        store.direct_put(ObjectBucket::PropertyMediaPublic, &key, &bytes, "text/html");

        let error = media
            .finalize_property_media_direct_upload(
                session.upload_id.as_deref().expect("the token"),
                &root_context(),
            )
            .await
            .expect_err("a mismatched content type must be refused");

        assert_eq!(error.code(), "MEDIA_DIRECT_CONTENT_TYPE_MISMATCH");
        assert!(store
            .object(ObjectBucket::PropertyMediaPublic, &key)
            .is_none());
        assert!(media.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn finalize_refuses_when_the_put_never_happened() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;
        let session = media
            .initiate_property_media_direct_upload(upload_request(&jpeg(400, 300)), &root_context())
            .await
            .expect("initiate");

        let error = media
            .finalize_property_media_direct_upload(
                session.upload_id.as_deref().expect("the token"),
                &root_context(),
            )
            .await
            .expect_err("no object was ever stored");

        assert_eq!(error.code(), "MEDIA_DIRECT_OBJECT_MISSING");
        assert_eq!(store.put_count(), 0);
        assert!(media.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn finalize_refuses_a_forged_or_expired_token() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;
        let bytes = jpeg(200, 200);
        let session = media
            .initiate_property_media_direct_upload(upload_request(&bytes), &root_context())
            .await
            .expect("initiate");
        let token = session.upload_id.as_deref().expect("the token");

        let mut forged: Vec<u8> = token.as_bytes().to_vec();
        let last = forged.len() - 1;
        forged[last] = if forged[last] == b'a' { b'b' } else { b'a' };
        let forged = String::from_utf8(forged).expect("the token stays ascii");
        let error = media
            .finalize_property_media_direct_upload(&forged, &root_context())
            .await
            .expect_err("a forged signature must be refused");
        assert_eq!(error.code(), TOKEN_INVALID);

        let expired = encode_pending_token(
            TEST_DIRECT_UPLOAD_SECRET,
            &expired_claims(&session.media_id, &bytes, true),
        )
        .expect("the expired token encodes");
        let error = media
            .finalize_property_media_direct_upload(&expired, &root_context())
            .await
            .expect_err("an expired token must be refused");
        assert_eq!(error.code(), TOKEN_EXPIRED);
        assert!(media.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn finalize_refuses_a_token_minted_for_another_user() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;
        let bytes = jpeg(200, 200);
        let (session, _) = initiate_and_put(&media, &store, &bytes, true).await;

        let error = media
            .finalize_property_media_direct_upload(
                session.upload_id.as_deref().expect("the token"),
                &another_root_context(),
            )
            .await
            .expect_err("a token is bound to the user that minted it");

        assert_eq!(error.code(), "MEDIA_DIRECT_UPLOAD_TOKEN_ACTOR_MISMATCH");
        assert!(media.repository.saved_sets().is_empty());
        // The token is untouched, so the rightful user can still finalize it.
        assert!(store
            .object(ObjectBucket::PropertyMediaPublic, &key_for(&session))
            .is_some());
    }

    #[tokio::test]
    async fn a_retried_finalize_answers_skipped_and_removes_the_orphan_object() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let mut media = service(repository, Some(store.clone())).await;
        let bytes = jpeg(300, 200);
        let (session, key) = initiate_and_put(&media, &store, &bytes, true).await;
        // The row was recorded by the first finalize whose answer never arrived.
        media.repository.existing_photo_id = Some("22222222-2222-4222-8222-222222222222".into());

        let stored = media
            .finalize_property_media_direct_upload(
                session.upload_id.as_deref().expect("the token"),
                &root_context(),
            )
            .await
            .expect("the retry is answered");

        assert!(stored.skipped);
        assert_eq!(stored.media_id, "22222222-2222-4222-8222-222222222222");
        assert!(
            store
                .object(ObjectBucket::PropertyMediaPublic, &key)
                .is_none(),
            "the orphaned object is removed once the row exists"
        );
        assert!(media.repository.saved_sets().is_empty());
        assert_eq!(store.put_count(), 0);
    }

    #[tokio::test]
    async fn a_failed_row_write_deletes_the_original_and_every_derivative() {
        let repository = FakeRepository {
            public: true,
            fail_save: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;
        let bytes = jpeg(400, 300);
        let (session, _) = initiate_and_put(&media, &store, &bytes, true).await;

        let error = media
            .finalize_property_media_direct_upload(
                session.upload_id.as_deref().expect("the token"),
                &root_context(),
            )
            .await
            .expect_err("the transaction failed");

        assert_eq!(store.put_count(), 3);
        assert_eq!(
            store.delete_count(),
            4,
            "the browser's original and all three copies must be deleted when no row points at them"
        );
        assert!(media.repository.saved_sets().is_empty());
        assert!(
            error.to_string().contains("transaction failed"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn finalize_refuses_when_the_publication_bucket_changed_in_flight() {
        let repository = FakeRepository::default(); // private when the upload is initiated
        let store = Arc::new(FakeStore::default());
        let mut media = service(repository, Some(store.clone())).await;
        let bytes = jpeg(400, 300);
        let session = media
            .initiate_property_media_direct_upload(upload_request(&bytes), &root_context())
            .await
            .expect("initiate");
        let key = key_for(&session);
        store.direct_put(
            ObjectBucket::PropertyMediaPrivate,
            &key,
            &bytes,
            "image/jpeg",
        );
        // The listing was published between initiate and finalize: the object sits in the private bucket.
        media.repository.public = true;

        let error = media
            .finalize_property_media_direct_upload(
                session.upload_id.as_deref().expect("the token"),
                &root_context(),
            )
            .await
            .expect_err("the wrong publication bucket must be refused");

        assert_eq!(error.code(), "MEDIA_PUBLICATION_CHANGED");
        assert!(store
            .object(ObjectBucket::PropertyMediaPrivate, &key)
            .is_none());
        assert!(media.repository.saved_sets().is_empty());
    }

    #[tokio::test]
    async fn after_an_abort_the_rightful_user_can_abort_but_not_a_stranger() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;
        let bytes = jpeg(200, 200);
        let session = media
            .initiate_property_media_direct_upload(upload_request(&bytes), &root_context())
            .await
            .expect("initiate");
        let key = key_for(&session);
        store.direct_put(
            ObjectBucket::PropertyMediaPublic,
            &key,
            &bytes,
            "image/jpeg",
        );
        let token = session.upload_id.as_deref().expect("the token");

        let error = media
            .abort_property_media_direct_upload(token, &another_root_context())
            .await
            .expect_err("a stranger cannot clean up someone else's upload");
        assert_eq!(error.code(), "MEDIA_DIRECT_UPLOAD_TOKEN_ACTOR_MISMATCH");

        let aborted = media
            .abort_property_media_direct_upload(token, &root_context())
            .await
            .expect("the owner aborts");
        assert!(aborted.deleted);
        assert!(!aborted.finalized);
        assert!(store
            .object(ObjectBucket::PropertyMediaPublic, &key)
            .is_none());
    }

    #[tokio::test]
    async fn abort_never_deletes_a_finalized_photograph() {
        let repository = FakeRepository {
            public: true,
            ..Default::default()
        };
        let store = Arc::new(FakeStore::default());
        let mut media = service(repository, Some(store.clone())).await;
        let bytes = jpeg(300, 200);
        let (session, key) = initiate_and_put(&media, &store, &bytes, true).await;
        let token = session.upload_id.as_deref().expect("the token");
        media
            .finalize_property_media_direct_upload(token, &root_context())
            .await
            .expect("the upload is finalized");
        // The fake repository now reports the row this media id is recorded under.
        media.repository.location = Some(db::MediaObjectLocation {
            bucket: "property_media_public".into(),
            key: key.clone(),
            status: "VERIFIED".into(),
            visibility: "public".into(),
        });

        let aborted = media
            .abort_property_media_direct_upload(token, &root_context())
            .await
            .expect("the abort is answered");

        assert!(!aborted.deleted);
        assert!(aborted.finalized);
        assert!(
            store
                .object(ObjectBucket::PropertyMediaPublic, &key)
                .is_some(),
            "a finalized photograph is not an orphan"
        );
    }
}
