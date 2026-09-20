//! Database boundary and repository infrastructure.
//!
//! The canonical SQL migration history remains at repository-level
//! `db/migrations/`. This crate is the only Rust workspace crate that owns a
//! PostgreSQL pool or transaction. Domain-specific DAOs receive database
//! capability from here; callers never construct raw pools or SQLx transactions.

mod error;
mod firm;
mod person;
mod pool;
mod project;
mod property;
mod security;
mod showing;
mod transaction;
mod wbs;

pub use domain;
pub use error::{DbFailure, DbFailureKind, DbResult};
pub use firm::FirmDao;
pub use person::PersonDao;
pub use pool::{resolve_declared_target, Database, DbTarget};
pub use project::{ProjectDao, ProjectTxDao};
pub use property::PropertyDao;
pub use security::SecurityDao;
pub use showing::ShowingDao;
pub use transaction::DbTransaction;
pub use wbs::WbsDao;
