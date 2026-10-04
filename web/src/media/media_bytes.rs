//! Moved from `mod.rs` (move only): media_bytes.

#[allow(unused_imports)]
use super::*;

impl<R: MediaRepository> MediaService<R> {
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

    /// Opens a chunked upload. The browser calls this before it sends any bytes, so the destination Property, the
    /// role and the declared size are all known by the time the first chunk lands.
    pub async fn begin_media_upload(
        &self,
        request: db::BeginMediaUpload,
        context: &ServiceContext,
    ) -> Result<BeginMediaUploadResult, CoreServiceError> {
        const OP: &str = "media.beginMediaUpload";
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
            let repository = &self.repository;
            repository
                .begin_media_upload(&request)
                .await
                .map_err(CoreServiceError::from)
        }
        .await;

        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    /// Stages one piece of the file.
    ///
    /// Authorized like any other media write. Successes are deliberately NOT
    /// audited: bytes arriving is transport, not a business event, and auditing
    /// each of them would bury the events that are. The `init` and the
    /// `complete` are the two moments worth recording — but a FAILED chunk is
    /// worth one row, so failures are audited below.
    pub async fn stage_media_chunk(
        &self,
        upload_id: &str,
        chunk_index: i32,
        bytes: Vec<u8>,
        context: &ServiceContext,
    ) -> Result<MediaUploadProgress, CoreServiceError> {
        const OP: &str = "media.stageMediaChunk";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let repository = &self.repository;
        let result = repository
            .stage_media_chunk(upload_id, chunk_index, &bytes)
            .await
            .map(|progress| MediaUploadProgress {
                received_chunks: progress.received_chunks,
                chunk_count: progress.chunk_count,
            })
            .map_err(CoreServiceError::from);
        if result.is_err() {
            audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        }

        result
    }

    /// Completes a chunked upload: assembles it, makes the copies that make it servable, and writes the photograph.
    ///
    /// The expensive part runs OFF the async runtime. Decoding a 13 MB photograph and re-encoding two copies is
    /// hundreds of milliseconds of straight CPU, and doing that on the runtime stalls every other request this
    /// container is serving while it runs.
    ///
    /// A failure here leaves the staged bytes in place on purpose: the upload is unfinished rather than broken, the
    /// browser can retry `complete`, and the sweep collects it if nobody does.
    /// Claims an upload for finishing (fast): after this, `finish_media_upload` is the only thing that may finish it.
    pub async fn claim_media_upload(
        &self,
        upload_id: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "media.claimMediaUpload";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = match self.repository.claim_media_upload(upload_id).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(CoreServiceError::business(
                "MEDIA_UPLOAD_NOT_CLAIMABLE",
                "This photo is already being finished, or the upload is unknown.",
            )),
            Err(error) => Err(error.into()),
        };
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

    /// Before a file is uploaded: is it already on the property, or partly uploaded (so it resumes)?
    pub async fn find_media_upload(
        &self,
        property_id: &str,
        filename: &str,
        byte_size: i64,
        chunk_size: i32,
        context: &ServiceContext,
    ) -> Result<UploadLookup, CoreServiceError> {
        const OP: &str = "media.findMediaUpload";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.write",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = async {
            if self
                .repository
                .property_has_photo(property_id, filename, byte_size)
                .await?
            {
                return Ok(UploadLookup::AlreadyStored);
            }
            Ok(
                match self
                    .repository
                    .resumable_media_upload(property_id, filename, byte_size, chunk_size)
                    .await?
                {
                    Some((upload_id, status, received)) => UploadLookup::Unfinished {
                        upload_id,
                        status,
                        received,
                    },
                    None => UploadLookup::New,
                },
            )
        }
        .await
        .map_err(|error: db::DbFailure| CoreServiceError::from(error));
        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    /// Where a chunked upload stands: `done` (stored and attached), `processing`, `failed`, or `uploading`.
    pub async fn media_upload_state(
        &self,
        upload_id: &str,
        context: &ServiceContext,
    ) -> Result<&'static str, CoreServiceError> {
        const OP: &str = "media.uploadState";
        let decision = authorize(
            &self.runtime,
            "media",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = match self.repository.media_upload_state(upload_id).await {
            Ok(None) => Ok("done"),
            Ok(Some(status)) => Ok(match status.as_str() {
                "complete" => "processing",
                "failed" => "failed",
                _ => "uploading",
            }),
            Err(error) => Err(error.into()),
        };
        audit_result(&self.runtime, "media", OP, context, decision, &result).await?;
        result
    }

    /// Claim and finish, in one call (the synchronous API route).
    pub async fn complete_media_upload(
        &self,
        upload_id: &str,
        context: &ServiceContext,
    ) -> Result<UploadPropertyMediaResult, CoreServiceError> {
        self.claim_media_upload(upload_id, context).await?;
        self.finish_media_upload(upload_id, context).await
    }

    /// Assembles, re-encodes, stores and attaches a CLAIMED upload. The slow step. A failure marks the upload failed,
    /// with its chunks kept, so it can be finished again.
    pub async fn finish_media_upload(
        &self,
        upload_id: &str,
        context: &ServiceContext,
    ) -> Result<UploadPropertyMediaResult, CoreServiceError> {
        const OP: &str = "media.completeMediaUpload";
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
            let repository = &self.repository;
            let assembly = repository
                .assemble_media_upload(upload_id)
                .await
                .map_err(CoreServiceError::from)?;

            let (assembly, derivatives) = tokio::task::spawn_blocking(move || {
                match imaging::derive_web_and_thumb(&assembly.bytes) {
                    Ok(derived) => {
                        let inputs = derived
                            .into_iter()
                            .map(|copy| db::MediaDerivativeInput {
                                kind: copy.kind,
                                bytes: copy.bytes,
                            })
                            .collect::<Vec<_>>();
                        Ok((assembly, inputs))
                    }
                    // The message is written for a person: whoever chose the file needs to know why it was refused.
                    Err(message) => Err(message),
                }
            })
            .await
            .map_err(|error| {
                CoreServiceError::business(
                    "MEDIA_DERIVATIVE_FAILED",
                    format!("the photograph could not be processed ({error})"),
                )
            })?
            .map_err(|message| CoreServiceError::business("MEDIA_DERIVATIVE_FAILED", message))?;

            repository
                .commit_media_upload(upload_id, &assembly, &derivatives)
                .await
                .map_err(CoreServiceError::from)
        }
        .await;

        if result.is_err() {
            // A database failure here reaches the durable capture through the DbFailure sink.
            let _ = self.repository.fail_media_upload(upload_id).await;
        }
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
