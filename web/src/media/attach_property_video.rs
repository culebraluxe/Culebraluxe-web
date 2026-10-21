//! Moved from `mod.rs` (move only): attach_property_video.

#[allow(unused_imports)]
use super::*;
use services::{ServiceFailureClass, ServiceRuntimeError};

impl<R: MediaRepository> MediaService<R> {
    pub async fn attach_property_video(
        &self,
        mut request: AttachPropertyVideoRequest,
        context: &ServiceContext,
    ) -> Result<AttachPropertyVideoResult, CoreServiceError> {
        const OP: &str = "media.attachPropertyVideo";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            request.property_id = request.property_id.trim().to_owned();
            request.role = request.role.trim().to_owned();
            request.mux_asset_id = request.mux_asset_id.trim().to_owned();
            request.mux_playback_id = request.mux_playback_id.trim().to_owned();
            request.duration_seconds = request
                .duration_seconds
                .take()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
            request.aspect_ratio = request
                .aspect_ratio
                .take()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
            request.caption = request
                .caption
                .take()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());

            if request.property_id.is_empty() {
                return Err(CoreServiceError::business(
                    "PROPERTY_REQUIRED",
                    "Property is required.",
                ));
            }
            if !matches!(request.role.as_str(), "video" | "short") {
                return Err(CoreServiceError::business(
                    "MEDIA_ROLE_INVALID",
                    "Video role must be video or short.",
                ));
            }
            if request.mux_asset_id.is_empty() || request.mux_playback_id.is_empty() {
                return Err(CoreServiceError::business(
                    "MUX_ID_REQUIRED",
                    "Mux asset and playback IDs are required.",
                ));
            }

            self.repository
                .attach_property_video(&request)
                .await
                .map_err(Into::into)
        }
        .await;

        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    pub async fn upload_property_media(
        &self,
        mut request: UploadPropertyMediaRequest,
        context: &ServiceContext,
    ) -> Result<UploadPropertyMediaResult, CoreServiceError> {
        const OP: &str = "media.uploadPropertyMedia";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            if request.property_id.trim().is_empty() {
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
            if !request.mime_type.to_ascii_lowercase().starts_with("image/") {
                return Err(CoreServiceError::business(
                    "MEDIA_TYPE_INVALID",
                    "Only image uploads are supported.",
                ));
            }

            request.filename = sanitize_media_filename(&request.filename);
            request.alt_text = request
                .alt_text
                .take()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());

            // OBJECT-FIRST ONLY. The legacy blob write is gone: without the store this door fails with a typed
            // configuration error instead of writing a `file_data` row no object-first reader could serve.
            if self.object_store.is_none() {
                return Err(CoreServiceError::Runtime(ServiceRuntimeError::Router {
                    code: "MEDIA_OBJECT_STORE_NOT_CONFIGURED".into(),
                    message: "the property upload path requires media object storage".into(),
                    retryable: false,
                    class: ServiceFailureClass::Infrastructure,
                }));
            }
            self.object_first_property_media(
                &request.property_id,
                &request.role,
                &request.filename,
                &request.mime_type,
                request.alt_text.clone(),
                &request.bytes,
            )
            .await
            .map_err(|message| {
                CoreServiceError::infrastructure("MEDIA_OBJECT_UPLOAD_FAILED", message)
            })
        }
        .await;

        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::test_support::{service, FakeRepository, FakeStore};
    use std::sync::Arc;

    fn root_context() -> ServiceContext {
        crate::media::test_support::root_context()
    }

    fn request() -> UploadPropertyMediaRequest {
        UploadPropertyMediaRequest {
            property_id: "0b0e0f56-89a1-4c4f-9b2b-6a1f0a1e0001".into(),
            role: "gallery".into(),
            filename: "front.jpg".into(),
            mime_type: "image/jpeg".into(),
            alt_text: None,
            bytes: b"original-bytes".to_vec(),
        }
    }

    /// The legacy direct door is OBJECT-FIRST ONLY: one object row plus the property link, never a blob.
    #[tokio::test]
    async fn the_legacy_property_door_writes_object_rows_only() {
        let repository = FakeRepository::default();
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;

        let stored = media
            .upload_property_media(request(), &root_context())
            .await
            .expect("the photograph is stored");

        assert_eq!(stored.role, "gallery");
        let rows = media.repository.single_rows();
        assert_eq!(rows.len(), 1, "one object-first row and the link");
        let (property_id, role, row) = &rows[0];
        assert_eq!(property_id, "0b0e0f56-89a1-4c4f-9b2b-6a1f0a1e0001");
        assert_eq!(role, "gallery");
        assert_eq!(row.object.bucket, "property_media_private");
        assert_eq!(row.object.status, "VERIFIED");
        assert_eq!(store.put_count(), 1);
    }

    #[tokio::test]
    async fn the_legacy_property_door_refuses_without_an_object_store() {
        let media = service(FakeRepository::default(), None).await;
        let error = media
            .upload_property_media(request(), &root_context())
            .await
            .expect_err("no object store");
        assert_eq!(error.code(), "MEDIA_OBJECT_STORE_NOT_CONFIGURED");
    }
}
