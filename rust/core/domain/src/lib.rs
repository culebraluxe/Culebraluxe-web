//! Infrastructure-free CulebraLuxe domain types and rules.
//!
//! This crate must not depend on database, HTTP, process, or provider adapters.

pub mod accounting;
pub mod calendar;
pub mod client;
pub mod cockpit;
pub mod comms;
pub mod contract;
pub mod deal_portal;
pub mod firm;
pub mod flight_recorder;
pub mod forms;
pub mod guide;
pub mod issue;
pub mod media;
pub mod person;
pub mod project;
pub mod property;
pub mod public_listing;
pub mod relationship_evidence;
pub mod security;
pub mod showing;
pub mod signature;
pub mod support;
pub mod task;
pub mod tech;
pub mod vault;
pub mod wbs;
pub mod website_lead;
pub mod workflow_portal;

pub use accounting::*;
pub use client::*;
pub use cockpit::*;
pub use firm::{FieldPatch, Firm, UpsertFirmRequest};
pub use person::{
    AttachPersonIdentityRequest, Person, PersonIdentity, PersonIdentityKind, PersonSearchResult,
    SearchPeopleRequest, SetPersonDisplayNameRequest, UpdatePersonAdminRequest,
};
pub use project::{
    CompleteProjectRequest, CreateProjectRequest, Project, ProjectStatus, UpdateProjectRequest,
    WbsCategory,
};

pub use property::{
    CreatePropertyAdminRequest, FindPropertyByAddressRequest, PersonPropertyContext,
    PersonPropertyRelation, Property, PropertyAddress, PropertyAddressPatch, PropertyAdminPage,
    PropertyAdminPageRequest, PropertyAdminRecord, PropertyAdminSummary, PropertyForPerson,
    PropertyStellarDetails, SavePropertyAdminRequest, SetPropertyDisplayNameRequest,
    SetPropertyStatusRequest, UpsertPropertyForPersonRequest,
};
pub use relationship_evidence::{RelationshipDecision, RelationshipEvidenceReview, RelationshipEvidenceRow, RelationshipReconcileResult};

pub use public_listing::{
    PublicListing, PublicListingCopy, PublicProperty, PublicPropertyImage, PublicPropertyMedia,
};
pub use website_lead::{WebsiteLead, WebsiteLeadNotice};

pub use security::{
    resolve_security_level, ActingUser, SecurityIdentityResolution, SecurityLevel,
    SecurityPrincipal,
};

pub use showing::{SaveShowingReportRequest, Showing, ShowingReportOutcome};
pub use support::*;

pub use wbs::{
    AppleReminderCommandReceipt, AppleReminderUpsertRequest, CreateWbsItemRequest,
    SaveWbsItemRequest, WbsEntityLink, WbsEntityType, WbsItem, WbsStatus,
};

pub use contract::{
    Contract, ContractEffectiveState, ContractFacts, ContractRole, ContractSummary,
    CreateContractFromFormRequest, ExecuteContractRequest, SaveContractDraftRequest,
    CONTRACT_FIRM_ROLE_CODES, CONTRACT_PERSON_ROLE_CODES,
};

pub use comms::{
    active_source_count, is_facetime_interaction, moment_channel_for, moment_dto,
    source_channel_for, source_dto, summarize_relationship_evidence, ActivityFeedEntry,
    CommsAggregate, CommsDirection, CommsMoment, CommsMomentChannel, CommsMomentPage,
    CommsMomentRecord, CommsPanel, CommsSource, CommsSourceChannel, CommsSourceRecord,
    CommsTimeline, GetCommsPanelRequest, GetCommsTimelineRequest, LastContactRecord,
    RelationshipEvidenceRecord, RelationshipSummary, COMMS_MAX_PAGE_SIZE, COMMS_MOMENT_LIMIT,
    COMMS_PAGE_SIZE, COMMS_SOURCE_SLOT_COUNT,
};

pub use flight_recorder::*;
pub use guide::GuideItem;
pub use issue::{IssueQueueRow, IssuesPage};

pub use forms::{
    BindFormInstanceToDirectContextRequest, BindFormInstanceToShowingRequest,
    BindListingFormContextRequest, CreateFormInstanceRequest, DealFormFacts, DirectFormContext,
    FormInstance, FormInstanceEvidence, FormInstanceListItem, FormInstanceStatus, FormSignerPerson,
    LatestFormEvidenceRequest, UpdateFormInstanceInput, UpdateFormInstanceRequest,
};

pub use vault::{
    ContractIssuedLineage, CreateTransactionDocumentRequest, IssueDocumentRequest,
    IssuedDocumentEvidence, IssuedDocumentForFormInstance, IssuedDocumentListItem,
    NextIssuedVersionRequest, SignedArtifactRef, TransactionDocument, TransactionDocumentSource,
    TransactionDocumentState, TransactionDocumentType, TransitionTransactionDocumentRequest,
    VaultActorScope, VaultArtifactFailure, VaultCommandOutcome, VaultCommandResult,
    VaultMediaBytes, VaultRenderRequest, VaultRenderedArtifact,
};

pub use calendar::{
    CalendarCommandReceipt, CalendarEvent, CalendarEventKind, CreateAppleCalendarEventRequest,
};

pub use media::{
    sanitize_media_filename, AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset,
    UploadPropertyMediaRequest, UploadPropertyMediaResult, MAX_MEDIA_UPLOAD_BYTES,
};

pub use signature::{
    normalize_signature_email, validate_signature_recipients, ApplySignatureStatusRequest,
    IssuedParticipantSlot, SendSignatureRequest, SignatureArtifactDownload,
    SignatureCommandOutcome, SignatureCommandResult, SignatureProviderActionResult,
    SignatureProviderEvent, SignatureProviderSendRequest, SignatureProviderSendResult,
    SignatureProviderStatusResult, SignatureRecipient, SignatureRecipientRole, SignatureRequest,
    SignatureRequestResult, SignatureRequestStatus, SignatureStatusResult,
    SignatureWebhookVerification,
};

pub use task::TaskCompletion;
pub use tech::*;

pub use workflow_portal::*;

pub use deal_portal::*;
