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

    /// The storage pointer for a row, without loading `file_data`. Test repositories return None.
    async fn media_object_location(
        &self,
        _media_id: &str,
    ) -> DbResult<Option<db::MediaObjectLocation>> {
        Ok(None)
    }

    /// Whether the owning property is publicly visible under the listing rule. Test repositories
    /// default to private so a shadow write can never publish an unpublished fixture.
    async fn property_is_public(&self, _property_id: &str) -> DbResult<bool> {
        Ok(false)
    }

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

    async fn commit_media_object_upload(
        &self,
        _upload_id: &str,
        _property_id: &str,
        _role: &str,
        _rows: &[db::MediaObjectRow],
    ) -> DbResult<UploadPropertyMediaResult> {
        Err(db::DbFailure::configuration(
            "media.commit_media_object_upload",
            "object-first uploads are not implemented by this repository",
        ))
    }

    async fn upload_standalone_object(&self, _row: &db::MediaObjectRow) -> DbResult<()> {
        Err(db::DbFailure::configuration(
            "media.upload_standalone_object",
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

    async fn media_object_location(
        &self,
        media_id: &str,
    ) -> DbResult<Option<db::MediaObjectLocation>> {
        MediaDao::media_object_location(self, media_id).await
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

    async fn commit_media_object_upload(
        &self,
        upload_id: &str,
        property_id: &str,
        role: &str,
        rows: &[db::MediaObjectRow],
    ) -> DbResult<UploadPropertyMediaResult> {
        MediaDao::commit_media_object_upload(self, upload_id, property_id, role, rows).await
    }

    async fn upload_standalone_object(&self, row: &db::MediaObjectRow) -> DbResult<()> {
        MediaDao::upload_standalone_object(self, row).await
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

/// What a read route should return for one media id: legacy bytes, or a redirect to the object store.
pub enum MediaRead {
    Bytes { mime_type: String, bytes: Vec<u8> },
    Redirect { url: String },
}

pub struct MediaService<R> {
    pub(super) repository: R,
    pub(super) runtime: ServiceRuntime,
    /// Present only when the media object store is configured. Shadow writes are additive: the
    /// Postgres blob is still canonical until the Phase 6 read switch, so a store error never fails
    /// an upload.
    pub(super) object_store: Option<Arc<dyn AbstractObjectStoreService>>,
    pub(super) object_reads: bool,
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

    /// Phase 4 switch. Default false; even when true, a row must be VERIFIED and the object URL
    /// must be generated successfully, otherwise the legacy blob is served.
    pub fn with_object_reads(mut self, enabled: bool) -> Self {
        self.object_reads = enabled;
        self
    }
}

impl<R: MediaRepository> MediaService<R> {
    async fn object_row(
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
        store
            .verify_object(bucket, &key, &sha, stored.size)
            .await
            .map_err(|error| error.to_string())?;
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
                store,
                bucket,
                visibility,
                &id,
                filename,
                mime_type,
                bytes,
                alt_text,
                "image",
                None,
                None,
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

    pub(crate) async fn object_first_chunk(
        &self,
        upload_id: &str,
        assembly: &db::MediaUploadAssembly,
        derivatives: &[db::MediaDerivativeInput],
    ) -> Result<UploadPropertyMediaResult, String> {
        let store = self
            .object_store
            .as_ref()
            .ok_or_else(|| "object store is not configured".to_owned())?;
        let public = self
            .repository
            .property_is_public(&assembly.property_id)
            .await
            .unwrap_or(false);
        let (bucket, visibility) = if public {
            (ObjectBucket::PropertyMediaPublic, "public")
        } else {
            (ObjectBucket::PropertyMediaPrivate, "private")
        };
        let original_id = uuid::Uuid::new_v4().to_string();
        let original = self
            .object_row(
                store,
                bucket,
                visibility,
                &original_id,
                &assembly.filename,
                &assembly.mime_type,
                &assembly.bytes,
                assembly.alt_text.clone(),
                "image",
                None,
                None,
            )
            .await?;
        let mut rows = vec![original];
        for derivative in derivatives {
            let id = uuid::Uuid::new_v4().to_string();
            let row = self
                .object_row(
                    store,
                    bucket,
                    visibility,
                    &id,
                    &format!("{}-{}.jpg", assembly.filename.trim_end_matches(".jpg"), derivative.kind),
                    "image/jpeg",
                    &derivative.bytes,
                    None,
                    "image",
                    Some(original_id.clone()),
                    Some(derivative.kind.to_owned()),
                )
                .await?;
            rows.push(row);
        }
        match self
            .repository
            .commit_media_object_upload(
                upload_id,
                &assembly.property_id,
                &assembly.role,
                &rows,
            )
            .await
        {
            Ok(result) => Ok(result),
            Err(error) => {
                for row in &rows {
                    let _ = store.delete_object(bucket, &row.object.key).await;
                }
                Err(error.to_string())
            }
        }
    }

    /// Phase 4 resolver:impl<R: MediaRepository> MediaService<R> {
    /// Phase 4 resolver: verified object first, legacy bytes as the always-available fallback.
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
            if self.object_reads {
                if let Some(store) = self.object_store.as_ref() {
                    if let Some(location) = self.repository.media_object_location(id).await? {
                        if location.status == "VERIFIED" && location.visibility != "vault" {
                            let bucket = ObjectBucket::from_db(&location.bucket);
                            if let Some(bucket) = bucket.filter(|bucket| *bucket != ObjectBucket::Vault) {
                                let url = if bucket == ObjectBucket::PropertyMediaPublic
                                    && location.visibility == "public"
                                {
                                    store.public_read_url(bucket, &location.key).await
                                } else {
                                    store
                                        .signed_read_url(
                                            bucket,
                                            &location.key,
                                            Duration::from_secs(300),
                                        )
                                        .await
                                };
                                match url {
                                    Ok(url) => return Ok(Some(MediaRead::Redirect { url })),
                                    Err(error) => tracing::warn!(
                                        media_id = id,
                                        %error,
                                        "media object read URL failed; falling back to legacy blob"
                                    ),
                                }
                            }
                        }
                    }
                }
            }
            let bytes = self.repository.media_bytes(id).await?;
            Ok(bytes.map(|(mime_type, bytes)| MediaRead::Bytes { mime_type, bytes }))
        }
        .await;

        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

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
        let public = self
            .repository
            .property_is_public(property_id)
            .await
            .unwrap_or(false);
        let bucket = if public {
            ObjectBucket::PropertyMediaPublic
        } else {
            ObjectBucket::PropertyMediaPrivate
        };
        let visibility = if public { "public" } else { "private" };
        let key = media_object_key(property_id, media_id, mime_type);
        let pending = db::MediaObjectMetadata {
            bucket: bucket.as_str().to_owned(),
            key: key.clone(),
            etag: None,
            sha256: None,
            size: bytes.len() as i64,
            status: "MIRROR_PENDING".to_owned(),
            visibility: Some(visibility.to_owned()),
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
                    visibility: Some(visibility.to_owned()),
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
                    visibility: Some(visibility.to_owned()),
                };
                let _ = self.repository.set_media_object(media_id, &failed).await;
                let _ = store.delete_object(bucket, &key).await;
            }
        }
    }
}
