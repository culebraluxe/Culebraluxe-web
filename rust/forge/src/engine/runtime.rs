use std::sync::Arc;

use workflow::{
    json, CompleteTaskParams, EngineOptions, MemoryStore, ProcessStatus, Result,
    StartProcessParams, StartProcessResult, Task, TaskStatus, TxStore, Value, WorkflowEngine,
    WorkflowError, WorkflowSubject,
};

use crate::engine::completion::{
    apply_completion_unit, CompletionLedger, CompletionRecord, MemoryLedger,
};
use crate::engine::definition::forge_sdlc_definition;
use crate::engine::facts::{evidence_from_value, ForgeGateEvidence};
use crate::engine::port::ForgeApplicationPort;
use crate::engine::topology::{
    ensure_topology, topology_from_graph, FORGE_SDLC_KEY, FORGE_SDLC_VERSION,
};
use crate::engine::writer::{ForgeEvidenceReader, ForgeReleaseExecutor, ForgeStateWriter};

pub fn completion_receipt_id(task_id: &str) -> String {
    format!("forge.completion:{task_id}")
}

#[derive(Debug, Clone)]
pub struct ActiveForgeRoleTask {
    pub task_id: String,
    pub process_instance_id: String,
    pub story_id: String,
    pub token_id: Option<String>,
    pub node_id: Option<String>,
    pub status: TaskStatus,
    pub assignee: Option<String>,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OpenForgeTask {
    pub task_id: String,
    pub node_id: Option<String>,
    pub claimed: bool,
    pub fork_child: bool,
    pub open_siblings: usize,
}

pub struct ForgeRuntime<S: TxStore = MemoryStore> {
    engine: WorkflowEngine<S>,
    port: Arc<ForgeApplicationPort>,
    writer: Arc<dyn ForgeStateWriter>,
    ledger: Arc<dyn CompletionLedger>,
}

impl ForgeRuntime<MemoryStore> {
    pub fn in_memory(writer: Arc<dyn ForgeStateWriter>) -> Result<Self> {
        Self::in_memory_full(writer, None, None)
    }

    /// Test fixture that is not the XML. Production always uses XML.
    pub fn in_memory_compact(writer: Arc<dyn ForgeStateWriter>) -> Result<Self> {
        Self::in_memory_compact_full(writer, None)
    }

    pub fn in_memory_compact_full(
        writer: Arc<dyn ForgeStateWriter>,
        release: Option<Arc<dyn ForgeReleaseExecutor>>,
    ) -> Result<Self> {
        Self::from_definition(
            writer,
            release,
            None,
            crate::engine::definition::forge_sdlc_compact_definition(),
        )
    }

    pub fn in_memory_full(
        writer: Arc<dyn ForgeStateWriter>,
        release: Option<Arc<dyn ForgeReleaseExecutor>>,
        evidence: Option<Arc<dyn ForgeEvidenceReader>>,
    ) -> Result<Self> {
        Self::from_definition(writer, release, evidence, forge_sdlc_definition())
    }

    fn from_definition(
        writer: Arc<dyn ForgeStateWriter>,
        release: Option<Arc<dyn ForgeReleaseExecutor>>,
        evidence: Option<Arc<dyn ForgeEvidenceReader>>,
        def: workflow::ProcessDefinition,
    ) -> Result<Self> {
        ForgeRuntime::from_store(MemoryStore::new(), writer, release, evidence, def)
    }

    pub fn with_ledger(mut self, ledger: Arc<dyn CompletionLedger>) -> Self {
        self.ledger = ledger;
        self
    }

    pub fn ledger(&self) -> &Arc<dyn CompletionLedger> {
        &self.ledger
    }
}

/// ApplicationPort is not cloneable; wrap Arc.
struct PortClone(Arc<ForgeApplicationPort>);

impl workflow::ApplicationPort for PortClone {
    fn execute_command(
        &self,
        request: &workflow::ApplicationCommandRequest,
    ) -> workflow::ApplicationCommandResult {
        self.0.execute_command(request)
    }
    fn read_facts(&self, subject: &WorkflowSubject) -> Value {
        self.0.read_facts(subject)
    }
}

impl<S: TxStore> ForgeRuntime<S> {
    pub fn from_store(
        store: S,
        writer: Arc<dyn ForgeStateWriter>,
        release: Option<Arc<dyn ForgeReleaseExecutor>>,
        evidence: Option<Arc<dyn ForgeEvidenceReader>>,
        def: workflow::ProcessDefinition,
    ) -> Result<Self> {
        let top = topology_from_graph(&def.key, def.version, &def.definition);
        ensure_topology(&top).map_err(WorkflowError::generic)?;
        let port = Arc::new(ForgeApplicationPort::new(writer.clone(), release, evidence));
        let port_for_engine: Arc<ForgeApplicationPort> = port.clone();
        let engine = WorkflowEngine::new(
            store,
            EngineOptions {
                app: Some(Box::new(PortClone(port_for_engine))),
                now: Box::new(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0)
                }),
            },
        );
        engine.seed_definition(def)?;
        Ok(Self {
            engine,
            port,
            writer,
            ledger: Arc::new(MemoryLedger::new()),
        })
    }

    pub fn engine(&self) -> &WorkflowEngine<S> {
        &self.engine
    }

    pub fn writer(&self) -> &Arc<dyn ForgeStateWriter> {
        &self.writer
    }

    pub fn port(&self) -> &ForgeApplicationPort {
        &self.port
    }

    pub fn find_active_instance(&self, story_id: &str) -> Result<Option<String>> {
        let rows = self.engine.list_instances(
            None,
            Some(&[ProcessStatus::Active]),
            Some(FORGE_SDLC_KEY),
            None,
            256,
            0,
        )?;
        Ok(rows
            .into_iter()
            .find(|i| {
                i.subject_type.as_deref() == Some("story")
                    && i.subject_id.as_deref() == Some(story_id)
            })
            .map(|i| i.id))
    }

    pub fn start_story(
        &self,
        story_id: &str,
        work_type: &str,
        evidence: ForgeGateEvidence,
    ) -> Result<StartProcessResult> {
        if let Some(id) = self.find_active_instance(story_id)? {
            return Err(WorkflowError::conflict(
                "INSTANCE_ALREADY_ACTIVE",
                format!("active FORGE_SDLC instance {id} already exists for story {story_id}"),
            ));
        }
        self.port.set_pending(evidence.clone());
        let mut variables = evidence.to_facts();
        variables.insert("workType", Value::from(work_type));
        let started = self.engine.start_process(StartProcessParams {
            definition_key: FORGE_SDLC_KEY.into(),
            version: Some(FORGE_SDLC_VERSION),
            business_key: Some(story_id.into()),
            variables,
            started_by: "forge".into(),
            tenant_id: None,
            subject: Some(WorkflowSubject {
                subject_type: "story".into(),
                subject_id: story_id.into(),
            }),
        });
        self.port.clear_pending();
        let started = started?;
        let _ = self.writer.mark_story_in_progress(story_id);
        Ok(started)
    }

    pub fn list_role_tasks(&mut self, story_id: &str) -> Result<Vec<ActiveForgeRoleTask>> {
        let Some(instance_id) = self.find_active_instance(story_id)? else {
            return Ok(vec![]);
        };
        let tasks = self.engine.tasks_for_instance(&instance_id)?;
        let tokens = self.engine.tokens_for_instance(&instance_id)?;
        Ok(tasks
            .into_iter()
            .filter(|t| t.status.is_actionable())
            .map(|t| map_role_task(t, &tokens))
            .collect())
    }

    pub fn find_open_task(&mut self, instance_id: &str) -> Result<Option<OpenForgeTask>> {
        let tasks = self.engine.tasks_for_instance(instance_id)?;
        let tokens = self.engine.tokens_for_instance(instance_id)?;
        let open: Vec<_> = tasks
            .into_iter()
            .filter(|t| t.status.is_actionable())
            .collect();
        let Some(newest) = open.last() else {
            return Ok(None);
        };
        let token = newest
            .token_id
            .as_ref()
            .and_then(|id| tokens.iter().find(|tk| &tk.id == id));
        let fork_parent = token.and_then(|t| t.parent_token_id.as_deref());
        let fork_child = fork_parent.is_some();
        let open_siblings = if let Some(parent) = fork_parent {
            open.iter()
                .filter(|t| {
                    t.id != newest.id
                        && t.token_id
                            .as_ref()
                            .and_then(|id| tokens.iter().find(|tk| &tk.id == id))
                            .and_then(|tk| tk.parent_token_id.as_deref())
                            == Some(parent)
                })
                .count()
        } else {
            0
        };
        Ok(Some(OpenForgeTask {
            task_id: newest.id.clone(),
            node_id: token.map(|t| t.node_id.clone()).or(newest.node_id.clone()),
            claimed: newest.assignee.is_some(),
            fork_child,
            open_siblings,
        }))
    }

    /// Resume door guard: refuse to complete an unclaimed fork sibling when
    /// other siblings are still open (ENG-FORGE-SPLIT-SIBLING-01).
    pub fn assert_resume_safe(&self, open: &OpenForgeTask) -> Result<()> {
        if open.fork_child && !open.claimed && open.open_siblings > 0 {
            return Err(WorkflowError::conflict(
                "SPLIT_SIBLING_UNCLAIMED",
                format!(
                    "refusing to complete unclaimed fork task {} while {} siblings are open",
                    open.task_id, open.open_siblings
                ),
            ));
        }
        Ok(())
    }

    pub fn claim_role_task(&mut self, task_id: &str, worker_id: &str) -> Result<()> {
        self.engine.claim_task(task_id, worker_id)
    }

    pub fn release_role_task(&self, task_id: &str, worker_id: &str) -> Result<()> {
        self.engine.release_task(task_id, worker_id)
    }

    pub fn cancel_instance(
        &self,
        instance_id: &str,
        actor: &str,
        reason: Option<&str>,
    ) -> Result<()> {
        self.engine.cancel_process(workflow::CancelProcessParams {
            process_instance_id: instance_id.into(),
            actor: actor.into(),
            reason: reason.map(|s| s.to_string()),
        })
    }

    pub fn complete_role_task(
        &self,
        task_id: &str,
        worker_id: &str,
        transition: Option<&str>,
        evidence: ForgeGateEvidence,
    ) -> Result<String> {
        let task = self.engine.get_task(task_id)?;
        let instance = self
            .engine
            .get_process_instance(&task.process_instance_id)?;
        let story_id = instance.subject_id.clone().unwrap_or_default();
        let tokens = self.engine.tokens_for_instance(&instance.id)?;
        let node_id = task.node_id.clone().or_else(|| {
            task.token_id.as_ref().and_then(|id| {
                tokens
                    .iter()
                    .find(|tk| &tk.id == id)
                    .map(|tk| tk.node_id.clone())
            })
        });

        self.port.set_pending(evidence.clone());
        let result = self.engine.complete_task(CompleteTaskParams {
            task_id: task_id.into(),
            user_id: worker_id.into(),
            form_data: evidence.to_facts(),
            transition_name: transition.map(|s| s.to_string()),
        });
        self.port.clear_pending();
        result?;

        apply_completion_unit(
            self.ledger.as_ref(),
            CompletionRecord {
                task_id: task_id.into(),
                process_instance_id: instance.id,
                story_id,
                node_id,
                evidence,
            },
        );
        Ok(completion_receipt_id(task_id))
    }

    /// Resume door: finish any won transition whose receipt never finalized.
    pub fn reconcile_completions(&mut self, story_id: &str) -> Result<usize> {
        let Some(instance_id) = self.find_active_instance(story_id)? else {
            return Ok(0);
        };
        let events = self.engine.history(&instance_id, 200)?;
        let tasks = self.engine.tasks_for_instance(&instance_id)?;
        let tokens = self.engine.tokens_for_instance(&instance_id)?;
        let watermark = self
            .ledger
            .watermark(crate::engine::neon_sql::RECEIPT_PREFIX);
        let mut applied = 0;
        for ev in events.into_iter().rev() {
            if ev.event_type != "task.completed" {
                continue;
            }
            if let Some(w) = watermark {
                if ev.created_at > 0 && ev.created_at <= w {
                    continue;
                }
            }
            let Some(task_id) = ev.task_id.clone() else {
                continue;
            };
            if self.ledger.has_final(&completion_receipt_id(&task_id)) {
                continue;
            }
            let form = ev
                .data
                .get("formData")
                .cloned()
                .unwrap_or_else(Value::object);
            let node_id = tasks.iter().find(|t| t.id == task_id).and_then(|t| {
                t.node_id.clone().or_else(|| {
                    t.token_id.as_ref().and_then(|id| {
                        tokens
                            .iter()
                            .find(|tk| &tk.id == id)
                            .map(|tk| tk.node_id.clone())
                    })
                })
            });
            if apply_completion_unit(
                self.ledger.as_ref(),
                CompletionRecord {
                    task_id,
                    process_instance_id: instance_id.clone(),
                    story_id: story_id.into(),
                    node_id,
                    evidence: evidence_from_value(&form),
                },
            ) {
                applied += 1;
            }
        }
        Ok(applied)
    }
}

fn map_role_task(t: Task, tokens: &[workflow::Token]) -> ActiveForgeRoleTask {
    let node_id = t.node_id.clone().or_else(|| {
        t.token_id.as_ref().and_then(|id| {
            tokens
                .iter()
                .find(|tk| &tk.id == id)
                .map(|tk| tk.node_id.clone())
        })
    });
    ActiveForgeRoleTask {
        task_id: t.id,
        process_instance_id: t.process_instance_id.clone(),
        story_id: String::new(),
        token_id: t.token_id,
        node_id,
        status: t.status,
        assignee: t.assignee,
        candidates: t.candidates,
    }
}

pub fn empty_vars() -> Value {
    json!({})
}
