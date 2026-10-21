//! Shared PDF/document object writer for Vault-owned artifacts.
//!
//! The writer owns bytes and verification only. Business services decide when a document is created
//! and the DB transaction owns links/state. Vault documents are always `Vault`/`vault`; the bucket
//! cannot be chosen by a caller.

use services::{AbstractObjectStoreService, ObjectBucket, ObjectMetadata, ObjectStoreError};
use std::sync::Arc;

/// A verified document object waiting for its DB row. The key is stable for this media id.
#[derive(Debug, Clone)]
pub struct StoredDocument {
    pub key: String,
    pub metadata: ObjectMetadata,
    pub mime_type: String,
}

pub struct DocumentObjectWriter {
    store: Arc<dyn AbstractObjectStoreService>,
}

impl DocumentObjectWriter {
    pub fn new(store: Arc<dyn AbstractObjectStoreService>) -> Self {
        Self { store }
    }

    /// Upload and verify one document. The caller must insert the DB row and, if that fails, call
    /// [`Self::discard`] with the returned key.
    pub async fn put(
        &self,
        media_id: &str,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> Result<StoredDocument, ObjectStoreError> {
        let extension = match mime_type.to_ascii_lowercase().as_str() {
            "application/pdf" => "pdf",
            "image/jpeg" | "image/jpg" => "jpg",
            "image/png" => "png",
            _ => filename.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("bin"),
        };
        let key = format!("documents/{media_id}/original.{extension}");
        let stored = self
            .store
            .put_object(ObjectBucket::Vault, &key, bytes, mime_type)
            .await?;
        let sha = stored
            .sha256
            .clone()
            .ok_or_else(|| ObjectStoreError::integrity("provider returned no SHA-256"))?;
        if let Err(error) = self
            .store
            .verify_object(ObjectBucket::Vault, &key, &sha, stored.size)
            .await
        {
            // A verification failure must not leave an unverified object behind.
            let _ = self.store.delete_object(ObjectBucket::Vault, &key).await;
            return Err(error);
        }
        Ok(StoredDocument {
            key,
            metadata: ObjectMetadata {
                sha256: Some(sha),
                ..stored
            },
            mime_type: mime_type.to_owned(),
        })
    }

    pub async fn discard(&self, key: &str) {
        let _ = self.store.delete_object(ObjectBucket::Vault, key).await;
    }
}
