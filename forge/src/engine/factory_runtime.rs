//! The factory's own runtime: FORGE_SDLC over the app's shared pool, with `ForgeApplicationPort`.
//!
//! This replaces the old `shared_forge_runtime` behavior that borrowed the app RE engine and
//! re-seeded `RE_supermodel` into it. The factory never needs the RE definition or the app RE port;
//! it builds a `WorkflowEngine<NeonStore>` with `ForgeApplicationPort` and seeds FORGE_SDLC.

use std::sync::Arc;

use workflow::{NeonStore, Result, TxStore, WorkflowError};

use crate::engine::completion::CompletionLedger;
use crate::engine::definition::forge_sdlc_definition;
use crate::engine::runtime::ForgeRuntime;
use crate::engine::vendor_session::with_shared;
use crate::engine::writer::{ForgeEvidenceReader, ForgeReleaseExecutor, ForgeStateWriter};

/// Build a factory runtime over a caller-supplied store. Kept generic so the isolation test can use
/// `MemoryStore` without a database.
pub(crate) fn forge_runtime_from_store<S: TxStore>(
    store: S,
    writer: Arc<dyn ForgeStateWriter>,
    release: Option<Arc<dyn ForgeReleaseExecutor>>,
    evidence: Option<Arc<dyn ForgeEvidenceReader>>,
    ledger: Arc<dyn CompletionLedger>,
) -> Result<ForgeRuntime<S>> {
    ForgeRuntime::from_store(
        store,
        writer,
        release,
        evidence,
        ledger,
        forge_sdlc_definition(),
    )
}

/// Production factory runtime: one store over the process's shared `db::shared` pool, its own
/// engine/port, and only the FORGE_SDLC definition.
pub fn shared_factory_runtime(
    writer: Arc<dyn ForgeStateWriter>,
    release: Option<Arc<dyn ForgeReleaseExecutor>>,
    evidence: Option<Arc<dyn ForgeEvidenceReader>>,
    ledger: Arc<dyn CompletionLedger>,
) -> Result<ForgeRuntime<NeonStore>> {
    let store = with_shared(|db, _rt| NeonStore::from_pool(db.pool().clone()))
        .map_err(WorkflowError::generic)?;
    forge_runtime_from_store(store, writer, release, evidence, ledger)
}

#[cfg(test)]
mod tests {
    use super::*;

    use workflow::{ApplicationPort, MemoryStore, Value, WorkflowSubject};

    use crate::engine::completion::MemoryLedger;
    use crate::engine::facts::ForgeGateEvidence;
    use crate::engine::topology::{FORGE_SDLC_KEY, FORGE_SDLC_VERSION};
    use crate::engine::writer::RecordingWriter;

    fn memory_runtime() -> ForgeRuntime<MemoryStore> {
        forge_runtime_from_store(
            MemoryStore::new(),
            Arc::new(RecordingWriter::default()),
            None,
            None,
            Arc::new(MemoryLedger::new()),
        )
        .expect("in-memory factory runtime")
    }

    #[test]
    fn the_factory_engine_is_seeded_with_forge_sdlc_only() {
        let runtime = memory_runtime();
        let engine = runtime.engine();
        let forge = engine
            .store()
            .with_tx(|tx| tx.load_definition(FORGE_SDLC_KEY, Some(FORGE_SDLC_VERSION), None))
            .expect("factory definition is seeded");
        assert_eq!(forge.key, FORGE_SDLC_KEY);
        assert_eq!(forge.version, FORGE_SDLC_VERSION);

        let re = engine
            .store()
            .with_tx(|tx| tx.load_definition("RE_supermodel", Some(1), None));
        assert!(
            re.is_err(),
            "factory engine must not be seeded with the app RE_supermodel"
        );
    }

    #[test]
    fn the_factory_uses_the_forge_application_port_for_story_facts() {
        let runtime = memory_runtime();
        let facts = runtime.port().read_facts(&WorkflowSubject {
            subject_type: "story".into(),
            subject_id: "story-without-evidence".into(),
        });
        assert!(
            facts.as_object().is_some(),
            "factory facts come from ForgeApplicationPort, not the app RE port"
        );
        assert_ne!(facts, Value::object(), "the Forge port answers story facts");
    }

    #[test]
    fn the_factory_runtime_can_look_for_an_active_story_instance() {
        let runtime = memory_runtime();
        assert!(
            runtime
                .find_active_instance("no-such-story")
                .expect("instance lookup is a read")
                .is_none(),
            "a fresh factory runtime starts with no active instances"
        );
    }

    #[test]
    fn the_factory_start_story_path_is_unchanged() {
        let runtime = memory_runtime();
        let started = runtime
            .start_story(
                "factory-isolation-story",
                "FEATURE",
                ForgeGateEvidence::default(),
            )
            .expect("factory start_story");
        assert!(!started.process_instance_id.is_empty());
        assert_eq!(
            runtime
                .find_active_instance("factory-isolation-story")
                .expect("active instance read"),
            Some(started.process_instance_id)
        );
    }
}
