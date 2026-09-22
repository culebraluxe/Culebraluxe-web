//! Authoritative CulebraLuxe server/backend.
//!
//! HTTP transport will be added here incrementally while the React/Next UI
//! remains in the existing TypeScript application.

pub mod api;
pub mod calendar;
pub mod clients;
pub mod cockpit;
pub mod communications;
pub mod composition;
pub mod contracts;
pub mod firms;
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
pub mod vault;
pub mod wbs;

pub use composition::CoreServices;
