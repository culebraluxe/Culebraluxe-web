//! App-side workflow host: the RE_supermodel definition and the residential-transaction host
//! modules that used to live inside the Forge crate.
//!
//! Depends only on the workflow project's `workflow-core`/`workflow-neon` (git tag) and the app's
//! `db` crate; it never depends on `forge`, `services`, `model`, `web` or `cli`.

pub mod commands;
pub mod db;
pub mod facts;
pub mod observability;
pub mod port;
pub mod re_receipt;
pub mod receipt;
pub mod runtime;
pub mod xml;

pub use port::ReApplicationPort;
