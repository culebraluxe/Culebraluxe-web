//! Shared in-memory test doubles for the media service.
//!
//! The whole-file upload and the direct-to-object upload test against the same fakes: a repository
//! that records what was written, and an object store that can simulate a browser's direct PUT
//! (including a tampered or oversize object).

#![allow(dead_code)]

#[allow(unused_imports)]
use super::*;
use crate::security::CasbinAuthorizationPort;
use async_trait::async_trait;
use db::{DbFailure, DbResult, MediaObjectLocation, MediaObjectRow};
use model::{
    AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset, UploadPropertyMediaRequest,
    UploadPropertyMediaResult,
};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, ObjectMetadata, ObjectStoreError, ServiceActor,
    ServiceActorKind, ServiceInfrastructure, ServicePrincipal,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// The signing key the test services use for direct-upload pending tokens.
pub(crate) const TEST_DIRECT_UPLOAD_SECRET: &[u8] = b"test-direct-upload-secret-000000000001";

pub(crate) const PROPERTY: &str = "0b0e0f56-89a1-4c4f-9b2b-6a1f0a1e0001";

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The one signed-in user these tests run as: ROOT, so the real Casbin policy admits `property.write`.
pub(crate) fn root_context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("root-user".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "media-whole-file-test".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "root-user".into(),
            level: "ADMIN".into(),
            role_codes: vec!["root".into()],
            account_type: "internal".into(),
            entitlement_codes: Vec::new(),
        }),
    }
}

/// One object as the fake store remembers it.
#[derive(Debug, Clone)]
pub(crate) struct StoredObject {
    pub(crate) sha: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) content_type: Option<String>,
}

#[derive(Default)]
pub(crate) struct FakeStore {
    pub(crate) objects: Mutex<BTreeMap<(String, String), StoredObject>>,
    pub(crate) puts: Mutex<Vec<String>>,
    pub(crate) deletes: Mutex<Vec<String>>,
    pub(crate) verifies: AtomicUsize,
    /// 1-based verify call that should fail (an integrity failure mid-set).
    pub(crate) fail_verify_at: Mutex<Option<usize>>,
    pub(crate) fail_urls: AtomicBool,
}

impl FakeStore {
    pub(crate) fn put_count(&self) -> usize {
        self.puts.lock().expect("puts poisoned").len()
    }

    pub(crate) fn delete_count(&self) -> usize {
        self.deletes.lock().expect("deletes poisoned").len()
    }

    /// Simulate the BROWSER's direct PUT: the object appears in the bucket without the service ever
    /// having written it. Tests use this to stand in for the request that never reaches the app.
    pub(crate) fn direct_put(
        &self,
        bucket: ObjectBucket,
        key: &str,
        bytes: &[u8],
        content_type: &str,
    ) {
        self.objects.lock().expect("objects poisoned").insert(
            (bucket.as_str().to_owned(), key.to_owned()),
            StoredObject {
                sha: sha256_hex(bytes),
                bytes: bytes.to_vec(),
                content_type: Some(content_type.to_owned()),
            },
        );
    }

    pub(crate) fn object(&self, bucket: ObjectBucket, key: &str) -> Option<StoredObject> {
        self.objects
            .lock()
            .expect("objects poisoned")
            .get(&(bucket.as_str().to_owned(), key.to_owned()))
            .cloned()
    }
}

#[async_trait]
impl AbstractObjectStoreService for FakeStore {
    async fn put_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        bytes: &[u8],
        content_type: &str,
    ) -> Result<ObjectMetadata, ObjectStoreError> {
        let sha = sha256_hex(bytes);
        self.objects.lock().expect("objects poisoned").insert(
            (bucket.as_str().to_owned(), key.to_owned()),
            StoredObject {
                sha: sha.clone(),
                bytes: bytes.to_vec(),
                content_type: Some(content_type.to_owned()),
            },
        );
        self.puts
            .lock()
            .expect("puts poisoned")
            .push(key.to_owned());
        Ok(ObjectMetadata {
            bucket,
            key: key.to_owned(),
            size: bytes.len() as i64,
            content_type: Some(content_type.to_owned()),
            etag: Some("etag".into()),
            sha256: Some(sha),
        })
    }

    async fn head_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
    ) -> Result<ObjectMetadata, ObjectStoreError> {
        let objects = self.objects.lock().expect("objects poisoned");
        let stored = objects
            .get(&(bucket.as_str().to_owned(), key.to_owned()))
            .ok_or_else(|| ObjectStoreError::not_found(key))?;
        Ok(ObjectMetadata {
            bucket,
            key: key.to_owned(),
            size: stored.bytes.len() as i64,
            content_type: stored.content_type.clone(),
            etag: Some("etag".into()),
            sha256: Some(stored.sha.clone()),
        })
    }

    async fn get_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
    ) -> Result<Vec<u8>, ObjectStoreError> {
        self.object(bucket, key)
            .map(|stored| stored.bytes)
            .ok_or_else(|| ObjectStoreError::not_found(key))
    }

    async fn delete_object(&self, bucket: ObjectBucket, key: &str) -> Result<(), ObjectStoreError> {
        self.objects
            .lock()
            .expect("objects poisoned")
            .remove(&(bucket.as_str().to_owned(), key.to_owned()));
        self.deletes
            .lock()
            .expect("deletes poisoned")
            .push(key.to_owned());
        Ok(())
    }

    async fn public_read_url(
        &self,
        _bucket: ObjectBucket,
        key: &str,
    ) -> Result<String, ObjectStoreError> {
        if self.fail_urls.load(Ordering::SeqCst) {
            return Err(ObjectStoreError::infrastructure("no URL", true));
        }
        Ok(format!("https://cdn.example/{key}"))
    }

    async fn signed_read_url(
        &self,
        _bucket: ObjectBucket,
        key: &str,
        _ttl: std::time::Duration,
    ) -> Result<String, ObjectStoreError> {
        if self.fail_urls.load(Ordering::SeqCst) {
            return Err(ObjectStoreError::infrastructure("no URL", true));
        }
        Ok(format!("https://signed.example/{key}"))
    }

    async fn signed_write_url(
        &self,
        _bucket: ObjectBucket,
        key: &str,
        _content_type: &str,
        _ttl: std::time::Duration,
    ) -> Result<String, ObjectStoreError> {
        Ok(format!("https://write.example/{key}"))
    }

    async fn verify_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        expected_sha256: &str,
        expected_size: i64,
    ) -> Result<ObjectMetadata, ObjectStoreError> {
        let call = self.verifies.fetch_add(1, Ordering::SeqCst) + 1;
        if *self.fail_verify_at.lock().expect("verify poisoned") == Some(call) {
            return Err(ObjectStoreError::integrity(format!("verify {call} failed")));
        }
        let stored = self
            .object(bucket, key)
            .ok_or_else(|| ObjectStoreError::not_found(key))?;
        let size = stored.bytes.len() as i64;
        if stored.sha != expected_sha256 || size != expected_size {
            return Err(ObjectStoreError::integrity("stored bytes do not match"));
        }
        Ok(ObjectMetadata {
            bucket,
            key: key.to_owned(),
            size,
            content_type: stored.content_type,
            etag: Some("etag".into()),
            sha256: Some(stored.sha),
        })
    }
}

/// A repository that records what an upload path did, plus the reads the read switch makes.
#[derive(Default)]
pub(crate) struct FakeRepository {
    pub(crate) public: bool,
    pub(crate) fail_save: bool,
    pub(crate) existing_photo_id: Option<String>,
    pub(crate) location: Option<MediaObjectLocation>,
    pub(crate) blob: Option<(String, Vec<u8>)>,
    pub(crate) saved_sets: Mutex<Vec<(String, String, Vec<MediaObjectRow>)>>,
    pub(crate) blob_reads: AtomicUsize,
    pub(crate) object_lookups: AtomicUsize,
}

impl FakeRepository {
    pub(crate) fn saved_sets(&self) -> Vec<(String, String, Vec<MediaObjectRow>)> {
        self.saved_sets.lock().expect("saved poisoned").clone()
    }
}

#[async_trait]
impl MediaRepository for FakeRepository {
    async fn media_bytes(&self, _id: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        self.blob_reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.blob.clone())
    }

    async fn upload_standalone(
        &self,
        _filename: &str,
        _mime_type: &str,
        _bytes: &[u8],
    ) -> DbResult<(String, String, String, i64)> {
        Err(unused())
    }

    async fn upload_property_media(
        &self,
        _request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult> {
        Err(unused())
    }

    async fn for_property(&self, _property_id: &str) -> DbResult<Vec<MediaAsset>> {
        Err(unused())
    }

    async fn set_property_hero(&self, _property_id: &str, _media_id: &str) -> DbResult<bool> {
        Err(unused())
    }

    async fn attach_property_video(
        &self,
        _request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult> {
        Err(unused())
    }

    async fn remove_property_media(&self, _property_id: &str, _media_id: &str) -> DbResult<bool> {
        Err(unused())
    }

    async fn media_object_location(
        &self,
        _media_id: &str,
    ) -> DbResult<Option<MediaObjectLocation>> {
        self.object_lookups.fetch_add(1, Ordering::SeqCst);
        Ok(self.location.clone())
    }

    async fn property_photo_id(
        &self,
        _property_id: &str,
        _filename: &str,
        _byte_size: i64,
    ) -> DbResult<Option<String>> {
        Ok(self.existing_photo_id.clone())
    }

    async fn property_is_public(&self, _property_id: &str) -> DbResult<bool> {
        Ok(self.public)
    }

    async fn save_property_media_object_set(
        &self,
        property_id: &str,
        role: &str,
        rows: &[MediaObjectRow],
    ) -> DbResult<UploadPropertyMediaResult> {
        if self.fail_save {
            return Err(DbFailure::configuration(
                "test.save_property_media_object_set",
                "the transaction failed",
            ));
        }
        self.saved_sets.lock().expect("saved poisoned").push((
            property_id.to_owned(),
            role.to_owned(),
            rows.to_vec(),
        ));
        Ok(UploadPropertyMediaResult {
            ok: true,
            media_id: rows[0].id.clone(),
            property_id: property_id.to_owned(),
            role: role.to_owned(),
        })
    }
}

pub(crate) fn unused() -> DbFailure {
    DbFailure::configuration(
        "test.unused",
        "this path is not part of the whole-file upload",
    )
}

pub(crate) fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let mut image = image::RgbImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8]);
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80)
        .encode_image(&image::DynamicImage::ImageRgb8(image))
        .expect("the fixture encodes");
    out
}

pub(crate) async fn service(
    repository: FakeRepository,
    store: Option<Arc<FakeStore>>,
    object_reads: bool,
) -> MediaService<FakeRepository> {
    let infrastructure = ServiceInfrastructure::new(
        Arc::new(
            CasbinAuthorizationPort::new()
                .await
                .expect("the policy loads"),
        ),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    );
    let service = MediaService::new(repository, infrastructure);
    let service = match store {
        Some(store) => service.with_object_store(store),
        None => service,
    };
    service
        .with_object_reads(object_reads)
        .with_direct_upload_secret(TEST_DIRECT_UPLOAD_SECRET.to_vec())
}
