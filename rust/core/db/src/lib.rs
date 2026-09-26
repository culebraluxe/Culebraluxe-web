//! Database boundary and repository infrastructure.
//!
//! The canonical SQL migration history remains at repository-level
//! `db/migrations/`. This crate is the only Rust workspace crate that owns a
//! PostgreSQL pool or transaction. Domain-specific DAOs receive database
//! capability from here; callers never construct raw pools or SQLx transactions.

mod app_error;
mod calendar;
mod capture;
mod client;
mod cockpit;
mod command_receipt;
mod comms;
mod contract;
mod deal_portal;
mod error;
mod firm;
mod flight_recorder;
mod forge_control;
mod forge_engine;
mod forms;
mod guest;
mod guide;
mod intake;
mod issue;
mod marketing;
mod media;
mod outbox;
mod person;
mod pool;
mod project;
mod property;
mod public_listing;
mod relationship_evidence;
mod website_lead;
mod whatsapp;
// Public because the `retrying_read!` macro expands inside other crates and has to name these helpers there.
pub mod accounting;
pub mod metrics;
pub mod retry;
mod security;
mod security_audit;
mod task;
mod tech;
// The one pool per process. Public because it is how the composition root hands its pool to components that cannot be
// constructed with one - the workflow engine's store, the Forge session helper.
pub mod shared;
mod showing;
mod signature;
mod support;
mod transaction;
mod vault;
mod wbs;
mod workflow_ops;
mod workflow_portal;

pub use accounting::AccountingDao;
pub use app_error::AppErrorDao;
pub use calendar::CalendarDao;
pub use capture::{has_sink, on_failure};
pub use client::ClientDao;
pub use cockpit::CockpitDao;
pub use command_receipt::{CommandReceiptDao, CommandReceiptRow};
pub use comms::CommsDao;
pub use contract::{ContractDao, ContractTxDao};
pub use deal_portal::DealPortalDao;
pub use domain;
pub use error::{DbFailure, DbFailureKind, DbResult};
pub use firm::FirmDao;
pub use flight_recorder::FlightRecorderDao;
pub use forge_control::{
    FlightFireResult, ForgeControlDao, LearnStaleClaimRow, ReadyAgentWorkRow, StaleAgentWorkRow,
};
pub use forge_engine::{
    DealWorkflowFactRow, ForgeAgentWorkRow, ForgeDecisionRow, ForgeEngineDao, ForgeEvidencePatch,
    ForgeHoldRow, ProcessDefinitionRow, StoryPacketRow, WorkflowCommandReceiptRow,
};
pub use forms::FormDao;
pub use guest::{GuestDao, GUEST_CODE_MAX_ATTEMPTS};
pub use guide::GuideDao;
pub use intake::IntakeDao;
pub use issue::IssueDao;
pub use marketing::MarketingDao;
pub use outbox::{DomainEventOutboxDao, OutboxEventInput, OutboxRecord};
pub use media::{
    BeginMediaUpload, MediaDao, MediaDerivativeInput, MediaUploadAssembly, MediaUploadStatus,
};
pub use person::PersonDao;
pub use pool::{resolve_declared_target, Database, DbTarget};
pub use project::{ProjectDao, ProjectTxDao};
pub use property::PropertyDao;
pub use public_listing::PublicListingDao;
pub use relationship_evidence::RelationshipEvidenceDao;
pub use retry::{retry, RetryPolicy};
pub use security::SecurityDao;
pub use security_audit::SecurityAuditDao;
pub use showing::ShowingDao;
pub use signature::SignatureDao;
pub use support::SupportDiagnosticsDao;
pub use task::TaskDao;
pub use tech::TechCockpitDao;
pub use transaction::DbTransaction;
pub use vault::VaultDao;
pub use wbs::WbsDao;
pub use website_lead::WebsiteLeadDao;
pub use whatsapp::{
    WhatsAppCanonicalInput, WhatsAppDao, WhatsAppLandingInput, WhatsAppProcessOutcome,
};
pub use workflow_ops::{PendingTimerJobRow, WorkflowOpsDao, WorkflowStatusRow};
pub use workflow_portal::WorkflowPortalDao;
