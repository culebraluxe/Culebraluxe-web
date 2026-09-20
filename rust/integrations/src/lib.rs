//! External-system adapters.
//!
//! Provider-specific concerns stay at this edge and must not leak into domain.

pub mod apple;
pub mod boldsign;
pub mod google;
pub mod mux;
pub mod neon;
pub mod whatsapp;
