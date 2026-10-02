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
pub mod service;
// The Forge doctor's own rules, in Rust: the pure halves the operator command composes. They live here
// rather than in the CLI because they decide what is TRUE about the control plane (a claim's age, whether a
// QA verdict agrees with the run that produced it, what an ROI row means) — the CLI decides only how it is
// printed.
pub mod doctor_report;
pub mod qa_consistency;
pub mod roi;
// The scope manifest's rules, in Rust: lanes, scoring, rendering, drift and the write refusal. Pure, so the
// ranking can be asserted without a filesystem or a git binary — the gather-and-print half is
// `rust/cli/src/forge/manifest.rs`, the same split as the doctor. Ported from `lib/scope-manifest.ts` +
// `scripts/forge-manifest.ts`, deleted with the TypeScript application in `4cf98110`.
pub mod scope_manifest;
// Cloud-sync debris (a numbered sibling beside its original). Two callers need ONE definition: the manifest
// generator must not index debris, and the `health --fix` cleanup must not delete a file a person wrote.
// Ported from `lib/git/sync-conflict.ts`, deleted in the same commit.
pub mod sync_conflict;

pub use service::ForgeService;
