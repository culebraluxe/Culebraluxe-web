//! Native Harness Evolution subsystem (Phase 3 extension).
//!
//! Algorithm adapted from Google Harness² (Apache-2.0; reimplemented natively, no Python sidecar):
//! `Propose -> Probe -> Contrast -> Compose -> Forge Validate -> Adopt/Reject -> Reflect`.
//!
//! Safety: immutable revisions; protected RED surfaces rejected before probing; probes pin the
//! full execution identity and refuse production; lessons are domain-scoped and evidence-backed;
//! a failed experiment never mutates the active harness.

pub mod adoption;
pub mod calibration;
pub mod catalog;
pub mod compose;
pub mod contrast;
pub mod dogfood;
pub mod experience;
pub mod experiment;
pub mod feed;
pub mod metrics;
pub mod model;
pub mod policy;
pub mod probe;
pub mod probe_replay;
pub mod propose;
pub mod proposer_llm;
pub mod sandbox_command;
pub mod store_db;
pub mod validate;
