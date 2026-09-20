//! Infrastructure-free CulebraLuxe domain types and rules.
//!
//! This crate must not depend on database, HTTP, process, or provider adapters.

pub mod comms;
pub mod contract;
pub mod firm;
pub mod person;
pub mod project;
pub mod property;
pub mod security;
pub mod showing;
pub mod wbs;

pub use firm::{FieldPatch, Firm, UpsertFirmRequest};
pub use person::{
    AttachPersonIdentityRequest, Person, PersonIdentity, PersonIdentityKind, PersonSearchResult,
    SearchPeopleRequest, SetPersonDisplayNameRequest,
};
pub use project::{
    CompleteProjectRequest, CreateProjectRequest, Project, ProjectStatus, UpdateProjectRequest,
    WbsCategory,
};

pub use property::{
    FindPropertyByAddressRequest, PersonPropertyContext, PersonPropertyRelation, Property,
    PropertyAddress, PropertyAddressPatch, PropertyForPerson, SetPropertyDisplayNameRequest,
    SetPropertyStatusRequest, UpsertPropertyForPersonRequest,
};

pub use security::{
    resolve_security_level, ActingUser, SecurityIdentityResolution, SecurityLevel,
    SecurityPrincipal,
};

pub use showing::{SaveShowingReportRequest, Showing, ShowingReportOutcome};

pub use wbs::{
    CreateWbsItemRequest, SaveWbsItemRequest, WbsEntityLink, WbsEntityType, WbsItem, WbsStatus,
};

pub use contract::{
    Contract, ContractEffectiveState, ContractFacts, ContractRole, ContractSummary,
    CreateContractFromFormRequest, ExecuteContractRequest, SaveContractDraftRequest,
    CONTRACT_FIRM_ROLE_CODES, CONTRACT_PERSON_ROLE_CODES,
};

pub use comms::{
    active_source_count, is_facetime_interaction, moment_channel_for, moment_dto,
    source_channel_for, source_dto, summarize_relationship_evidence, CommsAggregate,
    CommsDirection, CommsMoment, CommsMomentChannel, CommsMomentPage, CommsMomentRecord,
    CommsPanel, CommsSource, CommsSourceChannel, CommsSourceRecord, CommsTimeline,
    GetCommsPanelRequest, GetCommsTimelineRequest, LastContactRecord,
    RelationshipEvidenceRecord, RelationshipSummary, COMMS_MAX_PAGE_SIZE, COMMS_MOMENT_LIMIT,
    COMMS_PAGE_SIZE, COMMS_SOURCE_SLOT_COUNT,
};
