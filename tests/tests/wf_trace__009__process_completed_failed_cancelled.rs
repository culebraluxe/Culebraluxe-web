//! WF.TRACE — process completed / failed / cancelled (TST-WF-TRACE-009).
//!
//! Contract: a process reaches three terminal states, and each one is recorded on the durable log with the
//! outcome it died of — `Completed`, `Failed` (status `Error`) and `Cancelled` (status `Aborted`) — through the
//! matching exit event: `process.completed`, `process.failed`, `process.cancelled`.
//!
//! What ends a process is declared by the node it ends at, not by the name of the transition that reached it. An
//! `end` node carries `outcome` (`docs/workflow-xml-model.md:83`, `end-state` with outcome
//! completed|cancelled|failed|conflict), and `execute_node_leave.rs:33-39` reads exactly that
//! (`node.outcome.unwrap_or(Completed)`). A non-`Completed` end outcome passes through
//! `resolve_process_after_token` (`:280-296`) into `terminate_process` (`:204-278`), which maps the outcome to the
//! status (`Cancelled -> Aborted`, `Failed -> Error`), cancels the open tokens and jobs with their own events,
//! obsoletes the open tasks, and finally writes the single exit event (`process.cancelled` / `process.failed`)
//! carrying `data.outcome`. `Completed` is the other path: `check_process_completion` (`:176-202`) settles the
//! instance `Completed`/`Completed` and emits `process.completed` with `data.outcome = "completed"`.
//!
//! Explicit cancellation is its own boundary: `WorkflowEngine::cancel_process` (`engine_options.rs:465-488`)
//! terminates an `Active` instance `Cancelled` — the operator's exit, distinct from the graph's — and is
//! idempotent once the instance is already cancelled.
//!
//! Corrected against the engine: the first draft drove failure and cancellation through task transitions named
//! `force_error` and `cancel` and expected `complete_task` to refuse the `force_error` command. Transition names
//! carry no terminal semantics — the end node each one reaches does — and completing a task that routes into a
//! failing end node is not an error: the task command succeeded, it is the *process* that carries the failure.
//! All three exits are driven here through boundaries the engine actually has, plus the explicit operator
//! cancellation the draft never touched.
//!
//! Level: L2 Composition, harness `WorkflowHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__009__process_completed_failed_cancelled

use test_harness::EngineHarness;
use workflow::{
    CancelProcessParams, CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition,
    ProcessEvent, ProcessGraph, ProcessInstance, ProcessOutcome, ProcessStatus, StartProcessParams,
    Task, TaskStatus, TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L2 Composition";
const DEFINITION_KEY: &str = "TST-WF-TRACE-009";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const CANCEL_REASON: &str = "operator cancelled the run";
/// The engine's frozen clock: every scenario runs at the same deterministic instant.
const START_AT: i64 = 1_600_000_000_000;

const START_NODE: &str = "start";
const APPROVAL_NODE: &str = "approval";
const COMPLETE_NODE: &str = "end_completed";
const FAILURE_NODE: &str = "end_failed";
const CANCEL_NODE: &str = "end_cancelled";
const BEGIN: &str = "begin";
const APPROVE: &str = "approve";
const FORCE_ERROR: &str = "force_error";
const CANCEL: &str = "cancel";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

fn end_node(id: &str, outcome: ProcessOutcome) -> NodeDefinition {
    NodeDefinition {
        id: id.to_string(),
        node_type: "end".to_string(),
        outcome: Some(outcome),
        ..Default::default()
    }
}

/// `start -> approval (task) -> {end_completed | end_failed | end_cancelled}`.
///
/// One task, three exits, each declaring its own terminal outcome — the names of the transitions are the
/// command's vocabulary, the end nodes are the process's.
fn definition() -> ProcessDefinition {
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, APPROVAL_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        APPROVAL_NODE.to_string(),
        NodeDefinition {
            id: APPROVAL_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Approval".to_string()),
            transitions: Some(vec![
                transition(APPROVE, COMPLETE_NODE),
                transition(FORCE_ERROR, FAILURE_NODE),
                transition(CANCEL, CANCEL_NODE),
            ]),
            ..Default::default()
        },
    );
    for (node_id, outcome) in [
        (COMPLETE_NODE, ProcessOutcome::Completed),
        (FAILURE_NODE, ProcessOutcome::Failed),
        (CANCEL_NODE, ProcessOutcome::Cancelled),
    ] {
        nodes.insert(node_id.to_string(), end_node(node_id, outcome));
    }
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

fn instance(harness: &EngineHarness, instance_id: &str) -> ProcessInstance {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance_id))
        .expect("the instance is readable")
}

/// `(status, outcome)`, read from the row rather than from any returned summary.
fn terminal_state(harness: &EngineHarness, instance_id: &str) -> (ProcessStatus, Option<ProcessOutcome>) {
    let row = instance(harness, instance_id);
    (row.status, row.outcome)
}

fn history(harness: &EngineHarness, instance_id: &str) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance_id, 512))
        .expect("the instance history reads")
}

fn events_of_type(harness: &EngineHarness, instance_id: &str, event_type: &str) -> Vec<ProcessEvent> {
    history(harness, instance_id)
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

fn task_row(harness: &EngineHarness, task_id: &str) -> Task {
    harness
        .store()
        .with_tx(|tx| tx.get_task(task_id))
        .expect("the task row is readable")
}

/// Start a fresh process on the frozen clock and hand back `(instance_id, task_id)` of the parked task.
fn start_and_park(harness: &EngineHarness) -> (String, String) {
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on its approval task");
    let instance_id = started.process_instance_id;
    let task_id = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&instance_id))
        .expect("the instance's tasks are readable")
        .into_iter()
        .filter(|task| task.status == TaskStatus::Ready)
        .last()
        .expect("the process parks on a ready task before the command")
        .id;
    assert_eq!(
        instance(harness, &instance_id).status,
        ProcessStatus::Active,
        "{HARNESS}: a freshly started process is active"
    );
    (instance_id, task_id)
}

/// Complete the parked task along one named transition. Returns the command's own result, so a scenario can
/// distinguish "the command was refused" from "the process died".
fn complete_task(
    harness: &EngineHarness,
    task_id: &str,
    transition_name: &str,
) -> workflow::Result<()> {
    harness.engine().complete_task(CompleteTaskParams {
        task_id: task_id.to_string(),
        user_id: STARTED_BY.to_string(),
        form_data: Value::object(),
        transition_name: Some(transition_name.to_string()),
    })
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TRACE-009); the file and the assay use it.
fn wf_trace_009__process_completed_failed_cancelled() {
    let harness = EngineHarness::at_unix_millis(START_AT);
    harness
        .engine()
        .seed_definition(definition())
        .expect("the definition registers with the engine");

    // ── 1. COMPLETED: the task's `approve` exit reaches the end node that declares completion ──────────────
    let (completed_instance, completed_task) = start_and_park(&harness);
    complete_task(&harness, &completed_task, APPROVE).expect("the approval command is accepted");
    let (status, outcome) = terminal_state(&harness, &completed_instance);
    assert_eq!(
        status,
        ProcessStatus::Completed,
        "{HARNESS}: the last token left through the completed end node"
    );
    assert_eq!(
        outcome,
        Some(ProcessOutcome::Completed),
        "{HARNESS}: the end node's declared outcome is recorded"
    );
    assert_eq!(
        instance(&harness, &completed_instance).ended_at,
        Some(START_AT),
        "{HARNESS}: the process records when it ended"
    );
    let exit = events_of_type(&harness, &completed_instance, "process.completed");
    assert_eq!(
        exit.len(),
        1,
        "{HARNESS}: the completion is announced exactly once"
    );
    assert_eq!(
        exit[0].data.get("outcome").and_then(|v| v.as_str()),
        Some("completed")
    );
    assert_eq!(exit[0].actor, STARTED_BY, "{HARNESS}: the exit names who caused it");
    assert_eq!(exit[0].job_id, None, "{HARNESS}: a process exit is not a job fact");
    assert_eq!(exit[0].task_id, None, "{HARNESS}: a process exit is not a task fact");
    assert_eq!(
        events_of_type(&harness, &completed_instance, "process.failed").len(),
        0,
        "{HARNESS}: a completed process did not fail"
    );
    assert_eq!(
        events_of_type(&harness, &completed_instance, "process.cancelled").len(),
        0,
        "{HARNESS}: a completed process was not cancelled"
    );

    // ── 2. FAILED: the `force_error` exit reaches the end node that declares failure ────────────────────────
    let (failed_instance, failing_task) = start_and_park(&harness);
    // Completing the task is the command succeeding — it did what it was told. The failure belongs to the
    // process the command routed into, which is why this call is Ok and the *instance* carries the failure.
    complete_task(&harness, &failing_task, FORCE_ERROR)
        .expect("the command is accepted; the process it routes into is what fails");
    let (status, outcome) = terminal_state(&harness, &failed_instance);
    assert_eq!(
        status,
        ProcessStatus::Error,
        "{HARNESS}: a failed process is recorded in Error"
    );
    assert_eq!(outcome, Some(ProcessOutcome::Failed));
    let exit = events_of_type(&harness, &failed_instance, "process.failed");
    assert_eq!(
        exit.len(),
        1,
        "{HARNESS}: the failure is announced exactly once"
    );
    assert_eq!(
        exit[0].data.get("outcome").and_then(|v| v.as_str()),
        Some("failed")
    );
    assert_eq!(
        exit[0].data.get("reason").and_then(|v| v.as_str()),
        Some("end node outcome: Failed"),
        "{HARNESS}: the exit names the end outcome the process died of"
    );
    assert_eq!(
        events_of_type(&harness, &failed_instance, "process.completed").len(),
        0,
        "{HARNESS}: a failed process was not completed"
    );

    // ── 3. CANCELLED by the graph: the `cancel` exit reaches the end node that declares cancellation ────────
    let (cancelled_instance, cancelled_task) = start_and_park(&harness);
    complete_task(&harness, &cancelled_task, CANCEL).expect("the cancellation command is accepted");
    let (status, outcome) = terminal_state(&harness, &cancelled_instance);
    assert_eq!(
        status,
        ProcessStatus::Aborted,
        "{HARNESS}: cancellation is recorded as an abort"
    );
    assert_eq!(outcome, Some(ProcessOutcome::Cancelled));
    let exit = events_of_type(&harness, &cancelled_instance, "process.cancelled");
    assert_eq!(
        exit.len(),
        1,
        "{HARNESS}: the cancellation is announced exactly once"
    );
    assert_eq!(
        exit[0].data.get("outcome").and_then(|v| v.as_str()),
        Some("cancelled")
    );
    assert_eq!(
        events_of_type(&harness, &cancelled_instance, "process.completed").len(),
        0,
        "{HARNESS}: a cancelled process was not completed"
    );
    assert_eq!(
        events_of_type(&harness, &cancelled_instance, "process.failed").len(),
        0,
        "{HARNESS}: a cancelled process did not fail"
    );

    // ── 4. CANCELLED by the operator, on a process that is still waiting on its task ────────────────────────
    let (stopped_instance, waiting_task) = start_and_park(&harness);
    harness
        .engine()
        .cancel_process(CancelProcessParams {
            process_instance_id: stopped_instance.clone(),
            actor: STARTED_BY.to_string(),
            reason: Some(CANCEL_REASON.to_string()),
        })
        .expect("the operator cancels a running process");
    let (status, outcome) = terminal_state(&harness, &stopped_instance);
    assert_eq!(
        status,
        ProcessStatus::Aborted,
        "{HARNESS}: explicit cancellation aborts the instance"
    );
    assert_eq!(outcome, Some(ProcessOutcome::Cancelled));
    let exit = events_of_type(&harness, &stopped_instance, "process.cancelled");
    assert_eq!(
        exit.len(),
        1,
        "{HARNESS}: the operator's cancellation is announced once"
    );
    assert_eq!(
        exit[0].data.get("outcome").and_then(|v| v.as_str()),
        Some("cancelled")
    );
    assert_eq!(
        exit[0].data.get("reason").and_then(|v| v.as_str()),
        Some(CANCEL_REASON),
        "{HARNESS}: the exit carries the reason the operator gave"
    );
    assert_eq!(exit[0].actor, STARTED_BY);
    // The token the process was waiting on is cancelled too, and says why.
    let tokens = events_of_type(&harness, &stopped_instance, "token.cancelled");
    assert_eq!(
        tokens.len(),
        1,
        "{HARNESS}: the parked token is cancelled with the process"
    );
    assert_eq!(
        tokens[0].data.get("reason").and_then(|v| v.as_str()),
        Some(CANCEL_REASON)
    );
    // And the work nobody can hand in any more is obsoleted, not left Ready.
    let obsoleted = events_of_type(&harness, &stopped_instance, "task.obsoleted");
    assert_eq!(
        obsoleted.len(),
        1,
        "{HARNESS}: the open task is obsoleted with the process"
    );
    assert_eq!(obsoleted[0].task_id.as_deref(), Some(waiting_task.as_str()));
    assert_eq!(task_row(&harness, &waiting_task).status, TaskStatus::Obsolete);
    // Completing the obsoleted task afterwards is refused, and the refusal changes nothing.
    let refusal = complete_task(&harness, &waiting_task, APPROVE)
        .expect_err("an obsoleted task cannot be completed");
    assert_eq!(
        refusal.code(),
        "TASK_NOT_ACTIONABLE",
        "{HARNESS}: the refusal names the actionable-status rule"
    );
    let (still_stopped, still_cancelled) = terminal_state(&harness, &stopped_instance);
    assert_eq!(still_stopped, ProcessStatus::Aborted);
    assert_eq!(still_cancelled, Some(ProcessOutcome::Cancelled));
    assert_eq!(
        events_of_type(&harness, &stopped_instance, "process.completed").len(),
        0
    );
    // Cancelling again is a no-op, not a second exit.
    harness
        .engine()
        .cancel_process(CancelProcessParams {
            process_instance_id: stopped_instance.clone(),
            actor: STARTED_BY.to_string(),
            reason: None,
        })
        .expect("cancelling an already-cancelled process is idempotent");
    assert_eq!(
        events_of_type(&harness, &stopped_instance, "process.cancelled").len(),
        1,
        "{HARNESS}: the exit is recorded once, however often it is asked for"
    );

    // ── 5. The three exits are distinct, and every instance holds exactly one of them ───────────────────────
    for (instance_id, terminal_event, terminal_status, terminal_outcome) in [
        (
            completed_instance.as_str(),
            "process.completed",
            ProcessStatus::Completed,
            ProcessOutcome::Completed,
        ),
        (
            failed_instance.as_str(),
            "process.failed",
            ProcessStatus::Error,
            ProcessOutcome::Failed,
        ),
        (
            cancelled_instance.as_str(),
            "process.cancelled",
            ProcessStatus::Aborted,
            ProcessOutcome::Cancelled,
        ),
        (
            stopped_instance.as_str(),
            "process.cancelled",
            ProcessStatus::Aborted,
            ProcessOutcome::Cancelled,
        ),
    ] {
        let (status, outcome) = terminal_state(&harness, instance_id);
        assert_eq!(
            status, terminal_status,
            "{HARNESS}: {instance_id} ended {terminal_status:?}"
        );
        assert_eq!(outcome, Some(terminal_outcome));
        assert_eq!(
            events_of_type(&harness, instance_id, terminal_event).len(),
            1,
            "{HARNESS}: {instance_id} recorded its own exit"
        );
        let exits = history(&harness, instance_id)
            .into_iter()
            .filter(|event| event.event_type.starts_with("process."))
            .filter(|event| event.event_type != "process.started")
            .count();
        assert_eq!(
            exits, 1,
            "{HARNESS}: {instance_id} holds exactly one terminal process event"
        );
    }
}
