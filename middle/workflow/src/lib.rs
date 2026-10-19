//! Compatibility façade: `workflow-core` plus the `workflow-neon` adapter under the historical
//! `workflow::` paths.
//!
//! W-1 steps 1-4 moved the pure kernel into `workflow-core` and the Postgres/Neon adapter into
//! `workflow-neon`; this crate keeps `workflow::` stable for the Forge/app hosts until they consume
//! the crates directly (W-1 steps 5+, then W-2 pinned versions).

pub use workflow_core::*;
pub use workflow_neon::{neon, sql_contract, NeonStore};
