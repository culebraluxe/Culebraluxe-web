//! Database boundary and repository infrastructure.
//!
//! The canonical SQL migration history remains at repository-level
//! `db/migrations/`. This crate is the only Rust workspace crate that owns a
//! PostgreSQL pool or transaction. Domain-specific DAOs receive database
//! capability from here; callers never construct raw pools or SQLx transactions.

mod calendar;
mod capture;
mod client;
mod comms;
mod contract;
mod error;
mod firm;
mod forms;
mod media;
mod person;
mod pool;
mod project;
mod property;
// Public because the `retrying_read!` macro expands inside other crates and has to name these helpers there.
pub mod metrics;
pub mod retry;
mod security;
// The one pool per process. Public because it is how the composition root hands its pool to components that cannot be
// constructed with one - the workflow engine's store, the Forge session helper.
pub mod shared;
mod showing;
mod signature;
mod transaction;
mod vault;
mod wbs;

pub use calendar::CalendarDao;
pub use capture::{has_sink, on_failure};
pub use client::ClientDao;
pub use comms::CommsDao;
pub use contract::ContractDao;
pub use domain;
pub use error::{DbFailure, DbFailureKind, DbResult};
pub use firm::FirmDao;
pub use forms::FormDao;
pub use media::MediaDao;
pub use person::PersonDao;
pub use pool::{resolve_declared_target, Database, DbTarget};
pub use project::{ProjectDao, ProjectTxDao};
pub use property::PropertyDao;
pub use retry::{retry, RetryPolicy};
pub use security::SecurityDao;
pub use showing::ShowingDao;
pub use signature::SignatureDao;
pub use transaction::DbTransaction;
pub use vault::VaultDao;
pub use wbs::WbsDao;
