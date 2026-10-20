//! Object storage port.
//!
//! Storage is infrastructure: business services decide *which* object belongs to a domain, while
//! this port owns only bytes and signed access. The logical buckets are deliberately an enum rather
//! than strings so a provider cannot accidentally route Vault bytes into a public property bucket,
//! and so the bucket separation is visible at every call site.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The three storage domains. A provider maps these to physical bucket names from configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectBucket {
    /// Published property photography; anonymous reads are expected.
    PropertyMediaPublic,
    /// Unpublished/raw property media; signed reads only.
    PropertyMediaPrivate,
    /// Vault documents; private, signed reads only, never public.
    Vault,
}

impl ObjectBucket {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PropertyMediaPublic => "property_media_public",
            Self::PropertyMediaPrivate => "property_media_private",
            Self::Vault => "vault",
        }
    }
}

/// Metadata returned by a successful write or head. `sha256` is lowercase hex when known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectMetadata {
    pub bucket: ObjectBucket,
    pub key: String,
    pub size: i64,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub sha256: Option<String>,
}

/// A storage failure with a stable code and an explicit retry class. Provider SDK errors never
/// escape this type; callers decide whether to fall back, retry, or fail the operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ObjectStoreError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl ObjectStoreError {
    pub fn configuration(message: impl Into<String>) -> Self {
        Self {
            code: "OBJECT_STORE_CONFIGURATION".into(),
            message: message.into(),
            retryable: false,
        }
    }

    pub fn invalid_key(message: impl Into<String>) -> Self {
        Self {
            code: "OBJECT_KEY_INVALID".into(),
            message: message.into(),
            retryable: false,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: "OBJECT_NOT_FOUND".into(),
            message: message.into(),
            retryable: false,
        }
    }

    pub fn access_denied(message: impl Into<String>) -> Self {
        Self {
            code: "OBJECT_ACCESS_DENIED".into(),
            message: message.into(),
            retryable: false,
        }
    }

    pub fn infrastructure(message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: "OBJECT_STORE_UNAVAILABLE".into(),
            message: message.into(),
            retryable,
        }
    }

    pub fn integrity(message: impl Into<String>) -> Self {
        Self {
            code: "OBJECT_INTEGRITY_FAILED".into(),
            message: message.into(),
            retryable: false,
        }
    }
}

/// Validate an object key before it reaches a provider.
///
/// Keys are opaque, UUID-oriented paths: `/`-separated segments of ASCII letters, digits, `-`, `_`
/// and `.`. Absolute paths, empty segments, `.`/`..`, control characters and whitespace are rejected
/// so a key can never be interpreted as a bucket, a traversal, or a signed-URL override.
pub fn validate_object_key(key: &str) -> Result<(), ObjectStoreError> {
    if key.is_empty() {
        return Err(ObjectStoreError::invalid_key("object key is empty"));
    }
    if key.len() > 1024 {
        return Err(ObjectStoreError::invalid_key("object key exceeds 1024 bytes"));
    }
    if key.starts_with('/') || key.ends_with('/') {
        return Err(ObjectStoreError::invalid_key(
            "object key must be relative and cannot end in '/'",
        ));
    }
    for segment in key.split('/') {
        if segment.is_empty() {
            return Err(ObjectStoreError::invalid_key("object key has an empty segment"));
        }
        if segment == "." || segment == ".." {
            return Err(ObjectStoreError::invalid_key("object key has a traversal segment"));
        }
        if !segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            return Err(ObjectStoreError::invalid_key(
                "object key contains a character outside [A-Za-z0-9._-]",
            ));
        }
    }
    Ok(())
}

/// The storage port. Implementations own provider details; callers own domain authorization.
#[async_trait]
pub trait AbstractObjectStoreService: Send + Sync {
    async fn put_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        bytes: &[u8],
        content_type: &str,
    ) -> Result<ObjectMetadata, ObjectStoreError>;

    async fn head_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
    ) -> Result<ObjectMetadata, ObjectStoreError>;

    async fn delete_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
    ) -> Result<(), ObjectStoreError>;

    async fn exists(
        &self,
        bucket: ObjectBucket,
        key: &str,
    ) -> Result<bool, ObjectStoreError> {
        match self.head_object(bucket, key).await {
            Ok(_) => Ok(true),
            Err(error) if error.code == "OBJECT_NOT_FOUND" => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn signed_read_url(
        &self,
        bucket: ObjectBucket,
        key: &str,
        ttl: Duration,
    ) -> Result<String, ObjectStoreError>;

    async fn signed_write_url(
        &self,
        bucket: ObjectBucket,
        key: &str,
        content_type: &str,
        ttl: Duration,
    ) -> Result<String, ObjectStoreError>;

    /// Prove that the stored payload is the expected SHA-256 and size, not merely that a PUT
    /// returned 200. Migration and shadow writes use this before marking an object VERIFIED.
    async fn verify_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        expected_sha256: &str,
        expected_size: i64,
    ) -> Result<ObjectMetadata, ObjectStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_keys_are_accepted() {
        for key in [
            "properties/abc/media/def/original.jpg",
            "vault/doc-123/original.pdf",
            "a",
        ] {
            assert!(validate_object_key(key).is_ok(), "{key}");
        }
    }

    #[test]
    fn unsafe_keys_are_rejected() {
        for key in [
            "",
            "/absolute",
            "trailing/",
            "a//b",
            "a/../b",
            "a/./b",
            "a b",
            "a\\b",
            "a\nb",
        ] {
            assert!(validate_object_key(key).is_err(), "{key:?}");
        }
    }

    #[test]
    fn buckets_have_stable_names() {
        assert_eq!(ObjectBucket::PropertyMediaPublic.as_str(), "property_media_public");
        assert_eq!(ObjectBucket::PropertyMediaPrivate.as_str(), "property_media_private");
        assert_eq!(ObjectBucket::Vault.as_str(), "vault");
    }
}
