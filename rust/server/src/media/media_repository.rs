//! Moved from `mod.rs` (move only): MediaRepository, media_bytes, BeginMediaUploadResult, MediaUploadProgress, PropertyVideoUploadSession, PropertyVideoFinalizeResult, UploadLookup, MediaService.

#[allow(unused_imports)]
use super::*;

#[async_trait]
pub trait MediaRepository: Send {
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
    async fn property_has_photo(&self, property_id: &str, filename: &str, byte_size: i64) -> DbResult<bool>;
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

    async fn property_has_photo(&self, property_id: &str, filename: &str, byte_size: i64) -> DbResult<bool> {
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
    Unfinished { upload_id: String, status: String, received: Vec<i32> },
    New,
}

pub struct MediaService<R> {
    pub(super) repository: R,
    pub(super) runtime: ServiceRuntime,
}
