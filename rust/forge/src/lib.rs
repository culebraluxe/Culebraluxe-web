//! Forge runtime.
//!
//! Forge preserves the existing SDLC role boundaries and execution invariants.
//! QA has no Git/release authority. Release concerns belong to DEV_OPS/release.

pub mod engine;
pub mod evidence;
pub mod execution;
pub mod release;
pub mod roles;
pub mod routing;
pub mod runtime;
// The Forge doctor's own rules, in Rust: the pure halves the operator command composes. They live here
// rather than in the CLI because they decide what is TRUE about the control plane (a claim's age, whether a
// QA verdict agrees with the run that produced it, what an ROI row means) — the CLI decides only how it is
// printed.
pub mod doctor_report;
pub mod qa_consistency;
pub mod roi;
