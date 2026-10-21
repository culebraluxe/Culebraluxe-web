//! A deterministic in-memory media object store for contract tests.
//!
//! Tests that drive the production media services need a store that behaves like the provider at its seam: PUT
//! returns a SHA-256 and size, verify proves those bytes, and a read URL is generated. Nothing is persisted beyond
//! the test process, and no provider is called, which is exactly what the one-way rule asks of a harness.

use async_trait::async_trait;
use services::{AbstractObjectStoreService, ObjectBucket, ObjectMetadata, ObjectStoreError};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One stored object, as the fake remembers it.
#[derive(Debug, Clone)]
pub struct StoredObject {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

/// An in-memory object store. Cheap to clone; all clones share the same map so a test can inspect what the service
/// wrote through the `Arc` it handed to the service.
#[derive(Debug, Clone, Default)]
pub struct FakeObjectStore {
    objects: Arc<Mutex<BTreeMap<(String, String), StoredObject>>>,
}

impl FakeObjectStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many objects the test has stored (and not deleted).
    pub fn len(&self) -> usize {
        self.objects.lock().expect("objects poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The stored bytes for a key, for a test that needs to prove what was written.
    pub fn bytes(&self, bucket: ObjectBucket, key: &str) -> Option<Vec<u8>> {
        self.objects
            .lock()
            .expect("objects poisoned")
            .get(&(bucket.as_str().to_owned(), key.to_owned()))
            .map(|stored| stored.bytes.clone())
    }
}

/// A deterministic 64-hex tag for the bytes. Not a cryptographic digest — the fake only has to be CONSISTENT:
/// `verify_object` compares the tag the provider returned from `put` against the tag of the stored bytes.
fn fake_sha256(bytes: &[u8]) -> String {
    let mut acc: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        acc ^= u64::from(*byte);
        acc = acc.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{acc:016x}{:048x}", bytes.len())
}

#[async_trait]
impl AbstractObjectStoreService for FakeObjectStore {
    async fn put_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        bytes: &[u8],
        content_type: &str,
    ) -> Result<ObjectMetadata, ObjectStoreError> {
        self.objects.lock().expect("objects poisoned").insert(
            (bucket.as_str().to_owned(), key.to_owned()),
            StoredObject {
                bytes: bytes.to_vec(),
                content_type: content_type.to_owned(),
            },
        );
        Ok(ObjectMetadata {
            bucket,
            key: key.to_owned(),
            size: bytes.len() as i64,
            content_type: Some(content_type.to_owned()),
            etag: Some("etag".into()),
            sha256: Some(fake_sha256(bytes)),
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
            content_type: Some(stored.content_type.clone()),
            etag: Some("etag".into()),
            sha256: Some(fake_sha256(&stored.bytes)),
        })
    }

    async fn get_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
    ) -> Result<Vec<u8>, ObjectStoreError> {
        let objects = self.objects.lock().expect("objects poisoned");
        objects
            .get(&(bucket.as_str().to_owned(), key.to_owned()))
            .map(|stored| stored.bytes.clone())
            .ok_or_else(|| ObjectStoreError::not_found(key))
    }

    async fn delete_object(&self, bucket: ObjectBucket, key: &str) -> Result<(), ObjectStoreError> {
        self.objects
            .lock()
            .expect("objects poisoned")
            .remove(&(bucket.as_str().to_owned(), key.to_owned()));
        Ok(())
    }

    async fn public_read_url(
        &self,
        _bucket: ObjectBucket,
        key: &str,
    ) -> Result<String, ObjectStoreError> {
        Ok(format!("https://cdn.example/{key}"))
    }

    async fn signed_read_url(
        &self,
        _bucket: ObjectBucket,
        key: &str,
        _ttl: Duration,
    ) -> Result<String, ObjectStoreError> {
        Ok(format!("https://signed.example/{key}"))
    }

    async fn signed_write_url(
        &self,
        _bucket: ObjectBucket,
        key: &str,
        _content_type: &str,
        _ttl: Duration,
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
        let head = self.head_object(bucket, key).await?;
        if head.sha256.as_deref() != Some(expected_sha256) || head.size != expected_size {
            return Err(ObjectStoreError::integrity("stored bytes do not match"));
        }
        Ok(head)
    }
}
