pub mod imaging;

use crate::service_support::{audit_result, authorize, CoreServiceError};
use apis::mux::MuxClient;
use async_trait::async_trait;
use db::{DbResult, MediaDao};
use model::{
    sanitize_media_filename, AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset,
    UploadPropertyMediaRequest, UploadPropertyMediaResult, MAX_MEDIA_UPLOAD_BYTES,
};
use serde::Serialize;
use services::{
    AbstractObjectStoreService, ObjectBucket, OperationKind, ServiceContext,
    ServiceInfrastructure, ServiceRuntime,
};
use std::sync::Arc;
use std::time::Duration;
mod attach_property_video;
mod document_store;
mod neon_object_store;
mod media_bytes;
mod media_repository;
#[allow(unused_imports)]
pub use attach_property_video::*;
#[allow(unused_imports)]
pub use media_bytes::*;
#[allow(unused_imports)]
pub use media_repository::*;
pub use document_store::{DocumentObjectWriter, StoredDocument};
pub use neon_object_store::{from_env as object_store_from_env, NeonObjectStore, NeonObjectStoreConfig};

/// Phase 4 read switch. Default off; only the exact enabling spellings turn it on.
pub fn object_reads_from_env() -> bool {
    matches!(
        std::env::var("MEDIA_OBJECT_STORAGE_READS")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}
