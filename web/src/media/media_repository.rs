//! Moved from `mod.rs` (move only): MediaRepository, media_bytes, BeginMediaUploadResult, MediaUploadProgress, PropertyVideoUploadSession, PropertyVideoFinalizeResult, UploadLookup, MediaService.

#[allow(unused_imports)]
use super::*;

#[async_trait]
pub trait MediaRepository: Send + Sync {
    async fn media_bytes(&self, id: &str) -> DbResult<Option<(String, Vec<u8>)>>;
    async fn upload_standalone(
        &self,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> DbResult<(String, String, String, i64)>;
    async fn for_property(&self, property_id: &str) -> DbResult<Vec<MediaAsset>>;
    async fn set_property_hero(&self, property_id: &str, media_id: &str) -> DbResult<bool>;
    async fn upload_property_media(
        &self,
        request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult>;
    async fn attach_property_video(
        &self,
        request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult>;
    async fn begin_media_upload(
        &self,
        request: &db::BeginMediaUpload,
    ) -> DbResult<BeginMediaUploadResult>;
    async fn stage_media_chunk(
        &self,
        upload_id: &str,
        chunk_index: i32,
        bytes: &[u8],
    ) -> DbResult<db::MediaUploadStatus>;
    async fn remove_property_media(&self, property_id: &str, media_id: &str) -> DbResult<bool>;
    async fn property_has_photo(
        &self,
        property_id: &str,
        filename: &str,
        byte_size: i64,
    ) -> DbResult<bool>;
    async fn resumable_media_upload(
        &self,
        property_id: &str,
        filename: &str,
        byte_size: i64,
        chunk_size: i32,
    ) -> DbResult<Option<(String, String, Vec<i32>)>>;
    async fn claim_media_upload(&self, upload_id: &str) -> DbResult<bool>;
    async fn fail_media_upload(&self, upload_id: &str) -> DbResult<()>;
    async fn media_upload_state(&self, upload_id: &str) -> DbResult<Option<String>>;
    async fn assemble_media_upload(&self, upload_id: &str) -> DbResult<db::MediaUploadAssembly>;
    async fn commit_media_upload(
        &self,
        upload_id: &str,
        assembly: &db::MediaUploadAssembly,
        derivatives: &[db::MediaDerivativeInput],
    ) -> DbResult<UploadPropertyMediaResult>;

    /// Persist shadow-write metadata. The default keeps test repositories source-compatible; the
    /// production DAO overrides it. A false result means the media row no longer exists.
    async fn set_media_object(
        &self,
        _media_id: &str,
        _metadata: &db::MediaObjectMetadata,
    ) -> DbResult<bool> {
        Ok(false)
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

    async fn upload_property_media(
        &self,
        request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult> {
        MediaDao::upload_property_media(self, request).await
    }

    async fn attach_property_video(
        &self,
        request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult> {
        MediaDao::attach_property_video(self, request).await
    }

    async fn begin_media_upload(
        &self,
        request: &db::BeginMediaUpload,
    ) -> DbResult<BeginMediaUploadResult> {
        let status = MediaDao::begin_media_upload(self, request).await?;
        Ok(BeginMediaUploadResult {
            upload_id: status.upload_id,
            chunk_size: request.chunk_size,
            chunk_count: status.chunk_count,
            byte_size: status.byte_size,
        })
    }

    async fn stage_media_chunk(
        &self,
        upload_id: &str,
        chunk_index: i32,
        bytes: &[u8],
    ) -> DbResult<db::MediaUploadStatus> {
        MediaDao::stage_media_chunk(self, upload_id, chunk_index, bytes).await
    }

    async fn assemble_media_upload(&self, upload_id: &str) -> DbResult<db::MediaUploadAssembly> {
        MediaDao::assemble_media_upload(self, upload_id).await
    }

    async fn remove_property_media(&self, property_id: &str, media_id: &str) -> DbResult<bool> {
        MediaDao::remove_property_media(self, property_id, media_id).await
    }

    async fn property_has_photo(
        &self,
        property_id: &str,
        filename: &str,
        byte_size: i64,
    ) -> DbResult<bool> {
        MediaDao::property_has_photo(self, property_id, filename, byte_size).await
    }

    async fn resumable_media_upload(
        &self,
        property_id: &str,
        filename: &str,
        byte_size: i64,
        chunk_size: i32,
    ) -> DbResult<Option<(String, String, Vec<i32>)>> {
        MediaDao::resumable_media_upload(self, property_id, filename, byte_size, chunk_size).await
    }

    async fn claim_media_upload(&self, upload_id: &str) -> DbResult<bool> {
        MediaDao::claim_media_upload(self, upload_id).await
    }

    async fn fail_media_upload(&self, upload_id: &str) -> DbResult<()> {
        MediaDao::fail_media_upload(self, upload_id).await
    }

    async fn media_upload_state(&self, upload_id: &str) -> DbResult<Option<String>> {
        MediaDao::media_upload_state(self, upload_id).await
    }

    async fn commit_media_upload(
        &self,
        upload_id: &str,
        assembly: &db::MediaUploadAssembly,
        derivatives: &[db::MediaDerivativeInput],
    ) -> DbResult<UploadPropertyMediaResult> {
        MediaDao::commit_media_upload(self, upload_id, assembly, derivatives).await
    }

    async fn set_media_object(
        &self,
        media_id: &str,
        metadata: &db::MediaObjectMetadata,
    ) -> DbResult<bool> {
        MediaDao::set_media_object(self, media_id, metadata).await
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginMediaUploadResult {
    pub upload_id: String,
    pub chunk_size: i32,
    pub chunk_count: i32,
    pub byte_size: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaUploadProgress {
    pub received_chunks: i32,
    pub chunk_count: i32,
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

/// What the server already has of a file about to be uploaded.
#[derive(Debug, Clone, Serialize)]
pub enum UploadLookup {
    /// The property already shows this file: nothing to send.
    AlreadyStored,
    /// An earlier try left this upload: send only the chunks it lacks (`status` `complete` means it is being finished).
    Unfinished {
        upload_id: String,
        status: String,
        received: Vec<i32>,
    },
    New,
}

pub struct MediaService<R> {
    pub(super) repository: R,
    pub(super) runtime: ServiceRuntime,
    /// Present only when the media object store is configured. Shadow writes are additive: the
    /// Postgres blob is still canonical until the Phase 6 read switch, so a store error never fails
    /// an upload.
    pub(super) object_store: Option<Arc<dyn AbstractObjectStoreService>>,
}

/// Object keys are opaque and UUID-oriented. The extension is for operator readability only; the
/// media row, not the key, remains the source of truth.
fn media_object_key(property_id: &str, media_id: &str, mime_type: &str) -> String {
    let extension = match mime_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/avif" => "avif",
        _ => "bin",
    };
    format!("properties/{property_id}/media/{media_id}/original.{extension}")
}

impl<R> MediaService<R> {
    /// Attach the object store after construction. Tests and deployments without media object
    /// storage keep the exact legacy behavior.
    pub fn with_object_store(mut self, store: Arc<dyn AbstractObjectStoreService>) -> Self {
        self.object_store = Some(store);
        self
    }
}

impl<R: MediaRepository> MediaService<R> {
    /// Best-effort shadow write: the Postgres blob was just committed and remains canonical. Success
    /// records VERIFIED object metadata; any failure records MIGRATION_FAILED and leaves the upload
    /// successful. This is deliberately not in the error path of the caller.
    pub(crate) async fn shadow_property_media(
        &self,
        media_id: &str,
        property_id: &str,
        bytes: &[u8],
        mime_type: &str,
    ) {
        let Some(store) = self.object_store.as_ref() else {
            return;
        };
        let bucket = ObjectBucket::PropertyMediaPrivate;
        let key = media_object_key(property_id, media_id, mime_type);
        let pending = db::MediaObjectMetadata {
            bucket: bucket.as_str().to_owned(),
            key: key.clone(),
            etag: None,
            sha256: None,
            size: bytes.len() as i64,
            status: "MIRROR_PENDING".to_owned(),
        };
        if let Err(error) = self.repository.set_media_object(media_id, &pending).await {
            tracing::warn!(
                media_id,
                error = %error,
                "media object shadow: could not record MIRROR_PENDING; keeping legacy blob"
            );
            return;
        }

        let result = async {
            let stored = store
                .put_object(bucket, &key, bytes, mime_type)
                .await
                .map_err(|error| error.to_string())?;
            let sha = stored
                .sha256
                .clone()
                .ok_or_else(|| "provider returned no SHA-256".to_owned())?;
            store
                .verify_object(bucket, &key, &sha, stored.size)
                .await
                .map_err(|error| error.to_string())?;
            Ok::<_, String>((stored, sha))
        }
        .await;

        match result {
            Ok((stored, sha)) => {
                let verified = db::MediaObjectMetadata {
                    bucket: bucket.as_str().to_owned(),
                    key: key.clone(),
                    etag: stored.etag.clone(),
                    sha256: Some(sha),
                    size: stored.size,
                    status: "VERIFIED".to_owned(),
                };
                if let Err(error) = self.repository.set_media_object(media_id, &verified).await {
                    tracing::warn!(media_id, error = %error, "media object shadow: could not record VERIFIED");
                }
            }
            Err(error) => {
                tracing::warn!(media_id, key, error, "media object shadow failed; legacy blob remains canonical");
                let failed = db::MediaObjectMetadata {
                    bucket: bucket.as_str().to_owned(),
                    key: key.clone(),
                    etag: None,
                    sha256: None,
                    size: bytes.len() as i64,
                    status: "MIGRATION_FAILED".to_owned(),
                };
                let _ = self.repository.set_media_object(media_id, &failed).await;
                let _ = store.delete_object(bucket, &key).await;
            }
        }
    }
}
