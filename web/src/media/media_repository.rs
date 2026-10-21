//! The media repository port, the object-first service helpers, and the object-only read resolver.
//!
//! The chunk-staging protocol is retired: there is no manifest, no staged piece and no assembly method here any
//! more. Uploads arrive whole (`/api/property-media/full`), direct to object storage
//! (`/api/property-media/direct/*`), through the legacy direct door, or as a standalone document, and every one of
//! them records OBJECT rows only. Reads are object-only unconditionally. This module no longer reads or writes
//! `media.file_data` at all.

#[allow(unused_imports)]
use super::*;
use services::{ServiceFailureClass, ServiceRuntimeError};

#[async_trait]
pub trait MediaRepository: Send + Sync {
    async fn for_property(&self, property_id: &str) -> DbResult<Vec<MediaAsset>>;
    async fn set_property_hero(&self, property_id: &str, media_id: &str) -> DbResult<bool>;
    async fn attach_property_video(
        &self,
        request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult>;

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

    /// Object-first standalone upload: one media row with an object pointer and no property link. The
    /// standalone door used to write `file_data`; there is no blob variant any more.
    async fn save_standalone_object(&self, _row: &db::MediaObjectRow) -> DbResult<()> {
        Err(db::DbFailure::configuration(
            "media.save_standalone_object",
            "object-first standalone uploads are not implemented by this repository",
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

    async fn save_standalone_object(&self, row: &db::MediaObjectRow) -> DbResult<()> {
        MediaDao::upload_standalone_object(self, row).await
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

/// What a read route should return for one media id. Property media reads are object-only and produce
/// `Redirect`; `Bytes` is retained solely for the out-of-scope route arms that still match it and is never
/// produced by this service any more.
#[derive(Debug)]
pub enum MediaRead {
    Bytes { mime_type: String, bytes: Vec<u8> },
    Redirect { url: String },
}

pub struct MediaService<R> {
    pub(super) repository: R,
    pub(super) runtime: ServiceRuntime,
    /// Present only when the media object store is configured. Property media reads are object-only: a
    /// deployment without the store cannot serve media, and says so with a typed failure.
    pub(super) object_store: Option<Arc<dyn AbstractObjectStoreService>>,
    /// The key that signs a direct-to-object upload's pending token. `None` means read it at request
    /// time from `MEDIA_DIRECT_UPLOAD_SECRET` (falling back to `AUTH_SECRET`); tests inject one so they
    /// never depend on process environment.
    pub(super) direct_upload_secret: Option<Vec<u8>>,
}

impl<R> MediaService<R> {
    /// Attach the object store after construction. Media reads and uploads require one: without it they fail
    /// with a typed configuration error rather than writing or serving a `file_data` blob.
    pub fn with_object_store(mut self, store: Arc<dyn AbstractObjectStoreService>) -> Self {
        self.object_store = Some(store);
        self
    }

    /// COMPATIBILITY NO-OP. Reads are object-only unconditionally now; `composition.rs` still passes the
    /// retired `MEDIA_OBJECT_STORAGE_READS` value, so the builder method stays (and ignores it) rather than
    /// forcing an out-of-scope change to the composition root. Do not add behavior back behind it.
    pub fn with_object_reads(self, _enabled: bool) -> Self {
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

    /// Resolve one media id to a redirect to the object store. OBJECT-ONLY, unconditionally.
    ///
    /// There is no legacy path and no flag: a row with no object, an object that is not VERIFIED, or an object
    /// whose read URL cannot be generated is a TYPED FAILURE. `media.file_data` is not read by this service at
    /// all — the only reason it still exists is the migration tool and the non-media doors.
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
            let Some(store) = self.object_store.as_ref() else {
                return Err(CoreServiceError::Runtime(ServiceRuntimeError::Router {
                    code: "MEDIA_OBJECT_STORE_NOT_CONFIGURED".into(),
                    message: "media reads require object storage, which is not configured".into(),
                    retryable: false,
                    class: ServiceFailureClass::Infrastructure,
                }));
            };
            let Some(location) = self.repository.media_object_location(id).await? else {
                return Err(CoreServiceError::business(
                    "MEDIA_OBJECT_NOT_FOUND",
                    "This media has no object-storage copy.",
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
            let url =
                if bucket == ObjectBucket::PropertyMediaPublic && location.visibility == "public" {
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
