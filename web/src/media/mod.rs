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
    AbstractObjectStoreService, ObjectBucket, OperationKind, ServiceContext, ServiceInfrastructure,
    ServiceRuntime,
};
use std::sync::Arc;
use std::time::Duration;
mod attach_property_video;
mod direct_upload;
mod document_store;
mod media_bytes;
mod media_repository;
mod neon_object_store;
mod property_upload;
#[cfg(test)]
mod test_support;
#[allow(unused_imports)]
pub use attach_property_video::*;
#[allow(unused_imports)]
pub use direct_upload::*;
pub use document_store::{DocumentObjectWriter, StoredDocument};
#[allow(unused_imports)]
pub use media_bytes::*;
#[allow(unused_imports)]
pub use media_repository::*;
pub use neon_object_store::{
    from_env as object_store_from_env, NeonObjectStore, NeonObjectStoreConfig,
};
#[allow(unused_imports)]
pub use property_upload::*;

/// The retired read cutover switch (`MEDIA_OBJECT_STORAGE_READS`).
///
/// Media and public-listing reads are OBJECT-ONLY unconditionally and ignore this value; the flag and the builder
/// methods that take it survive only because `composition.rs` (outside this lane's write scope) still passes the
/// same boolean to the Vault, which is the last service with its own cutover switch. Do not reintroduce a blob
/// fallback behind it.
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
