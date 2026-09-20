use std::sync::{Arc, Mutex};
use workflow::{ApplicationCommandRequest, ApplicationCommandResult, ApplicationPort, Value, WorkflowSubject};
use crate::engine::dispatch::ForgeCommandRegistry;
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::writer::{ForgeEvidenceReader, ForgeReleaseExecutor, ForgeStateWriter};

pub struct ForgeApplicationPort {
    registry: ForgeCommandRegistry,
    evidence: Option<Arc<dyn ForgeEvidenceReader>>,
    pending: Mutex<Option<ForgeGateEvidence>>,
}

impl ForgeApplicationPort {
    pub fn new(
        writer: Arc<dyn ForgeStateWriter>,
        release: Option<Arc<dyn ForgeReleaseExecutor>>,
        evidence: Option<Arc<dyn ForgeEvidenceReader>>,
    ) -> Self {
        Self {
            registry: ForgeCommandRegistry::build(writer, release),
            evidence,
            pending: Mutex::new(None),
        }
    }
    pub fn set_pending(&self, evidence: ForgeGateEvidence) {
        *self.pending.lock().unwrap() = Some(evidence);
    }
    pub fn clear_pending(&self) {
        *self.pending.lock().unwrap() = None;
    }
    pub fn dispatch(&self, req: &ApplicationCommandRequest) -> ApplicationCommandResult {
        self.registry.dispatch(req)
    }
}

impl ApplicationPort for ForgeApplicationPort {
    fn execute_command(&self, request: &ApplicationCommandRequest) -> ApplicationCommandResult {
        self.registry.dispatch(request)
    }
    fn read_facts(&self, subject: &WorkflowSubject) -> Value {
        if subject.subject_type != "story" {
            return Value::object();
        }
        let durable = self.evidence.as_ref().map(|r| r.read(&subject.subject_id)).unwrap_or_default();
        let pending = self.pending.lock().unwrap().clone();
        let merged = match pending {
            Some(p) => p.merge_over(&durable),
            None => durable,
        };
        merged.to_facts()
    }
}

pub struct MapEvidence(pub std::sync::Mutex<std::collections::BTreeMap<String, ForgeGateEvidence>>);
impl ForgeEvidenceReader for MapEvidence {
    fn read(&self, story_id: &str) -> ForgeGateEvidence {
        self.0.lock().unwrap().get(story_id).cloned().unwrap_or_default()
    }
}
