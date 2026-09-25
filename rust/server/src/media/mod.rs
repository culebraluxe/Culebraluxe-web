pub mod imaging;

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, MediaDao};
use domain::{
    sanitize_media_filename, AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset,
    UploadPropertyMediaRequest, UploadPropertyMediaResult, MAX_MEDIA_UPLOAD_BYTES,
};
use integrations::mux::MuxClient;
use serde::Serialize;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait MediaRepository: Send {
    async fn for_property(&mut self, property_id: &str) -> DbResult<Vec<MediaAsset>>;
    async fn upload_property_media(
        &mut self,
        request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult>;
    async fn attach_property_video(
        &mut self,
        request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult>;
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

    async fn attach_property_video(
        &mut self,
        request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult> {
        MediaDao::attach_property_video(self, request).await
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

    pub async fn create_property_video_upload(
        &mut self,
        mux: &MuxClient,
        cors_origin: &str,
        context: &ServiceContext,
    ) -> Result<PropertyVideoUploadSession, CoreServiceError> {
        const OP: &str = "media.createPropertyVideoUpload";
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
            let origin = cors_origin.trim();
            if origin.is_empty()
                || !(origin.starts_with("https://") || origin.starts_with("http://"))
            {
                return Err(CoreServiceError::business(
                    "MUX_CORS_ORIGIN_INVALID",
                    "A valid browser origin is required for video upload.",
                ));
            }

            let upload = mux
                .create_direct_upload(origin)
                .await
                .map_err(|error| CoreServiceError::business("MUX_API", error.message))?;
            let upload_url = upload.url.ok_or_else(|| {
                CoreServiceError::business(
                    "MUX_UPLOAD_URL_MISSING",
                    "Mux did not return a Direct Upload URL.",
                )
            })?;

            Ok(PropertyVideoUploadSession {
                upload_id: upload.id,
                upload_url,
            })
        }
        .await;

        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    pub async fn finalize_property_video_upload(
        &mut self,
        mux: &MuxClient,
        property_id: &str,
        upload_id: &str,
        role: &str,
        caption: Option<String>,
        context: &ServiceContext,
    ) -> Result<PropertyVideoFinalizeResult, CoreServiceError> {
        const OP: &str = "media.finalizePropertyVideoUpload";
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
            let property_id = property_id.trim();
            let upload_id = upload_id.trim();
            let role = role.trim();

            if property_id.is_empty() || upload_id.is_empty() {
                return Err(CoreServiceError::business(
                    "VIDEO_UPLOAD_REQUIRED",
                    "Property and Mux upload IDs are required.",
                ));
            }
            if !matches!(role, "video" | "short") {
                return Err(CoreServiceError::business(
                    "MEDIA_ROLE_INVALID",
                    "Video role must be video or short.",
                ));
            }

            let upload = mux
                .direct_upload(upload_id)
                .await
                .map_err(|error| CoreServiceError::business("MUX_API", error.message))?;

            match upload.status.as_str() {
                "waiting" => {
                    return Ok(PropertyVideoFinalizeResult {
                        status: "waiting".into(),
                        attached: false,
                        media_id: None,
                        mux_asset_id: None,
                        mux_playback_id: None,
                    });
                }
                "errored" | "cancelled" | "timed_out" => {
                    return Err(CoreServiceError::business(
                        "MUX_UPLOAD_FAILED",
                        upload
                            .error_message
                            .unwrap_or_else(|| format!("Mux upload {}.", upload.status)),
                    ));
                }
                "asset_created" => {}
                other => {
                    return Ok(PropertyVideoFinalizeResult {
                        status: other.to_owned(),
                        attached: false,
                        media_id: None,
                        mux_asset_id: None,
                        mux_playback_id: None,
                    });
                }
            }

            let asset_id = upload.asset_id.ok_or_else(|| {
                CoreServiceError::business(
                    "MUX_ASSET_ID_MISSING",
                    "Mux upload completed without an asset ID.",
                )
            })?;
            let asset = mux
                .asset(&asset_id)
                .await
                .map_err(|error| CoreServiceError::business("MUX_API", error.message))?;

            match asset.status.as_str() {
                "preparing" => {
                    return Ok(PropertyVideoFinalizeResult {
                        status: "preparing".into(),
                        attached: false,
                        media_id: None,
                        mux_asset_id: Some(asset.id),
                        mux_playback_id: None,
                    });
                }
                "errored" => {
                    return Err(CoreServiceError::business(
                        "MUX_ASSET_FAILED",
                        "Mux could not prepare this video.",
                    ));
                }
                "ready" => {}
                other => {
                    return Ok(PropertyVideoFinalizeResult {
                        status: other.to_owned(),
                        attached: false,
                        media_id: None,
                        mux_asset_id: Some(asset.id),
                        mux_playback_id: None,
                    });
                }
            }

            let playback_id = asset
                .playback_ids
                .iter()
                .find(|item| item.policy == "public")
                .map(|item| item.id.clone())
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "MUX_PLAYBACK_ID_MISSING",
                        "Mux asset has no public playback ID.",
                    )
                })?;

            let attached = self
                .repository
                .attach_property_video(&AttachPropertyVideoRequest {
                    property_id: property_id.to_owned(),
                    role: role.to_owned(),
                    mux_asset_id: asset.id.clone(),
                    mux_playback_id: playback_id.clone(),
                    duration_seconds: asset.duration_seconds,
                    aspect_ratio: asset.aspect_ratio,
                    caption: caption
                        .map(|value| value.trim().to_owned())
                        .filter(|value| !value.is_empty()),
                })
                .await
                .map_err(CoreServiceError::from)?;

            Ok(PropertyVideoFinalizeResult {
                status: "ready".into(),
                attached: true,
                media_id: Some(attached.media_id),
                mux_asset_id: Some(attached.mux_asset_id),
                mux_playback_id: Some(attached.mux_playback_id),
            })
        }
        .await;

        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
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

    pub async fn attach_property_video(
        &mut self,
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
