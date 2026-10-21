//! Neon Object Storage provider. The S3 SDK type never leaves this module.

use async_trait::async_trait;
use aws_sdk_s3::config::retry::RetryConfig;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use services::{
    validate_object_key, AbstractObjectStoreService, ObjectBucket, ObjectMetadata, ObjectStoreError,
};
use sha2::{Digest, Sha256};
use std::time::Duration;

const ENDPOINT: &str = "MEDIA_OBJECT_STORE_ENDPOINT";
const REGION: &str = "MEDIA_OBJECT_STORE_REGION";
const ACCESS_KEY: &str = "MEDIA_OBJECT_STORE_ACCESS_KEY_ID";
const SECRET_KEY: &str = "MEDIA_OBJECT_STORE_SECRET_ACCESS_KEY";
const BUCKET_PROPERTY_PUBLIC: &str = "MEDIA_BUCKET_PROPERTY_PUBLIC";
const BUCKET_PROPERTY_PRIVATE: &str = "MEDIA_BUCKET_PROPERTY_PRIVATE";
const BUCKET_VAULT: &str = "MEDIA_BUCKET_VAULT";
/// Optional same-origin edge base (for example `https://www.culebraluxe.com`). When set, public
/// reads point at the app's `/media-cdn/` rewrite so Vercel caches the object at its edge PoPs.
const PUBLIC_CDN_BASE: &str = "MEDIA_PUBLIC_CDN_BASE";

#[derive(Debug, Clone)]
pub struct NeonObjectStoreConfig {
    pub endpoint: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub property_public_bucket: String,
    pub property_private_bucket: String,
    pub vault_bucket: String,
    pub public_cdn_base: Option<String>,
}

impl NeonObjectStoreConfig {
    /// Read the app's dedicated media credentials. A completely unset store is `None` so the app
    /// can boot before the media cutover; a half-configured store is a hard configuration error.
    pub fn from_env() -> Result<Option<Self>, ObjectStoreError> {
        let read = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        };
        let endpoint = read(ENDPOINT);
        let region = read(REGION);
        let access = read(ACCESS_KEY);
        let secret = read(SECRET_KEY);
        let public = read(BUCKET_PROPERTY_PUBLIC);
        let private = read(BUCKET_PROPERTY_PRIVATE);
        let vault = read(BUCKET_VAULT);
        let public_cdn_base = read(PUBLIC_CDN_BASE);

        if endpoint.is_none()
            && region.is_none()
            && access.is_none()
            && secret.is_none()
            && public.is_none()
            && private.is_none()
            && vault.is_none()
        {
            return Ok(None);
        }

        let config = Self {
            endpoint: endpoint.ok_or_else(|| missing(ENDPOINT))?,
            region: region.ok_or_else(|| missing(REGION))?,
            access_key_id: access.ok_or_else(|| missing(ACCESS_KEY))?,
            secret_access_key: secret.ok_or_else(|| missing(SECRET_KEY))?,
            property_public_bucket: public.ok_or_else(|| missing(BUCKET_PROPERTY_PUBLIC))?,
            property_private_bucket: private.ok_or_else(|| missing(BUCKET_PROPERTY_PRIVATE))?,
            vault_bucket: vault.ok_or_else(|| missing(BUCKET_VAULT))?,
            public_cdn_base,
        };
        config.validate()?;
        Ok(Some(config))
    }

    fn validate(&self) -> Result<(), ObjectStoreError> {
        if !self.endpoint.starts_with("https://") {
            return Err(ObjectStoreError::configuration(
                "MEDIA_OBJECT_STORE_ENDPOINT must be an https URL",
            ));
        }
        if self
            .public_cdn_base
            .as_deref()
            .is_some_and(|base| !base.starts_with("https://"))
        {
            return Err(ObjectStoreError::configuration(
                "MEDIA_PUBLIC_CDN_BASE must be an https URL",
            ));
        }
        let names = [
            &self.property_public_bucket,
            &self.property_private_bucket,
            &self.vault_bucket,
        ];
        for (index, name) in names.iter().enumerate() {
            if !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
            {
                return Err(ObjectStoreError::configuration(
                    "media bucket names may contain only letters, digits, '-' and '.'",
                ));
            }
            if names[index + 1..].iter().any(|other| other == name) {
                return Err(ObjectStoreError::configuration(
                    "property public, property private and vault buckets must be distinct",
                ));
            }
        }
        Ok(())
    }

    fn bucket_name(&self, bucket: ObjectBucket) -> &str {
        match bucket {
            ObjectBucket::PropertyMediaPublic => &self.property_public_bucket,
            ObjectBucket::PropertyMediaPrivate => &self.property_private_bucket,
            ObjectBucket::Vault => &self.vault_bucket,
        }
    }
}

fn missing(name: &str) -> ObjectStoreError {
    ObjectStoreError::configuration(format!("media object storage is missing {name}"))
}

#[derive(Clone)]
pub struct NeonObjectStore {
    client: Client,
    config: NeonObjectStoreConfig,
}

impl NeonObjectStore {
    pub fn new(config: NeonObjectStoreConfig) -> Self {
        let credentials = Credentials::new(
            config.access_key_id.clone(),
            config.secret_access_key.clone(),
            None,
            None,
            "culebraluxe-media-env",
        );
        let sdk_config = aws_sdk_s3::config::Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .endpoint_url(config.endpoint.clone())
            .credentials_provider(credentials)
            .force_path_style(true)
            .retry_config(RetryConfig::standard().with_max_attempts(4))
            .build();
        Self {
            client: Client::from_conf(sdk_config),
            config,
        }
    }

    pub fn config(&self) -> &NeonObjectStoreConfig {
        &self.config
    }

    fn bucket(&self, bucket: ObjectBucket) -> &str {
        self.config.bucket_name(bucket)
    }
}

/// The deployment's provider if the dedicated media store is configured.
pub fn from_env() -> Result<Option<NeonObjectStore>, ObjectStoreError> {
    Ok(NeonObjectStoreConfig::from_env()?.map(NeonObjectStore::new))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn map_sdk_error<E: std::fmt::Debug>(operation: &str, error: SdkError<E>) -> ObjectStoreError {
    match &error {
        SdkError::ServiceError(context) => {
            let status = context.raw().status().as_u16();
            if status == 404 {
                return ObjectStoreError::not_found(format!("{operation}: object not found"));
            }
            if status == 403 {
                return ObjectStoreError::access_denied(format!(
                    "{operation}: object storage denied access"
                ));
            }
            let retryable = status == 429 || status >= 500;
            ObjectStoreError::infrastructure(format!("{operation}: HTTP {status}"), retryable)
        }
        SdkError::TimeoutError(_) => {
            ObjectStoreError::infrastructure(format!("{operation}: timeout"), true)
        }
        SdkError::DispatchFailure(_) => ObjectStoreError::infrastructure(
            format!("{operation}: object storage unreachable"),
            true,
        ),
        _ => ObjectStoreError::infrastructure(
            format!("{operation}: object storage request failed: {error:?}"),
            false,
        ),
    }
}

/// Public property photography is immutable under its media id: a replacement upload is a new row.
/// Private/Vault bytes are served through short-lived signed URLs and must not trade that for shared caching.
fn cache_control(bucket: ObjectBucket) -> &'static str {
    match bucket {
        ObjectBucket::PropertyMediaPublic => "public, max-age=31536000, immutable",
        ObjectBucket::PropertyMediaPrivate => "private, max-age=300",
        ObjectBucket::Vault => "private, no-store",
    }
}

#[async_trait]
impl AbstractObjectStoreService for NeonObjectStore {
    async fn put_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        bytes: &[u8],
        content_type: &str,
    ) -> Result<ObjectMetadata, ObjectStoreError> {
        validate_object_key(key)?;
        if content_type.trim().is_empty() {
            return Err(ObjectStoreError::configuration("content type is required"));
        }
        let sha256 = sha256_hex(bytes);
        self.client
            .put_object()
            .bucket(self.bucket(bucket))
            .key(key)
            .body(ByteStream::from(bytes.to_vec()))
            .content_type(content_type)
            .cache_control(cache_control(bucket))
            .metadata("sha256", &sha256)
            .send()
            .await
            .map_err(|error| map_sdk_error("put_object", error))?;
        let metadata = self.head_object(bucket, key).await?;
        if metadata.size != bytes.len() as i64 {
            return Err(ObjectStoreError::integrity(format!(
                "put_object: stored {} bytes, expected {}",
                metadata.size,
                bytes.len()
            )));
        }
        Ok(ObjectMetadata {
            sha256: Some(sha256),
            ..metadata
        })
    }

    async fn head_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
    ) -> Result<ObjectMetadata, ObjectStoreError> {
        validate_object_key(key)?;
        let output = self
            .client
            .head_object()
            .bucket(self.bucket(bucket))
            .key(key)
            .send()
            .await
            .map_err(|error| map_sdk_error("head_object", error))?;
        Ok(ObjectMetadata {
            bucket,
            key: key.to_owned(),
            size: output.content_length.unwrap_or_default(),
            content_type: output.content_type,
            etag: output.e_tag,
            sha256: output
                .metadata
                .and_then(|metadata| metadata.get("sha256").cloned()),
        })
    }

    async fn get_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
    ) -> Result<Vec<u8>, ObjectStoreError> {
        validate_object_key(key)?;
        let output = self
            .client
            .get_object()
            .bucket(self.bucket(bucket))
            .key(key)
            .send()
            .await
            .map_err(|error| map_sdk_error("get_object", error))?;
        let body = output.body.collect().await.map_err(|error| {
            ObjectStoreError::infrastructure(format!("get_object body: {error}"), true)
        })?;
        Ok(body.into_bytes().to_vec())
    }

    async fn delete_object(&self, bucket: ObjectBucket, key: &str) -> Result<(), ObjectStoreError> {
        validate_object_key(key)?;
        let result = self
            .client
            .delete_object()
            .bucket(self.bucket(bucket))
            .key(key)
            .send()
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(error) if matches!(&error, SdkError::ServiceError(context) if context.raw().status().as_u16() == 404) => {
                Ok(())
            }
            Err(error) => Err(map_sdk_error("delete_object", error)),
        }
    }

    async fn public_read_url(
        &self,
        bucket: ObjectBucket,
        key: &str,
    ) -> Result<String, ObjectStoreError> {
        if bucket != ObjectBucket::PropertyMediaPublic {
            return Err(ObjectStoreError::configuration(
                "public_read_url is only valid for the public property-media bucket",
            ));
        }
        validate_object_key(key)?;
        if let Some(base) = self.config.public_cdn_base.as_deref() {
            return Ok(format!(
                "{}/media-cdn/{}",
                base.trim_end_matches('/'),
                key
            ));
        }
        Ok(format!(
            "{}/{}/{}",
            self.config.endpoint.trim_end_matches('/'),
            self.config.property_public_bucket,
            key
        ))
    }

    async fn signed_read_url(
        &self,
        bucket: ObjectBucket,
        key: &str,
        ttl: Duration,
    ) -> Result<String, ObjectStoreError> {
        validate_object_key(key)?;
        let presigning = PresigningConfig::expires_in(ttl).map_err(|error| {
            ObjectStoreError::configuration(format!("invalid URL expiry: {error}"))
        })?;
        self.client
            .get_object()
            .bucket(self.bucket(bucket))
            .key(key)
            .presigned(presigning)
            .await
            .map(|request| request.uri().to_string())
            .map_err(|error| map_sdk_error("signed_read_url", error))
    }

    async fn signed_write_url(
        &self,
        bucket: ObjectBucket,
        key: &str,
        content_type: &str,
        ttl: Duration,
    ) -> Result<String, ObjectStoreError> {
        validate_object_key(key)?;
        if content_type.trim().is_empty() {
            return Err(ObjectStoreError::configuration("content type is required"));
        }
        let presigning = PresigningConfig::expires_in(ttl).map_err(|error| {
            ObjectStoreError::configuration(format!("invalid URL expiry: {error}"))
        })?;
        self.client
            .put_object()
            .bucket(self.bucket(bucket))
            .key(key)
            .content_type(content_type)
            .presigned(presigning)
            .await
            .map(|request| request.uri().to_string())
            .map_err(|error| map_sdk_error("signed_write_url", error))
    }

    async fn verify_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        expected_sha256: &str,
        expected_size: i64,
    ) -> Result<ObjectMetadata, ObjectStoreError> {
        validate_object_key(key)?;
        let output = self
            .client
            .get_object()
            .bucket(self.bucket(bucket))
            .key(key)
            .send()
            .await
            .map_err(|error| map_sdk_error("verify_object", error))?;
        let metadata = ObjectMetadata {
            bucket,
            key: key.to_owned(),
            size: output.content_length.unwrap_or_default(),
            content_type: output.content_type,
            etag: output.e_tag,
            sha256: output
                .metadata
                .and_then(|metadata| metadata.get("sha256").cloned()),
        };
        let body = output
            .body
            .collect()
            .await
            .map_err(|error| {
                ObjectStoreError::infrastructure(format!("verify_object body: {error}"), true)
            })?
            .into_bytes();
        if metadata.size != expected_size || body.len() as i64 != expected_size {
            return Err(ObjectStoreError::integrity(format!(
                "verify_object: size mismatch (head {}, body {}, expected {expected_size})",
                metadata.size,
                body.len()
            )));
        }
        let actual = sha256_hex(&body);
        if actual != expected_sha256.to_ascii_lowercase() {
            return Err(ObjectStoreError::integrity(
                "verify_object: SHA-256 mismatch between legacy blob and object",
            ));
        }
        Ok(ObjectMetadata {
            sha256: Some(actual),
            ..metadata
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> NeonObjectStoreConfig {
        NeonObjectStoreConfig {
            endpoint: "https://br-test.storage.example".into(),
            region: "us-east-2".into(),
            access_key_id: "access".into(),
            secret_access_key: "secret".into(),
            property_public_bucket: "property-public".into(),
            property_private_bucket: "property-private".into(),
            vault_bucket: "vault".into(),
            public_cdn_base: None,
        }
    }

    #[test]
    fn distinct_buckets_are_required() {
        let mut c = config();
        c.vault_bucket = c.property_private_bucket.clone();
        assert!(c.validate().is_err());
    }

    #[test]
    fn buckets_map_to_logical_domains() {
        let c = config();
        assert_eq!(
            c.bucket_name(ObjectBucket::PropertyMediaPublic),
            "property-public"
        );
        assert_eq!(
            c.bucket_name(ObjectBucket::PropertyMediaPrivate),
            "property-private"
        );
        assert_eq!(c.bucket_name(ObjectBucket::Vault), "vault");
    }

    #[test]
    fn non_https_endpoint_is_rejected() {
        let mut c = config();
        c.endpoint = "http://localhost".into();
        assert!(c.validate().is_err());
    }

    #[tokio::test]
    #[ignore = "requires MEDIA_OBJECT_STORE_* and MEDIA_BUCKET_* dev credentials"]
    async fn neon_object_store_roundtrip() {
        let Some(store) = from_env().expect("media object store config") else {
            panic!("media object store env is not configured");
        };
        let bytes = b"culebraluxe object-store roundtrip";
        let key = "media/probe/roundtrip.txt";
        let stored = store
            .put_object(ObjectBucket::PropertyMediaPublic, key, bytes, "text/plain")
            .await
            .expect("put");
        assert_eq!(stored.size, bytes.len() as i64);
        assert_eq!(stored.sha256.as_deref(), Some(sha256_hex(bytes).as_str()));
        let head = store
            .head_object(ObjectBucket::PropertyMediaPublic, key)
            .await
            .expect("head");
        assert_eq!(head.content_type.as_deref(), Some("text/plain"));
        let url = store
            .signed_read_url(
                ObjectBucket::PropertyMediaPublic,
                key,
                Duration::from_secs(60),
            )
            .await
            .expect("signed read");
        assert!(url.starts_with("https://"));
        let verified = store
            .verify_object(
                ObjectBucket::PropertyMediaPublic,
                key,
                stored.sha256.as_deref().expect("sha"),
                bytes.len() as i64,
            )
            .await
            .expect("verify");
        assert_eq!(verified.size, bytes.len() as i64);
        store
            .delete_object(ObjectBucket::PropertyMediaPublic, key)
            .await
            .expect("delete");
        assert!(!store
            .exists(ObjectBucket::PropertyMediaPublic, key)
            .await
            .expect("exists"));
    }
}
