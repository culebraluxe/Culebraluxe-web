//! Infrastructure-free CulebraLuxe domain types and rules.
//!
//! This crate must not depend on database, HTTP, process, or provider adapters.

pub mod accounting;
pub mod apple_messages;
pub mod calendar;
pub mod client;
pub mod client_room;
pub mod cockpit;
pub mod catch_up;
pub mod comms;
pub mod contract;
pub mod deal_portal;
pub mod firm;
pub mod flight_recorder;
pub mod forms;
pub mod forms_applied_signature;
pub mod forms_broker_signature;
pub mod forms_execution;
pub mod forms_font;
pub mod forms_font_metrics;
pub mod forms_format;
pub mod forms_geometry;
pub mod forms_template;
pub mod guide;
pub mod intake;
pub mod issue;
pub mod marketing;
pub mod media;
pub mod person;
pub mod project;
pub mod publishing;
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
pub use apple_messages::{
    APPLE_LOCAL_SOURCE_ACCOUNT, APPLE_MESSAGES_SOURCE, APPLE_PREVIEW_MAX_LENGTH,
    AppleHandleEvidence, AppleHandleLookup, AppleMessagesExport, AppleMessagesHandle,
    AppleMessagesMessage, IdentityEvidence, REL_INTEL_RULE_VERSION, apple_nanos_to_iso,
    apple_service_to_channel, bounded_preview, build_handle_evidence, decide_apple_handle,
    derive_source_account, effective_date_iso, fingerprint, handle_to_identities,
    is_group_chat_guid, normalize_email, normalize_phone,
};
pub use client::*;
pub use client_room::*;
pub use cockpit::*;
pub use catch_up::*;
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
    SetPropertyListingTypeRequest, SetPropertyStatusRequest, UpsertPropertyForPersonRequest,
};
pub use relationship_evidence::{
    RelationshipDecision, RelationshipEvidenceReview, RelationshipEvidenceRow,
    RelationshipReconcileResult,
};

pub use public_listing::{
    PublicListing, PublicListingCopy, PublicProperty, PublicPropertyImage, PublicPropertyMedia,
};
pub use publishing::{PublishingListing, PublishingSnapshot};
pub use website_lead::{WebsiteLead, WebsiteLeadNotice};

pub use security::{
    resolve_security_level, ActingUser, SecurityIdentityResolution, SecurityLevel,
    SecurityPrincipal,
};

pub use showing::{SaveShowingReportRequest, Showing, ShowingReportOutcome};
pub use support::*;

pub use wbs::{
    AppleReminderCommandReceipt, AppleReminderLanding, AppleReminderUpsertRequest,
    dependency_creates_cycle, validate_planned_dates, CreateWbsItemRequest, SaveWbsItemRequest,
    WbsDependency, WbsEntityLink, WbsEntityType, WbsItem, WbsStatus,
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
pub use intake::{
    CatchupLeadRequest, CatchupLeadResult, WebsiteIntakeRequest, WebsiteIntakeResult,
};
pub use issue::{IssueQueueRow, IssuesPage};
pub use marketing::{MarketingContentBlock, MarketingContentItem};

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
    CalendarCommandReceipt, CalendarCommandState, CalendarEvent, CalendarEventKind,
    CalendarLandingEvent, CalendarViewportQuery, CreateAppleCalendarEventRequest,
    UpdateAppleCalendarEventRequest,
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
