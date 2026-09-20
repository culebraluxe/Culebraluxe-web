//! Forge runtime.
//!
//! Forge preserves the existing SDLC role boundaries and execution invariants.
//! QA has no Git/release authority. Release concerns belong to DEV_OPS/release.

pub mod evidence;
pub mod execution;
pub mod release;
pub mod roles;
pub mod routing;
pub mod runtime;
pub mod engine;
