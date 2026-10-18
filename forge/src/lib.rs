//! Forge runtime.
//!
//! Forge preserves the existing SDLC role boundaries and execution invariants.
//! QA has no Git/release authority. Release concerns belong to DEV_OPS/release.

pub mod engine;
// The binaries' observability seam: one subscriber, installed once per process, so the engine's `tracing` events
// reach an operator's stderr instead of being dropped for want of a subscriber. `web/src/observability.rs` is the
// HTTP process's half of the same decision; both use `try_init`, so a process that links both keeps the first.
pub mod observability;
pub mod pianola;
// The five placeholder modules this file used to declare — `evidence`, `execution`, `routing`, `runtime`, `release` —
// are gone. Each held a one-line doc comment and no items, nothing referenced them, and their names invited policy code
// to land outside `engine/`, which is where every real implementation already lives (`engine::release` for release,
// `engine::diagnostics`/`engine::scope` for execution-scoped work, `engine::role_mapping` for lanes). A module that
// only sounds like an architecture is worse than no module: it makes the runtime look layered when it is not.
pub mod roles;
pub mod service;
// The Forge doctor's own rules, in Rust: the pure halves the operator command composes. They live here
// rather than in the CLI because they decide what is TRUE about the control plane (a claim's age, whether a
// QA verdict agrees with the run that produced it, what an ROI row means) — the CLI decides only how it is
// printed.
pub mod doctor_report;
pub mod qa_consistency;
pub mod roi;
// The scorecard's rules, in Rust: per-role/agent outcomes from the durable run rows, pure, so the honesty
// rules (a null metric is not zero; an unpopulated one prints NOT CAPTURED; widgets are not dollars) are
// unit tests rather than a screenshot. The read half is `db::forge_read`, the command `cli/src/forge/`.
pub mod scorecard;
// The scope manifest's rules, in Rust: lanes, scoring, rendering, drift and the write refusal. Pure, so the
// ranking can be asserted without a filesystem or a git binary — the gather-and-print half is
// `cli/src/forge/manifest.rs`, the same split as the doctor. Ported from `lib/scope-manifest.ts` +
// `scripts/forge-manifest.ts`, deleted with the TypeScript application in `4cf98110`.
pub mod scope_manifest;
// Cloud-sync debris (a numbered sibling beside its original). Two callers need ONE definition: the manifest
// generator must not index debris, and the `health --fix` cleanup must not delete a file a person wrote.
// Ported from `lib/git/sync-conflict.ts`, deleted in the same commit.
pub mod sync_conflict;

pub use service::ForgeService;
