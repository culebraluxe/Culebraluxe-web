//! WF.DEFINITION — unsupported node type (TST-WF-DEFINITION-009).
//!
//! Contract: the definition vocabulary is closed. `parse_node` (`forge/src/engine/xml.rs:275-339`) matches the
//! element name against exactly ten kinds and ends with
//!
//! ```text
//! other => return Err(XmlError(format!("unsupported <{other}>"))),
//! ```
//!
//! so an element the engine has no handler for is refused by name at parse time. This matters more than a typical
//! "unknown tag" rejection: `execute_node_leave` dispatches on `node_type` with a catch-all `_ =>` arm
//! (`middle/workflow/src/engine/execute_node_leave.rs:68-83`), so a node kind that reached the graph would be driven
//! as an ordinary state node — following its first transition — instead of being run as the thing it was written
//! as. Refusing at the parse is what stops a `<fork-node>` from silently degrading into a plain state.
//!
//! The clause is proved as a **closed set with an exact match**, not as "most things work":
//!
//! - all ten declared kinds are accepted and map to their node types, so the vocabulary is exactly what the parser
//!   says it is;
//! - every other name is refused and named. The distinguishing negatives are the near misses — `<fork-node>`,
//!   `<join-node>`, `<decision-node>`, `<task>`, `<end>`, `<transition>` and the capitalised `<STATE>` are ALL
//!   refused, because the match is exact and case-sensitive. An implementation that lower-cased, stemmed or
//!   prefix-matched any of these would accept a definition that means something else.
//!
//! The one degree of freedom is **whitespace after the element name**, and it is pinned as such rather than
//! asserted as a refusal: the parser skips it, so `<state>` and `<state  >` are the same element. Case and suffixes
//! are not structure; spacing after the name is not structure either. Whitespace *before* the name is different
//! again — that is a syntax fault, not an unrecognised node kind — and all three outcomes are pinned, because a test
//! that asserted any of them wrongly would be asserting a rule production does not have.
//!
//! `<display-order>` is the one element that is not a node kind and is not refused: the parser handles it before
//!   `parse_node` (`forge/src/engine/xml.rs:368-375`). That exception is pinned so it cannot be mistaken for a hole
//! in the rule, and so a future "reject anything unknown" sweep does not break every definition that declares one.
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__009__unsupported_node_type

use forge::engine::validate::validate_definition_xml;
use forge::engine::xml::{definition_from_xml, parse_process_definition_xml, FORGE_SDLC_V6_XML};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const KEY: &str = "TST-WF-DEFINITION-009";

/// The ten element names `parse_node` accepts, with the node type each one must produce. The set is the vocabulary.
const VOCABULARY: &[(&str, &str)] = &[
    ("start-state", "start"),
    ("state", "state"),
    ("end-state", "end"),
    ("task-node", "task"),
    ("command-node", "command"),
    ("decision", "decision"),
    ("fork", "fork"),
    ("join", "join"),
    ("timer", "timer"),
    ("dynamic-fork", "dynamic-fork"),
];

/// A legal one-node element of the given kind, carrying whatever that kind requires so the ONLY possible refusal is
/// the kind itself.
fn element(kind: &str, id: &str) -> String {
    match kind {
        "start-state" => format!("  <start-state id=\"{id}\"><transition name=\"go\" to=\"done\"/></start-state>"),
        "state" => format!("  <state id=\"{id}\"><transition name=\"go\" to=\"done\"/></state>"),
        "end-state" => format!("  <end-state id=\"{id}\"/>"),
        "task-node" => format!("  <task-node id=\"{id}\" responsibility=\"smith\"/>"),
        "command-node" => {
            format!("  <command-node id=\"{id}\" command-type=\"forge.migrate_dev\"/>")
        }
        "decision" => {
            format!("  <decision id=\"{id}\"><transition name=\"go\" to=\"done\"/></decision>")
        }
        "fork" => format!("  <fork id=\"{id}\"><transition name=\"go\" to=\"done\"/></fork>"),
        "join" => format!("  <join id=\"{id}\"><transition name=\"go\" to=\"done\"/></join>"),
        "timer" => format!("  <timer id=\"{id}\" on-fire=\"done\"/>"),
        "dynamic-fork" => format!(
            "  <dynamic-fork id=\"{id}\" count-variable=\"n\" branch-command-type=\"forge.launch_builder\" join=\"{id}\"/>"
        ),
        // Anything else is emitted verbatim, so an unsupported element is well-formed XML that the parser must still
        // reject on its NAME alone.
        other => format!("  <{other} id=\"{id}\"/>"),
    }
}

/// `<start-state id="start"/> <end-state id="done"/>` plus the given elements, so a clause varies one element and
/// leaves the rest of the definition legal.
fn definition_with(extra: &str) -> String {
    format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Vocabulary\">\n{extra}\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
    )
}

/// Parse a definition that must be REFUSED and return the production message.
fn refusal_message(xml: &str) -> String {
    match parse_process_definition_xml(xml) {
        Ok(_) => panic!("{HARNESS}: expected an unsupported-node refusal — source:\n{xml}"),
        Err(err) => err.to_string(),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-009); the file and the assay use it.
fn wf_definition_009__unsupported_node_type() {
    // 1. THE VOCABULARY IS EXACTLY TEN KINDS, EACH PRODUCING ITS NODE TYPE. This is the positive control: without it,
    //    a parser that refused every element would satisfy every refusal clause below.
    for (kind, expected) in VOCABULARY {
        let parsed = parse_process_definition_xml(&definition_with(&element(kind, "probe")))
            .unwrap_or_else(|e| {
                panic!("{HARNESS}: <{kind}> is in the vocabulary and must parse, got {e}")
            });
        let node = parsed
            .graph
            .nodes
            .get("probe")
            .unwrap_or_else(|| panic!("{HARNESS}: <{kind}> must reach the node map"));
        assert_eq!(
            node.node_type, *expected,
            "{HARNESS}: <{kind}> must read as node_type '{expected}'"
        );
    }
    assert_eq!(
        VOCABULARY.len(),
        10,
        "{HARNESS}: the declared vocabulary is ten kinds — an eleventh would be a new match arm in production"
    );

    // 2. ANY OTHER ELEMENT IS REFUSED AND NAMED. `unsupported <NAME>` is the production message, so an operator can
    //    repair the definition from the error alone.
    for name in [
        "mystery",
        "task",
        "end",
        "fork-node",
        "join-node",
        "decision-node",
        "task_node",
        "start_node",
        "end_node",
        "command",
        "timer-node",
        "dynamicfork",
        "parallel-gateway",
        "subprocess",
        "call-activity",
        "user-task",
    ] {
        let xml = definition_with(&format!("  <{name} id=\"probe\"/>"));
        assert_ne!(
            xml,
            definition_with(""),
            "{HARNESS}: the unsupported element must actually be in the source: <{name}>"
        );
        assert_eq!(
            refusal_message(&xml),
            format!("unsupported <{name}>"),
            "{HARNESS}: <{name}> is not in the vocabulary and is refused by name"
        );
    }

    // 3. THE MATCH IS EXACT AND CASE-SENSITIVE. Every spelling below is the same idea written differently; a parser
    //    that lower-cased or stripped a `-node` suffix would accept one of these and then drive it as a state node.
    for name in [
        "STATE",
        "State",
        "sTATE",
        "START-STATE",
        "Start-State",
        "FORK",
        "JOIN",
        "TASK-NODE",
        "End-State",
        "Dynamic-Fork",
        "states",
        "state-",
        "state_",
    ] {
        let message = refusal_message(&definition_with(&format!("  <{name} id=\"probe\"/>")));
        assert_eq!(
            message,
            format!("unsupported <{name}>"),
            "{HARNESS}: <{name}> must not match any vocabulary kind — the match is exact"
        );
    }

    // 3b. WHITESPACE **AFTER** THE ELEMENT NAME IS NOT STRUCTURE, AND THAT IS THE ONE DEGREE OF FREEDOM. The parser
    //     skips whitespace before the name's terminator (`forge/src/engine/xml.rs`), so `<state>` and `<state  >` are
    //     the SAME element and both parse. This is pinned separately from step 3 precisely because it is the opposite
    //     result: whitespace after the name is tolerated, case and suffixes are not. A test asserting `<state >` was
    //     refused would be asserting a rule production does not have.
    for spelling in [
        "<state id=\"probe\"/>",
        "<state  id=\"probe\" />",
        "<state\n  id=\"probe\"\n/>",
    ] {
        let xml = definition_with(&format!("  {spelling}"));
        let parsed = parse_process_definition_xml(&xml).unwrap_or_else(|e| {
            panic!("{HARNESS}: {spelling} is the same element as <state> and must parse, got {e}")
        });
        assert_eq!(
            parsed.graph.nodes["probe"].node_type, "state",
            "{HARNESS}: {spelling} reads as a state node — whitespace after the name is not structure"
        );
    }
    // Whitespace BEFORE the name is a different matter, and is a SYNTAX fault rather than an unsupported element:
    // a name must follow `<` directly. Pinned because the two are easy to conflate, and an operator seeing
    // "expected name" needs to know it is not looking at an unrecognised node kind.
    let leading_space = definition_with("  < state id=\"probe\"/>");
    let message = match parse_process_definition_xml(&leading_space) {
        Ok(_) => panic!("{HARNESS}: whitespace between `<` and the name is not parseable"),
        Err(e) => e.to_string(),
    };
    assert!(
        message.contains("expected name"),
        "{HARNESS}: `< state>` is a syntax fault, not an unsupported node kind: {message}"
    );
    assert_ne!(
        message, "unsupported <state>",
        "{HARNESS}: a syntax fault must not be reported as an unsupported element"
    );

    // 4. AN UNSUPPORTED ELEMENT WITH CHILDREN IS STILL REFUSED — and it is refused on its NAME, before anything about
    //    its body matters. A parser that only inspected leaf elements would let a `<fork-node>` carrying a real
    //    `<transition>` through, which is precisely the dangerous shape.
    let with_children = definition_with(
        "  <fork-node id=\"probe\"><transition name=\"go\" to=\"done\"/></fork-node>",
    );
    assert_eq!(
        refusal_message(&with_children),
        "unsupported <fork-node>",
        "{HARNESS}: an unsupported element is refused on its name, children or not"
    );
    // 4b. THE CLOSED VOCABULARY IS ABOUT TOP-LEVEL DECLARATIONS, NOT ABOUT NESTED CHILDREN — and this is the honest
    //     boundary of the rule, stated rather than papered over. `parse_node` walks a node's children looking only for
    //     `<transition>` (`forge/src/engine/xml.rs:244-259`) and, for a decision, `<on>` (`:306-315`); every other
    //     child is ignored. So a junk element INSIDE a task node parses and is dropped, while the same element at the
    //     top level is refused. Pinned here because a reader of this file must not conclude the vocabulary is enforced
    //     at every depth — it is not, and the consequence is that a misspelt child directive is silently inert.
    let nested = definition_with(
        "  <task-node id=\"probe\" responsibility=\"smith\"><mystery-child id=\"x\"/></task-node>",
    );
    let parsed = parse_process_definition_xml(&nested).expect(
        "{HARNESS}: an unrecognised CHILD is ignored, not refused — parse_node only reads <transition> and <on>",
    );
    let task = &parsed.graph.nodes["probe"];
    assert_eq!(
        task.node_type, "task",
        "{HARNESS}: the task itself parsed correctly"
    );
    assert!(
        task.transitions.is_none(),
        "{HARNESS}: the ignored child contributed no transition to the node"
    );
    assert!(
        !task
            .description
            .as_deref()
            .unwrap_or("")
            .contains("mystery"),
        "{HARNESS}: and the ignored child leaked no attribute into any field"
    );
    // The control: the SAME element name at the top level IS refused, so the difference is depth and not the name.
    assert_eq!(
        refusal_message(&definition_with("  <mystery-child id=\"x\"/>")),
        "unsupported <mystery-child>",
        "{HARNESS}: the same element at the top level is refused — the vocabulary guards declarations, not children"
    );

    // 5. POSITION DOES NOT MATTER — the refusal is the same whether the bad element is declared before the start
    //    node, after it, or between two legal nodes. A parser that only inspected the first child would pass the
    //    first case and fail the others.
    let before = format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Before\">\n  <mystery-node id=\"x\"/>\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
    );
    let after = format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"After\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n  <mystery-node id=\"x\"/>\n</process-definition>"
    );
    let between =
        definition_with("  <state id=\"a\"/>\n  <mystery-node id=\"x\"/>\n  <state id=\"b\"/>");
    for (label, xml) in [
        ("before the start node", &before),
        ("after the end node", &after),
        ("between nodes", &between),
    ] {
        assert_eq!(
            refusal_message(xml),
            "unsupported <mystery-node>",
            "{HARNESS}: an unsupported element {label} is refused identically"
        );
    }

    // 6. `<display-order>` IS THE ONE NON-KIND ELEMENT, AND IT IS NOT A HOLE IN THE RULE. The parser handles it
    //    before `parse_node` (`forge/src/engine/xml.rs:368-375`), so it neither reaches the vocabulary match nor is
    //    refused. Pinned explicitly, because a "reject every unknown element" sweep would otherwise break every
    //    definition that declares one — and because a reader must not conclude the rule has an unstated exception.
    let with_display_order = parse_process_definition_xml(
        "<process-definition key=\"TST-WF-DEFINITION-009\" version=\"1\" name=\"Display order\">\n  <display-order>\n    <node ref=\"start\"/>\n    <node ref=\"done\"/>\n  </display-order>\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>",
    )
    .expect("<display-order> is handled before the node-kind match, so it is not refused");
    assert_eq!(
        with_display_order.graph.display_order.as_deref(),
        Some(["start".to_string(), "done".to_string()].as_slice()),
        "{HARNESS}: <display-order> is read as an ordering, not as a node"
    );
    assert_eq!(
        with_display_order.graph.nodes.len(),
        2,
        "{HARNESS}: <display-order> contributes no node to the map"
    );
    // Its own `<node>` child is likewise not a kind — a `<node>` element outside a display-order IS refused, so the
    // exception is scoped to the one parent, not to the element name.
    assert_eq!(
        refusal_message(&definition_with("  <node id=\"probe\"/>")),
        "unsupported <node>",
        "{HARNESS}: a <node> element outside <display-order> is an unsupported kind"
    );
    // A display-order that declares no `<node>` refs is still handled by name alone.
    let empty_display = parse_process_definition_xml(
        "<process-definition key=\"TST-WF-DEFINITION-009\" version=\"1\" name=\"Empty display\">\n  <display-order/>\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>",
    )
    .expect("an empty display-order parses — it declares an empty order, not an unknown element");
    assert_eq!(
        empty_display.graph.display_order, None,
        "{HARNESS}: a display-order with no <node> refs declares no order at all"
    );

    // 7. THE OTHER SEAMS AGREE. `definition_from_xml` (what deploy calls) and `validate_definition_xml` (what the
    //    validator reports) consume the same parser, so an unsupported element is refused by both.
    let unsupported = definition_with("  <fork-node id=\"probe\"/>");
    assert_eq!(
        definition_from_xml(&unsupported)
            .expect_err("{HARNESS}: refused")
            .to_string(),
        "unsupported <fork-node>",
        "{HARNESS}: definition_from_xml reports the same refusal"
    );
    let report = validate_definition_xml(&unsupported);
    assert!(
        !report.valid,
        "{HARNESS}: an unsupported kind does not validate"
    );
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("unsupported <fork-node>")),
        "{HARNESS}: the validator carries the refusal: {:?}",
        report.errors
    );
    // It is an XML fault by the validator's keyword rule — the message names none of `missing`/`expected`/`unknown`.
    assert!(
        report
            .xml_errors
            .iter()
            .any(|e| e.contains("unsupported <fork-node>")),
        "{HARNESS}: 'unsupported' is classified as an xml fault: {:?}",
        report
    );
    assert!(
        report.grammar_errors.is_empty(),
        "{HARNESS}: and not as a grammar fault: {:?}",
        report.grammar_errors
    );

    // 8. THE SHIPPED DEFINITION USES ONLY VOCABULARY KINDS, AND ITS OWN ELEMENTS ARE THE ANCHOR PROVING THE RULE
    //    APPLIES TO IT. Injecting an unsupported element into the deployed file makes the shipped definition itself
    //    be refused, so the rule is not satisfied by a parser that only checks small fixtures.
    let shipped = definition_from_xml(FORGE_SDLC_V6_XML).expect("the shipped definition parses");
    let mut kinds: Vec<&str> = shipped
        .definition
        .nodes
        .values()
        .map(|n| n.node_type.as_str())
        .collect();
    let kind_count = kinds.len();
    kinds.sort_unstable();
    kinds.dedup();
    for kind in &kinds {
        let declared = VOCABULARY.iter().any(|(_, node_type)| node_type == kind);
        assert!(
            declared,
            "{HARNESS}: the shipped definition's node type '{kind}' comes from the declared vocabulary"
        );
    }
    assert!(
        kind_count >= 8,
        "{HARNESS}: the shipped definition really does exercise many kinds ({kind_count} nodes)"
    );
    // Both tags change together, so the edit is well-formed XML naming a kind that does not exist — the fault under
    // test, not a malformed-tag error.
    let shipped_polluted = FORGE_SDLC_V6_XML
        .replacen("  <start-state", "  <start-state-node", 1)
        .replacen("</start-state>", "</start-state-node>", 1);
    assert!(
        shipped_polluted != FORGE_SDLC_V6_XML,
        "{HARNESS}: the shipped definition must contain the anchor this clause edits"
    );
    assert_eq!(
        refusal_message(&shipped_polluted),
        "unsupported <start-state-node>",
        "{HARNESS}: an unsupported element injected into the shipped definition is refused"
    );

    // 9. NON-VACUITY. The same parser accepts the vocabulary and refuses everything else, on the same fixture shape —
    //    so neither answer can be a constant.
    assert!(
        parse_process_definition_xml(&definition_with("")).is_ok(),
        "{HARNESS}: a definition using only vocabulary kinds is accepted"
    );
    assert!(
        parse_process_definition_xml(&definition_with("  <not-a-kind id=\"probe\"/>")).is_err(),
        "{HARNESS}: the same fixture plus one unsupported element is refused"
    );
}
