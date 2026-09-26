//! Shared CulebraLuxe service kernel.
//!
//! This crate owns transport-neutral service context plus the authorization,
//! audit, and domain-event ports used by authoritative business services.

mod abstract_service;
mod audit;
mod authorization;
mod context;
mod events;
mod execution;
mod lifecycle;
mod mailbox;
mod observability;
mod router;
mod runtime;
mod signature_provider;

pub use abstract_service::{
    AbstractService, ServiceCapability, ServiceDescriptor, ServiceDispatchError, ServiceEnvelope,
    ServiceFailureClass,
};
pub use audit::{AuditPort, CapturingAuditPort, ServiceAuditEvent, ServiceOutcome};
pub use authorization::{
    AuthorizationDecision, AuthorizationPort, AuthorizationRequest, DefaultAuthorizationPort,
};
pub use context::{
    OperationKind, ServiceActor, ServiceActorKind, ServiceContext, ServicePrincipal,
    AUTHJS_EDGE_ACTOR,
};
pub use events::{CapturingDomainEventPort, DomainEventPort, ServiceDomainEvent};
pub use execution::{ServiceExecutionMode, ServiceExecutionPolicy};
pub use lifecycle::{
    ServiceControlCommand, ServiceControlResult, ServiceHealth, ServiceLifecycle,
    ServiceLifecycleError, ServiceStatus,
};
pub use mailbox::{ServiceMailbox, ServiceMailboxConfig};
pub use observability::{
    CapturingServiceAlertPort, CapturingServiceErrorSink, ServiceAlert, ServiceAlertPort,
    ServiceErrorRecord, ServiceErrorSink, ServiceFailureSeverity, TracingServiceAlertPort,
    TracingServiceErrorSink,
};
pub use router::{DeferredServiceRouter, ServiceRouter};
pub use runtime::{ServiceInfrastructure, ServicePortError, ServiceRuntime, ServiceRuntimeError};

pub use signature_provider::SignatureProvider;
