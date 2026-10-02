//! Forge SDLC roles.

pub mod architect;
pub mod dev_ops;
pub mod inspector;
pub mod lead;
pub mod qa;
pub mod scout;
pub mod service;
pub mod smith;

pub use service::{AbstractForgeService, ForgeServiceDescriptor, ForgeServiceRouter};
