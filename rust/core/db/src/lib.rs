//! Database boundary and repository infrastructure.
//!
//! The canonical SQL migration history remains at repository-level
//! `db/migrations/`. This crate is the only Rust workspace crate that owns a
//! PostgreSQL pool. Domain-specific DAOs live here and receive that shared pool
//! through `Database`; callers never construct raw pools.

mod error;
mod pool;
mod project;

pub use domain;
pub use error::{DbFailure, DbFailureKind, DbResult};
pub use pool::{resolve_declared_target, Database, DbTarget};
pub use project::ProjectDao;
