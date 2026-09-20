//! Projects domain application boundary.

mod repository;
mod service;

pub use repository::ProjectRepository;
pub use service::{ProjectService, ProjectServiceError};
