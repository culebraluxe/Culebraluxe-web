//! WF.DEFINITION — missing target (TST-WF-DEFINITION-005).
//!
//! Contract: the Forge v6 XML **is** the definition (`forge/src/engine/xml.rs:1-2`,
//! `forge/definitions/FORGE_SDLC-v6.xml`), so the production definition parser refuses a definition whose
//! `<transition>` names a node that the same definition does not declare. "Missing target" is the parser's exact
//! refusal, not a fuzzy lookup: `parse_process_definition_xml` (`forge/src/engine/xml.rs:387-396`) builds the
//! node map first and then, for every edge it collected, demands `nodes.contains_key(&t.to)`. A definition that would
//! ship an edge no runtime could ever follow is never accepted — `deploy_xml`
//! (`forge/src/engine/deploy.rs:83-92`) and the engine binary (`forge/src/bin/forge_task.rs:45`) both call
//! this one parser, so the refusal is the same one production deploys through.
//!
//! The production pieces this file exercises, all pure:
//!
//! - `definition_from_xml` / `parse_process_definition_xml` (`forge/src/engine/xml.rs:352-426`) — the one
//!   parser production uses; its `XmlError` message names the edge, its declaring node and the missing target.
//! - `validate_definition_xml` (`forge/src/engine/validate.rs:16-75`) — the second production boundary that
//!   consumes the parser and reports the definition `valid == false`, so the two seams may not disagree.
//! - `FORGE_SDLC_V6_XML` (`forge/src/engine/xml.rs:428`) — the shipped definition itself, which must carry no
//!   missing target, and whose own `begin` edge is the edit anchor that proves the rule applies to it.
//!
//! The shape matters. The contract is demonstrated in both directions:
//!
//! - an honest definition — every target declared, including a legal self-reference — parses; and
//! - a definition with a missing target is **refused**, on every node kind that can carry a `<transition>`
//!   (`start-state`, `state`, `task-node`, `fork`, `join`, `end-state`), because the check is a property of the edge
//!   and not of one node type, and over **every declared node** — including one unreachable from the start.
//!
//! The distinguishing negatives are what stop the test passing vacuously: the "exists" set is the **node map**, so a
//! target declared only in `<display-order>` is still missing; the lookup is **exact**, so a case variant, a padded
//! or whitespace-damaged id, and a prefix of a real id are all missing; and removing the declared node refuses the
//! edge that still names it while the same edge is accepted when the node is present.
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider: the
//! inputs are literal XML strings and the output is the production parser's and validator's own.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__005__missing_target

use forge::engine::validate::validate_definition_xml;
use forge::engine::xml::{definition_from_xml, parse_process_definition_xml, FORGE_SDLC_V6_XML};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
/// The node id no fixture declares — the missing target every refusal names.
const MISSING_TARGET: &str = "ghost";
/// The definition's identity, as the XML declares it.
const KEY: &str = "TST-WF-DEFINITION-005";
const VERSION: i32 = 1;

/// A complete, honest definition: `start -> work -> done`, and every `<transition>` target is a declared node. It is
/// the positive control — the edits below are refused only because they make an edge point at a node that is absent.
const VALID_DEFINITION: &str = r#"<process-definition key="TST-WF-DEFINITION-005" version="1" name="Missing target">
  <start-state id="start" label="Start">
    <transition name="begin" to="work"/>
  </start-state>
  <task-node id="work" label="Work" responsibility="smith">
    <transition name="finish" to="done"/>
  </task-node>
  <end-state id="done" label="Done"/>
</process-definition>"#;

/// The same contract on one node of every kind that can carry a `<transition>`: `start-state`, `state`, `task-node`,
/// `fork`, `join`, `end-state`. Parses as written; each of the five edges is edited to a missing target below.
// The key says what it is: gitleaks reads a taxonomy-shaped key as a generic API key.
const KINDS_DEFINITION: &str = r#"<process-definition key="definition-kinds-under-test-not-a-secret" version="1" name="Kinds">
  <start-state id="start">
    <transition name="begin" to="check"/>
  </start-state>
  <state id="check">
    <transition name="via-state" to="fan"/>
  </state>
  <fork id="fan">
    <transition name="fork-1" to="joined"/>
  </fork>
  <join id="joined">
    <transition name="join-1" to="work"/>
  </join>
  <task-node id="work" responsibility="qa">
    <transition name="finish" to="done"/>
  </task-node>
  <end-state id="done"/>
</process-definition>"#;

/// A definition whose only edge is a self-reference: `start -> loop -> loop`. It parses, proving the refusal is about
/// a target that does not exist, not about transitions, loops or repeated node ids.
// The key says what it is: gitleaks reads a taxonomy-shaped key as a generic API key.
const SELF_LOOP_DEFINITION: &str = r#"<process-definition key="definition-self-loop-under-test-not-a-secret" version="1" name="Self loop">
  <start-state id="start">
    <transition name="begin" to="loop"/>
  </start-state>
  <state id="loop">
    <transition name="again" to="loop"/>
  </state>
</process-definition>"#;

/// Parse a definition that must be refused and return the production error message. A definition that parses is a
/// failure of the clause itself, not of the parser.
fn refusal_message(xml: &str) -> String {
    match definition_from_xml(xml) {
        Ok(def) => panic!(
            "{HARNESS}: expected the parser to refuse a missing target, but it parsed key '{}'",
            def.key
        ),
        Err(err) => err.to_string(),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-005); the file and the assay use it.
fn wf_definition_005__missing_target() {
    // 1. POSITIVE CONTROL — an honest definition parses and every edge resolves inside it. Without this, a refusal
    //    below could be the parser rejecting the fixture rather than the missing target.
    let parsed =
        parse_process_definition_xml(VALID_DEFINITION).expect("the honest definition must parse");
    assert_eq!(parsed.key, KEY, "{HARNESS}: the XML declares its own key");
    assert_eq!(
        parsed.version, VERSION,
        "{HARNESS}: the XML declares version 1"
    );
    assert_eq!(
        parsed.name, "Missing target",
        "{HARNESS}: the XML declares its own name"
    );
    assert_eq!(
        parsed.graph.start_node_id, "start",
        "{HARNESS}: the XML declares its own start node"
    );
    for id in ["start", "work", "done"] {
        assert!(
            parsed.graph.nodes.contains_key(id),
            "{HARNESS}: the honest definition declares node '{id}'"
        );
    }
    for (id, node) in &parsed.graph.nodes {
        for transition in node.transitions.as_deref().unwrap_or(&[]) {
            assert!(
                parsed.graph.nodes.contains_key(&transition.to),
                "{HARNESS}: the honest definition's edge '{}' on '{id}' resolves to '{}'",
                transition.name,
                transition.to
            );
        }
    }
    assert!(
        definition_from_xml(VALID_DEFINITION).is_ok(),
        "{HARNESS}: the honest definition loads as a definition"
    );

    // 2. THE SUBJECT — an edge whose target names no declared node is REFUSED, and the refusal names the edge, its
    //    declaring node and the missing target, so an operator can repair the definition from the error alone. The
    //    anchor assertion makes the edit self-proving: a no-op `replace` would leave the source honest and the
    //    `refusal_message` call would panic instead of passing.
    let broken = VALID_DEFINITION.replace("to=\"work\"", "to=\"ghost\"");
    assert_ne!(
        broken, VALID_DEFINITION,
        "{HARNESS}: the missing-target edit must actually change the source"
    );
    let message = refusal_message(&broken);
    assert!(
        message.contains("targets missing 'ghost'"),
        "{HARNESS}: the refusal names the missing target: {message}"
    );
    assert!(
        message.contains("on 'start'"),
        "{HARNESS}: the refusal names the node that declared the missing edge: {message}"
    );
    assert!(
        message.contains("'begin'"),
        "{HARNESS}: the refusal names the edge that is missing its target: {message}"
    );
    assert!(
        message.contains(MISSING_TARGET),
        "{HARNESS}: the missing target is the one the source declared: {message}"
    );

    // 2b. The same refusal from a task node's edge — the rule is about the edge, not the start node.
    let broken_task = VALID_DEFINITION.replace("to=\"done\"", "to=\"ghost\"");
    assert_ne!(
        broken_task, VALID_DEFINITION,
        "{HARNESS}: the task-node edit must actually change the source"
    );
    let task_message = refusal_message(&broken_task);
    assert!(
        task_message.contains("on 'work' targets missing 'ghost'"),
        "{HARNESS}: a task-node edge to a missing node is refused and names its node: {task_message}"
    );

    // 2c. Every node kind that can carry a `<transition>` is checked. The parser collects transitions before it
    //     switches on the element name, so the guarantee is universal; each of the five edges is broken in turn and
    //     each refusal must name the node that declared it.
    let kinds = parse_process_definition_xml(KINDS_DEFINITION)
        .expect("the multi-kind definition must parse");
    assert_eq!(
        kinds.graph.nodes.len(),
        6,
        "{HARNESS}: the multi-kind definition declares six nodes"
    );
    for (declaring, target) in [
        ("start", "check"),
        ("check", "fan"),
        ("fan", "joined"),
        ("joined", "work"),
        ("work", "done"),
    ] {
        let broken = KINDS_DEFINITION.replace(&format!("to=\"{target}\""), "to=\"ghost\"");
        assert_ne!(
            broken, KINDS_DEFINITION,
            "{HARNESS}: the '{declaring}' target '{target}' must be a unique anchor"
        );
        let message = refusal_message(&broken);
        assert!(
            message.contains(&format!("on '{declaring}' targets missing 'ghost'")),
            "{HARNESS}: a missing target is refused on node '{declaring}': {message}"
        );
    }

    // 2d. THE CHECK IS OVER EVERY DECLARED NODE, NOT ONLY THOSE REACHABLE FROM START. Production walks the whole node
    //     map (`forge/src/engine/xml.rs:387-396`), so an edge on an orphan node — one no path from the start ever
    //     reaches — is still refused. A parser that validated only reachable nodes would ship a definition with a
    //     dangling edge hidden behind an unreachable node, and this clause fails.
    // The key says what it is: gitleaks reads a taxonomy-shaped key as a generic API key.
    let orphan = r#"<process-definition key="definition-orphan-under-test-not-a-secret" version="1" name="Orphan">
  <start-state id="start">
    <transition name="begin" to="done"/>
  </start-state>
  <end-state id="done"/>
  <state id="orphan">
    <transition name="drift" to="ghost"/>
  </state>
</process-definition>"#;
    let orphan_message = refusal_message(orphan);
    assert!(
        orphan_message.contains("on 'orphan' targets missing 'ghost'"),
        "{HARNESS}: a missing target on a node unreachable from start is still refused: {orphan_message}"
    );

    // 2e. AN END-STATE'S OWN EDGE IS CHECKED TOO. `collect_transitions` runs before the element-name switch
    //     (`forge/src/engine/xml.rs:266`), so an `<end-state>` that declares a `<transition>` is subject to the
    //     same rule; a missing target there is refused, not ignored because the node is a terminus.
    let end_edge = r#"<process-definition key="TST-WF-DEFINITION-005-END-EDGE" version="1" name="End edge">
  <start-state id="start">
    <transition name="begin" to="done"/>
  </start-state>
  <end-state id="done">
    <transition name="bounce" to="ghost"/>
  </end-state>
</process-definition>"#;
    let end_edge_message = refusal_message(end_edge);
    assert!(
        end_edge_message.contains("on 'done' targets missing 'ghost'"),
        "{HARNESS}: an end-state's edge to a missing node is refused: {end_edge_message}"
    );

    // 3. BYPASS (exact, not fuzzy) — "missing" means no node carries that id. A target that is a case variant, a
    //    padded or whitespace-damaged id, or a prefix of a real id must ALL be refused. A parser that normalised
    //    case, trimmed the value or prefix-matched would accept one of these and let a broken definition deploy.
    for near in ["WORK", " work", "work ", "wor", "wo rk"] {
        let broken = VALID_DEFINITION.replace("to=\"work\"", &format!("to=\"{near}\""));
        assert_ne!(
            broken, VALID_DEFINITION,
            "{HARNESS}: the near-miss edit 'to=\"{near}\"' must change the source"
        );
        let message = refusal_message(&broken);
        assert!(
            message.contains("targets missing"),
            "{HARNESS}: the near-miss target '{near}' must not resolve to the node 'work': {message}"
        );
    }

    // 3b. BYPASS (the "exists" set is the node map) — a target listed in `<display-order>` is still not a node. The
    //     parser reads display-order refs without declaring nodes, so an edge to a ref that appears only there must
    //     be refused; otherwise a definition could satisfy an edge with an ordering entry and ship a dead edge.
    let display_only = r#"<process-definition key="TST-WF-DEFINITION-005-DISPLAY" version="1" name="Display only">
  <display-order>
    <node ref="start"/>
    <node ref="ghost"/>
  </display-order>
  <start-state id="start">
    <transition name="begin" to="ghost"/>
  </start-state>
</process-definition>"#;
    let display_message = refusal_message(display_only);
    assert!(
        display_message.contains("targets missing 'ghost'"),
        "{HARNESS}: a target named only by display-order is still a missing target: {display_message}"
    );

    // 3c. BYPASS (self-proving pair) — remove the declared node and the edge that named it is refused; the identical
    //     edge passed in step 1 while the node was present, so the difference is exactly the node's existence.
    let task_block = r#"  <task-node id="work" label="Work" responsibility="smith">
    <transition name="finish" to="done"/>
  </task-node>
"#;
    let removed = VALID_DEFINITION.replace(task_block, "");
    assert_ne!(
        removed, VALID_DEFINITION,
        "{HARNESS}: removing the declared node must change the source"
    );
    let removed_message = refusal_message(&removed);
    assert!(
        removed_message.contains("on 'start' targets missing 'work'"),
        "{HARNESS}: removing the target node refuses the edge that still names it: {removed_message}"
    );

    // 4. NON-VACUITY — a transition to an existing node is accepted, including a self-reference. If the parser
    //    refused every edge, step 1 would already have failed; this pins that only a MISSING target is the refusal.
    let self_loop = parse_process_definition_xml(SELF_LOOP_DEFINITION)
        .expect("a self-referencing edge is legal");
    assert!(
        self_loop.graph.nodes.contains_key("loop"),
        "{HARNESS}: the self-loop definition declares its own target"
    );
    assert!(
        definition_from_xml(SELF_LOOP_DEFINITION).is_ok(),
        "{HARNESS}: the parser refuses a missing target, not a transition or a loop"
    );

    // 5. THE SHIPPED DEFINITION OBEYS IT. FORGE_SDLC-v6 is the definition the engine actually deploys, so it must
    //    itself carry no missing target; and its own `begin` edge is the anchor proving the rule applies to it.
    let forge = definition_from_xml(FORGE_SDLC_V6_XML)
        .expect("FORGE_SDLC-v6 is the definition and must parse");
    for (id, node) in &forge.definition.nodes {
        for transition in node.transitions.as_deref().unwrap_or(&[]) {
            assert!(
                forge.definition.nodes.contains_key(&transition.to),
                "{HARNESS}: the shipped definition's edge '{}' on '{id}' must not target a missing node ('{}')",
                transition.name,
                transition.to
            );
        }
    }
    let forged_broken = FORGE_SDLC_V6_XML.replace(
        "<transition name=\"begin\" to=\"classify_work\"/>",
        "<transition name=\"begin\" to=\"no_such_node\"/>",
    );
    assert_ne!(
        forged_broken, FORGE_SDLC_V6_XML,
        "{HARNESS}: the production definition's begin edge must be the anchor"
    );
    let forged_message = refusal_message(&forged_broken);
    assert!(
        forged_message.contains("on 'start' targets missing 'no_such_node'"),
        "{HARNESS}: the shipped definition is refused when one of its edges loses its target: {forged_message}"
    );

    // 6. THE VALIDATION BOUNDARY AGREES. `validate_definition_xml` consumes this same parser, so the honest
    //    definition is valid with no graph errors and the missing-target definition is invalid and its structured
    //    `errors` carry the refusal. Two seams that disagreed would be two sources for one fact.
    let valid_report = validate_definition_xml(VALID_DEFINITION);
    assert!(
        valid_report.valid,
        "{HARNESS}: the honest definition validates: {:?}",
        valid_report.errors
    );
    assert!(
        valid_report.graph_errors.is_empty(),
        "{HARNESS}: the honest definition has no graph errors: {:?}",
        valid_report.graph_errors
    );
    let invalid_report = validate_definition_xml(&broken);
    assert!(
        !invalid_report.valid,
        "{HARNESS}: a definition with a missing target does not validate"
    );
    assert!(
        invalid_report
            .errors
            .iter()
            .any(|entry| entry.contains("targets missing 'ghost'")),
        "{HARNESS}: the validator's structured errors name the missing target: {:?}",
        invalid_report.errors
    );
}
