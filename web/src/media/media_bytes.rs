//! Moved from `mod.rs` (move only): media_bytes.

#[allow(unused_imports)]
use super::*;

impl<R: MediaRepository> MediaService<R> {
    /// LEGACY BLOB READ. The servable bytes straight from `file_data`.
    ///
    /// Nothing routes here any more (the private media door is `media_read`, which redirects to object storage
    /// when the read switch is on, and the public door belongs to `PublicListingService`). It is kept because
    /// service-level callers and the entitlement tests name `media.bytes`; the object-first replacement is
    /// `media_read` or `media_object_location` + `AbstractObjectStoreService::get_object`.
    pub async fn media_bytes(
        &self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<Option<(String, Vec<u8>)>, CoreServiceError> {
        const OP: &str = "media.bytes";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.media_bytes(id).await.map_err(Into::into);
        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    pub async fn upload_standalone(
        &self,
        filename: &str,
        mime_type: &str,
        bytes: Vec<u8>,
        context: &ServiceContext,
    ) -> Result<(String, String, String, i64), CoreServiceError> {
        const OP: &str = "media.uploadStandalone";
        let decision = authorize(
            &self.runtime,
            "media",
            "listing.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            if bytes.is_empty() {
                return Err(CoreServiceError::business(
                    "MEDIA_EMPTY",
                    "Image file is required.",
                ));
            }
            if bytes.len() > MAX_MEDIA_UPLOAD_BYTES {
                return Err(CoreServiceError::business(
                    "MEDIA_TOO_LARGE",
                    "Image is too large (max 50 MB).",
                ));
            }
            if !mime_type.to_ascii_lowercase().starts_with("image/") {
                return Err(CoreServiceError::business(
                    "MEDIA_TYPE_INVALID",
                    "Only image uploads are supported.",
                ));
            }
            let filename = sanitize_media_filename(filename);
            self.repository
                .upload_standalone(&filename, mime_type, &bytes)
                .await
                .map_err(Into::into)
        }
        .await;

        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
            object_store: None,
            object_reads: false,
            direct_upload_secret: None,
        }
    }

    pub async fn create_property_video_upload(
        &self,
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
                .map_err(|error| CoreServiceError::infrastructure("MUX_API", error.message))?;
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
        &self,
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
                .map_err(|error| CoreServiceError::infrastructure("MUX_API", error.message))?;

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
                .map_err(|error| CoreServiceError::infrastructure("MUX_API", error.message))?;

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

    /// Takes a photograph off a property (and deletes it, with its copies, unless another property shows it).
    pub async fn remove_property_media(
        &self,
        property_id: &str,
        media_id: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "media.removePropertyMedia";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = match self
            .repository
            .remove_property_media(property_id.trim(), media_id.trim())
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(CoreServiceError::business(
                "MEDIA_NOT_ON_PROPERTY",
                "That photo is not on this property.",
            )),
            Err(error) => Err(error.into()),
        };
        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    /// Make one of the property's photographs its hero (the previous hero returns to the gallery).
    pub async fn set_property_hero(
        &self,
        property_id: &str,
        media_id: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "media.setPropertyHero";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = match self
            .repository
            .set_property_hero(property_id.trim(), media_id.trim())
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(CoreServiceError::business(
                "MEDIA_NOT_ON_PROPERTY",
                "That photo is not on this property.",
            )),
            Err(error) => Err(error.into()),
        };
        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    pub async fn for_property(
        &self,
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
}
