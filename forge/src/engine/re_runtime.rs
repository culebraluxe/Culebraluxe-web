//! The factory's runtime entry.
//!
//! The app RE runtime moved to `re_host::runtime` in W-1 step 8; the app-function re-exports are
//! gone. This file keeps only `shared_forge_runtime`, the factory bridge that builds the factory's
//! own FORGE_SDLC engine over the shared `db::shared` pool (see `factory_runtime`).

use std::sync::Arc;

use workflow::{NeonStore, Result};

use crate::engine::completion::CompletionLedger;
use crate::engine::runtime::ForgeRuntime;
use crate::engine::writer::{ForgeEvidenceReader, ForgeReleaseExecutor, ForgeStateWriter};

/// Create the production factory runtime: its own FORGE_SDLC engine over the shared pool with
/// `ForgeApplicationPort`, never the app RE engine or definition.
pub fn shared_forge_runtime(
    writer: Arc<dyn ForgeStateWriter>,
    release: Option<Arc<dyn ForgeReleaseExecutor>>,
    evidence: Option<Arc<dyn ForgeEvidenceReader>>,
    ledger: Arc<dyn CompletionLedger>,
) -> Result<ForgeRuntime<NeonStore>> {
    crate::engine::factory_runtime::shared_factory_runtime(writer, release, evidence, ledger)
}
