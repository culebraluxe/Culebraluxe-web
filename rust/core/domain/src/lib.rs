//! Infrastructure-free CulebraLuxe domain types and rules.
//!
//! This crate must not depend on database, HTTP, process, or provider adapters.

pub mod firm;
pub mod person;
pub mod project;
pub mod property;
pub mod showing;
pub mod security;

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
