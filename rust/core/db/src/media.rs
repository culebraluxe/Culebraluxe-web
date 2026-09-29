use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use domain::{
    AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset, UploadPropertyMediaRequest,
    UploadPropertyMediaResult,
};
use sqlx::FromRow;
mod media_row;
mod assemble_media_upload;
#[allow(unused_imports)]
pub use media_row::*;
#[allow(unused_imports)]
pub use assemble_media_upload::*;

