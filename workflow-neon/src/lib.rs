//! Neon/PostgreSQL adapter for `workflow-core`: the pool, retry/timeout policy, SQL and store.
//!
//! This crate is the only place `db`, `sqlx` and `tokio` may be used by the workflow kernel. The
//! `crate::` names the moved Neon modules use are re-exported from `workflow-core` here so the
//! adapter keeps its engine types without the core depending on the app database crate.

pub use workflow_core::*;

pub mod neon;
pub mod sql_contract;

pub use neon::NeonStore;
