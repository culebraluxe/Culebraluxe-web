//! The media repository port, the object-first service helpers, and the object-only read resolver.
//!
//! The chunk-staging protocol is retired: there is no manifest, no staged piece and no assembly method here any
//! more. Uploads arrive whole (`/api/property-media/full`) or direct to object storage
//! (`/api/property-media/direct/*`), and both record through `save_property_media_object_set`. The two remaining
//! legacy shapes — the blob reader `media_bytes` and the blob writer `upload_standalone` — are marked and are
//! the only `file_data` use left in this module.

#[allow(unused_imports)]
use super::*;
use services::{ServiceFailureClass, ServiceRuntimeError};

#[async_trait]
pub trait MediaRepository: Send + Sync {
    /// LEGACY BLOB READ. Only the `MEDIA_OBJECT_STORAGE_READS`-off branch of `media_read` calls this; the
    /// object-first path reads the pointer through `media_object_location`. It goes with `media.file_data`.
    async fn media_bytes(&self, id: &str) -> DbResult<Option<(String, Vec<u8>)>>;

    /// LEGACY BLOB WRITE. The standalone media door (a signed-contract PDF) still stores bytes in
    /// `media.file_data`; its object-first replacement is `MediaDao::upload_standalone_object`.
    async fn upload_standalone(
        &self,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> DbResult<(String, String, String, i64)>;

    async fn for_property(&self, property_id: &str) -> DbResult<Vec<MediaAsset>>;
    async fn set_property_hero(&self, property_id: &str, media_id: &str) -> DbResult<bool>;
    async fn attach_property_video(
        &self,
        request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult>;

    async fn upload_property_media(
        &self,
        request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult>;
    async fn remove_property_media(&self, property_id: &str, media_id: &str) -> DbResult<bool>;

    /// The STORAGE POINTER of the copy a reader should be served (the `web` derivative when the row is an
    /// original), without loading `file_data`. Test repositories return None.
    async fn media_object_location(
        &self,
        _media_id: &str,
    ) -> DbResult<Option<db::MediaObjectLocation>> {
        Ok(None)
    }

    /// Photograph id already attached to this property under this name and size (the upload retry guard).
    async fn property_photo_id(
        &self,
        _property_id: &str,
        _filename: &str,
        _byte_size: i64,
    ) -> DbResult<Option<String>> {
        Ok(None)
    }

    /// Whether the owning property is publicly visible under the listing rule. Test repositories
    /// default to private so a write can never publish an unpublished fixture.
    async fn property_is_public(&self, _property_id: &str) -> DbResult<bool> {
        Ok(false)
    }

    /// Object-first single-row upload (the legacy direct door): one media row plus the property link.
    /// Test repositories that do not need it keep the default refusal.
    async fn save_property_media_object(
        &self,
        _property_id: &str,
        _role: &str,
        _row: &db::MediaObjectRow,
    ) -> DbResult<UploadPropertyMediaResult> {
        Err(db::DbFailure::configuration(
            "media.save_property_media_object",
            "object-first uploads are not implemented by this repository",
        ))
    }

    /// Object-first upload: every row plus the property link in one transaction. The only row-creating
    /// writer for uploaded property media; no staging table is involved.
    async fn save_property_media_object_set(
        &self,
        _property_id: &str,
        _role: &str,
        _rows: &[db::MediaObjectRow],
    ) -> DbResult<UploadPropertyMediaResult> {
        Err(db::DbFailure::configuration(
            "media.save_property_media_object_set",
            "object-first uploads are not implemented by this repository",
        ))
    }
}

#[async_trait]
impl MediaRepository for MediaDao {
    async fn media_bytes(&self, id: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        let id = id.to_owned();
        db::retrying_read!(MediaDao::media_bytes(self, &id))
    }

    async fn upload_standalone(
        &self,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> DbResult<(String, String, String, i64)> {
        MediaDao::upload_standalone(self, filename, mime_type, bytes).await
    }

    async fn for_property(&self, property_id: &str) -> DbResult<Vec<MediaAsset>> {
        MediaDao::for_property(self, property_id).await
    }

    async fn set_property_hero(&self, property_id: &str, media_id: &str) -> DbResult<bool> {
        MediaDao::set_property_hero(self, property_id, media_id).await
    }

    async fn attach_property_video(
        &self,
        request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult> {
        MediaDao::attach_property_video(self, request).await
    }

    async fn remove_property_media(&self, property_id: &str, media_id: &str) -> DbResult<bool> {
        MediaDao::remove_property_media(self, property_id, media_id).await
    }

    async fn media_object_location(
        &self,
        media_id: &str,
    ) -> DbResult<Option<db::MediaObjectLocation>> {
        MediaDao::media_object_location(self, media_id).await
    }

    async fn property_photo_id(
        &self,
        property_id: &str,
        filename: &str,
        byte_size: i64,
    ) -> DbResult<Option<String>> {
        MediaDao::property_photo_id(self, property_id, filename, byte_size).await
    }

    async fn property_is_public(&self, property_id: &str) -> DbResult<bool> {
        MediaDao::property_is_public(self, property_id).await
    }

    async fn upload_property_media(
        &self,
        request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult> {
        MediaDao::upload_property_media(self, request).await
    }

    async fn save_property_media_object(
        &self,
        property_id: &str,
        role: &str,
        row: &db::MediaObjectRow,
    ) -> DbResult<UploadPropertyMediaResult> {
        MediaDao::save_property_media_object(self, property_id, role, row).await
    }

    async fn save_property_media_object_set(
        &self,
        property_id: &str,
        role: &str,
        rows: &[db::MediaObjectRow],
    ) -> DbResult<UploadPropertyMediaResult> {
        MediaDao::save_property_media_object_set(self, property_id, role, rows).await
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyVideoUploadSession {
    pub upload_id: String,
    pub upload_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyVideoFinalizeResult {
    pub status: String,
    pub attached: bool,
    pub media_id: Option<String>,
    pub mux_asset_id: Option<String>,
    pub mux_playback_id: Option<String>,
}

/// What a read route should return for one media id: legacy bytes, or a redirect to the object store.
/// The bytes variant is only reachable while `object_reads` is off; with the switch on, an
/// unaddressable object is an error instead.
#[derive(Debug)]
pub enum MediaRead {
    Bytes { mime_type: String, bytes: Vec<u8> },
    Redirect { url: String },
}

pub struct MediaService<R> {
    pub(super) repository: R,
    pub(super) runtime: ServiceRuntime,
    /// Present only when the media object store is configured. With `object_reads` off, shadow writes
    /// are additive and the Postgres blob stays canonical, so a store error never fails an upload.
    pub(super) object_store: Option<Arc<dyn AbstractObjectStoreService>>,
    pub(super) object_reads: bool,
    /// The key that signs a direct-to-object upload's pending token. `None` means read it at request
    /// time from `MEDIA_DIRECT_UPLOAD_SECRET` (falling back to `AUTH_SECRET`); tests inject one so they
    /// never depend on process environment.
    pub(super) direct_upload_secret: Option<Vec<u8>>,
}

impl<R> MediaService<R> {
    /// Attach the object store after construction. Tests and deployments without media object
    /// storage keep the exact legacy behavior.
    pub fn with_object_store(mut self, store: Arc<dyn AbstractObjectStoreService>) -> Self {
        self.object_store = Some(store);
        self
    }

    /// The read cutover switch. Default false, which keeps the legacy blob canonical and the read
    /// path exactly as it was. When true, object storage is the ONLY source: a missing, unverified or
    /// unaddressable object is a typed failure, never a fallback to `file_data`.
    pub fn with_object_reads(mut self, enabled: bool) -> Self {
        self.object_reads = enabled;
        self
    }

    /// Inject the key that signs direct-to-object upload tokens. Production leaves this unset and the
    /// key is read from the environment when a token is minted or checked; tests set it explicitly.
    pub fn with_direct_upload_secret(mut self, secret: impl Into<Vec<u8>>) -> Self {
        self.direct_upload_secret = Some(secret.into());
        self
    }
}

impl<R: MediaRepository> MediaService<R> {
    /// Upload one object, prove it is the bytes we meant to store, and return its metadata row.
    ///
    /// Shared by the direct and whole-file object-first paths. A put that cannot be verified is
    /// deleted here rather than left in the bucket: an unverified object is not a copy of anything, and the
    /// caller's failure cleanup only knows about rows that came back.
    pub(super) async fn object_row(
        &self,
        store: &Arc<dyn AbstractObjectStoreService>,
        bucket: ObjectBucket,
        visibility: &str,
        id: &str,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
        alt_text: Option<String>,
        media_type: &str,
        derivative_of: Option<String>,
        derivative_kind: Option<String>,
    ) -> Result<db::MediaObjectRow, String> {
        let extension = match mime_type.to_ascii_lowercase().as_str() {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            "image/gif" => "gif",
            "image/avif" => "avif",
            "application/pdf" => "pdf",
            _ => "bin",
        };
        let key = format!("media/{id}/original.{extension}");
        let stored = store
            .put_object(bucket, &key, bytes, mime_type)
            .await
            .map_err(|error| error.to_string())?;
        let sha = stored
            .sha256
            .clone()
            .ok_or_else(|| "provider returned no SHA-256".to_owned())?;
        if let Err(error) = store.verify_object(bucket, &key, &sha, stored.size).await {
            let _ = store.delete_object(bucket, &key).await;
            return Err(error.to_string());
        }
        Ok(db::MediaObjectRow {
            id: id.to_owned(),
            filename: filename.to_owned(),
            mime_type: mime_type.to_owned(),
            alt_text,
            media_type: media_type.to_owned(),
            derivative_of,
            derivative_kind,
            object: db::MediaObjectMetadata {
                bucket: bucket.as_str().to_owned(),
                key,
                etag: stored.etag,
                sha256: Some(sha),
                size: stored.size,
                status: "VERIFIED".to_owned(),
                visibility: Some(visibility.to_owned()),
            },
        })
    }

    pub(crate) async fn object_first_property_media(
        &self,
        property_id: &str,
        role: &str,
        filename: &str,
        mime_type: &str,
        alt_text: Option<String>,
        bytes: &[u8],
    ) -> Result<UploadPropertyMediaResult, String> {
        let store = self
            .object_store
            .as_ref()
            .ok_or_else(|| "object store is not configured".to_owned())?;
        let public = self
            .repository
            .property_is_public(property_id)
            .await
            .unwrap_or(false);
        let (bucket, visibility) = if public {
            (ObjectBucket::PropertyMediaPublic, "public")
        } else {
            (ObjectBucket::PropertyMediaPrivate, "private")
        };
        let id = uuid::Uuid::new_v4().to_string();
        let row = self
            .object_row(
                store, bucket, visibility, &id, filename, mime_type, bytes, alt_text, "image",
                None, None,
            )
            .await?;
        match self
            .repository
            .save_property_media_object(property_id, role, &row)
            .await
        {
            Ok(result) => Ok(result),
            Err(error) => {
                let _ = store.delete_object(bucket, &row.object.key).await;
                Err(error.to_string())
            }
        }
    }

    /// Resolve one media id to what a read route should send: a redirect to the object store, or the
    /// legacy blob.
    ///
    /// TWO MODES, ONE RULE. With `object_reads` OFF (the default) the legacy blob is canonical and this
    /// is unchanged. With it ON, the object store is the ONLY source: a row with no object, an object
    /// that is not VERIFIED, or an object whose read URL cannot be generated is a TYPED FAILURE. It is
    /// never a silent fall back to `file_data` — a fallback would hide a half-migrated row as a working
    /// read and keep the blob columns load-bearing for every reader, which is exactly what this switch
    /// exists to end.
    pub async fn media_read(
        &self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<Option<MediaRead>, CoreServiceError> {
        const OP: &str = "media.read";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = async {
            if !self.object_reads {
                let bytes = self.repository.media_bytes(id).await?;
                return Ok(bytes.map(|(mime_type, bytes)| MediaRead::Bytes { mime_type, bytes }));
            }

            let Some(store) = self.object_store.as_ref() else {
                return Err(CoreServiceError::Runtime(ServiceRuntimeError::Router {
                    code: "MEDIA_OBJECT_STORE_NOT_CONFIGURED".into(),
                    message: "media object reads are enabled but no object store is configured".into(),
                    retryable: false,
                    class: ServiceFailureClass::Infrastructure,
                }));
            };
            let Some(location) = self.repository.media_object_location(id).await? else {
                return Err(CoreServiceError::business(
                    "MEDIA_OBJECT_NOT_FOUND",
                    "This media has no object-storage copy, and the legacy blob reader is disabled.",
                ));
            };
            if location.status != "VERIFIED" {
                return Err(CoreServiceError::business(
                    "MEDIA_OBJECT_NOT_VERIFIED",
                    format!(
                        "This media's object is {}, not VERIFIED; refusing to serve it.",
                        location.status
                    ),
                ));
            }
            // Vault bytes have their own doors. A media row that points at the Vault bucket must not be
            // readable here, whatever the row's visibility column says.
            let Some(bucket) = ObjectBucket::from_db(&location.bucket) else {
                return Err(CoreServiceError::business(
                    "MEDIA_OBJECT_BUCKET_UNKNOWN",
                    "This media's object bucket is not a storage domain this reader serves.",
                ));
            };
            if bucket == ObjectBucket::Vault || location.visibility == "vault" {
                return Err(CoreServiceError::business(
                    "MEDIA_OBJECT_NOT_FOUND",
                    "Not found.",
                ));
            }
            let url = if bucket == ObjectBucket::PropertyMediaPublic
                && location.visibility == "public"
            {
                store.public_read_url(bucket, &location.key).await
            } else {
                store
                    .signed_read_url(bucket, &location.key, Duration::from_secs(300))
                    .await
            };
            match url {
                Ok(url) => Ok(Some(MediaRead::Redirect { url })),
                Err(error) => Err(CoreServiceError::infrastructure(
                    "MEDIA_OBJECT_READ_URL_FAILED",
                    error.to_string(),
                )),
            }
        }
        .await;

        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }
}
