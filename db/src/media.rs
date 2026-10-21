use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use model::{
    AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset, UploadPropertyMediaResult,
};
use sqlx::FromRow;

/// Where a media row's bytes live, read without touching `file_data`.
#[derive(Debug, Clone)]
pub struct MediaObjectLocation {
    pub bucket: String,
    pub key: String,
    pub status: String,
    pub visibility: String,
}

/// One object-first media row: the DB metadata plus its verified object pointer. No bytes.
#[derive(Debug, Clone)]
pub struct MediaObjectRow {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub alt_text: Option<String>,
    pub media_type: String,
    pub derivative_of: Option<String>,
    pub derivative_kind: Option<String>,
    pub object: MediaObjectMetadata,
}

/// Outcome of a shadow/object write for one media row. `bucket` is the logical storage domain
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

mod object_write;
mod media_row;
mod object_media;
#[allow(unused_imports)]
pub use object_write::*;
pub use object_media::{
    insert_media_object_conn, insert_media_object_tx, insert_media_object_with_id_conn,
    insert_media_object_with_id_tx,
};
#[allow(unused_imports)]
pub use media_row::*;
