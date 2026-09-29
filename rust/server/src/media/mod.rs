pub mod imaging;

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, MediaDao};
use domain::{
    sanitize_media_filename, AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset,
    UploadPropertyMediaRequest, UploadPropertyMediaResult, MAX_MEDIA_UPLOAD_BYTES,
};
use integrations::mux::MuxClient;
use serde::Serialize;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
mod media_repository;
mod media_bytes;
mod attach_property_video;
#[allow(unused_imports)]
pub use media_repository::*;
#[allow(unused_imports)]
pub use media_bytes::*;
#[allow(unused_imports)]
pub use attach_property_video::*;

