//! Authoritative CulebraLuxe server/backend.
//!
//! HTTP transport will be added here incrementally while the React/Next UI
//! remains in the existing TypeScript application.

pub mod api;
pub mod accounting;
pub mod calendar;
pub mod clients;
pub mod cockpit;
pub mod communications;
pub mod composition;
pub mod contracts;
pub mod deals;
pub mod firms;
pub mod flight_recorder;
pub mod forms;
pub mod lookup;
pub mod media;
pub mod people;
pub mod projects;
pub mod properties;
pub mod security;
pub mod service_support;
pub mod showings;
pub mod signature;
pub mod task;
pub mod vault;
pub mod wbs;
pub mod workflow_portal;

pub use composition::CoreServices;
