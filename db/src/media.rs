use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use model::{
    AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset, UploadPropertyMediaRequest,
    UploadPropertyMediaResult,
};
use sqlx::FromRow;

/// Outcome of a shadow write for one media row. `bucket` is the logical storage domain
/// (`property_media_public`, `property_media_private`, `vault`), not a provider-specific name.
#[derive(Debug, Clone)]
pub struct MediaObjectMetadata {
    pub bucket: String,
    pub key: String,
    pub etag: Option<String>,
    pub sha256: Option<String>,
    pub size: i64,
    pub status: String,
}

mod assemble_media_upload;
mod media_row;
#[allow(unused_imports)]
pub use assemble_media_upload::*;
#[allow(unused_imports)]
pub use media_row::*;
