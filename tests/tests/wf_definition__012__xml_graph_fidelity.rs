//! WF.DEFINITION — XML to graph fidelity (TST-WF-DEFINITION-012).
//!
//! Contract: the production XML parser (`parse_process_definition_xml`,
//! `forge/src/engine/xml.rs:352-426`) is a faithful reader — the graph it returns contains **exactly** what the
//! XML declares, field for field, and nothing else. Every node kind the definition language offers is pinned:
//!
//! - `start-state` → `start`, carrying its transitions (`xml.rs:276`);
//! - `task-node` → `task`, with `label` as the name, `description`, `responsibility` as the single
//!   `candidate_groups` entry, `form-key`, numeric `priority`, and transitions whose `condition` and `required`
//!   flag survive (`xml.rs:287-296`, `collect_transitions` at `xml.rs:243-258` — note `required` stays `None`
//!   when absent, so the engine's required-default is a runtime rule, not a parse rewrite);
//! - `decision` → `decision`, with `refresh-facts` and its `<on condition transition/>` arms (`xml.rs:302-317`);
//! - `fork` → `fork`, carrying every branch transition (`xml.rs:318`);
//! - `command-node` → `command`, with `command-type` and its `transition` target (`xml.rs:297-301`);
//! - `join` → `join`, carrying its onward transition (`xml.rs:319`);
//! - `end-state` → `end`, with the declared `outcome` (`xml.rs:278-286`);
//! - `dynamic-fork` → `dynamic-fork`, with `count-variable`, `branch-command-type`, `join`, `branch-node`,
//!   `plan-variable`, `minimum` and `maximum` (`xml.rs:328-337`);
//! - `display-order` → the graph's declared order (`xml.rs:368-377`).
//!
//! Fidelity is shown in both directions: the honest fixture parses to exactly ten nodes with every field above,
//! and a structurally dishonest fixture (an edge to a missing node, an unknown element, a duplicate id, a root
//! that is not `<process-definition>`) is **refused**, never parsed into a graph that drops or invents structure.
//! The validator (`validate_definition_xml`, `forge/src/engine/validate.rs`) agrees the honest fixture is valid,
//! so the two production seams cannot disagree about what the XML means.
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider: the
//! inputs are literal XML strings and the outputs are the production parser's and validator's own.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__012__xml_graph_fidelity

use forge::engine::validate::validate_definition_xml;
use forge::engine::xml::{definition_from_xml, parse_process_definition_xml};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
/// The definition identity the honest fixture declares for itself.
const KEY: &str = "TST-WF-DEFINITION-012";
const VERSION: i32 = 3;

/// A complete, honest definition exercising every node kind: `start -> triage -> route -+-> fan -> worker -+
/// -> joined -> done`, with `route -> slow -> failed` and `fan -> publish -> done` alongside, plus a detached
/// `dynamic-fork` whose control fields must all survive the parse.
const HONEST_DEFINITION: &str = r#"<process-definition key="TST-WF-DEFINITION-012" version="3" name="Fidelity" description="Graph fidelity fixture">
  <display-order>
    <node ref="start"/>
    <node ref="triage"/>
    <node ref="route"/>
    <node ref="fan"/>
    <node ref="slow"/>
    <node ref="worker"/>
    <node ref="publish"/>
    <node ref="joined"/>
    <node ref="splitter"/>
    <node ref="done"/>
    <node ref="failed"/>
  </display-order>
  <start-state id="start" label="Start">
    <transition name="begin" to="triage"/>
  </start-state>
  <task-node id="triage" label="Triage" description="Triage the work." responsibility="lead" form-key="forge.triage" priority="10">
    <transition name="route" to="route" condition="ready == true" required="true"/>
  </task-node>
  <decision id="route" refresh-facts="true">
    <on condition="kind == 'fast'" transition="take_fast"/>
    <on condition="kind == 'slow'" transition="take_slow"/>
    <transition name="take_fast" to="fan"/>
    <transition name="take_slow" to="slow"/>
  </decision>
  <fork id="fan">
    <transition name="a" to="worker"/>
    <transition name="b" to="publish" required="false"/>
  </fork>
  <task-node id="slow" label="Slow lane">
    <transition name="give_up" to="failed"/>
  </task-node>
  <task-node id="worker" label="Worker">
    <transition name="done" to="joined"/>
  </task-node>
  <command-node id="publish" command-type="forge.publish_candidate" transition="done"/>
  <join id="joined">
    <transition name="onward" to="done"/>
  </join>
  <dynamic-fork id="splitter" count-variable="n" branch-command-type="forge.noop" join="joined" branch-node="worker" plan-variable="plan" minimum="1" maximum="4"/>
  <end-state id="done" label="Done" outcome="completed"/>
  <end-state id="failed" label="Failed" outcome="failed"/>
</process-definition>"#;

/// Parse a definition that must be refused and return the production error message. A definition that parses is a
/// failure of the clause itself, not of the parser.
fn refusal_message(xml: &str) -> String {
    match definition_from_xml(xml) {
        Ok(def) => panic!(
            "{HARNESS}: expected the parser to refuse a dishonest definition, but it parsed key '{}'",
            def.key
        ),
        Err(err) => err.to_string(),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-012); the file and the assay use it.
fn wf_definition_012__xml_graph_fidelity() {
    // 1. IDENTITY AND INVENTORY — the graph is exactly the declared node set, no more and no fewer. A parser that
    //    dropped a node kind or invented a phantom node would fail the count before any field is even read.
    let parsed =
        parse_process_definition_xml(HONEST_DEFINITION).expect("the honest definition must parse");
    assert_eq!(parsed.key, KEY, "{HARNESS}: the XML declares its own key");
    assert_eq!(
        parsed.version, VERSION,
        "{HARNESS}: the XML declares its own version"
    );
    assert_eq!(
        parsed.name, "Fidelity",
        "{HARNESS}: the XML declares its own name"
    );
    assert_eq!(
        parsed.description.as_deref(),
        Some("Graph fidelity fixture"),
        "{HARNESS}: the XML declares its own description"
    );
    assert_eq!(
        parsed.graph.start_node_id, "start",
        "{HARNESS}: the XML declares its own start node"
    );
    assert_eq!(
        parsed.graph.nodes.len(),
        11,
        "{HARNESS}: the graph holds exactly the eleven declared nodes"
    );
    let expected_order: Vec<String> = [
        "start",
        "triage",
        "route",
        "fan",
        "slow",
        "worker",
        "publish",
        "joined",
        "splitter",
        "done",
        "failed",
    ]
    .iter()
    .map(|id| id.to_string())
    .collect();
    assert_eq!(
        parsed.graph.display_order,
        Some(expected_order),
        "{HARNESS}: display-order survives as the declared sequence"
    );
    let node = |id: &str| {
        parsed
            .graph
            .nodes
            .get(id)
            .unwrap_or_else(|| panic!("the graph declares node '{id}'"))
    };

    // 2. START AND TASK FIDELITY — the start edge and every task field land where the engine will read them.
    let start = node("start");
    assert_eq!(start.node_type, "start", "{HARNESS}: <start-state> reads as start");
    assert_eq!(
        start.transitions.as_deref().map(|ts| ts
            .iter()
            .map(|t| (t.name.as_str(), t.to.as_str()))
            .collect::<Vec<_>>()),
        Some(vec![("begin", "triage")]),
        "{HARNESS}: the start edge keeps its name and target"
    );
    let triage = node("triage");
    assert_eq!(triage.node_type, "task", "{HARNESS}: <task-node> reads as task");
    assert_eq!(
        triage.name.as_deref(),
        Some("Triage"),
        "{HARNESS}: the label reads as the node name"
    );
    assert_eq!(
        triage.description.as_deref(),
        Some("Triage the work."),
        "{HARNESS}: the description survives the parse"
    );
    assert_eq!(
        triage.candidate_groups.as_deref(),
        Some(["lead".to_string()].as_slice()),
        "{HARNESS}: responsibility reads as the candidate roster"
    );
    assert_eq!(
        triage.form_key.as_deref(),
        Some("forge.triage"),
        "{HARNESS}: form-key survives the parse"
    );
    assert_eq!(
        triage.priority,
        Some(10),
        "{HARNESS}: the numeric priority survives the parse"
    );
    let route_edge = triage
        .transitions
        .as_deref()
        .and_then(|ts| ts.iter().find(|t| t.name == "route"))
        .expect("the triage edge is declared");
    assert_eq!(route_edge.to, "route", "{HARNESS}: the triage edge targets route");
    assert_eq!(
        route_edge.condition.as_deref(),
        Some("ready == true"),
        "{HARNESS}: the edge condition survives the parse"
    );
    assert_eq!(
        route_edge.required,
        Some(true),
        "{HARNESS}: the edge required flag survives the parse"
    );

    // 3. DECISION FIDELITY — the arms and their transitions arrive as declared, with refresh-facts intact.
    let route = node("route");
    assert_eq!(
        route.node_type, "decision",
        "{HARNESS}: <decision> reads as decision"
    );
    assert_eq!(
        route.refresh_facts,
        Some(true),
        "{HARNESS}: refresh-facts survives the parse"
    );
    let arms = route.decisions.as_deref().expect("the decision arms are declared");
    assert_eq!(arms.len(), 2, "{HARNESS}: both decision arms survive");
    assert_eq!(
        (arms[0].condition.as_str(), arms[0].transition.as_str()),
        ("kind == 'fast'", "take_fast"),
        "{HARNESS}: the first arm keeps its condition and transition"
    );
    assert_eq!(
        (arms[1].condition.as_str(), arms[1].transition.as_str()),
        ("kind == 'slow'", "take_slow"),
        "{HARNESS}: the second arm keeps its condition and transition"
    );
    let take_fast = route
        .transitions
        .as_deref()
        .and_then(|ts| ts.iter().find(|t| t.name == "take_fast"))
        .expect("the take_fast edge is declared");
    assert_eq!(take_fast.to, "fan", "{HARNESS}: take_fast targets the fork");

    // 4. FORK FIDELITY — every branch transition arrives, and an absent required flag stays absent. The parser
    //    must not rewrite "absent" to "true": the required default is the engine's runtime rule, and a parser
    //    that defaulted early would erase the distinction the definition wrote.
    let fan = node("fan");
    assert_eq!(fan.node_type, "fork", "{HARNESS}: <fork> reads as fork");
    let branches = fan.transitions.as_deref().expect("the fork branches are declared");
    assert_eq!(branches.len(), 2, "{HARNESS}: both fork branches survive");
    assert_eq!(
        (branches[0].name.as_str(), branches[0].to.as_str(), branches[0].required),
        ("a", "worker", None),
        "{HARNESS}: a branch without a required flag keeps required=None"
    );
    assert_eq!(
        (branches[1].name.as_str(), branches[1].to.as_str(), branches[1].required),
        ("b", "publish", Some(false)),
        "{HARNESS}: a branch with required=false keeps required=Some(false)"
    );

    // 5. COMMAND, JOIN, DYNAMIC-FORK AND END FIDELITY — the remaining control fields arrive as declared.
    let publish = node("publish");
    assert_eq!(
        publish.node_type, "command",
        "{HARNESS}: <command-node> reads as command"
    );
    assert_eq!(
        publish.command_type.as_deref(),
        Some("forge.publish_candidate"),
        "{HARNESS}: command-type survives the parse"
    );
    assert_eq!(
        publish.transition.as_deref(),
        Some("done"),
        "{HARNESS}: the command transition target survives the parse"
    );
    let joined = node("joined");
    assert_eq!(joined.node_type, "join", "{HARNESS}: <join> reads as join");
    assert_eq!(
        joined
            .transitions
            .as_deref()
            .map(|ts| ts.iter().map(|t| (t.name.as_str(), t.to.as_str())).collect::<Vec<_>>()),
        Some(vec![("onward", "done")]),
        "{HARNESS}: the join's onward edge keeps its name and target"
    );
    let splitter = node("splitter");
    assert_eq!(
        splitter.node_type, "dynamic-fork",
        "{HARNESS}: <dynamic-fork> reads as dynamic-fork"
    );
    assert_eq!(
        splitter.count_variable.as_deref(),
        Some("n"),
        "{HARNESS}: count-variable survives the parse"
    );
    assert_eq!(
        splitter.branch_command_type.as_deref(),
        Some("forge.noop"),
        "{HARNESS}: branch-command-type survives the parse"
    );
    assert_eq!(
        splitter.join.as_deref(),
        Some("joined"),
        "{HARNESS}: the join reference survives the parse"
    );
    assert_eq!(
        splitter.branch_node.as_deref(),
        Some("worker"),
        "{HARNESS}: branch-node survives the parse"
    );
    assert_eq!(
        splitter.plan_variable.as_deref(),
        Some("plan"),
        "{HARNESS}: plan-variable survives the parse"
    );
    assert_eq!(
        (splitter.minimum, splitter.maximum),
        (Some(1), Some(4)),
        "{HARNESS}: minimum and maximum survive the parse"
    );
    assert_eq!(
        node("done").outcome,
        Some(workflow::ProcessOutcome::Completed),
        "{HARNESS}: outcome=completed reads as Completed"
    );
    assert_eq!(
        node("failed").outcome,
        Some(workflow::ProcessOutcome::Failed),
        "{HARNESS}: outcome=failed reads as Failed"
    );
    assert_eq!(
        node("start").outcome, None,
        "{HARNESS}: a node that declares no outcome carries none"
    );

    // 6. THE DEFINITION LOADS AND VALIDATES — `definition_from_xml` forms the keyed identity and the validator
    //    agrees the honest fixture is valid, so the two production seams cannot disagree about what the XML means.
    let def = definition_from_xml(HONEST_DEFINITION).expect("the honest definition loads");
    assert_eq!(
        def.id,
        format!("{KEY}-v{VERSION}"),
        "{HARNESS}: the definition id is derived as key-v<version>"
    );
    let report = validate_definition_xml(HONEST_DEFINITION);
    assert!(
        report.valid,
        "{HARNESS}: the honest definition validates: {:?}",
        report.errors
    );

    // 7. NEGATIVE — a structurally dishonest fixture is REFUSED, never parsed into a graph that drops structure.
    //    Each edit is anchored (asserted to change the source) so no clause can pass on a no-op.
    let dangling = HONEST_DEFINITION.replace("to=\"triage\"", "to=\"ghost\"");
    assert_ne!(
        dangling, HONEST_DEFINITION,
        "{HARNESS}: the dangling-target edit must change the source"
    );
    assert!(
        refusal_message(&dangling).contains("targets missing 'ghost'"),
        "{HARNESS}: an edge to a missing node is refused"
    );
    let unknown = HONEST_DEFINITION.replacen("  <fork", "  <mystery id=\"m0\"/>\n  <fork", 1);
    assert_ne!(
        unknown, HONEST_DEFINITION,
        "{HARNESS}: the unknown-element edit must change the source"
    );
    assert!(
        refusal_message(&unknown).contains("unsupported <mystery>"),
        "{HARNESS}: an unknown element is refused"
    );
    let duplicated = HONEST_DEFINITION.replacen("<task-node id=\"slow\"", "<task-node id=\"worker\"", 1);
    assert_ne!(
        duplicated, HONEST_DEFINITION,
        "{HARNESS}: the duplicate-id edit must change the source"
    );
    assert!(
        refusal_message(&duplicated).contains("duplicate node worker"),
        "{HARNESS}: a duplicate node id is refused"
    );
    let wrong_root = HONEST_DEFINITION
        .replacen("<process-definition", "<definitions", 1)
        .replacen("</process-definition>", "</definitions>", 1);
    assert_ne!(
        wrong_root, HONEST_DEFINITION,
        "{HARNESS}: the wrong-root edit must change the source"
    );
    assert!(
        refusal_message(&wrong_root).contains("root is <definitions>"),
        "{HARNESS}: a root that is not <process-definition> is refused"
    );

    // 8. NON-VACUITY — the assertions discriminate: the same source parses identically twice, while a one-field
    //    edit (the triage priority) parses to a graph that differs in exactly that field.
    let again = parse_process_definition_xml(HONEST_DEFINITION).expect("the XML parses again");
    assert_eq!(
        again.graph.nodes.len(),
        parsed.graph.nodes.len(),
        "{HARNESS}: parsing twice yields the same inventory"
    );
    assert_eq!(
        again.graph.nodes["triage"].priority,
        Some(10),
        "{HARNESS}: parsing twice yields the same fields"
    );
    let reprioritised = HONEST_DEFINITION.replacen("priority=\"10\"", "priority=\"11\"", 1);
    let changed =
        parse_process_definition_xml(&reprioritised).expect("a reprioritised fixture still parses");
    assert_eq!(
        changed.graph.nodes["triage"].priority,
        Some(11),
        "{HARNESS}: a changed priority field reads changed — the assertions above are not constant"
    );
    assert_eq!(
        changed.graph.nodes.len(),
        parsed.graph.nodes.len(),
        "{HARNESS}: the one-field edit changes the field, not the inventory"
    );
}
