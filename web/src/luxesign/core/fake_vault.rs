//! In-memory Vault double for the Luxesign object-first unit tests.
//!
//! Records writes and deletes and can fail one `put` by its 1-based position, which is how the
//! upload-before-insert cleanup path is exercised without a real bucket.

use async_trait::async_trait;
use services::{AbstractObjectStoreService, ObjectBucket, ObjectMetadata, ObjectStoreError};
use sha2::Digest as _;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Default)]
pub(crate) struct FakeVault {
    puts: Mutex<Vec<String>>,
    deletes: Mutex<Vec<String>>,
    fail_put_at: Mutex<Option<usize>>,
}

impl FakeVault {
    pub(crate) fn with_failure_on(put_number: usize) -> Self {
        let fake = Self::default();
        *fake.fail_put_at.lock().unwrap() = Some(put_number);
        fake
    }

    pub(crate) fn puts(&self) -> Vec<String> {
        self.puts.lock().unwrap().clone()
    }

    pub(crate) fn deletes(&self) -> Vec<String> {
        self.deletes.lock().unwrap().clone()
    }
}

fn object_metadata(bucket: ObjectBucket, key: &str, size: i64) -> ObjectMetadata {
    ObjectMetadata {
        bucket,
        key: key.to_owned(),
        size,
        content_type: Some("application/pdf".to_owned()),
        etag: Some("etag".to_owned()),
        sha256: Some("0".repeat(64)),
    }
}

#[async_trait]
impl AbstractObjectStoreService for FakeVault {
    async fn put_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        bytes: &[u8],
        content_type: &str,
    ) -> Result<ObjectMetadata, ObjectStoreError> {
        let (number, fail_at) = {
            let mut puts = self.puts.lock().unwrap();
            puts.push(key.to_owned());
            (puts.len(), *self.fail_put_at.lock().unwrap())
        };
        if fail_at == Some(number) {
            return Err(ObjectStoreError::infrastructure("put failed", true));
        }
        let mut stored = object_metadata(bucket, key, bytes.len() as i64);
        stored.sha256 = Some(format!("{:x}", sha2::Sha256::digest(bytes)));
        stored.content_type = Some(content_type.to_owned());
        Ok(stored)
    }

    async fn head_object(
        &self,
        _bucket: ObjectBucket,
        _key: &str,
    ) -> Result<ObjectMetadata, ObjectStoreError> {
        Err(ObjectStoreError::not_found("missing"))
    }

    async fn delete_object(
        &self,
        _bucket: ObjectBucket,
        key: &str,
    ) -> Result<(), ObjectStoreError> {
        self.deletes.lock().unwrap().push(key.to_owned());
        Ok(())
    }

    async fn public_read_url(
        &self,
        _bucket: ObjectBucket,
        _key: &str,
    ) -> Result<String, ObjectStoreError> {
        Err(ObjectStoreError::access_denied("the Vault is private"))
    }

    async fn signed_read_url(
        &self,
        _bucket: ObjectBucket,
        key: &str,
        _ttl: Duration,
    ) -> Result<String, ObjectStoreError> {
        Ok(format!("signed://{key}"))
    }

    async fn signed_write_url(
        &self,
        _bucket: ObjectBucket,
        _key: &str,
        _content_type: &str,
        _ttl: Duration,
    ) -> Result<String, ObjectStoreError> {
        Err(ObjectStoreError::access_denied(
            "direct writes are not allowed",
        ))
    }

    async fn verify_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        expected_sha256: &str,
        expected_size: i64,
    ) -> Result<ObjectMetadata, ObjectStoreError> {
        let mut stored = object_metadata(bucket, key, expected_size);
        stored.sha256 = Some(expected_sha256.to_owned());
        Ok(stored)
    }
}
