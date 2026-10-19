//! Luxesign — document signing domain.
//!
//! Structure:
//! - `core`     — the Luxesign service/domain (envelopes, recipients, fields, finalize/sweep)
//! - `signer`   — recipient signing sessions, consent, access tokens and signer edge
//! - `signature`— e-signature provider service (prepare/send/status/webhook/artifacts)
//! - `worker`   — Luxesign background/finalize worker
pub mod commands;
pub mod core;
pub mod signer;
pub mod signature;
pub mod worker;

pub use core::*;
pub use signature::SignatureService as LuxesignSignatureService;
pub use signer::SignerService as LuxesignSignerService;
