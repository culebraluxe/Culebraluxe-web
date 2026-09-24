//! Shared CulebraLuxe service kernel.
//!
//! This crate owns transport-neutral service context plus the authorization,
//! audit, and domain-event ports used by authoritative business services.

mod audit;
mod authorization;
mod context;
mod events;
mod runtime;
mod signature_provider;

pub use audit::{AuditPort, CapturingAuditPort, ServiceAuditEvent, ServiceOutcome};
pub use authorization::{
    AuthorizationDecision, AuthorizationPort, AuthorizationRequest, DefaultAuthorizationPort,
};
pub use context::{
    AUTHJS_EDGE_ACTOR, OperationKind, ServiceActor, ServiceActorKind, ServiceContext,
    ServicePrincipal,
};
pub use events::{CapturingDomainEventPort, DomainEventPort, ServiceDomainEvent};
pub use runtime::{ServiceInfrastructure, ServicePortError, ServiceRuntime, ServiceRuntimeError};

pub use signature_provider::SignatureProvider;
