//! Media service doors: standalone uploads, property video session/finalize, and the hero/remove/read operations.

#[allow(unused_imports)]
use super::*;
use services::{ServiceFailureClass, ServiceRuntimeError};

impl<R: MediaRepository> MediaService<R> {
    /// Upload one file with NO property link: an image for the internal media door, or a signed-contract PDF for
    /// the Vault. OBJECT-FIRST, always: the bytes go to the media bucket and only metadata is recorded, so this
    /// door never writes `media.file_data` and never needs a blob fallback. A deployment without the object store
    /// is a typed configuration failure.
    ///
    /// The bucket follows the artefact: a PDF is a Vault document (`Vault`/`vault`, `media_type = 'document'`, which
    /// is what the Vault's byte doors resolve), an image is private property media (`PropertyMediaPrivate`/`private`).
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
                    "A file is required.",
                ));
            }
            if bytes.len() > MAX_MEDIA_UPLOAD_BYTES {
                return Err(CoreServiceError::business(
                    "MEDIA_TOO_LARGE",
                    "File is too large (max 50 MB).",
                ));
            }
            let mime = mime_type.trim().to_ascii_lowercase();
            let (bucket, visibility, media_type) = match mime.as_str() {
                "application/pdf" => (ObjectBucket::Vault, "vault", "document"),
                image if image.starts_with("image/") => {
                    (ObjectBucket::PropertyMediaPrivate, "private", "image")
                }
                _ => {
                    return Err(CoreServiceError::business(
                        "MEDIA_TYPE_INVALID",
                        "Only images and PDFs can be uploaded standalone.",
                    ))
                }
            };
            let store = self.object_store.as_ref().ok_or_else(|| {
                CoreServiceError::Runtime(ServiceRuntimeError::Router {
                    code: "MEDIA_OBJECT_STORE_NOT_CONFIGURED".into(),
                    message: "the standalone upload path requires media object storage".into(),
                    retryable: false,
                    class: ServiceFailureClass::Infrastructure,
                })
            })?;
            let filename = sanitize_media_filename(filename);
            let id = uuid::Uuid::new_v4().to_string();
            let row = self
                .object_row(
                    store, bucket, visibility, &id, &filename, &mime, &bytes, None, media_type,
                    None, None,
                )
                .await
                .map_err(|message| {
                    CoreServiceError::infrastructure("MEDIA_OBJECT_UPLOAD_FAILED", message)
                })?;
            let size = row.object.size;
            match self.repository.save_standalone_object(&row).await {
                Ok(()) => Ok((id, filename, mime, size)),
                Err(error) => {
                    // The object has no row to point at it; remove it rather than orphan it.
                    let _ = store.delete_object(bucket, &row.object.key).await;
                    Err(error.into())
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::test_support::{service, FakeRepository, FakeStore};
    use std::sync::Arc;

    fn root_context() -> ServiceContext {
        crate::media::test_support::root_context()
    }

    /// A signed-contract PDF through the standalone door: object-first, in the VAULT bucket, recorded as a
    /// `document` with `vault` visibility — exactly what the Vault's object byte doors resolve.
    #[tokio::test]
    async fn a_signed_pdf_is_stored_object_first_as_a_vault_document() {
        let repository = FakeRepository::default();
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;

        let (id, filename, mime, size) = media
            .upload_standalone(
                "signed.pdf",
                "application/pdf",
                b"%PDF-1.7 signed".to_vec(),
                &root_context(),
            )
            .await
            .expect("the PDF is stored");

        assert!(!id.is_empty());
        assert_eq!(filename, "signed.pdf");
        assert_eq!(mime, "application/pdf");
        assert_eq!(size, b"%PDF-1.7 signed".len() as i64);
        let rows = media.repository.standalone_rows();
        assert_eq!(rows.len(), 1, "one object-first row, no blob");
        let row = &rows[0];
        assert_eq!(row.media_type, "document");
        assert_eq!(row.mime_type, "application/pdf");
        assert_eq!(row.object.bucket, "vault");
        assert_eq!(row.object.visibility.as_deref(), Some("vault"));
        assert_eq!(row.object.status, "VERIFIED");
        assert_eq!(store.put_count(), 1);
    }

    /// A standalone IMAGE is private property media, and a file that is neither an image nor a PDF is refused before
    /// anything is uploaded.
    #[tokio::test]
    async fn a_standalone_image_is_private_and_a_non_media_file_is_refused() {
        let repository = FakeRepository::default();
        let store = Arc::new(FakeStore::default());
        let media = service(repository, Some(store.clone())).await;

        media
            .upload_standalone("front.webp", "image/webp", vec![1, 2, 3], &root_context())
            .await
            .expect("an image is accepted");
        let rows = media.repository.standalone_rows();
        assert_eq!(rows[0].object.bucket, "property_media_private");
        assert_eq!(rows[0].object.visibility.as_deref(), Some("private"));
        assert_eq!(rows[0].media_type, "image");

        let error = media
            .upload_standalone("payload.html", "text/html", vec![1, 2, 3], &root_context())
            .await
            .expect_err("an html file is not media");
        assert_eq!(error.code(), "MEDIA_TYPE_INVALID");
        assert_eq!(
            media.repository.standalone_rows().len(),
            1,
            "the refusal happened before persistence"
        );
        assert_eq!(
            store.put_count(),
            1,
            "the refusal happened before the upload"
        );
    }

    #[tokio::test]
    async fn a_standalone_upload_without_an_object_store_is_a_typed_failure() {
        let media = service(FakeRepository::default(), None).await;
        let error = media
            .upload_standalone(
                "signed.pdf",
                "application/pdf",
                b"%PDF-".to_vec(),
                &root_context(),
            )
            .await
            .expect_err("no object store");
        assert_eq!(error.code(), "MEDIA_OBJECT_STORE_NOT_CONFIGURED");
    }
}
