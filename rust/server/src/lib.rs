//! Authoritative CulebraLuxe server/backend.
//!
//! HTTP transport will be added here incrementally while the React/Next UI
//! remains in the existing TypeScript application.

pub mod accounting;
pub mod agreement_execution;
pub mod api;
pub mod calendar;
pub mod clients;
pub mod cockpit;
pub mod command_runtime;
pub mod communications;
pub mod composition;
pub mod contracts;
pub mod deals;
pub mod firms;
pub mod flight_recorder;
pub mod forms;
pub mod guide;
pub mod http_runtime;
pub mod intake;
pub mod issues;
pub mod lookup;
pub mod marketing;
pub mod media;
pub mod mq_runtime;
pub mod observability;
pub mod people;
pub mod projects;
pub mod properties;
pub mod public_listings;
pub mod relationship_evidence;
pub mod security;
pub mod service_bootstrap;
pub mod service_gateway;
pub mod service_harness;
pub mod service_kernel;
pub mod service_observability;
pub mod service_support;
pub mod showings;
pub mod signature;
pub mod support;
pub mod task;
pub mod tech;
pub mod vault;
pub mod wbs;
pub mod website_leads;
pub mod whatsapp;
pub mod workflow_portal;

pub use agreement_execution::Crm26AgreementExecutionSubscriber;
pub use command_runtime::{CommandDispatchError, CommandDispatcher};
pub use composition::CoreServices;
pub use mq_runtime::{
    MqProofSubscriber, MqRuntime, MqRuntimeConfig, MqSubscriber, MqSubscriberError,
};
pub use service_gateway::ServiceGateway;
pub use service_harness::{ServiceHarness, ServiceHarnessHealth};
pub use service_kernel::{ServiceKernel, ServiceKernelHealth, ServiceRegistry};
