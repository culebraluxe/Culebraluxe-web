//! WF.TOKEN — start creates one root token (TST-WF-TOKEN-001).
//!
//! Contract: starting a process mints **exactly one root token** — one, and only one, per started instance, and it is
//! the instance's own. `WorkflowEngine::start_process` allocates the instance id (`middle/workflow/src/engine/
//! engine_options.rs:107`), builds the row (`:123-140`), mints the single root token id with `tx.new_id("tok")`
//! (`:142`), writes that one token (`:143-157`), and links it onto the instance with `set_root_token` (`:158`). The
//! root is a root because it carries no parent (`:147`) — every other token in the instance descends from it. Only
//! then does the graph execute (`:177-185`), and any node that forks mints *children of that root*, never a second
//! root beside it.
//!
//! "Exactly one" is the load-bearing word, and it has to hold in three directions, each of which is a separate way
//! this could be wrong: the token is minted once per start (no duplicate on a re-entered step), a second start mints
//! its *own* root rather than reusing or colliding with the first, and a start that cannot finish its single insert
//! leaves *nothing* behind rather than an instance with no root or a root with no instance. `MemoryStore::with_tx`
//! restores its snapshot when the step fails (`middle/workflow/src/memory.rs:46-51`), which is what makes the third
//! direction hold for the store production runs on and for Neon, where one `with_tx` is one BEGIN/ROLLBACK
//! (`middle/workflow/src/concurrency.rs:10-12`).
//!
//! This file exercises the production boundary, not a re-declaration of it: the positive and the subject-dedup
//! negatives drive the real `WorkflowEngine<MemoryStore>` through `start_process`, and the rollback negative injects
//! a fault at the `Store` seam only — a fake at the defined production interface whose every other byte of storage is
//! the production `MemoryStore`'s.
//!
//! Level: L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed clock and an in-memory store;
//! no database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_token__001__start_creates_one_root_token

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    DefinitionStatus, EngineOptions, MemoryStore, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessInstance, ProcessOutcome, ProcessStatus, Result, StartProcessParams,
    Store, Token, TransitionDefinition, TxStore, Value, WorkflowEngine, WorkflowError,
    WorkflowSubject,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";

/// The definition key the engine half registers.
const DEFINITION_KEY: &str = "TST-WF-TOKEN-001";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// The graph the engine half starts: `start -> hold (task)`. It parks on a human task so the single root token is
/// still live (and still the only token) when the assertions read it back.
const START_NODE: &str = "start";
const HOLD_NODE: &str = "hold";
const ENTER: &str = "enter";

/// The subject both starts in the dedup negative share.
const SUBJECT_TYPE: &str = "property";
const SUBJECT_ID: &str = "subject-1";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> hold (task)`.
fn linear_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(ENTER, HOLD_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        HOLD_NODE.to_string(),
        NodeDefinition {
            id: HOLD_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Hold".to_string()),
            transitions: Some(vec![transition("done", "done")]),
            ..Default::default()
        },
    );
    ProcessDefinition {
        id: format!("{DEFINITION_KEY}-def"),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: DEFINITION_KEY.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START_NODE.to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

fn start_params() -> StartProcessParams {
    StartProcessParams {
        definition_key: DEFINITION_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

fn start_params_for(subject_id: &str) -> StartProcessParams {
    StartProcessParams {
        subject: Some(WorkflowSubject {
            subject_type: SUBJECT_TYPE.to_string(),
            subject_id: subject_id.to_string(),
        }),
        ..start_params()
    }
}

fn read_instance(store: &MemoryStore, id: &str) -> ProcessInstance {
    store
        .with_tx(|tx| tx.get_instance(id))
        .expect("the instance is readable")
}

/// `Store::history` is newest-first (`middle/workflow/src/memory.rs:568` sorts by descending id), so the collected
/// list is reversed here: ids are assigned in insertion order and the test clock never moves, so descending id is
/// exactly reverse commit order.
fn events_of_type(store: &MemoryStore, instance_id: &str, event_type: &str) -> Vec<ProcessEvent> {
    let mut events: Vec<ProcessEvent> = store
        .with_tx(|tx| tx.history(instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect();
    events.reverse();
    events
}

fn instances_for_definition(store: &MemoryStore) -> Vec<ProcessInstance> {
    store
        .with_tx(|tx| tx.find_instances(None, None, Some(DEFINITION_KEY), None, 10, 0))
        .expect("instances read")
}

fn tokens_for(store: &MemoryStore, instance_id: &str) -> Vec<Token> {
    store
        .with_tx(|tx| tx.tokens_for_instance(instance_id))
        .expect("tokens read")
}

/// A `Store` interposer that refuses the token insert, delegating all other storage to the production `MemoryStore`.
///
/// This is the smallest seam that can make "start creates one root token" fail in the middle: the engine under test is
/// the real `WorkflowEngine`, and the refusal is the same `Err` any store failure returns, so the rollback the test
/// asserts is the production rollback rather than a simulated one. Every method but `insert_token` is forwarded to the
/// production implementation.
struct RefusingInsertTokenStore<'a> {
    inner: &'a mut dyn Store,
}

macro_rules! forward_to_inner {
    ($($name:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty;)*) => {
        $(
            fn $name(&mut self, $($arg: $ty),*) -> $ret {
                self.inner.$name($($arg),*)
            }
        )*
    };
}

impl Store for RefusingInsertTokenStore<'_> {
    forward_to_inner! {
        new_id(prefix: &str) -> String;
        load_definition(key: &str, version: Option<i32>, tenant_id: Option<&str>) -> Result<ProcessDefinition>;
        definition_by_id(id: &str) -> Result<ProcessDefinition>;
        ensure_definition(def: ProcessDefinition) -> Result<ProcessDefinition>;
        insert_instance(inst: ProcessInstance) -> Result<ProcessInstance>;
        set_root_token(instance_id: &str, token_id: &str) -> Result<()>;
        get_instance(id: &str) -> Result<ProcessInstance>;
        find_active_by_subject(definition_id: &str, subject_type: &str, subject_id: &str) -> Result<Option<ProcessInstance>>;
        lock_instance(id: &str) -> Result<ProcessInstance>;
        update_instance_variables(id: &str, variables: Value) -> Result<()>;
        terminate_instance(id: &str, status: ProcessStatus, outcome: ProcessOutcome, ended_at: i64) -> Result<()>;
        get_token(id: &str) -> Result<Token>;
        lock_token(id: &str) -> Result<Token>;
        move_token(id: &str, expected_version: i32, to_node: &str) -> Result<bool>;
        complete_token(id: &str, outcome: workflow::TokenOutcome, ended_at: i64) -> Result<()>;
        count_active_tokens(instance_id: &str) -> Result<i32>;
        list_active_tokens(instance_id: &str) -> Result<Vec<Token>>;
        count_required_active_siblings(parent_id: &str) -> Result<i32>;
        list_optional_active_siblings(parent_id: &str) -> Result<Vec<Token>>;
        list_children(parent_id: &str) -> Result<Vec<Token>>;
        tokens_for_instance(instance_id: &str) -> Result<Vec<Token>>;
        insert_task(task: workflow::Task) -> Result<workflow::Task>;
        get_task(id: &str) -> Result<workflow::Task>;
        lock_task(id: &str) -> Result<workflow::Task>;
        cas_task(task: &workflow::Task) -> Result<bool>;
        tasks_for_instance(instance_id: &str) -> Result<Vec<workflow::Task>>;
        open_tasks_for_instance(instance_id: &str) -> Result<Vec<workflow::Task>>;
        open_tasks_for_token(token_id: &str) -> Result<Vec<workflow::Task>>;
        active_tasks_for_user(user_id: &str, tenant_id: Option<&str>) -> Result<Vec<workflow::Task>>;
        patch_ready_task_form(token_id: &str, form_data: Value) -> Result<()>;
        insert_job(job: workflow::Job) -> Result<workflow::Job>;
        get_job(id: &str) -> Result<workflow::Job>;
        lock_job(id: &str) -> Result<workflow::Job>;
        update_job(job: &workflow::Job) -> Result<()>;
        claim_job(job_id: &str, worker_id: &str, now: i64, lease_until: i64) -> Result<Option<workflow::Job>>;
        claim_due_jobs(worker_id: &str, now: i64, lease_until: i64, limit: usize) -> Result<Vec<workflow::Job>>;
        claim_due_jobs_by_type(worker_id: &str, job_type: &str, now: i64, lease_until: i64, limit: usize) -> Result<Vec<workflow::Job>>;
        reclaim_stale_jobs(now: i64, batch: usize, instance_id: Option<&str>) -> Result<usize>;
        open_jobs_for_instance(instance_id: &str) -> Result<Vec<workflow::Job>>;
        open_jobs_for_token(token_id: &str) -> Result<Vec<workflow::Job>>;
        list_overdue_jobs(now: i64, limit: usize) -> Result<Vec<workflow::Job>>;
        insert_event(event: ProcessEvent) -> Result<()>;
        history(instance_id: &str, limit: usize) -> Result<Vec<ProcessEvent>>;
        command_visit_count(instance_id: &str, node_id: &str) -> Result<i32>;
        insert_command(cmd: workflow::ProcessCommand) -> Result<()>;
        find_instances(tenant_id: Option<&str>, status: Option<&[ProcessStatus]>, definition_key: Option<&str>, business_key: Option<&str>, limit: usize, offset: usize) -> Result<Vec<ProcessInstance>>;
    }

    /// The injected fault: the single root-token insert fails exactly as a broken connection would.
    fn insert_token(&mut self, _token: Token) -> Result<Token> {
        Err(WorkflowError::generic("injected: token insert refused"))
    }
}

/// The production `MemoryStore`, driven only through `Store`, with every `insert_token` refused at the seam.
#[derive(Clone)]
struct RefusingRootTokenStore {
    memory: MemoryStore,
}

impl RefusingRootTokenStore {
    fn new() -> Self {
        Self {
            memory: MemoryStore::new(),
        }
    }
}

impl TxStore for RefusingRootTokenStore {
    fn with_tx<R, F>(&self, mut f: F) -> Result<R>
    where
        F: FnMut(&mut dyn Store) -> Result<R>,
    {
        self.memory.with_tx(|tx| {
            let mut refusing = RefusingInsertTokenStore { inner: tx };
            f(&mut refusing)
        })
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TOKEN-001); the file and the assay use it.
fn wf_token_001__start_creates_one_root_token() {
    // ── SCENARIO 1: one start mints exactly one root token, and the instance owns that one ──────────────────────
    // The subject is `WorkflowEngine::start_process`, the production entry point. Nothing here re-declares token
    // creation: the tokens read back are the rows the engine inserted.
    let harness = EngineHarness::at_unix_millis(1_700_000_000_000);
    harness
        .engine()
        .seed_definition(linear_definition())
        .expect("the definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on the human task");
    let instance_id = started.process_instance_id.clone();
    let root_token_id = started.root_token_id.clone();

    // Exactly one instance, and exactly one token in it. A start that minted a second root, or minted none, fails
    // the first assertion or the second.
    let instances = instances_for_definition(harness.store());
    assert_eq!(
        instances.len(),
        1,
        "{HARNESS}: one start must commit exactly one process instance"
    );
    assert_eq!(
        instances[0].id, instance_id,
        "{HARNESS}: the committed instance is the one start_process returned"
    );

    let tokens = tokens_for(harness.store(), &instance_id);
    assert_eq!(
        tokens.len(),
        1,
        "{HARNESS}: one start must mint exactly one token, got {:?}",
        tokens.iter().map(|t| t.id.as_str()).collect::<Vec<_>>()
    );

    // The one token IS the returned root token, and the instance points at it. This is what `set_root_token`
    // (`engine_options.rs:158`) writes, so a start that forgot the link, or linked a different id, fails here.
    assert_eq!(
        tokens[0].id, root_token_id,
        "{HARNESS}: the one token minted is the root token start_process returned"
    );
    assert_eq!(
        read_instance(harness.store(), &instance_id)
            .root_token_id
            .as_deref(),
        Some(root_token_id.as_str()),
        "{HARNESS}: the instance records that same token as its root"
    );

    // It is a ROOT, not a child: nothing above it. A second root would arrive with a parent or as a second row, and
    // both are already excluded by the `tokens.len() == 1` and `root_token_id` assertions above; this pins the
    // reason the engine considered it a root in the first place.
    assert_eq!(
        tokens[0].parent_token_id, None,
        "{HARNESS}: the root token is parentless — every other token descends from it, not beside it"
    );
    assert_eq!(
        tokens[0].process_instance_id, instance_id,
        "{HARNESS}: the root token belongs to the instance that minted it"
    );
    assert_eq!(
        tokens[0].node_id, HOLD_NODE,
        "{HARNESS}: the root token carries the graph execution forward to where the process parked"
    );
    assert_eq!(
        tokens[0].status,
        workflow::TokenStatus::Active,
        "{HARNESS}: the root token is live at the human task"
    );

    // The start is recorded once. Two `process.started` events would mean the step ran twice and the token with it.
    assert_eq!(
        events_of_type(harness.store(), &instance_id, "process.started").len(),
        1,
        "{HARNESS}: starting the process records exactly one process.started"
    );

    // ── SCENARIO 2: a second start mints its OWN root — no reuse, no id collision ────────────────────────────────
    // Two starts are two processes, each with one root. If the root id were derived from anything but the store's
    // allocator, these would collide and one start would overwrite the other's token.
    let second = harness
        .engine()
        .start_process(start_params())
        .expect("a second start succeeds for a different subject");
    assert_ne!(
        second.root_token_id, root_token_id,
        "{HARNESS}: a second start must mint its own root token id, not reuse the first one's"
    );
    assert_ne!(
        second.process_instance_id, instance_id,
        "{HARNESS}: a second start is a second process instance"
    );
    assert_eq!(
        tokens_for(harness.store(), &second.process_instance_id).len(),
        1,
        "{HARNESS}: the second instance also holds exactly one token"
    );
    // The first instance's root was not disturbed by the second start.
    assert_eq!(
        read_instance(harness.store(), &instance_id)
            .root_token_id
            .as_deref(),
        Some(root_token_id.as_str()),
        "{HARNESS}: starting another process must not re-point the first instance's root"
    );

    // ── SCENARIO 3 (negative): the same subject cannot start a second active process ───────────────────────────
    // `find_active_by_subject` (`engine_options.rs:112-121`) refuses a second active instance for one subject. That
    // refusal is what keeps "one active process, one root token" true for a subject: without it a second start would
    // happily mint a second root for a subject already in flight.
    let first = harness
        .engine()
        .start_process(start_params_for(SUBJECT_ID))
        .expect("the first start for this subject succeeds");
    let duplicate = harness
        .engine()
        .start_process(start_params_for(SUBJECT_ID))
        .expect_err("a second active process for one subject must be refused");
    assert_eq!(
        duplicate.code(),
        "INSTANCE_ALREADY_ACTIVE",
        "{HARNESS}: a repeated start for the same subject is refused by the subject guard"
    );

    // The refused start committed nothing: the subject still has exactly one instance and exactly one root token.
    assert_eq!(
        tokens_for(harness.store(), &first.process_instance_id).len(),
        1,
        "{HARNESS}: the refused duplicate start must not add a second token to the live instance"
    );
    // The lookup is its own transaction: `MemoryStore::with_tx` takes a non-reentrant store mutex for the whole
    // step (`middle/workflow/src/memory.rs:42`), so a nested `with_tx` on the same store would deadlock.
    let definition_id = read_instance(harness.store(), &first.process_instance_id).definition_id;
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.find_active_by_subject(&definition_id, SUBJECT_TYPE, SUBJECT_ID))
            .expect("the subject lookup reads")
            .map(|instance| instance.id),
        Some(first.process_instance_id.clone()),
        "{HARNESS}: the refused start left the original instance as the subject's one active process"
    );

    // ── SCENARIO 4 (negative, fault injection): a start that cannot mint its root commits nothing ──────────────
    // The invariant is "exactly one", so the interesting failure is a half-written start: an instance with no root,
    // or a root with no instance. The fault is injected at the `Store` seam — `insert_token` answers `Err` exactly
    // as a failed write does, and every other byte of storage is the production `MemoryStore`'s. The store under test
    // is the real `WorkflowEngine`; only the persistence response is hostile.
    let store = RefusingRootTokenStore::new();
    let reader = store.clone();
    let engine = WorkflowEngine::new(
        store,
        EngineOptions {
            app: None,
            now: Box::new(|| 1_700_000_000_000),
        },
    );
    engine
        .seed_definition(linear_definition())
        .expect("the definition registers even while token inserts are refused");
    let refused = engine
        .start_process(start_params())
        .expect_err("a start that cannot mint its root token must fail the step");
    assert_eq!(
        refused.code(),
        "ERROR",
        "{HARNESS}: the refused insert surfaces as the step's own error"
    );

    // Rollback is the whole contract here: the instance insert that happened before the token insert is undone, so
    // "start creates one root token" never leaves an instance without one.
    let leaked = reader
        .memory
        .with_tx(|tx| tx.find_instances(None, None, Some(DEFINITION_KEY), None, 10, 0))
        .expect("instances read");
    assert!(
        leaked.is_empty(),
        "{HARNESS}: a start that failed to mint its root token must roll back the instance too — found {:?}",
        leaked.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
    );
}
