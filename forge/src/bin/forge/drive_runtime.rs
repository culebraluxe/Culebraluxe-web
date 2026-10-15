//! The whole-story drive that MINTS its runtime: it builds the engine runtime over the store it is handed
//! — with the durable receipt row, never a process-local one — and hands the work to the shared driver.
//! Moved out of `main.rs` on 2026-10-14 (the bin root was over the 800-line ceiling) — move-only apart
//! from `pub(super)` and `use super::*;`.

use super::*;

pub(super) fn drive<S: TxStore>(
    store: S,
    release: Arc<dyn ForgeReleaseExecutor>,
    writer: Arc<dyn ForgeStateWriter>,
    evidence_reader: Option<Arc<dyn ForgeEvidenceReader>>,
    harness: Arc<dyn RoleHarness>,
    harness_backend: forge::engine::harness::HarnessBackend,
    model_intent: forge::engine::harness::ModelSelection,
    story: &str,
    work_type: &str,
    stop_after: Option<ForgeStopTarget>,
    story_run_id: Option<String>,
    model_generation_id: Option<String>,
    bench_intent: Option<String>,
    test_mode: Option<String>,
    contract_assay_commands: Vec<String>,
    contract_acceptance_mapped: bool,
) -> Result<DriveSummary, WorkflowError> {
    let mut rt = match ForgeRuntime::from_store(
        store,
        writer.clone(),
        Some(release.clone()),
        evidence_reader.clone(),
        // The receipt row, not a process-local set: this binary IS a child process per dispatch, so a
        // memory ledger would let every new process re-apply each completion in the instance history
        // (2026-09-29). The ledger is a required argument for exactly this reason.
        forge::engine::durable_completion_ledger(),
        forge_sdlc_definition(),
    ) {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("{e}");
            return Err(e);
        }
    };
    drive_with_runtime(
        &mut rt,
        writer,
        evidence_reader,
        harness,
        harness_backend,
        model_intent,
        story,
        work_type,
        stop_after,
        story_run_id,
        model_generation_id,
        bench_intent,
        test_mode,
        contract_assay_commands,
        contract_acceptance_mapped,
    )
}
