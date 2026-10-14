//! WF.TOKEN — stale token cannot advance (TST-WF-TOKEN-003).
//!
//! Contract: a workflow token that has moved on underneath its holder **cannot advance the process**. The engine's
//! only way to move a token is `WorkflowEngine::move_token`, and it is a compare-and-swap: it submits
//! `token.version` — the version carried by the snapshot it is holding — and refuses the swap unless the durable row
//! still carries it (`middle/workflow/src/engine/execute_node_leave.rs:95-100`). A refused swap becomes
//! `WorkflowError::stale_token` (`:96-99`), whose code is `STALE_TOKEN` (`middle/workflow/src/error.rs:70`), and the
//! refusal is a hard stop, not a warning: `execute_node_leave` returns the error, the enclosing `with_tx` fails, and
//! the whole step rolls back (`middle/workflow/src/memory.rs:46-51` — one BEGIN/ROLLBACK on Neon,
//! `middle/workflow/src/concurrency.rs:10-12`).
//!
//! "Cannot advance the process" is wider than "the swap returned false". A holder that ignored the refusal would still
//! leave the durable state wrong, so this test asserts all three: the swap is refused, the *process* does not advance
//! (the token keeps its node and version, the instance stays put), and the step leaves **no partial write** behind —
//! the task completion that ran before the move is undone too, so a failed advance is not a half-finished one.
//!
//! Staleness is injected the way it actually happens: not by making the store say no, but by handing the engine a
//! snapshot from *before* the concurrent commit. [`StaleTokenStore`] downgrades the `version` every `get_token` and
//! `lock_token` returns, delegating all storage to the production `MemoryStore`. The engine under test is the real
//! `WorkflowEngine`, the CAS is the production one, and the refusal is computed by production code from a value the
//! engine genuinely believed.
//!
//! Level: L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed clock and an in-memory store;
//! no database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_token__003__stale_token_cannot_advance

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    CompleteTaskParams, DefinitionStatus, EngineOptions, MemoryStore, NodeDefinition,
    ProcessDefinition, ProcessEvent, ProcessGraph, ProcessInstance, ProcessOutcome, ProcessStatus,
    Result, StartProcessParams, Store, Task, TaskStatus, Token, TokenOutcome, TokenStatus,
    TransitionDefinition, TxStore, Value, WorkflowEngine,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";

/// The definition key the engine half registers.
const DEFINITION_KEY: &str = "TST-WF-TOKEN-003";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// The graph the engine half drives: `start -> hold (task) -> done (end)`.
const START_NODE: &str = "start";
const HOLD_NODE: &str = "hold";
const DONE_NODE: &str = "done";
const BEGIN: &str = "begin";
const FINISH: &str = "finish";

/// The version a parked root token carries on this graph: minted at 1, one move onto `hold` at 2.
const PARKED_VERSION: i32 = 2;

/// Nodes the standalone CAS half moves a token between.
const CAS_FROM: &str = "left";
const CAS_TO: &str = "right";
const CAS_THIRD: &str = "elsewhere";
const CAS_TOKEN: &str = "tok-stale";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> hold (task) -> done (end)`.
fn linear_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, HOLD_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        HOLD_NODE.to_string(),
        NodeDefinition {
            id: HOLD_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Hold".to_string()),
            transitions: Some(vec![transition(FINISH, DONE_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        DONE_NODE.to_string(),
        NodeDefinition {
            id: DONE_NODE.to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Completed),
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

fn read_token(store: &MemoryStore, id: &str) -> Token {
    store
        .with_tx(|tx| tx.get_token(id))
        .expect("the token is readable")
}

fn read_instance(store: &MemoryStore, id: &str) -> ProcessInstance {
    store
        .with_tx(|tx| tx.get_instance(id))
        .expect("the instance is readable")
}

/// `Store::history` is newest-first, so the collected list is reversed to commit order (the test clock never moves,
/// so descending id is exactly reverse insertion order).
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

/// Seed one active token row directly through the production `Store`.
fn seed_token(store: &MemoryStore, id: &str, node: &str, version: i32) {
    let token = Token {
        id: id.to_string(),
        tenant_id: None,
        process_instance_id: "pi-stale".to_string(),
        parent_token_id: None,
        node_id: node.to_string(),
        status: TokenStatus::Active,
        outcome: None,
        required: true,
        is_able_to_reactivate_parent: true,
        started_at: 0,
        ended_at: None,
        version,
    };
    store
        .with_tx(|tx| tx.insert_token(token.clone()))
        .expect("the seed token commits");
}

/// A `Store` interposer that hands the engine a **stale snapshot**: the token the durable row holds today, reported
/// with the version it carried one commit ago.
///
/// This is the shape of the real race. The row is untouched — `MemoryStore` still holds the current version — so the
/// only thing wrong is the value the engine is reasoning from, exactly as when a reader took a token before another
/// writer advanced it. Every method is forwarded except the two that read a token back, and even those forward and
/// then downgrade the version, so no storage logic is re-implemented here.
struct StaleTokenStore<'a> {
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

/// The version this test's store reports is always one commit behind the durable row.
const STALE_BY: i32 = 1;

fn stale_snapshot(mut token: Token) -> Token {
    token.version = (token.version - STALE_BY).max(0);
    token
}

impl Store for StaleTokenStore<'_> {
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
        insert_token(token: Token) -> Result<Token>;
        complete_token(id: &str, outcome: TokenOutcome, ended_at: i64) -> Result<()>;
        count_active_tokens(instance_id: &str) -> Result<i32>;
        list_active_tokens(instance_id: &str) -> Result<Vec<Token>>;
        count_required_active_siblings(parent_id: &str) -> Result<i32>;
        list_optional_active_siblings(parent_id: &str) -> Result<Vec<Token>>;
        list_children(parent_id: &str) -> Result<Vec<Token>>;
        tokens_for_instance(instance_id: &str) -> Result<Vec<Token>>;
        insert_task(task: Task) -> Result<Task>;
        get_task(id: &str) -> Result<Task>;
        lock_task(id: &str) -> Result<Task>;
        cas_task(task: &Task) -> Result<bool>;
        tasks_for_instance(instance_id: &str) -> Result<Vec<Task>>;
        open_tasks_for_instance(instance_id: &str) -> Result<Vec<Task>>;
        open_tasks_for_token(token_id: &str) -> Result<Vec<Task>>;
        active_tasks_for_user(user_id: &str, tenant_id: Option<&str>) -> Result<Vec<Task>>;
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

    /// The engine's read-back of a token, reported one commit behind the row.
    fn get_token(&mut self, id: &str) -> Result<Token> {
        self.inner.get_token(id).map(stale_snapshot)
    }

    /// The row lock a step takes on a token, reported one commit behind it.
    fn lock_token(&mut self, id: &str) -> Result<Token> {
        self.inner.lock_token(id).map(stale_snapshot)
    }

    /// The production CAS, unwrapped: the point of the test is that the *refusal* is computed by production code.
    fn move_token(&mut self, id: &str, expected_version: i32, to_node: &str) -> Result<bool> {
        self.inner.move_token(id, expected_version, to_node)
    }
}

/// The production `MemoryStore`, driven only through `Store`, with every token read-back downgraded one version.
#[derive(Clone)]
struct StaleSnapshotStore {
    memory: MemoryStore,
}

impl StaleSnapshotStore {
    fn new() -> Self {
        Self {
            memory: MemoryStore::new(),
        }
    }
}

impl TxStore for StaleSnapshotStore {
    fn with_tx<R, F>(&self, mut f: F) -> Result<R>
    where
        F: FnMut(&mut dyn Store) -> Result<R>,
    {
        self.memory.with_tx(|tx| {
            let mut stale = StaleTokenStore { inner: tx };
            f(&mut stale)
        })
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TOKEN-003); the file and the assay use it.
fn wf_token__003__stale_token_cannot_advance() {
    // ── SCENARIO 1: the control — a current token advances the process ────────────────────────────────────────
    // This is what a refusal must be distinguishable from. If the very same call could not advance for some reason
    // other than staleness, the negatives below would pass for the wrong reason.
    let control = EngineHarness::at_unix_millis(1_700_000_000_000);
    control
        .engine()
        .seed_definition(linear_definition())
        .expect("the control definition registers with the engine");
    let started = control
        .engine()
        .start_process(start_params())
        .expect("the control process starts and parks on the human task");
    let control_instance = started.process_instance_id.clone();
    let control_token = started.root_token_id.clone();
    let parked = read_token(control.store(), &control_token);
    assert_eq!(
        parked.node_id, HOLD_NODE,
        "{HARNESS}: the control token parks on the human task"
    );
    assert_eq!(
        parked.version, PARKED_VERSION,
        "{HARNESS}: the control token carries the version the graph left it at"
    );

    let control_task = control
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&control_token))
        .expect("the control task reads")
        .into_iter()
        .next()
        .expect("the engine parked exactly one control task");
    control
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: control_task.id.clone(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH.to_string()),
        })
        .expect("completing the control task advances its token");

    let advanced = read_token(control.store(), &control_token);
    assert_eq!(
        advanced.node_id, DONE_NODE,
        "{HARNESS}: the control token advanced to the end node — the advance this contract protects"
    );
    assert_eq!(
        advanced.status,
        TokenStatus::Completed,
        "{HARNESS}: the control token completed at the end node"
    );
    // Two versions, and exactly two: the completing transition's move commits one, and the end node's own
    // `complete_token` commits the second. Stating the accounting is the point — a caller that advanced by some other
    // amount would be advancing without the CAS pairing each version with a swap.
    assert_eq!(
        advanced.version,
        PARKED_VERSION + 2,
        "{HARNESS}: the control advance consumed the transition move's version and the end node's completion"
    );
    assert_eq!(
        events_of_type(control.store(), &control_instance, "token.moved").len(),
        2,
        "{HARNESS}: the control run records one move for the graph walk and one for the completing transition"
    );

    // ── SCENARIO 2 (negative): a stale token is refused, and the process does not advance ───────────────────────
    // The engine holds a snapshot one version behind the durable row. Its move submits that stale version, the
    // production CAS refuses it, and the engine turns the refusal into `STALE_TOKEN`.
    let store = StaleSnapshotStore::new();
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
        .expect("the stale-snapshot definition registers with the engine");
    let started = engine
        .start_process(start_params())
        .expect("a start under stale reads still parks the token");
    let instance_id = started.process_instance_id.clone();
    let token_id = started.root_token_id.clone();

    // The durable row is genuinely current — only the engine's view of it is behind. Pinning this is what makes the
    // refusal a staleness refusal and not a broken fixture.
    let durable = read_token(&reader.memory, &token_id);
    assert_eq!(
        durable.node_id, HOLD_NODE,
        "{HARNESS}: the durable token is parked before the stale attempt"
    );
    assert_eq!(
        durable.version, PARKED_VERSION,
        "{HARNESS}: the durable row carries the current version; only the engine's snapshot is behind"
    );

    let task = reader
        .memory
        .with_tx(|tx| tx.open_tasks_for_token(&token_id))
        .expect("the stale run's task reads")
        .into_iter()
        .next()
        .expect("the engine parked exactly one task");
    let refusal = engine
        .complete_task(CompleteTaskParams {
            task_id: task.id.clone(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH.to_string()),
        })
        .expect_err("a stale token must not advance the process");
    assert_eq!(
        refusal.code(),
        "STALE_TOKEN",
        "{HARNESS}: a refused stale swap surfaces as the engine's own STALE_TOKEN"
    );

    // The process did not advance. Every field a holder would have overwritten is exactly as the winner left it.
    let after = read_token(&reader.memory, &token_id);
    assert_eq!(
        after.node_id, HOLD_NODE,
        "{HARNESS}: a stale token must not move the token's node"
    );
    assert_eq!(
        after.version, PARKED_VERSION,
        "{HARNESS}: a stale token must not consume a version"
    );
    assert_eq!(
        after.status,
        TokenStatus::Active,
        "{HARNESS}: a stale token must not complete or suspend the token"
    );
    assert_eq!(
        read_instance(&reader.memory, &instance_id).status,
        ProcessStatus::Active,
        "{HARNESS}: the process must not advance past a refused stale token"
    );

    // No partial write survives the refusal. `complete_task` marks the task completed and records `task.completed`
    // *before* it reaches the move (`middle/workflow/src/engine/engine_options.rs:415-434`); a failed advance that
    // kept those would leave the process half-finished, which is the failure this assertion exists to catch.
    let task_after = reader
        .memory
        .with_tx(|tx| tx.get_task(&task.id))
        .expect("the task reads after the refusal");
    assert_eq!(
        task_after.status,
        TaskStatus::Ready,
        "{HARNESS}: a refused stale advance must roll back the task completion too, not half-apply it"
    );
    assert_eq!(
        task_after.version, 1,
        "{HARNESS}: the task version must be rolled back with the completion"
    );
    assert!(
        events_of_type(&reader.memory, &instance_id, "task.completed").is_empty(),
        "{HARNESS}: a refused stale advance must leave no task.completed event behind"
    );
    assert_eq!(
        events_of_type(&reader.memory, &instance_id, "token.moved").len(),
        1,
        "{HARNESS}: only the graph walk's own move is recorded — the stale transition added none"
    );

    // And the process is still exactly where it was: the task is still open and completable, so a caller holding a
    // current token can still finish it. A refusal that left the process unrecoverable would be a different bug.
    assert_eq!(
        reader
            .memory
            .with_tx(|tx| tx.open_tasks_for_token(&token_id))
            .expect("the task lookup reads")
            .len(),
        1,
        "{HARNESS}: the refused advance left the task open, so the process is still completable"
    );

    // ── SCENARIO 3: the stale holder cannot advance past the winner, at the persistence boundary ───────────────
    // The same invariant at its narrowest: a snapshot taken before a commit cannot move the token afterwards, and the
    // winner's node and version survive the attempt untouched.
    let store = MemoryStore::new();
    seed_token(&store, CAS_TOKEN, CAS_FROM, 1);

    // The stale holder takes its snapshot here, at version 1.
    let held = read_token(&store, CAS_TOKEN);
    assert_eq!(held.version, 1);

    // Another holder commits first: the version this one is holding is now spent.
    let won = store
        .with_tx(|tx| tx.move_token(CAS_TOKEN, held.version, CAS_TO))
        .expect("the winner's move step commits");
    assert!(
        won,
        "{HARNESS}: the first holder's swap commits, spending the version the stale one holds"
    );

    // The stale holder now tries to advance the token with the snapshot it took. It is refused, and — the direction
    // this contract is about — the token does not go where the stale holder wanted it.
    let refused = store
        .with_tx(|tx| tx.move_token(CAS_TOKEN, held.version, CAS_THIRD))
        .expect("a refusal is an answer, not an error");
    assert!(
        !refused,
        "{HARNESS}: a token snapshot one commit behind must not advance the token"
    );
    let survivor = read_token(&store, CAS_TOKEN);
    assert_eq!(
        survivor.node_id, CAS_TO,
        "{HARNESS}: a refused stale advance leaves the token where the winner left it"
    );
    assert_eq!(
        survivor.version, 2,
        "{HARNESS}: a refused stale advance consumes no version"
    );

    // The stale holder's own version is permanently spent: retrying it does not eventually succeed. Only a current
    // version advances the token, which is what stops a losing holder from advancing a second time.
    let retried = store
        .with_tx(|tx| tx.move_token(CAS_TOKEN, held.version, CAS_THIRD))
        .expect("a second refusal is an answer, not an error");
    assert!(
        !retried,
        "{HARNESS}: retrying with the same stale version must stay refused"
    );
    assert_eq!(
        read_token(&store, CAS_TOKEN).node_id,
        CAS_TO,
        "{HARNESS}: a retried stale advance still leaves the token alone"
    );
}
