use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use model::{
    AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset, UploadPropertyMediaRequest,
    UploadPropertyMediaResult,
};
use sqlx::FromRow;

/// One legacy media row selected for object-store migration. `file_data` is loaded deliberately:
/// the migration must hash the exact stored bytes before uploading them.
#[derive(Debug, sqlx::FromRow)]
pub struct MediaMigrationRow {
    pub id: String,
    pub media_type: String,
    pub mime_type: String,
    pub filename: String,
    pub file_data: Vec<u8>,
    pub derivative_of: Option<String>,
    pub derivative_kind: Option<String>,
    pub property_id: Option<String>,
    pub is_public: bool,
    pub storage_status: String,
}

/// Where a media row's bytes live, read without touching `file_data`.
#[derive(Debug, Clone)]
pub struct MediaObjectLocation {
    pub bucket: String,
    pub key: String,
    pub status: String,
    pub visibility: String,
}

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
    /// `public`, `private` or `vault`; `None` leaves the existing value unchanged.
    pub visibility: Option<String>,
}

mod assemble_media_upload;
mod media_row;
#[allow(unused_imports)]
pub use assemble_media_upload::*;
#[allow(unused_imports)]
pub use media_row::*;
