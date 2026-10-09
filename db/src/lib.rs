//! Database boundary and repository infrastructure.
//!
//! The canonical SQL migration history remains at repository-level
//! `db/migrations/`. This crate is the only Rust workspace crate that owns a
//! PostgreSQL pool or transaction. Domain-specific DAOs receive database
//! capability from here; callers never construct raw pools or SQLx transactions.

mod agreement_execution;
mod app_error;
mod apple_ods;
mod boot_gate;
// LISA'S PRE-SIGNATURE IS PORTED AND NOT YET WIRED, AND THAT IS WHY THIS MODULE IS ALLOWED TO BE UNUSED.
//
// `broker_signature.rs` holds the rows, the authority and the drawing its port produced, and nothing calls it: the
// vault issuance path still works without it. Its ten dead-code warnings were the loudest thing in
// `cargo check --workspace`, and deleting the file to quieten them would delete the port.
//
// So the allowance is here, ON THE MODULE DECLARATION, with the reason, and it is one line to remove: wiring the
// issuance path in `server/src/vault` deletes this attribute and the warnings come back if the wiring is partial.
// (Found 2026-09-28 while making the workspace warning-free for `RUSTFLAGS=-D warnings`.)
#[allow(dead_code)]
mod broker_signature;
mod calendar;
mod capture;
mod catch_up;
mod client;
mod client_room;
mod cockpit;
mod command_receipt;
mod comms;
mod contract;
mod deal_portal;
mod document_sign;
mod email;
mod error;
mod firm;
mod flight_recorder;
mod forge_control;
// The reads behind `forge doctor`: the control plane in one command, read-only.
pub mod forge_doctor;
// The Forge reset / recover / clean writer. One writer, three modes, all of them deliberate.
mod forge_engine;
pub mod forge_reset;
// The sanctioned Forge read path (the `forge board` / `forge story-show` / `forge batch-status` operator
// tools). Public because the CLI is the caller and SQL belongs here, not there.
pub mod forge_read;
mod forms;
mod guest;
mod guide;
mod intake;
mod issue;
mod landing;
mod marketing;
mod media;
mod outbox;
mod person;
mod pool;
mod project;
mod property;
mod public_listing;
mod publishing;
mod relationship_evidence;
mod website_lead;
mod whatsapp;
// Public because the `retrying_read!` macro expands inside other crates and has to name these helpers there.
pub mod accounting;
pub mod metrics;
pub mod retry;
// Schema parity (the `pnpm db:parity` release gate). Public and pure: a snapshot in, a report out, so it is
// unit testable without a database and a Rust DEV_OPS gate can share the one comparison.
mod schema_migration;
pub mod schema_parity;
mod security;
mod security_audit;
mod task;
mod tech;
// The one pool per process. Public because it is how the composition root hands its pool to components that cannot be
// constructed with one - the workflow engine's store, the Forge session helper.
pub mod shared;
mod showing;
mod signature;
mod signer;
mod support;
mod transaction;
mod unit_of_work;
mod vault;
mod wbs;
mod workflow_ops;
mod workflow_portal;

pub use accounting::AccountingDao;
pub use agreement_execution::{AgreementExecutionDao, IssuedAgreementDocumentRow};
pub use app_error::AppErrorDao;
pub use apple_ods::{
    AppleMailLanding, EmailLanding, IntakeCheckpoint, IntakeCheckpointUpdate, InteractionDraft,
};
pub use boot_gate::{assert_boot_ready, missing_required_migrations, REQUIRED_BOOT_MIGRATIONS};
pub use calendar::CalendarDao;
pub use capture::{has_sink, on_failure};
pub use catch_up::CatchUpDao;
pub use client::ClientDao;
pub use client_room::ClientRoomDao;
pub use cockpit::CockpitDao;
pub use command_receipt::{CommandReceiptDao, CommandReceiptRow};
pub use comms::CommsDao;
pub use contract::{ContractDao, ContractTxDao};
pub use deal_portal::DealPortalDao;
pub use document_sign::DocumentSignDao;
pub use email::EmailDao;
pub use error::{DbFailure, DbFailureKind, DbResult};
pub use firm::FirmDao;
pub use flight_recorder::FlightRecorderDao;
pub use forge_control::{
    wait_for_forge_work, FlightFireResult, ForgeControlDao, ForgeRuntimeControlRow,
    LearnStaleClaimRow, StaleAgentWorkRow,
};
pub use forge_doctor::{
    ClaimRow, ControlPlaneCounts, EngineQueuedCardRow, EngineRunCardRow, ForgeDoctorDao, QaRunRow,
    RoiAttemptRow,
};
pub use forge_engine::{
    AgentWorkOutcome, AgentWorkSettlement, BeginAgentWorkRun, ClaimFence, CompletionApply,
    CompletionSpend, CompletionUnit, DealWorkflowFactRow, DispatchReconcile, EnsureDispatch,
    ForgeAgentWorkRow, ForgeDecisionRow, ForgeEngineDao, ForgeEvidencePatch, ForgeHoldRow,
    NewToolArtifact, ProcessDefinitionRow, SettlementResult, StoryPacketRow, ToolArtifactRow,
    UnfinishedForgeCompletion, WorkflowCommandReceiptRow, WorkflowReceiptClaim,
};
pub use forge_read::{
    ForgeBatchRow, ForgeBenchRow, ForgeQueueWorkRow, ForgeReadDao, ForgeStoryBoardRow,
    ForgeStoryFindingRow, ForgeStoryHoldRow, ForgeStoryMigrationRow, ForgeStoryReceiptRow,
    ForgeStoryShow, ForgeStoryStatusRow,
};
pub use forge_reset::{ForgeResetDao, RemainingCounts, ResetReport};
pub use forms::FormDao;
pub use guest::{GuestDao, GUEST_CODE_MAX_ATTEMPTS};
pub use guide::GuideDao;
pub use intake::IntakeDao;
pub use issue::IssueDao;
pub use landing::{
    CallLanding, ImessageLanding, LandingDao, LatestInteraction, LatestInteractionOutcome,
};
pub use marketing::MarketingDao;
pub use media::{
    BeginMediaUpload, MediaDao, MediaDerivativeInput, MediaUploadAssembly, MediaUploadStatus,
};
pub use model;
pub use outbox::{DomainEventOutboxDao, OutboxDelivery, OutboxEventInput};
pub use person::PersonDao;
pub use pool::{
    disable_statement_timeout, resolve_declared_target, resolve_forge_target, Database, DbTarget,
};
pub use project::{ProjectDao, ProjectTxDao};
pub use property::PropertyDao;
pub use public_listing::PublicListingDao;
pub use publishing::PublishingDao;
pub use relationship_evidence::{
    EvidenceCoverage, EvidenceUpsert, PersonIdentityOwner, RelationshipEvidenceDao,
    SourcePersonLink,
};
pub use retry::{retry, RetryPolicy};
pub use schema_migration::{MigrationLedgerRow, SchemaMigrationDao};
pub use security::{IdentityPrincipal, SecurityDao};
pub use security_audit::SecurityAuditDao;
pub use showing::ShowingDao;
pub use signature::SignatureDao;
pub use signer::{
    FinalizeEvent, FinalizeField, FinalizeInputs, FinalizeRecipient, SignerAccessRecord, SignerDao,
};
pub use support::SupportDiagnosticsDao;
pub use task::TaskDao;
pub use tech::{
    TechCockpitDao, LAUNCH_FLIGHT_FIRE_STORIES_SQL, LAUNCH_FLIGHT_QUEUE_ITEMS_SQL,
    LAUNCH_FLIGHT_ROUTE_ITEMS_SQL,
};
pub use transaction::DbTransaction;
pub use unit_of_work::{service_mutation, DbConnection};
pub use vault::VaultDao;
pub use wbs::WbsDao;
pub use website_lead::WebsiteLeadDao;
pub use whatsapp::{
    WhatsAppCanonicalInput, WhatsAppDao, WhatsAppLandingInput, WhatsAppProcessOutcome,
};
pub use workflow_ops::{PendingTimerJobRow, WorkflowOpsDao, WorkflowStatusRow};
pub use workflow_portal::WorkflowPortalDao;
