use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, MediaDao};
use domain::{
    sanitize_media_filename, MediaAsset, UploadPropertyMediaRequest, UploadPropertyMediaResult,
    MAX_MEDIA_UPLOAD_BYTES,
};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait MediaRepository: Send {
    async fn for_property(&mut self, property_id: &str) -> DbResult<Vec<MediaAsset>>;
    async fn upload_property_media(
        &mut self,
        request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult>;
}

#[async_trait]
impl MediaRepository for MediaDao {
    async fn for_property(&mut self, property_id: &str) -> DbResult<Vec<MediaAsset>> {
        MediaDao::for_property(self, property_id).await
    }

    async fn upload_property_media(
        &mut self,
        request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult> {
        MediaDao::upload_property_media(self, request).await
    }
}

pub struct MediaService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: MediaRepository> MediaService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn for_property(
        &mut self,
        property_id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<MediaAsset>, CoreServiceError> {
        const OP: &str = "media.forProperty";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .for_property(property_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    pub async fn upload_property_media(
        &mut self,
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
