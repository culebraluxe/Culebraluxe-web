//! WF.TOKEN — move uses CAS (TST-WF-TOKEN-002).
//!
//! Contract: moving a workflow token is a **compare-and-swap** at the persistence boundary, not a blind overwrite.
//! `Store::move_token(id, expected_version, to_node)` commits the move only when the row's `version` still equals the
//! `expected_version` the caller holds; on success it sets `node_id` to `to_node` and advances `version` by exactly
//! one (`middle/workflow/src/memory.rs:238-249`, `middle/workflow/src/neon/new_id.rs:253-265`). A move carrying
//! a stale version is refused — `Ok(false)`, no node change, no version bump — and a move for an absent row is refused
//! the same way (it is not a create). The engine's only move path relies on that result: `move_token` at
//! `middle/workflow/src/engine/execute_node_leave.rs:95` turns a refused swap into `WorkflowError::stale_token`
//! (`:96-99`), so a lost race is a hard refusal rather than a silent second write.
//!
//! This is why the contract is a CAS and not an `UPDATE ... SET node_id`: two holders of the same token cannot both
//! commit; only the caller whose expected version is still current swaps the row, and every other caller is refused.
//!
//! This file exercises the production boundary, not a re-declaration of it. The CAS half drives the production
//! `Store` interface implemented by the production `MemoryStore` (the same interface `NeonStore` implements with the
//! identical `WHERE id = $1 AND version = $2` predicate, `middle/workflow/src/neon/new_id.rs:257-258`). The engine
//! half drives the real `WorkflowEngine<MemoryStore>` through `start_process` and `complete_task`, observing the
//! durable `token.moved` events and the token's own durable version, so the moves it reads back are the ones the
//! production method committed. The engine-side negative injects a refused CAS at the `Store` seam — a fake only at
//! the defined production interface, delegating every byte of storage to `MemoryStore` — and pins that the engine
//! surfaces `STALE_TOKEN` and rolls the step back instead of advancing the token anyway.
//!
//! Level: L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed clock and an in-memory store;
//! no database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_token__002__move_uses_cas

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    CompleteTaskParams, DefinitionStatus, EngineOptions, Job, MemoryStore, NodeDefinition,
    ProcessCommand, ProcessDefinition, ProcessEvent, ProcessGraph, ProcessInstance, ProcessOutcome,
    ProcessStatus, Result, StartProcessParams, Store, Task, Token, TokenOutcome, TokenStatus,
    TransitionDefinition, TxStore, Value, WorkflowEngine,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";

/// The definition key/version the engine half registers.
const DEFINITION_KEY: &str = "TST-WF-TOKEN-002";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// The linear graph the engine half drives: `start -> work (service) -> hold (task) -> done (end)`.
const START_NODE: &str = "start";
const WORK_NODE: &str = "work";
const HOLD_NODE: &str = "hold";
const DONE_NODE: &str = "done";
const BEGIN: &str = "begin";
const NEXT: &str = "next";
const FINISH: &str = "finish";

/// Nodes the standalone CAS half moves a token between.
const CAS_FROM: &str = "left";
const CAS_TO: &str = "right";
const CAS_THIRD: &str = "elsewhere";
const CAS_TOKEN: &str = "tok-cas";
const CAS_ABSENT: &str = "tok-absent";

/// Seed one active token row directly through the production `Store`.
fn seed_token(store: &MemoryStore, id: &str, node: &str, version: i32) {
    let token = Token {
        id: id.to_string(),
        tenant_id: None,
        process_instance_id: "pi-cas".to_string(),
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

fn read_token(store: &MemoryStore, id: &str) -> Token {
    store
        .with_tx(|tx| tx.get_token(id))
        .expect("the token is readable")
}

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> work (service) -> hold (task) -> done (end)`.
fn linear_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, WORK_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        WORK_NODE.to_string(),
        NodeDefinition {
            id: WORK_NODE.to_string(),
            node_type: "service".to_string(),
            name: Some("Work".to_string()),
            transitions: Some(vec![transition(NEXT, HOLD_NODE)]),
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

/// The `(from, to, transition)` of every committed `token.moved`, in commit order.
///
/// `Store::history` is newest-first (`middle/workflow/src/memory.rs:568` sorts by descending id), so the collected
/// list is reversed here: ids are assigned in insertion order and the test clock never moves, so descending id is
/// exactly reverse commit order.
fn token_moves(store: &MemoryStore, instance_id: &str) -> Vec<(String, String, String)> {
    let mut moves: Vec<(String, String, String)> = store
        .with_tx(|tx| tx.history(instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == "token.moved")
        .map(|event| {
            (
                event
                    .data
                    .get("from")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                event.node_id.clone().unwrap_or_default(),
                event
                    .data
                    .get("transition")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect();
    moves.reverse();
    moves
}

/// A `Store` interposer that refuses every token move, delegating all other storage to the production `MemoryStore`.
///
/// This is the smallest seam that can control the CAS outcome the engine sees without a second store and without
/// duplicating business logic: every method but `move_token` is forwarded to the production implementation, and
/// `move_token` answers `Ok(false)` exactly as a lost race does. It is the `Store` boundary — the same interface
/// `NeonStore` implements — so the engine under test is the real `WorkflowEngine`.
struct RefusingMoveStore<'a> {
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

impl Store for RefusingMoveStore<'_> {
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
        get_token(id: &str) -> Result<Token>;
        lock_token(id: &str) -> Result<Token>;
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
        insert_job(job: Job) -> Result<Job>;
        get_job(id: &str) -> Result<Job>;
        lock_job(id: &str) -> Result<Job>;
        update_job(job: &Job) -> Result<()>;
        claim_job(job_id: &str, worker_id: &str, now: i64, lease_until: i64) -> Result<Option<Job>>;
        claim_due_jobs(worker_id: &str, now: i64, lease_until: i64, limit: usize) -> Result<Vec<Job>>;
        claim_due_jobs_by_type(worker_id: &str, job_type: &str, now: i64, lease_until: i64, limit: usize) -> Result<Vec<Job>>;
        reclaim_stale_jobs(now: i64, batch: usize, instance_id: Option<&str>) -> Result<usize>;
        open_jobs_for_instance(instance_id: &str) -> Result<Vec<Job>>;
        open_jobs_for_token(token_id: &str) -> Result<Vec<Job>>;
        list_overdue_jobs(now: i64, limit: usize) -> Result<Vec<Job>>;
        insert_event(event: ProcessEvent) -> Result<()>;
        history(instance_id: &str, limit: usize) -> Result<Vec<ProcessEvent>>;
        command_visit_count(instance_id: &str, node_id: &str) -> Result<i32>;
        insert_command(cmd: ProcessCommand) -> Result<()>;
        find_instances(tenant_id: Option<&str>, status: Option<&[ProcessStatus]>, definition_key: Option<&str>, business_key: Option<&str>, limit: usize, offset: usize) -> Result<Vec<ProcessInstance>>;
    }

    /// The refused swap: exactly what a lost compare-and-swap returns, and nothing is written.
    fn move_token(&mut self, _id: &str, _expected_version: i32, _to_node: &str) -> Result<bool> {
        Ok(false)
    }
}

/// The production `MemoryStore`, driven only through `Store`, with every `move_token` refused at the seam.
#[derive(Clone)]
struct RefusingCasStore {
    memory: MemoryStore,
}

impl RefusingCasStore {
    fn new() -> Self {
        Self {
            memory: MemoryStore::new(),
        }
    }
}

impl TxStore for RefusingCasStore {
    fn with_tx<R, F>(&self, mut f: F) -> Result<R>
    where
        F: FnMut(&mut dyn Store) -> Result<R>,
    {
        self.memory.with_tx(|tx| {
            let mut refusing = RefusingMoveStore { inner: tx };
            f(&mut refusing)
        })
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TOKEN-002); the file and the assay use it.
fn wf_token_002__move_uses_cas() {
    // ── SCENARIO 1: the swap itself — current version commits, stale version is refused without a write ────────
    // The subject is `Store::move_token`, the production persistence boundary the engine calls. A blind
    // `UPDATE ... SET node_id` would satisfy the positive and fail the stale negative below, which is the point.
    let store = MemoryStore::new();
    seed_token(&store, CAS_TOKEN, CAS_FROM, 1);

    // POSITIVE: the caller holds the current version, so the swap commits and advances the version by exactly one.
    let moved = store
        .with_tx(|tx| tx.move_token(CAS_TOKEN, 1, CAS_TO))
        .expect("the move step commits");
    assert!(
        moved,
        "{HARNESS}: a move carrying the current version must commit"
    );
    let after = read_token(&store, CAS_TOKEN);
    assert_eq!(
        after.node_id, CAS_TO,
        "{HARNESS}: a committed move writes the destination node"
    );
    assert_eq!(
        after.version, 2,
        "{HARNESS}: a committed move advances the version by exactly one"
    );

    // NEGATIVE (the load-bearing one): the version the caller held is now spent. A move carrying it must be refused
    // and must leave the row exactly as the winning move left it. A blind overwrite would have moved to CAS_THIRD
    // and/or bumped the version, so this is what distinguishes a CAS from a write.
    let refused = store
        .with_tx(|tx| tx.move_token(CAS_TOKEN, 1, CAS_THIRD))
        .expect("a refusal is an answer, not an error");
    assert!(
        !refused,
        "{HARNESS}: a move carrying a stale version must be refused"
    );
    let unchanged = read_token(&store, CAS_TOKEN);
    assert_eq!(
        unchanged.node_id, CAS_TO,
        "{HARNESS}: a refused move must not change the node"
    );
    assert_eq!(
        unchanged.version, 2,
        "{HARNESS}: a refused move must not bump the version"
    );

    // The CAS tracks the moving version, not a fixed one: the new current version (2) commits the next move.
    let moved_again = store
        .with_tx(|tx| tx.move_token(CAS_TOKEN, 2, CAS_THIRD))
        .expect("the second move step commits");
    assert!(
        moved_again,
        "{HARNESS}: the version the last committed move wrote is the current one and must be accepted"
    );
    let third = read_token(&store, CAS_TOKEN);
    assert_eq!(third.node_id, CAS_THIRD);
    assert_eq!(third.version, 3);

    // And once more: the version spent by the second move is now stale and is refused.
    let refused_again = store
        .with_tx(|tx| tx.move_token(CAS_TOKEN, 2, CAS_FROM))
        .expect("a refusal is an answer, not an error");
    assert!(
        !refused_again,
        "{HARNESS}: the version the second move spent is stale and must be refused"
    );
    let still_third = read_token(&store, CAS_TOKEN);
    assert_eq!(still_third.node_id, CAS_THIRD);
    assert_eq!(still_third.version, 3);

    // NEGATIVE: a move for an absent row is a refusal, not a create — no row is conjured and the caller is told so.
    let absent = store
        .with_tx(|tx| tx.move_token(CAS_ABSENT, 1, CAS_TO))
        .expect("a refusal is an answer, not an error");
    assert!(
        !absent,
        "{HARNESS}: a move for an absent token must be refused"
    );
    assert!(
        store.with_tx(|tx| tx.get_token(CAS_ABSENT)).is_err(),
        "{HARNESS}: a refused move on an absent token must not create a row"
    );

    // ── SCENARIO 2: the engine's move path goes through that production method, version by version ──────────────
    // The real `WorkflowEngine<MemoryStore>` is driven through its public steps. Every node change it commits is a
    // `Store::move_token`, so the durable node and version are the CAS's own output (there is no other writer).
    let harness = EngineHarness::at_unix_millis(1_700_000_000_000);
    harness
        .engine()
        .seed_definition(linear_definition())
        .expect("the linear definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on the human task");
    let instance = started.process_instance_id.clone();
    let root_token = started.root_token_id.clone();

    // start (v1) -> work (v2) -> hold (v3): two moves, each one version, then the task node parks the token.
    let parked = harness
        .store()
        .with_tx(|tx| tx.get_token(&root_token))
        .expect("the parked token reads");
    assert_eq!(
        parked.node_id, HOLD_NODE,
        "{HARNESS}: the engine moved the token to the node the graph reaches"
    );
    assert_eq!(
        parked.version, 3,
        "{HARNESS}: the engine committed two moves, and each move advanced the version by exactly one"
    );
    assert_eq!(
        parked.status,
        TokenStatus::Active,
        "{HARNESS}: the token is still live at the human task"
    );

    let first_moves = token_moves(harness.store(), &instance);
    assert_eq!(
        first_moves,
        vec![
            (
                START_NODE.to_string(),
                WORK_NODE.to_string(),
                BEGIN.to_string()
            ),
            (
                WORK_NODE.to_string(),
                HOLD_NODE.to_string(),
                NEXT.to_string()
            ),
        ],
        "{HARNESS}: every committed move is durably recorded with its from/to and transition"
    );

    // Completing the task advances the token once more through the same production move, then the end node
    // completes it.
    let task = harness
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&root_token))
        .expect("the open task reads")
        .into_iter()
        .next()
        .expect("the engine parked exactly one task");
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task.id.clone(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH.to_string()),
        })
        .expect("completing the task advances the token");

    let finished = harness
        .store()
        .with_tx(|tx| tx.get_token(&root_token))
        .expect("the finished token reads");
    assert_eq!(
        finished.node_id, DONE_NODE,
        "{HARNESS}: completing the task moves the token to the end node"
    );
    assert_eq!(
        finished.status,
        TokenStatus::Completed,
        "{HARNESS}: the end node completes the token"
    );
    // Three moves (v1->v2->v3->v4) plus the completion's own version bump (v4->v5).
    assert_eq!(
        finished.version, 5,
        "{HARNESS}: three moves then one completion advance the version one step at a time"
    );
    let all_moves = token_moves(harness.store(), &instance);
    assert_eq!(
        all_moves.len(),
        3,
        "{HARNESS}: exactly one token.moved per engine move, no more and no fewer"
    );
    assert_eq!(
        all_moves[2],
        (
            HOLD_NODE.to_string(),
            DONE_NODE.to_string(),
            FINISH.to_string()
        ),
        "{HARNESS}: the completing move is the task transition, recorded like every other move"
    );
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance))
            .expect("the instance reads")
            .status,
        ProcessStatus::Completed,
        "{HARNESS}: the process reaches its terminal state through the moves"
    );

    // ── SCENARIO 3: the engine refuses a refused swap — it must not advance a token whose CAS was lost ──────────
    // The fault is injected at the `Store` seam: every other byte of storage is `MemoryStore`'s, and `move_token`
    // answers `false` exactly as a lost race does. This is the negative that fails if the engine ever ignored the
    // CAS result and advanced the token anyway.
    let store = RefusingCasStore::new();
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
        .expect("the definition registers even while moves are refused");
    let refusal = engine
        .start_process(start_params())
        .expect_err("a refused token move must fail the step, not be ignored");
    assert_eq!(
        refusal.code(),
        "STALE_TOKEN",
        "{HARNESS}: a refused CAS surfaces as STALE_TOKEN, the engine's own refusal"
    );

    // The refused step is rolled back: no instance survived it, so nothing was advanced past the lost swap.
    let leaked = reader
        .memory
        .with_tx(|tx| tx.find_instances(None, None, Some(DEFINITION_KEY), None, 10, 0))
        .expect("instances read");
    assert!(
        leaked.is_empty(),
        "{HARNESS}: a refused swap rolls the whole step back — no instance is committed"
    );
}
