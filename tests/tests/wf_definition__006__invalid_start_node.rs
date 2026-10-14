//! WF.DEFINITION — invalid start node (TST-WF-DEFINITION-006).
//!
//! Contract: a definition must name where execution begins, and the name it gives is the graph's
//! `start_node_id`. `parse_process_definition_xml` derives that from the XML itself
//! (`forge/src/engine/xml.rs:377-386`): every `<start-state>` element captured sets `start_id`, and the graph is
//! refused if none ever did:
//!
//! ```text
//! let start_node_id = start_id.ok_or_else(|| XmlError("missing start-state".into()))?;
//! ```
//!
//! A definition with no start node is a graph the engine could never enter, so it is refused rather than defaulted.
//! The refusal is a plain `missing start-state`, which carries no name — because there is no name to carry.
//!
//! Three properties of that rule are proved here, and each has bitten someone:
//!
//! - **the start id is the element's own id, not a convention.** The parser does not require the id to be `start`;
//!   a definition whose `<start-state id="begin_here">` starts there. A parser that hard-coded `"start"` would both
//!   break every definition that names it differently and wrongly accept one whose real start is elsewhere.
//! - **the start node is not exempt from the duplicate rule, but two DISTINCT starts are not a duplicate.** Two
//!   `<start-state>` elements sharing an id is refused as a duplicate id; two with different ids parse, and the later
//!   declaration wins (`forge/src/engine/xml.rs:378-380`). That is honest current behaviour, recorded here so it
//!   cannot change silently — it is not a duplicate-id case, and this file says so.
//! - **the order of the checks is part of the contract.** A `<start-state>` with no id is refused as
//!   `missing id`, not `missing start-state`, because `parse_node` requires the id before the node is classified as
//!   a start. A definition with both faults reports the id, which is the fault an operator must fix first.
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__006__invalid_start_node

use forge::engine::validate::validate_definition_xml;
use forge::engine::xml::{definition_from_xml, parse_process_definition_xml, FORGE_SDLC_V6_XML};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const KEY: &str = "TST-WF-DEFINITION-006";

/// A definition whose `<start-state>` carries the given id and whose edges all resolve.
fn definition_starting_at(id: &str) -> String {
    format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Start\">\n  <start-state id=\"{id}\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
    )
}

/// Parse a definition that must be REFUSED and return the production message.
fn refusal_message(xml: &str) -> String {
    match parse_process_definition_xml(xml) {
        Ok(d) => panic!(
            "{HARNESS}: expected a start-node refusal, but it parsed with start '{}' — source:\n{xml}",
            d.graph.start_node_id
        ),
        Err(err) => err.to_string(),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-006); the file and the assay use it.
fn wf_definition_006__invalid_start_node() {
    // 1. POSITIVE CONTROL — a definition with a `<start-state>` parses, and `start_node_id` is a real member of the
    //    map. Without this, a parser that refused everything would satisfy every clause below.
    let honest = parse_process_definition_xml(&definition_starting_at("start"))
        .expect("the honest definition parses");
    assert_eq!(
        honest.graph.start_node_id, "start",
        "{HARNESS}: the start node is the one <start-state> declared"
    );
    assert!(
        honest.graph.nodes.contains_key(&honest.graph.start_node_id),
        "{HARNESS}: the start node is a declared node, not a dangling reference"
    );
    let start_node = &honest.graph.nodes[&honest.graph.start_node_id];
    assert_eq!(
        start_node.node_type, "start",
        "{HARNESS}: the node the graph starts at really is a start node"
    );

    // 2. THE START ID IS THE ELEMENT'S OWN — IT IS NOT A CONVENTION. Any id works, and the graph starts exactly
    //    there. A parser that hard-coded "start" would fail each of these.
    for id in ["begin_here", "s", "Start", "0", "start.node", "st art"] {
        let parsed =
            parse_process_definition_xml(&definition_starting_at(id)).unwrap_or_else(|e| {
                panic!("{HARNESS}: a start-state named '{id}' must parse, got {e}")
            });
        assert_eq!(
            parsed.graph.start_node_id, id,
            "{HARNESS}: the start node is the declared id, verbatim"
        );
        assert_eq!(
            parsed.graph.nodes[id].node_type, "start",
            "{HARNESS}: '{id}' is in the map as a start node"
        );
    }

    // 3. THE SUBJECT — NO `<start-state>` AT ALL IS REFUSED. Three shapes of the same fault: the element is not
    //    there, the element is there under another name, or the only candidate lost its id. Each is a definition the
    //    engine could never enter.
    for (nodes, expected) in [
        (
            "  <state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </state>",
            "missing start-state",
        ),
        (
            "  <task-node id=\"start\" responsibility=\"smith\"/>",
            "missing start-state",
        ),
        ("  <end-state id=\"start\"/>", "missing start-state"),
        // A `<start-state>` with an EMPTY id is a different fault — the id is missing, and `parse_node` refuses it
        // before the element is classified as a start. Pinned separately below.
        ("", "missing start-state"),
    ] {
        let xml = format!(
            "<process-definition key=\"{KEY}\" version=\"1\" name=\"No start\">\n{nodes}\n  <end-state id=\"done\"/>\n</process-definition>"
        );
        let message = refusal_message(&xml);
        assert_eq!(
            message, expected,
            "{HARNESS}: a definition with no usable <start-state> is refused — nodes {nodes:?}"
        );
    }
    // A definition with nothing but an end-state is the minimal instance, and it is refused.
    assert_eq!(
        refusal_message(&format!(
            "<process-definition key=\"{KEY}\" version=\"1\" name=\"Bare\">\n  <end-state id=\"done\"/>\n</process-definition>"
        )),
        "missing start-state",
        "{HARNESS}: a definition with no start-state at all is refused"
    );

    // 3b. NO START NODE ANYWHERE, EVEN WHEN THE GRAPH IS OTHERWISE COMPLETE. Removing the start-state from the
    //     honest fixture leaves valid edges and a valid terminus, so nothing else is wrong — the refusal is the
    //     start node alone.
    let start_removed = VALID_WITHOUT_START;
    let message = refusal_message(&start_removed);
    assert_eq!(
        message, "missing start-state",
        "{HARNESS}: an otherwise-complete definition is refused for the missing start node alone"
    );

    // 4. THE START NODE IS NOT EXEMPT FROM THE DUPLICATE RULE — a second `<start-state>` reusing the start id is a
    //    duplicate-id refusal, not a start-node one. The two rules are adjacent in the parser and must not be
    //    confused: an operator told "missing start-state" when the real fault is a clash would edit the wrong line.
    let duplicate_start = format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Clash\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <start-state id=\"start\" label=\"Second\"/>\n  <end-state id=\"done\"/>\n</process-definition>"
    );
    let message = refusal_message(&duplicate_start);
    assert_eq!(
        message, "duplicate node start",
        "{HARNESS}: a repeated start id is a DUPLICATE fault, not a missing-start fault"
    );
    assert_ne!(
        message, "missing start-state",
        "{HARNESS}: the two faults must not be reported as each other"
    );
    // And an unrelated node may not take the start id either.
    assert_eq!(
        refusal_message(&format!(
            "<process-definition key=\"{KEY}\" version=\"1\" name=\"Shadow\">\n  <state id=\"start\"/>\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
        )),
        "duplicate node start",
        "{HARNESS}: a plain <state> shadowing the start id is a duplicate fault"
    );

    // 5. TWO DISTINCT STARTS ARE NOT A DUPLICATE — the later declaration wins. This is honest current behaviour,
    //    recorded so it cannot change silently; it is the boundary between this file and the duplicate-id rule.
    let two_starts =
        parse_process_definition_xml(&format!("<process-definition key=\"{KEY}\" version=\"1\" name=\"Two starts\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <start-state id=\"begin_here\">\n    <transition name=\"again\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"))
            .expect("two start-states with distinct ids are not a duplicate");
    assert_eq!(
        two_starts.graph.start_node_id, "begin_here",
        "{HARNESS}: with two distinct starts the later declaration is the graph's start"
    );
    assert!(
        two_starts.graph.nodes.contains_key("start")
            && two_starts.graph.nodes.contains_key("begin_here"),
        "{HARNESS}: both start-states are declared nodes; the ids differ so neither is a duplicate"
    );

    // 6. THE CHECK ORDER IS PART OF THE CONTRACT. A `<start-state>` with no id is refused as `missing id`, because
    //    `parse_node` requires the id (`:262`) before the element is classified as a start (`:378`). Both faults are
    //    present in this source, and the id is reported — which is the one an operator must fix first.
    let idless_start = format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Idless\">\n  <start-state>\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
    );
    assert_eq!(
        refusal_message(&idless_start),
        "missing id on <start-state>",
        "{HARNESS}: an idless <start-state> is an ID fault, not a missing-start fault — the checks are ordered"
    );
    // The control: give it an id and the same source's remaining shape is fine, so the id was the whole fault.
    let named_start = parse_process_definition_xml(&idless_start.replacen(
        "<start-state>",
        "<start-state id=\"start\">",
        1,
    ))
    .expect("the same source with an id parses");
    assert_eq!(
        named_start.graph.start_node_id, "start",
        "{HARNESS}: adding only the id makes the start node valid"
    );

    // 7. THE OTHER SEAMS AGREE ON ALL THREE FAULTS — the bare refusal, the duplicate, and the missing id each
    //    surface identically through `definition_from_xml` (what deploy calls) and `validate_definition_xml`.
    for (xml, expected) in [
        (
            format!(
                "<process-definition key=\"{KEY}\" version=\"1\" name=\"Bare\">\n  <end-state id=\"done\"/>\n</process-definition>"
            ),
            "missing start-state",
        ),
        (duplicate_start.clone(), "duplicate node start"),
        (idless_start.clone(), "missing id on <start-state>"),
    ] {
        assert_eq!(
            definition_from_xml(&xml).expect_err("{HARNESS}: refused").to_string(),
            expected,
            "{HARNESS}: definition_from_xml reports the same fault: {expected}"
        );
        let report = validate_definition_xml(&xml);
        assert!(!report.valid, "{HARNESS}: {expected} does not validate");
        assert!(
            report.errors.iter().any(|e| e.contains(expected)),
            "{HARNESS}: the validator carries the fault: {:?}",
            report.errors
        );
    }
    // The bare refusal is a GRAMMAR fault by the validator's keyword rule (the message says "missing"), and the
    // duplicate is an XML fault (it names none of the three keywords). Both layer assignments are pinned so the
    // two faults cannot be silently reclassified into each other.
    let bare_report = validate_definition_xml(&format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Bare\">\n  <end-state id=\"done\"/>\n</process-definition>"
    ));
    assert!(
        bare_report
            .grammar_errors
            .iter()
            .any(|e| e.contains("missing start-state")),
        "{HARNESS}: 'missing start-state' is a grammar fault: {:?}",
        bare_report
    );
    let duplicate_report = validate_definition_xml(&duplicate_start);
    assert!(
        duplicate_report
            .xml_errors
            .iter()
            .any(|e| e.contains("duplicate node start")),
        "{HARNESS}: 'duplicate node start' is an xml fault: {:?}",
        duplicate_report
    );

    // 8. THE SHIPPED DEFINITION HAS A REAL START NODE, AND ITS OWN ID IS THE ANCHOR. The deployed file must satisfy
    //    this rule too, and demoting its `<start-state>` must make the shipped definition itself be refused — so the
    //    rule cannot be satisfied by a parser that ignores start nodes.
    let shipped = definition_from_xml(FORGE_SDLC_V6_XML).expect("the shipped definition parses");
    assert_eq!(
        shipped.definition.nodes[&shipped.definition.start_node_id].node_type, "start",
        "{HARNESS}: the shipped definition's start node is a start node"
    );
    // The shipped start-state block demoted to `<state>`. Both tags change together, so the edit is a syntactically
    // valid definition that simply has no start node — the fault under test, not a malformed-tag error.
    let shipped_demoted = FORGE_SDLC_V6_XML
        .replacen("<start-state", "<state", 1)
        .replacen("</start-state>", "</state>", 1);
    assert!(
        shipped_demoted != FORGE_SDLC_V6_XML,
        "{HARNESS}: the shipped definition declares the anchor this clause edits"
    );
    assert_eq!(
        refusal_message(&shipped_demoted),
        "missing start-state",
        "{HARNESS}: demoting the shipped definition's start-state is refused"
    );

    // 9. NON-VACUITY — THE RULE HAS BOTH ANSWERS, ON ONE SOURCE. Renaming the element restores the definition, so
    //    the refusal above is the start rule and nothing else.
    assert!(
        parse_process_definition_xml(&definition_starting_at("start")).is_ok(),
        "{HARNESS}: a definition WITH a start-state is accepted"
    );
    assert!(
        parse_process_definition_xml(&definition_starting_at("start").replacen(
            "<start-state",
            "<state",
            1
        ))
        .is_err(),
        "{HARNESS}: the same definition WITHOUT a start-state is refused"
    );
}

/// The honest fixture with its `<start-state>` demoted to `<state>` — used by step 3b to show the start rule firing
/// on an otherwise-complete definition.
const VALID_WITHOUT_START: &str = r#"<process-definition key="TST-WF-DEFINITION-006" version="1" name="No start">
  <state id="start">
    <transition name="begin" to="done"/>
  </state>
  <task-node id="work" label="Work" responsibility="smith">
    <transition name="finish" to="done"/>
  </task-node>
  <end-state id="done"/>
</process-definition>"#;
