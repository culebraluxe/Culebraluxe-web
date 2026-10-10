//! WF.COMMAND — an uncertain external effect parks on the node's explicit hold edge and
//! retains its original operation identity through a settlement-only retry.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use test_harness::{EngineHarness, TestClock};
use workflow::{
    ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TransitionDefinition,
    Value, WorkflowSubject,
};

const KEY: &str = "TST-WF-COMMAND-010";
const START: &str = "start";
const EMIT: &str = "publish";
const HOLD: &str = "hold";
const DONE: &str = "done";

fn edge(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.into(),
        to: to.into(),
        condition: None,
        required: None,
    }
}

fn definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START.into(),
        NodeDefinition {
            id: START.into(),
            node_type: "start".into(),
            transitions: Some(vec![edge("begin", EMIT)]),
            ..Default::default()
        },
    );
    nodes.insert(
        EMIT.into(),
        NodeDefinition {
            id: EMIT.into(),
            node_type: "command".into(),
            command_type: Some("forge.publish_candidate".into()),
            transition: Some("success".into()),
            transitions: Some(vec![edge("success", DONE), edge("hold", HOLD)]),
            ..Default::default()
        },
    );
    nodes.insert(
        HOLD.into(),
        NodeDefinition {
            id: HOLD.into(),
            node_type: "task".into(),
            name: Some("Release settlement hold".into()),
            transitions: Some(vec![edge("settle", EMIT)]),
            ..Default::default()
        },
    );
    nodes.insert(
        DONE.into(),
        NodeDefinition {
            id: DONE.into(),
            node_type: "end".into(),
            outcome: Some(ProcessOutcome::Completed),
            ..Default::default()
        },
    );
    ProcessDefinition {
        id: format!("{KEY}-definition"),
        tenant_id: None,
        key: KEY.into(),
        version: 1,
        name: KEY.into(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START.into(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

#[derive(Clone)]
struct ScriptedPort {
    outcomes: Arc<Mutex<VecDeque<ApplicationCommandOutcome>>>,
    requests: Arc<Mutex<Vec<ApplicationCommandRequest>>>,
    facts: Value,
}

impl ApplicationPort for ScriptedPort {
    fn execute_command(&self, request: &ApplicationCommandRequest) -> ApplicationCommandResult {
        self.requests.lock().unwrap().push(request.clone());
        let outcome = self.outcomes.lock().unwrap().pop_front().unwrap();
        ApplicationCommandResult {
            command_id: request.command_id.clone(),
            outcome,
            message: Some("release evidence needs settlement".into()),
        }
    }

    fn read_facts(&self, _: &WorkflowSubject) -> Value {
        self.facts.clone()
    }
}

#[test]
fn settlement_required_parks_then_resumes_with_original_operation_cause() {
    let port = ScriptedPort {
        outcomes: Arc::new(Mutex::new(VecDeque::from([
            ApplicationCommandOutcome::SettlementRequired,
            ApplicationCommandOutcome::Success,
        ]))),
        requests: Arc::new(Mutex::new(Vec::new())),
        facts: Value::object(),
    };
    let requests = port.requests.clone();
    let harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_800_000_000_000),
        Box::new(port),
    );
    harness.engine().seed_definition(definition()).unwrap();
    let started = harness
        .engine()
        .start_process(StartProcessParams {
            definition_key: KEY.into(),
            version: Some(1),
            business_key: None,
            variables: Value::object(),
            started_by: "test".into(),
            tenant_id: None,
            subject: Some(WorkflowSubject {
                subject_type: "story".into(),
                subject_id: "TST-RELEASE-010".into(),
            }),
        })
        .unwrap();
    let instance_id = started.process_instance_id;

    let parked = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .unwrap();
    assert_eq!(parked.status, ProcessStatus::Active);
    assert_eq!(parked.outcome, None);
    let hold_token = harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(&instance_id))
        .unwrap()
        .into_iter()
        .find(|token| token.node_id == HOLD)
        .expect("uncertain release must take its explicit hold transition");
    assert_eq!(hold_token.status, workflow::TokenStatus::Active);

    let first_request = requests.lock().unwrap()[0].clone();
    let first_command = harness
        .store()
        .with_tx(|tx| tx.history(&instance_id, 128))
        .unwrap()
        .into_iter()
        .find(|event| event.event_type == "command.settlement_required")
        .expect("the uncertainty is durably recorded");
    assert_eq!(
        first_command
            .data
            .get("operationCommandId")
            .and_then(Value::as_str),
        Some(first_request.command_id.as_str())
    );

    let hold_task: Task = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&instance_id))
        .unwrap()
        .into_iter()
        .find(|task| task.node_id.as_deref() == Some(HOLD))
        .expect("the hold transition creates an actionable task");
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: hold_task.id,
            user_id: "operator".into(),
            form_data: Value::object(),
            transition_name: Some("settle".into()),
        })
        .unwrap();

    let calls = requests.lock().unwrap().clone();
    assert_eq!(calls.len(), 2);
    assert_ne!(calls[0].command_id, calls[1].command_id);
    assert_eq!(
        calls[1].causation_id.as_deref(),
        Some(calls[0].command_id.as_str()),
        "the recovery command has a fresh workflow identity but retains the original external-operation identity"
    );

    let history = harness
        .store()
        .with_tx(|tx| tx.history(&instance_id, 128))
        .unwrap();
    assert!(history
        .iter()
        .any(|event| event.event_type == "command.settlement_resolved"));
    assert!(history
        .iter()
        .any(|event| event.event_type == "command.completed"));
    assert!(!history
        .iter()
        .any(|event| event.event_type == "command.failed"));
    let finished = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .unwrap();
    assert_eq!(finished.status, ProcessStatus::Completed);
    let tokens = harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(&instance_id))
        .unwrap();
    assert!(tokens.iter().any(|token| token.node_id == DONE));
}

fn run_production_publish_route(
    start_node: &str,
    expected_result_node: &str,
    retry_outcome: ApplicationCommandOutcome,
) {
    let mut facts = Value::object();
    facts.insert("resumeTarget", "PUBLISH".into());
    facts.insert("publishSucceeded", true.into());
    facts.insert("migrationRequired", false.into());
    facts.insert("derivedRefreshRequired", false.into());
    facts.insert("deploymentRequired", false.into());
    let port = ScriptedPort {
        outcomes: Arc::new(Mutex::new(VecDeque::from([
            ApplicationCommandOutcome::SettlementRequired,
            retry_outcome,
        ]))),
        requests: Arc::new(Mutex::new(Vec::new())),
        facts,
    };
    let harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_800_000_000_000),
        Box::new(port),
    );
    let mut definition = forge::engine::definition::forge_sdlc_definition();
    definition.definition.start_node_id = start_node.into();
    harness.engine().seed_definition(definition).unwrap();
    let mut variables = Value::object();
    variables.insert("resumeTarget", "PUBLISH".into());
    let started = harness
        .engine()
        .start_process(StartProcessParams {
            definition_key: forge::engine::topology::FORGE_SDLC_KEY.into(),
            version: Some(forge::engine::topology::FORGE_SDLC_VERSION),
            business_key: None,
            variables,
            started_by: "test".into(),
            tenant_id: None,
            subject: Some(WorkflowSubject {
                subject_type: "story".into(),
                subject_id: format!("TST-{start_node}"),
            }),
        })
        .unwrap();
    let instance_id = started.process_instance_id;
    let hold_task: Task = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&instance_id))
        .unwrap()
        .into_iter()
        .find(|task| task.node_id.as_deref() == Some("hold"))
        .expect("the production publish node must park on the production hold task");

    if retry_outcome == ApplicationCommandOutcome::Success {
        harness
            .engine()
            .complete_task(CompleteTaskParams {
                task_id: hold_task.id,
                user_id: "operator".into(),
                form_data: Value::object(),
                transition_name: Some("resolve".into()),
            })
            .unwrap();
        let history = harness
            .store()
            .with_tx(|tx| tx.history(&instance_id, 256))
            .unwrap();
        assert!(
            history
                .iter()
                .any(|event| event.node_id.as_deref() == Some(expected_result_node)),
            "recovery must follow the original command node's success route"
        );
        let finished = harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance_id))
            .unwrap();
        assert_eq!(
            finished.status,
            ProcessStatus::Completed,
            "successful settlement reaches complete"
        );
    } else {
        harness
            .engine()
            .complete_task(CompleteTaskParams {
                task_id: hold_task.id,
                user_id: "operator".into(),
                form_data: Value::object(),
                transition_name: Some("resolve".into()),
            })
            .unwrap();
        let history = harness
            .store()
            .with_tx(|tx| tx.history(&instance_id, 256))
            .unwrap();
        assert!(
            !history
                .iter()
                .any(|event| event.event_type == "command.completed"),
            "an unsettled retry must not emit command.completed"
        );
        let finished = harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance_id))
            .unwrap();
        assert_eq!(
            finished.status,
            ProcessStatus::Active,
            "failed settlement remains held and recoverable"
        );
        assert!(harness
            .store()
            .with_tx(|tx| tx.tokens_for_instance(&instance_id))
            .unwrap()
            .iter()
            .any(|token| token.node_id == "hold" && token.status == workflow::TokenStatus::Active));
    }
}

#[test]
fn settlement_recovery_preserves_production_fast_and_normal_publish_routes() {
    run_production_publish_route(
        "fast_publish",
        "fast_publish_result",
        ApplicationCommandOutcome::Success,
    );
    run_production_publish_route(
        "publish_candidate",
        "publish_result",
        ApplicationCommandOutcome::Success,
    );
}

#[test]
fn failed_settlement_does_not_follow_either_production_publish_success_route() {
    run_production_publish_route(
        "fast_publish",
        "fast_publish_result",
        ApplicationCommandOutcome::SettlementRequired,
    );
    run_production_publish_route(
        "publish_candidate",
        "publish_result",
        ApplicationCommandOutcome::SettlementRequired,
    );
}
