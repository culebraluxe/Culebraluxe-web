//! Compatibility façade: the host-neutral `workflow-core` primitives plus the Neon adapter.
//!
//! Step 1-2 of W-1 moved the pure modules (types/value/status/ids/sha256/error/expr/json_codec/
//! store/memory/concurrency and the engine) into `workflow-core`; this crate re-exports them under
//! the historical `workflow::` paths so existing hosts compile unchanged. The Neon adapter
//! (`neon`, `NeonStore`, `sql_contract`) stays here until W-1 step 4.

pub use model;

pub use workflow_core::*;

pub mod neon;
pub mod sql_contract;

pub use neon::NeonStore;
