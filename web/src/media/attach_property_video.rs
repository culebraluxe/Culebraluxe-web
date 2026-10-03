//! Moved from `mod.rs` (move only): attach_property_video.

#[allow(unused_imports)]
use super::*;

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

            self.repository
                .upload_property_media(&request)
                .await
                .map_err(Into::into)
        }
        .await;

        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }
}
