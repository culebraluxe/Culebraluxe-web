use std::sync::Arc;

use workflow::{
    json, wall_clock_ms, CompleteTaskParams, EngineOptions, MemoryStore, ProcessStatus, Result,
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
    /// Authoritative declared write surface from the workflow task form data.
    pub write_surface: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct OpenForgeTask {
    pub task_id: String,
    pub node_id: Option<String>,
    pub claimed: bool,
    pub fork_child: bool,
    pub open_siblings: usize,
}

/// Refuse to run on a stored definition that is not the one this binary was built with.
///
/// `seed_definition` registers a `(key, version)` only when it is absent and otherwise returns what is stored, so a
/// graph edited under an unchanged version is silently NOT the graph that runs: production ran a v6 without the ASSAY
/// branch for three weeks while the XML said otherwise. Naming the drift at boot — before a token moves — is the only
/// place it can be caught cheaply; the remedy is the version bump the message asks for.
fn ensure_stored_definition_current<S: TxStore>(
    engine: &WorkflowEngine<S>,
    embedded: &workflow::ProcessDefinition,
) -> Result<()> {
    let stored = engine.store().with_tx(|tx| {
        tx.load_definition(
            &embedded.key,
            Some(embedded.version),
            embedded.tenant_id.as_deref(),
        )
    })?;
    if crate::engine::version_policy::graphs_equal(&stored.definition, &embedded.definition) {
        return Ok(());
    }
    Err(WorkflowError::generic(format!(
        "the stored workflow definition {} v{} is not the one this build ships: the graph was edited without a \
         version bump, and an executed version is immutable. Bump the version in forge/definitions/FORGE_SDLC-v6.xml \
         and `topology::FORGE_SDLC_VERSION` together; the new version is inserted on the next start.",
        embedded.key, embedded.version
    )))
}

pub struct ForgeRuntime<S: TxStore = MemoryStore> {
    pub(crate) engine: Arc<WorkflowEngine<S>>,
    pub(crate) port: Arc<ForgeApplicationPort>,
    pub(crate) writer: Arc<dyn ForgeStateWriter>,
    pub(crate) ledger: Arc<dyn CompletionLedger>,
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
        // A fixture ledger: these constructors exist for tests, which run one process. Production names
        // its ledger at the call site (`ForgeRuntime::from_store` takes one).
        ForgeRuntime::from_store(
            MemoryStore::new(),
            writer,
            release,
            evidence,
            Arc::new(MemoryLedger::new()),
            def,
        )
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
    /// The completion ledger is a PARAMETER, not a default (2026-09-29).
    ///
    /// It used to be `Arc::new(MemoryLedger::new())` here, and this constructor is what the engine binary
    /// called. Because the engine is one child process per dispatch, that default made every new process
    /// re-apply every completion in the instance history: the evidence merge was replayed and the story's
    /// repair/replan counters grew once per process that ever ran. A caller now has to name the ledger it
    /// wants; the `in_memory*` fixtures below are the only users of the process-local one, and the engine
    /// binary passes the receipt row (`engine::durable_completion_ledger`).
    pub fn from_store(
        store: S,
        writer: Arc<dyn ForgeStateWriter>,
        release: Option<Arc<dyn ForgeReleaseExecutor>>,
        evidence: Option<Arc<dyn ForgeEvidenceReader>>,
        ledger: Arc<dyn CompletionLedger>,
        def: workflow::ProcessDefinition,
    ) -> Result<Self> {
        let top = topology_from_graph(&def.key, def.version, &def.definition);
        ensure_topology(&top).map_err(WorkflowError::generic)?;
        let port = Arc::new(ForgeApplicationPort::new(writer.clone(), release, evidence));
        let port_for_engine: Arc<ForgeApplicationPort> = port.clone();
        let engine = Arc::new(WorkflowEngine::new(
            store,
            EngineOptions {
                app: Some(Box::new(PortClone(port_for_engine))),
                now: Box::new(wall_clock_ms),
            },
        ));
        engine.seed_definition(def.clone())?;
        ensure_stored_definition_current(&engine, &def)?;
        Ok(Self {
            engine,
            port,
            writer,
            ledger,
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

    pub fn with_ledger(mut self, ledger: Arc<dyn CompletionLedger>) -> Self {
        self.ledger = ledger;
        self
    }

    pub fn ledger(&self) -> &Arc<dyn CompletionLedger> {
        &self.ledger
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
        // A canonical Story Board write is not optional. Discarding this failure left the pair
        // workflow=Active / queue=Running / story=Ready, and the settlement rail (`storyboard_story` forbids
        // completion >= 100 unless status = 'Complete') then correctly refuses `Done` and puts a story that
        // actually succeeded on Hold (2026-09-29). A failed canonical write after the instance started is a
        // failed start and is reported as one.
        if let Err(error) = self.writer.mark_story_in_progress(story_id) {
            return Err(WorkflowError::generic(format!(
                "mark_story_in_progress({story_id}) failed after instance {} started: {error}",
                started.process_instance_id
            )));
        }
        Ok(started)
    }

    pub fn list_role_tasks(&self, story_id: &str) -> Result<Vec<ActiveForgeRoleTask>> {
        let Some(instance_id) = self.find_active_instance(story_id)? else {
            return Ok(vec![]);
        };
        let tasks = self.engine.tasks_for_instance(&instance_id)?;
        let tokens = self.engine.tokens_for_instance(&instance_id)?;
        Ok(tasks
            .into_iter()
            .filter(|t| t.status.is_actionable())
            .map(|t| map_role_task(t, &tokens, story_id))
            .collect())
    }

    pub fn find_open_task(&self, instance_id: &str) -> Result<Option<OpenForgeTask>> {
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

    pub fn claim_role_task(&self, task_id: &str, worker_id: &str) -> Result<()> {
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
        )?;
        Ok(completion_receipt_id(task_id))
    }

    /// Resume door: finish any won transition whose receipt never finalized.
    pub fn reconcile_completions(&self, story_id: &str) -> Result<usize> {
        const PAGE_SIZE: usize = 128;
        const MAX_PER_RESUME: usize = 512;

        // The durable ledger queries accepted events against final receipts. Drain all pages for
        // this story before wake_story can start a fresh instance or reset its story-scoped budget.
        if let Some(first_page) = self.ledger.unfinished(Some(story_id), PAGE_SIZE)? {
            let mut applied = 0;
            let mut observed = 0;
            let mut records = first_page;
            loop {
                if records.is_empty() {
                    return Ok(applied);
                }
                for record in records {
                    if record.story_id != story_id {
                        return Err(WorkflowError::generic(format!(
                            "completion recovery returned story {} while reconciling {story_id}",
                            record.story_id
                        )));
                    }
                    observed += 1;
                    if apply_completion_unit(self.ledger.as_ref(), record)?.applied() {
                        applied += 1;
                    }
                    if observed >= MAX_PER_RESUME {
                        if self
                            .ledger
                            .unfinished(Some(story_id), 1)?
                            .is_some_and(|more| !more.is_empty())
                        {
                            return Err(WorkflowError::generic(format!(
                                "completion recovery for {story_id} reached its {MAX_PER_RESUME}-event resume limit; retry before starting a fresh instance"
                            )));
                        }
                        return Ok(applied);
                    }
                }
                records = self
                    .ledger
                    .unfinished(Some(story_id), PAGE_SIZE)?
                    .expect("durable discovery stays supported");
            }
        }

        self.reconcile_workflow_store_completions(story_id, PAGE_SIZE, MAX_PER_RESUME)
    }

    fn reconcile_workflow_store_completions(
        &self,
        story_id: &str,
        page_size: usize,
        max_events: usize,
    ) -> Result<usize> {
        let mut instances = Vec::new();
        let mut offset = 0;
        loop {
            let page = self.engine.list_instances(
                None,
                None,
                Some(FORGE_SDLC_KEY),
                None,
                page_size,
                offset,
            )?;
            let page_len = page.len();
            instances.extend(page.into_iter().filter(|instance| {
                instance.subject_id.as_deref() == Some(story_id)
                    || instance.business_key.as_deref() == Some(story_id)
            }));
            if page_len < page_size {
                break;
            }
            offset += page_len;
        }
        for instance in &instances {
            if instance.subject_type.as_deref() != Some("story")
                || instance.subject_id.as_deref() != Some(story_id)
                || instance.business_key.as_deref() != Some(story_id)
            {
                return Err(WorkflowError::generic(format!(
                    "completion recovery instance {} has invalid story provenance for {story_id}",
                    instance.id
                )));
            }
        }
        instances.sort_by(|a, b| a.started_at.cmp(&b.started_at).then(a.id.cmp(&b.id)));

        let mut applied = 0;
        let mut observed = 0;
        for instance in instances {
            let tasks = self.engine.tasks_for_instance(&instance.id)?;
            let tokens = self.engine.tokens_for_instance(&instance.id)?;
            let mut after_event_id = 0;
            loop {
                let events =
                    self.engine
                        .history_after_event_id(&instance.id, after_event_id, page_size)?;
                if events.is_empty() {
                    break;
                }
                after_event_id = events
                    .last()
                    .map(|event| event.id)
                    .unwrap_or(after_event_id);
                for event in events
                    .iter()
                    .filter(|event| event.event_type == "task.completed")
                {
                    let task_id = event.task_id.as_deref().ok_or_else(|| {
                        WorkflowError::generic(format!(
                            "completion recovery event {} in {} has no task identity",
                            event.id, instance.id
                        ))
                    })?;
                    if self.ledger.has_final(&completion_receipt_id(task_id))? {
                        continue;
                    }
                    if observed >= max_events {
                        return Err(WorkflowError::generic(format!(
                            "completion recovery for {story_id} reached its {max_events}-event resume limit; retry before starting a fresh instance"
                        )));
                    }
                    observed += 1;
                    let form = event
                        .data
                        .get("formData")
                        .filter(|form| form.as_object().is_some())
                        .ok_or_else(|| {
                            WorkflowError::generic(format!(
                                "completion recovery event {} in {} has missing or invalid accepted form data",
                                event.id, instance.id
                            ))
                        })?;
                    let node_id = event.node_id.clone().or_else(|| {
                        tasks
                            .iter()
                            .find(|task| task.id == task_id)
                            .and_then(|task| {
                                task.node_id.clone().or_else(|| {
                                    task.token_id.as_ref().and_then(|token_id| {
                                        tokens
                                            .iter()
                                            .find(|token| &token.id == token_id)
                                            .map(|token| token.node_id.clone())
                                    })
                                })
                            })
                    });
                    let node_id =
                        node_id
                            .filter(|node| !node.trim().is_empty())
                            .ok_or_else(|| {
                                WorkflowError::generic(format!(
                                    "completion recovery event {} in {} has no workflow node",
                                    event.id, instance.id
                                ))
                            })?;
                    if apply_completion_unit(
                        self.ledger.as_ref(),
                        CompletionRecord {
                            task_id: task_id.to_string(),
                            process_instance_id: instance.id.clone(),
                            story_id: story_id.into(),
                            node_id: Some(node_id),
                            evidence: evidence_from_value(form),
                        },
                    )?
                    .applied()
                    {
                        applied += 1;
                    }
                }
                if events.len() < page_size {
                    break;
                }
            }
        }
        Ok(applied)
    }
}

/// The story id is not on `Task` — a workflow task knows its instance, not the story — so the caller that
/// resolved the instance passes it in.
///
/// ONE IDENTITY, PASSED IN (2026-09-29). This used to be `story_id: String::new()`, and the runner then
/// substituted the process-instance UUID for it at all three write sites. `forge_hold_record.story_id` is a
/// foreign key to `storyboard_story(id)`, whose ids are human keys (`ENG-*`), so every hold written that way
/// was rejected by the foreign key and the rejection was discarded: a rejected deliverable left no row and no
/// error. The story id is known one boundary away and is never reconstructed from anything else.
fn map_role_task(t: Task, tokens: &[workflow::Token], story_id: &str) -> ActiveForgeRoleTask {
    let write_surface = crate::engine::role_slice::forge_lane_surface(Some(&t.form_data));
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
        story_id: story_id.to_string(),
        token_id: t.token_id,
        node_id,
        status: t.status,
        assignee: t.assignee,
        candidates: t.candidates,
        write_surface,
    }
}

pub fn empty_vars() -> Value {
    json!({})
}

#[cfg(test)]
mod role_task_identity_tests {
    use super::*;

    fn fixture_task() -> Task {
        Task {
            id: "t-1".into(),
            tenant_id: None,
            process_instance_id: "p-1".into(),
            token_id: Some("k-1".into()),
            node_id: Some("architect".into()),
            name: "architect review".into(),
            description: None,
            status: workflow::TaskStatus::Ready,
            assignee: None,
            candidates: vec!["architect".into()],
            swimlane: None,
            priority: 0,
            due_date: None,
            form_key: None,
            form_data: Value::Null,
            created_at: 0,
            claimed_at: None,
            completed_at: None,
            completed_by: None,
            version: 1,
        }
    }

    /// The regression this guards (2026-09-29): a role task listed for a story came back with an empty
    /// `story_id`, and the runner filled it with the process-instance UUID at three write sites — the observer
    /// `correlation_id`, `mark_story_human_hold`, and the `forge_hold_record` row, whose story id is a foreign
    /// key to a table of human keys.
    #[test]
    fn a_mapped_role_task_carries_the_story_it_was_listed_for() {
        let mapped = map_role_task(fixture_task(), &[], "ENG-GUARD-REPO-RUST-01");
        assert_eq!(mapped.story_id, "ENG-GUARD-REPO-RUST-01");
        assert_eq!(mapped.process_instance_id, "p-1");
        assert_ne!(mapped.story_id, mapped.process_instance_id);
    }

    /// It is the caller's story that lands on the task, not any story the task itself might suggest.
    #[test]
    fn a_mapped_role_task_takes_the_story_from_its_caller() {
        let a = map_role_task(fixture_task(), &[], "ENG-01");
        let b = map_role_task(fixture_task(), &[], "ENG-02");
        assert_eq!(a.story_id, "ENG-01");
        assert_eq!(b.story_id, "ENG-02");
    }

    #[test]
    fn a_mapped_role_task_carries_its_authoritative_declared_surface() {
        let mut task = fixture_task();
        task.form_data = Value::Object(std::collections::BTreeMap::from([(
            "surface".into(),
            Value::Array(vec![Value::from("src/forge.rs"), Value::from("tests/")]),
        )]));
        let mapped = map_role_task(task, &[], "ENG-03");
        assert_eq!(
            mapped.write_surface,
            Some(vec!["src/forge.rs".into(), "tests/".into()])
        );
    }
}
