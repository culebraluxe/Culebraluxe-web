//! Infrastructure-free CulebraLuxe domain types and rules.
//!
//! This crate must not depend on database, HTTP, process, or provider adapters.

pub mod project;

pub use project::{
    CompleteProjectRequest, CreateProjectRequest, Project, ProjectStatus, UpdateProjectRequest,
    WbsCategory,
};
