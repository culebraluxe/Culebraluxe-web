//! Database boundary and repository infrastructure.
//!
//! The canonical SQL migration history remains at repository-level
//! `db/migrations/`. This crate is the only Rust workspace crate that owns a
//! PostgreSQL pool or transaction. Domain-specific DAOs receive database
//! capability from here; callers never construct raw pools or SQLx transactions.

mod calendar;
mod capture;
mod client;
mod cockpit;
mod comms;
mod contract;
mod deal_portal;
mod error;
mod firm;
mod forge_control;
mod flight_recorder;
mod forms;
mod guest;
mod guide;
mod media;
mod person;
mod pool;
mod project;
mod property;
mod public_listing;
mod website_lead;
// Public because the `retrying_read!` macro expands inside other crates and has to name these helpers there.
pub mod accounting;
pub mod metrics;
pub mod retry;
mod security;
mod task;
mod tech;
// The one pool per process. Public because it is how the composition root hands its pool to components that cannot be
// constructed with one - the workflow engine's store, the Forge session helper.
pub mod shared;
mod showing;
mod signature;
mod transaction;
mod vault;
mod wbs;
mod workflow_portal;
mod workflow_ops;

pub use accounting::AccountingDao;
pub use calendar::CalendarDao;
pub use capture::{has_sink, on_failure};
pub use client::ClientDao;
pub use cockpit::CockpitDao;
pub use comms::CommsDao;
pub use contract::ContractDao;
pub use deal_portal::DealPortalDao;
pub use domain;
pub use error::{DbFailure, DbFailureKind, DbResult};
pub use firm::FirmDao;
pub use forge_control::{FlightFireResult, ForgeControlDao, LearnStaleClaimRow, ReadyAgentWorkRow, StaleAgentWorkRow};
pub use flight_recorder::FlightRecorderDao;
pub use forms::FormDao;
pub use guest::{GuestDao, GUEST_CODE_MAX_ATTEMPTS};
pub use guide::GuideDao;
pub use media::{
    BeginMediaUpload, MediaDao, MediaDerivativeInput, MediaUploadAssembly, MediaUploadStatus,
};
pub use person::PersonDao;
pub use pool::{resolve_declared_target, Database, DbTarget};
pub use project::{ProjectDao, ProjectTxDao};
pub use property::PropertyDao;
pub use public_listing::PublicListingDao;
pub use retry::{retry, RetryPolicy};
pub use security::SecurityDao;
pub use showing::ShowingDao;
pub use signature::SignatureDao;
pub use task::TaskDao;
pub use tech::TechCockpitDao;
pub use transaction::DbTransaction;
pub use vault::VaultDao;
pub use wbs::WbsDao;
pub use website_lead::WebsiteLeadDao;
pub use workflow_portal::WorkflowPortalDao;
pub use workflow_ops::{PendingTimerJobRow, WorkflowOpsDao, WorkflowStatusRow};
