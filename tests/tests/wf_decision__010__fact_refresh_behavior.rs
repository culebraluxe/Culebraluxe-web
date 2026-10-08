//! WF.DECISION — fact refresh behavior (TST-WF-DECISION-010).
//!
//! Contract: a `decision` node re-reads the world before it routes, unless the definition says not to. That is the
//! whole of `execute_node_leave`'s decision arm (`middle/workflow/src/engine/execute_node_leave.rs:45-62`):
//!
//! ```text
//! "decision" => {
//!     let vars = if node.refresh_facts != Some(false) && self.app.is_some() {
//!         self.refresh_facts(tx, instance, variables)?
//!     } else {
//!         variables.clone()
//!     };
//!     let chosen = self.evaluate_decision(node, &vars, preferred)...
//! ```
//!
//! Three facts about that one line are the contract, and each is easy to get wrong:
//!
//! - **the default is to refresh.** `!= Some(false)` means an absent `refresh-facts` REFRESHES. A definition that
//!   says nothing gets fresh facts, which is the safe direction: a decision never routes on a stale world.
//! - **`refresh-facts="false"` is the only opt-out**, and it is a decision, not an optimisation — it is how a
//!   definition pins a routing decision to the variables the run was started with.
//! - **the refresh is a MERGE, not a replacement.** `refresh_facts` (`handle_join.rs:307-327`) clones the current
//!   variables, merges the facts over them, and **persists the merged map** with
//!   `update_instance_variables` — so facts become part of the instance's own state and outlive the decision.
//!
//! The refresh needs both an `ApplicationPort` and a subject: `read_facts` is asked about the instance's
//!   `subject_type`/`subject_id`, and with either missing the facts are simply the variables unchanged
//!   (`handle_join.rs:313-318`). Those are two independent ways to get no refresh, and both are pinned.
//!
//! The negative/fault case is the opt-out: a decision whose arm can only match a FRESH value, with
//! `refresh-facts="false"`, must **refuse** rather than route on the stale value — the same refusal
//! `wf_decision__004__missing_null` pins for a missing name. If the opt-out were dropped, that decision would take
//! its other arm and this test would fail on the route rather than on the refusal.
//!
//! The parse side is pinned too: `refresh-facts` is read as `Some(true)` / `Some(false)` / `None`
//! (`forge/src/engine/xml.rs:304`), and only the literal `"true"` is true — so `refresh-facts="yes"` and
//! `refresh-facts="1"` silently mean **false**, which is the opposite of what an author writing them expects.
//!
//! Level L0 Pure, harness `WorkflowHarness`. The real `WorkflowEngine<MemoryStore>` is driven through the
//! production decision boundary, with a deterministic fake substituted at the production `ApplicationPort` seam —
//! no database, no network, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__010__fact_refresh_behavior

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use test_harness::{EngineHarness, TestClock};
use workflow::value::obj;
use workflow::{
    ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, DecisionArm, DefinitionStatus, NodeDefinition, ProcessDefinition,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, StartProcessResult,
    TransitionDefinition, Value, WorkflowSubject,
};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const DECIDE_NODE: &str = "decide";
const STARTED_BY: &str = "tst";
const DEFINITION_VERSION: i32 = 1;
/// The fact the fake application reports for a subject, and the variable the decision routes on.
const FACT: &str = "ready";

/// The external-facing `ApplicationPort`, faked at the production adapter seam.
///
/// It performs no I/O and names no provider. It records every `read_facts` subject it is asked about, so a test can
/// assert whether the refresh happened — the engine's only observable trace of it — rather than inferring it from
/// the route alone. `execute_command` succeeds and is never reached: these graphs contain no `command` node.
#[derive(Clone, Default)]
struct RecordingFactsPort {
    subjects: Arc<Mutex<Vec<WorkflowSubject>>>,
    facts: Value,
}

impl RecordingFactsPort {
    fn new(facts: Value) -> Self {
        Self {
            subjects: Arc::new(Mutex::new(Vec::new())),
            facts,
        }
    }

    /// The subjects the engine asked about, in order.
    fn subjects(&self) -> Vec<WorkflowSubject> {
        self.subjects
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl ApplicationPort for RecordingFactsPort {
    fn execute_command(&self, request: &ApplicationCommandRequest) -> ApplicationCommandResult {
        ApplicationCommandResult {
            command_id: request.command_id.clone(),
            outcome: ApplicationCommandOutcome::Success,
            message: None,
        }
    }

    fn read_facts(&self, subject: &WorkflowSubject) -> Value {
        self.subjects
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(subject.clone());
        self.facts.clone()
    }
}

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.into(),
        to: to.into(),
        condition: None,
        required: None,
    }
}

/// `start -> decide -> end_when_ready | end_when_not`. `refresh_facts` is the node's own `refresh-facts` setting,
/// `otherwise` declares the unguarded edge — without it a fact no arm answers is refused.
fn definition_for(key: &str, refresh_facts: Option<bool>, otherwise: bool) -> ProcessDefinition {
    let mut transitions = vec![transition("when_ready", "end_when_ready")];
    if otherwise {
        transitions.push(transition("otherwise", "end_when_not"));
    }
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".into(),
            node_type: "start".into(),
            transitions: Some(vec![transition("begin", DECIDE_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        DECIDE_NODE.to_string(),
        NodeDefinition {
            id: DECIDE_NODE.into(),
            node_type: "decision".into(),
            refresh_facts,
            decisions: Some(vec![DecisionArm {
                condition: format!("{FACT} == true"),
                transition: "when_ready".into(),
            }]),
            transitions: Some(transitions),
            ..Default::default()
        },
    );
    for (id, outcome) in [
        ("end_when_ready", ProcessOutcome::Completed),
        ("end_when_not", ProcessOutcome::Completed),
    ] {
        nodes.insert(
            id.to_string(),
            NodeDefinition {
                id: id.into(),
                node_type: "end".into(),
                name: Some(id.into()),
                outcome: Some(outcome),
                ..Default::default()
            },
        );
    }
    ProcessDefinition {
        id: format!("{key}-def"),
        tenant_id: None,
        key: key.into(),
        version: DEFINITION_VERSION,
        name: key.into(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".into(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

/// An engine wired to a facts port reporting `{ FACT: true }`.
fn seeded(
    key: &str,
    refresh_facts: Option<bool>,
    otherwise: bool,
) -> (EngineHarness, RecordingFactsPort) {
    let port = RecordingFactsPort::new(obj([(FACT, Value::Bool(true))]));
    let harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_001_000_000),
        Box::new(port.clone()),
    );
    harness
        .engine()
        .seed_definition(definition_for(key, refresh_facts, otherwise))
        .expect("definition registers");
    (harness, port)
}

/// Start the process with `ready` initially FALSE, and no subject, so the only way `ready` can become true is a
/// refresh reading the application. The stale starting value is what makes each clause's route meaningful.
fn start_stale(
    harness: &EngineHarness,
    key: &str,
    subject: Option<WorkflowSubject>,
) -> workflow::Result<StartProcessResult> {
    harness.engine().start_process(StartProcessParams {
        definition_key: key.into(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: obj([(FACT, Value::Bool(false))]),
        started_by: STARTED_BY.into(),
        tenant_id: None,
        subject,
    })
}

fn resting_nodes(harness: &EngineHarness, instance_id: &str) -> Vec<String> {
    harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(instance_id))
        .expect("tokens readable")
        .into_iter()
        .map(|t| t.node_id)
        .collect()
}

fn instance_variables(harness: &EngineHarness, instance_id: &str) -> Value {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance_id))
        .expect("instance readable")
        .variables
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DECISION-010); the file and the assay use it.
fn wf_decision_010__fact_refresh_behavior() {
    let subject = Some(WorkflowSubject {
        subject_type: "story".into(),
        subject_id: "STORY-1".into(),
    });

    // 1. THE DEFAULT IS TO REFRESH. `refresh_facts` is None — the attribute was never declared — and the decision
    //    still re-reads the application, so a fact that was FALSE at start becomes TRUE before routing. If the
    //    predicate were `== Some(true)` this would route on the stale value and land on the wrong end-state.
    const KEY_DEFAULT: &str = "TST-WF-DECISION-010-DEFAULT";
    let (default_on, default_port) = seeded(KEY_DEFAULT, None, true);
    let started = start_stale(&default_on, KEY_DEFAULT, subject.clone())
        .expect("a refreshing decision routes on the fresh fact");
    assert_eq!(
        default_port.subjects().len(),
        1,
        "{HARNESS}: an undeclared refresh-facts STILL refreshes — the default is to refresh"
    );
    let asked = default_port.subjects()[0].clone();
    assert_eq!(
        (asked.subject_type.as_str(), asked.subject_id.as_str()),
        ("story", "STORY-1"),
        "{HARNESS}: the refresh asks about the instance's own subject: {asked:?}"
    );
    let instance = default_on
        .store()
        .with_tx(|tx| tx.get_instance(&started.process_instance_id))
        .expect("readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: the fresh fact drove the process to completion"
    );
    let passed = resting_nodes(&default_on, &instance.id);
    assert!(
        passed.iter().any(|n| n == "end_when_ready"),
        "{HARNESS}: the refreshed fact selected end_when_ready, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|n| n == "end_when_not"),
        "{HARNESS}: and NOT the stale-fact branch, got {passed:?}"
    );

    // 1b. `refresh-facts="true"` BEHAVES IDENTICALLY — the explicit spelling of the same default.
    const KEY_EXPLICIT: &str = "TST-WF-DECISION-010-EXPLICIT";
    let (explicit_on, explicit_port) = seeded(KEY_EXPLICIT, Some(true), true);
    let started = start_stale(&explicit_on, KEY_EXPLICIT, subject.clone())
        .expect("an explicitly-refreshing decision routes on the fresh fact");
    assert_eq!(
        explicit_port.subjects().len(),
        1,
        "{HARNESS}: refresh-facts=true reads the application"
    );
    let instance = explicit_on
        .store()
        .with_tx(|tx| tx.get_instance(&started.process_instance_id))
        .expect("readable");
    assert!(
        resting_nodes(&explicit_on, &instance.id)
            .iter()
            .any(|n| n == "end_when_ready"),
        "{HARNESS}: refresh-facts=true selects the fresh-fact branch"
    );

    // 2. THE OPT-OUT. `refresh-facts="false"` is the only way to decline, and it must mean the decision sees the
    //    variables the run started with — false — so it takes the other branch.
    const KEY_OPTOUT: &str = "TST-WF-DECISION-010-OPTOUT";
    let (optout, optout_port) = seeded(KEY_OPTOUT, Some(false), true);
    let started = start_stale(&optout, KEY_OPTOUT, subject.clone())
        .expect("a non-refreshing decision routes on the stale variable");
    assert_eq!(
        optout_port.subjects().len(),
        0,
        "{HARNESS}: refresh-facts=false must NOT read the application at all"
    );
    let instance = optout
        .store()
        .with_tx(|tx| tx.get_instance(&started.process_instance_id))
        .expect("readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: the stale fact still routes — to the OTHER branch"
    );
    let passed = resting_nodes(&optout, &instance.id);
    assert!(
        passed.iter().any(|n| n == "end_when_not"),
        "{HARNESS}: without a refresh the stale FALSE selects end_when_not, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|n| n == "end_when_ready"),
        "{HARNESS}: and NOT the fresh-fact branch, got {passed:?}"
    );

    // 3. THE NEGATIVE/FAULT CASE — A DECISION THAT CAN ONLY BE ANSWERED BY FRESH FACTS REFUSES WHEN THE REFRESH IS
    //    DECLINED. There is no otherwise, so a stale `ready: false` matches no arm. The engine must REFUSE rather
    //    than invent a route; if the opt-out were ignored, or the refusal replaced by a fallback, this would complete
    //    on `end_when_ready` instead of erroring.
    const KEY_STARVED: &str = "TST-WF-DECISION-010-STARVED";
    let (starved, starved_port) = seeded(KEY_STARVED, Some(false), false);
    let refused = start_stale(&starved, KEY_STARVED, subject.clone()).expect_err(
        "{HARNESS}: a stale-only decision with no otherwise must be refused, not routed",
    );
    let message = refused.to_string();
    assert!(
        message.contains("No valid transition from decision node"),
        "{HARNESS}: the refusal is the production one: {message}"
    );
    assert!(
        message.contains(DECIDE_NODE),
        "{HARNESS}: the refusal names the decision node: {message}"
    );
    assert_eq!(
        starved_port.subjects().len(),
        0,
        "{HARNESS}: and the refusal came with no application read — the opt-out held"
    );
    // The control on the SAME graph shape with the refresh ENABLED: now the fresh fact answers the arm and the
    // process completes. So the refusal above is caused by the declined refresh, not by a broken graph.
    const KEY_STARVED_REFRESHING: &str = "TST-WF-DECISION-010-STARVED-REFRESHING";
    let (starved_ok, starved_ok_port) = seeded(KEY_STARVED_REFRESHING, Some(true), false);
    let started = start_stale(&starved_ok, KEY_STARVED_REFRESHING, subject.clone())
        .expect("the same graph answers once the refresh is enabled");
    assert_eq!(
        starved_ok_port.subjects().len(),
        1,
        "{HARNESS}: the refreshing variant does read the application"
    );
    let instance = starved_ok
        .store()
        .with_tx(|tx| tx.get_instance(&started.process_instance_id))
        .expect("readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: and it completes on the fresh fact"
    );

    // 4. THE REFRESH IS A MERGE, NOT A REPLACEMENT, AND THE MERGED MAP IS PERSISTED. The instance starts carrying
    //    variables the facts say nothing about; after the decision they must all still be there, with the fact added.
    const KEY_MERGE: &str = "TST-WF-DECISION-010-MERGE";
    let merge_port = RecordingFactsPort::new(obj([
        (FACT, Value::Bool(true)),
        ("addedByFacts", Value::from("yes")),
    ]));
    let merge = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_001_000_000),
        Box::new(merge_port.clone()),
    );
    merge
        .engine()
        .seed_definition(definition_for(KEY_MERGE, Some(true), true))
        .expect("definition registers");
    let started = merge
        .engine()
        .start_process(StartProcessParams {
            definition_key: KEY_MERGE.into(),
            version: Some(DEFINITION_VERSION),
            business_key: None,
            // `kept` is present before the decision and absent from the facts; the fact's own name collides with
            // `FACT` so the merge has something to overwrite.
            variables: obj([
                (FACT, Value::Bool(false)),
                ("kept", Value::from("original")),
            ]),
            started_by: STARTED_BY.into(),
            tenant_id: None,
            subject: subject.clone(),
        })
        .expect("the merging decision routes");
    let persisted = instance_variables(&merge, &started.process_instance_id);
    assert_eq!(
        persisted.get("kept"),
        Some(&Value::from("original")),
        "{HARNESS}: `kept` is untouched — the refresh merged over the variables, it did not replace them"
    );
    assert_eq!(
        persisted.get("addedByFacts"),
        Some(&Value::from("yes")),
        "{HARNESS}: the fact's own key was added: {persisted:?}"
    );
    assert_eq!(
        persisted.get(FACT),
        Some(&Value::Bool(true)),
        "{HARNESS}: the fact overwrote the stale value: {persisted:?}"
    );

    // 5. NO SUBJECT, NO REFRESH. `refresh_facts` returns the variables unchanged when the instance has no
    //    subject_type/subject_id (`handle_join.rs:316-318`) — the second independent route to "no refresh", and one
    //    that a test which only set `refresh-facts` would miss.
    const KEY_NO_SUBJECT: &str = "TST-WF-DECISION-010-NO-SUBJECT";
    let (no_subject, no_subject_port) = seeded(KEY_NO_SUBJECT, Some(true), true);
    let started = start_stale(&no_subject, KEY_NO_SUBJECT, None)
        .expect("a subject-less instance routes on its own variables");
    assert_eq!(
        no_subject_port.subjects().len(),
        0,
        "{HARNESS}: with no subject the engine never asks the application"
    );
    let instance = no_subject
        .store()
        .with_tx(|tx| tx.get_instance(&started.process_instance_id))
        .expect("readable");
    let passed = resting_nodes(&no_subject, &instance.id);
    assert!(
        passed.iter().any(|n| n == "end_when_not"),
        "{HARNESS}: the stale value stands, so the other branch is taken, got {passed:?}"
    );

    // 6. NO APPLICATION PORT, NO REFRESH. `self.app.is_some()` is the other half of the guard
    //    (`execute_node_leave.rs:46`), so an engine with no port cannot refresh even when the node asks it to.
    const KEY_NO_PORT: &str = "TST-WF-DECISION-010-NO-PORT";
    let no_port = EngineHarness::new(TestClock::at_unix_millis(1_700_001_000_000));
    no_port
        .engine()
        .seed_definition(definition_for(KEY_NO_PORT, Some(true), true))
        .expect("definition registers");
    let started = no_port
        .engine()
        .start_process(StartProcessParams {
            definition_key: KEY_NO_PORT.into(),
            version: Some(DEFINITION_VERSION),
            business_key: None,
            variables: obj([(FACT, Value::Bool(false))]),
            started_by: STARTED_BY.into(),
            tenant_id: None,
            subject: subject.clone(),
        })
        .expect("an engine with no port routes on the variables it has");
    let instance = no_port
        .store()
        .with_tx(|tx| tx.get_instance(&started.process_instance_id))
        .expect("readable");
    let passed = resting_nodes(&no_port, &instance.id);
    assert!(
        passed.iter().any(|n| n == "end_when_not"),
        "{HARNESS}: with no ApplicationPort a refreshing decision sees the stale value, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|n| n == "end_when_ready"),
        "{HARNESS}: and cannot have read the fresh one, got {passed:?}"
    );

    // 7. THE PARSE SIDE: `refresh-facts` IS THREE-VALUED, AND ONLY THE LITERAL "true" IS TRUE. This is the trap worth
    //     pinning — `refresh-facts="yes"` and `refresh-facts="1"` both read as `Some(false)`, silently DECLINING a
    //     refresh the author plainly asked for.
    for (declared, expected) in [
        ("true", Some(true)),
        ("false", Some(false)),
        ("yes", Some(false)),
        ("1", Some(false)),
        ("TRUE", Some(false)),
        ("", Some(false)),
    ] {
        let xml = format!(
            "<process-definition key=\"TST-WF-DECISION-010\" version=\"1\" name=\"Refresh\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"decide\"/>\n  </start-state>\n  <decision id=\"decide\" refresh-facts=\"{declared}\">\n    <transition name=\"go\" to=\"done\"/>\n  </decision>\n  <end-state id=\"done\"/>\n</process-definition>"
        );
        let parsed = forge::engine::xml::parse_process_definition_xml(&xml).unwrap_or_else(|e| {
            panic!("{HARNESS}: refresh-facts=\"{declared}\" must parse, got {e}")
        });
        assert_eq!(
            parsed.graph.nodes["decide"].refresh_facts,
            expected,
            "{HARNESS}: refresh-facts=\"{declared}\" reads as {expected:?} — only the literal \"true\" is true"
        );
    }
    // And the absent attribute is None, which the engine treats as refresh — the step-1 case restated at the parse.
    let absent = forge::engine::xml::parse_process_definition_xml(
        "<process-definition key=\"TST-WF-DECISION-010\" version=\"1\" name=\"No refresh attr\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"decide\"/>\n  </start-state>\n  <decision id=\"decide\">\n    <transition name=\"go\" to=\"done\"/>\n  </decision>\n  <end-state id=\"done\"/>\n</process-definition>",
    )
    .expect("a decision with no refresh-facts attribute parses");
    assert_eq!(
        absent.graph.nodes["decide"].refresh_facts, None,
        "{HARNESS}: an undeclared refresh-facts is None, and None means REFRESH"
    );

    // 8. NON-VACUITY. The engine refreshes and does not refresh on the same graph, differing only in the node's one
    //    flag; and the port's own record is what distinguishes them, so neither answer can be an artefact of a
    //    constant predicate or a constant route.
    // The two graphs are identical apart from one `Option<bool>` on one node, and they differ on BOTH observable
    // axes: whether the application was asked, and where the process ended. A constant predicate fails the first; a
    // constant route fails the second.
    assert_eq!(
        default_port.subjects().len(),
        1,
        "{HARNESS}: the refreshing graph asked the application once"
    );
    assert_eq!(
        optout_port.subjects().len(),
        0,
        "{HARNESS}: the identical graph with refresh-facts=false asked it never"
    );
    let optout_started = start_stale(&optout, KEY_OPTOUT, subject.clone())
        .expect("a second instance of the opt-out graph routes on its stale variable");
    let optout_passed = resting_nodes(
        &optout,
        &optout
            .store()
            .with_tx(|tx| tx.get_instance(&optout_started.process_instance_id))
            .expect("readable")
            .id,
    );
    assert!(
        optout_passed.iter().any(|n| n == "end_when_not")
            && !optout_passed.iter().any(|n| n == "end_when_ready"),
        "{HARNESS}: and it routed differently from the refreshing graph: {optout_passed:?}"
    );
    assert_eq!(
        optout_port.subjects().len(),
        0,
        "{HARNESS}: a second instance still asks the application never — the opt-out is not per-run luck"
    );
}
