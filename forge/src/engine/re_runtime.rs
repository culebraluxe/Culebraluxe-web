//! Compatibility bridge for the app RE runtime, and the factory's runtime entry.
//!
//! The app RE runtime itself moved to `re_host::runtime` in W-1 step 8. This module keeps only:
//! 1. a re-export shim so `forge::engine::re_runtime::*` callers (notably `web` until step 9) still
//!    compile;
//! 2. `shared_forge_runtime`, which is now the factory bridge: default builds the factory's own
//!    FORGE_SDLC engine (`factory_runtime`); the old app-RE-engine path remains behind the
//!    temporary `legacy-shared-re-engine` feature for one gate.

use std::sync::Arc;

#[cfg(feature = "legacy-shared-re-engine")]
use workflow::WorkflowError;
use workflow::{NeonStore, Result};

use crate::engine::completion::CompletionLedger;
use crate::engine::runtime::ForgeRuntime;
use crate::engine::writer::{ForgeEvidenceReader, ForgeReleaseExecutor, ForgeStateWriter};

pub use re_host::runtime::{
    assert_re_definition_ready, complete_engine_task, complete_workflow_task, find_active_instance,
    instance_is_active, materialize_open_workflow_tasks, re_engine, reclaim_stale_jobs,
    reclaim_stale_jobs_for_instance, reconcile_closing_timer, reconcile_deadline_timer,
    reconcile_residential_transactions, reconcile_workflows, reset_dev_workflows, run_due_jobs,
    start_residential_transaction, workflow_status, ReconcileReport, StartResult, WorkflowStatus,
    CLOSING_TIMER_NODE_ID, RESIDENTIAL_TRANSACTION_KEY, RESIDENTIAL_TRANSACTION_VERSION,
};

/// Create the production factory runtime. The default path builds the factory's own
/// FORGE_SDLC engine over the shared `db::shared` pool with `ForgeApplicationPort`; it never
/// borrows or re-seeds the app RE engine. The old app-RE-backed bridge remains only behind the
/// temporary `legacy-shared-re-engine` feature for one gate.
pub fn shared_forge_runtime(
    writer: Arc<dyn ForgeStateWriter>,
    release: Option<Arc<dyn ForgeReleaseExecutor>>,
    evidence: Option<Arc<dyn ForgeEvidenceReader>>,
    ledger: Arc<dyn CompletionLedger>,
) -> Result<ForgeRuntime<NeonStore>> {
    #[cfg(feature = "legacy-shared-re-engine")]
    {
        legacy_shared_forge_runtime(writer, release, evidence, ledger)
    }
    #[cfg(not(feature = "legacy-shared-re-engine"))]
    {
        crate::engine::factory_runtime::shared_factory_runtime(writer, release, evidence, ledger)
    }
}

/// Old path: app RE engine + re-seeded RE definition. Kept behind the feature for one gate only.
#[cfg(feature = "legacy-shared-re-engine")]
fn legacy_shared_forge_runtime(
    writer: Arc<dyn ForgeStateWriter>,
    release: Option<Arc<dyn ForgeReleaseExecutor>>,
    evidence: Option<Arc<dyn ForgeEvidenceReader>>,
    ledger: Arc<dyn CompletionLedger>,
) -> Result<ForgeRuntime<NeonStore>> {
    let engine = re_host::runtime::re_engine()?;
    let port = Arc::new(crate::engine::port::ForgeApplicationPort::new(
        writer.clone(),
        release,
        evidence,
    ));
    // Re-seed the definition in case the shared engine was built with a different port.
    let def =
        re_host::xml::parse_re_supermodel().map_err(|error| WorkflowError::generic(error.0))?;
    engine.seed_definition(def)?;
    Ok(ForgeRuntime {
        engine,
        port,
        writer,
        ledger,
    })
}
