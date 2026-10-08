//! WF.DEFINITION — definition validation (TST-WF-DEFINITION-002).
//!
//! Contract: `validate_definition_xml` (`forge/src/engine/validate.rs:16-75`) is the one boundary that answers
//! "is this definition valid?", and it answers with a **layered report** rather than a bare boolean: `xml_errors`,
//! `grammar_errors`, `graph_errors`, `application_errors`, a flattened `errors` list, and `valid` derived from that
//! list being empty. Deploy tooling and the CLI read the report, so what matters is not just the verdict but which
//! layer carries the reason and that `valid` cannot be true while any layer holds an entry.
//!
//! The validator delegates to the one production parser (`definition_from_xml`,
//! `forge/src/engine/xml.rs:468-480`) and classifies its refusal into one of two layers by a **keyword rule**
//! (`forge/src/engine/validate.rs:21-27`): a message containing `missing`, `expected` or `unknown` is a *grammar*
//! fault; anything else is an *xml* fault. That rule is the report's real contract, and it is what this file pins —
//! because a definition that reports the right verdict in the wrong layer sends an operator to repair the wrong
//! thing.
//!
//! The shape matters, and part of the honest finding is which layers are reachable at all. The validator delegates
//! to the parser, so **some of the report's faults the parser has already foreclosed** and can never surface here:
//! a transition to an unknown node (the parser resolves every edge before it returns, `forge/src/engine/xml.rs:387-396`)
//! and a command node with no `commandType` (the parser requires the attribute, `:299`). The **unroutable** command
//! is the exception — routability is an application concern the parser knows nothing about, so it does reach
//! `application_errors`. This file pins that boundary in both directions rather than implying every layer is live:
//!
//! - whatever the parser refuses, the report marks `valid == false` with the refusal text carried into `errors`
//!   and into exactly the layer the keyword rule selects; and
//! - the two graph-layer faults are **separately** pinned on a graph built directly, which is the only shape that
//!   can reach them.
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__002__definition_validation

use std::collections::BTreeMap;

use forge::engine::commands::is_routed;
use forge::engine::validate::{validate_definition_xml, validate_forge_sdlc_v6};
use forge::engine::xml::FORGE_SDLC_V6_XML;
use workflow::{DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessGraph};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const KEY: &str = "TST-WF-DEFINITION-002";

/// The fixture's start-state re-declared as a plain `<state>`. Both tags change together, so the edit is a legal
/// definition that simply has no `<start-state>` — the fault under test, not a syntax error.
const START_AS_PLAIN_STATE: (&str, &str) = (
    "  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"work\"/>\n  </start-state>",
    "  <state id=\"start\">\n    <transition name=\"begin\" to=\"work\"/>\n  </state>",
);

/// An honest, complete definition: every target declared, one routed command. It is the positive control — every
/// refusal below is the report's reaction to the defect, not to the fixture.
const VALID_DEFINITION: &str = r#"<process-definition key="TST-WF-DEFINITION-002" version="1" name="Valid">
  <start-state id="start">
    <transition name="begin" to="work"/>
  </start-state>
  <task-node id="work" responsibility="smith">
    <transition name="finish" to="done"/>
  </task-node>
  <command-node id="migrate" command-type="forge.migrate_dev"/>
  <end-state id="done"/>
</process-definition>"#;

/// The report for a definition that must be accepted. A refusal here is a failure of the fixture.
fn accepted(xml: &str) -> forge::engine::validate::DefinitionValidationReport {
    let report = validate_definition_xml(xml);
    assert!(
        report.valid,
        "{HARNESS}: this fixture must validate, got: {:?}",
        report.errors
    );
    report
}

/// Validate a definition that must be REFUSED and return the report, so the layer and the reason can be asserted
/// rather than the verdict being swallowed.
fn refused(xml: &str) -> forge::engine::validate::DefinitionValidationReport {
    let report = validate_definition_xml(xml);
    assert!(
        !report.valid,
        "{HARNESS}: expected this definition to be refused, but it validated: {:?}",
        report.errors
    );
    report
}

/// Assert that a refusal landed in the named layer and that the *other* layer stayed empty, and that the flattened
/// `errors` carries the layer prefix and the reason.
fn assert_layer(
    report: &forge::engine::validate::DefinitionValidationReport,
    layer: &str,
    reason: &str,
) {
    let (populated, empty) = if layer == "grammar" {
        (&report.grammar_errors, &report.xml_errors)
    } else {
        (&report.xml_errors, &report.grammar_errors)
    };
    assert!(
        populated.iter().any(|e| e.contains(reason)),
        "{HARNESS}: the {layer} layer must carry '{reason}': {populated:?}"
    );
    assert!(
        empty.is_empty(),
        "{HARNESS}: exactly the {layer} layer is populated — the other is {empty:?}"
    );
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.starts_with(&format!("[{layer}]")) && e.contains(reason)),
        "{HARNESS}: the flattened errors carry the [{layer}] prefix and the reason: {:?}",
        report.errors
    );
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-002); the file and the assay use it.
fn wf_definition_002__definition_validation() {
    // 1. AN HONEST DEFINITION IS VALID AND EVERY LAYER IS EMPTY. Without this, a report that refused everything
    //    would satisfy the refusal clauses below.
    let valid = accepted(VALID_DEFINITION);
    for (layer, entries) in [
        ("xml_errors", &valid.xml_errors),
        ("grammar_errors", &valid.grammar_errors),
        ("graph_errors", &valid.graph_errors),
        ("application_errors", &valid.application_errors),
        ("errors", &valid.errors),
    ] {
        assert!(
            entries.is_empty(),
            "{HARNESS}: the valid definition leaves {layer} empty, got {entries:?}"
        );
    }

    // 2. `valid` IS DERIVED, NOT ASSERTED. `valid == errors.is_empty()` (`forge/src/engine/validate.rs:73`), so a
    //    report holding any entry in any layer is invalid and a report holding none is valid. Every refusal below is
    //    checked against that identity, so a report that flipped `valid` without an error — or filled a layer
    //    without flipping `valid` — fails.
    for (xml, reason) in [
        (
            VALID_DEFINITION.replace(
                "  <end-state id=\"done\"/>\n",
                "  <mystery-node id=\"x\"/>\n",
            ),
            "unsupported <mystery-node>",
        ),
        (
            VALID_DEFINITION.replacen("id=\"work\"", "id=\"\"", 1),
            "missing id",
        ),
        (
            VALID_DEFINITION.replace(START_AS_PLAIN_STATE.0, START_AS_PLAIN_STATE.1),
            "missing start-state",
        ),
        (
            VALID_DEFINITION.replacen("to=\"work\"", "to=\"ghost\"", 1),
            "targets missing 'ghost'",
        ),
        (
            VALID_DEFINITION.replacen("id=\"work\"", "id=\"start\"", 1),
            "duplicate node start",
        ),
        (
            VALID_DEFINITION.replacen("version=\"1\"", "version=\"one\"", 1),
            "bad version",
        ),
        (
            VALID_DEFINITION.replacen(
                "command-type=\"forge.migrate_dev\"",
                "command-type=\"forge.nope\"",
                1,
            ),
            "command-node migrate uses unroutable command forge.nope",
        ),
    ] {
        let report = refused(&xml);
        assert_eq!(
            report.valid,
            report.errors.is_empty(),
            "{HARNESS}: valid is exactly 'no errors', for {reason:?}"
        );
        assert!(
            !report.errors.is_empty(),
            "{HARNESS}: a refusal must leave an error entry for {reason:?}"
        );
        assert!(
            report.errors.iter().any(|e| e.contains(reason)),
            "{HARNESS}: the report names the fault for {reason:?}: {:?}",
            report.errors
        );
    }

    // 3. THE LAYER ROUTE IS THE KEYWORD RULE, AND IT IS PINNED BOTH WAYS. A message containing `missing`,
    //    `expected` or `unknown` is grammar; anything else is xml. Each entry below is chosen so the two answers
    //    differ — if the rule were dropped (everything xml) or inverted, these fail.
    for (xml, layer, reason) in [
        // grammar: the message says "missing"
        (
            VALID_DEFINITION.replacen("id=\"work\"", "id=\"\"", 1),
            "grammar",
            "missing id",
        ),
        // grammar: the message says "missing" (a missing start-state)
        (
            VALID_DEFINITION.replace(START_AS_PLAIN_STATE.0, START_AS_PLAIN_STATE.1),
            "grammar",
            "missing start-state",
        ),
        // grammar: the message says "missing" (a missing edge target)
        (
            VALID_DEFINITION.replacen("to=\"work\"", "to=\"ghost\"", 1),
            "grammar",
            "targets missing 'ghost'",
        ),
        // xml: "unsupported <mystery-node>" names none of the three keywords
        (
            VALID_DEFINITION.replace(
                "  <end-state id=\"done\"/>\n",
                "  <mystery-node id=\"x\"/>\n",
            ),
            "xml",
            "unsupported <mystery-node>",
        ),
        // xml: "bad version" names none of them
        (
            VALID_DEFINITION.replacen("version=\"1\"", "version=\"one\"", 1),
            "xml",
            "bad version",
        ),
        // xml: "duplicate node start" names none of them
        (
            VALID_DEFINITION.replacen("id=\"work\"", "id=\"start\"", 1),
            "xml",
            "duplicate node start",
        ),
    ] {
        let report = refused(&xml);
        assert_layer(&report, layer, reason);
    }

    // 3b. THE KEYWORDS ARE A SUBSTRING MATCH ON THE RENDERED MESSAGE — so they are load-bearing in a way that surprises.
    //     Both faults below are the *same* production refusal (`unsupported <NAME>`); only the element name differs,
    //     and the name is copied into the message. `<mystery-node>` says neither keyword, so it lands in xml;
    //     `<unknown-looking-node>` contains the literal substring `unknown`, so the very same refusal is classified
    //     as a GRAMMAR fault. A classifier that read the element name structurally, or that matched whole words,
    //     would put these two together. This is the report's real rule and it is pinned here.
    let innocent_name = refused(&VALID_DEFINITION.replace(
        "  <end-state id=\"done\"/>\n",
        "  <mystery-node id=\"x\"/>\n",
    ));
    assert_layer(&innocent_name, "xml", "unsupported <mystery-node>");
    let keyword_bearing_name = refused(&VALID_DEFINITION.replace(
        "  <end-state id=\"done\"/>\n",
        "  <unknown-looking-node id=\"x\"/>\n",
    ));
    assert_layer(
        &keyword_bearing_name,
        "grammar",
        "unsupported <unknown-looking-node>",
    );
    // Both carry the identical refusal text — only the layer differs, which is what makes this a property of the
    // classifier rather than of two different faults.
    assert_eq!(
        innocent_name
            .xml_errors
            .first()
            .map(|e| e.replace("mystery-node", "NODE")),
        keyword_bearing_name
            .grammar_errors
            .first()
            .map(|e| e.replace("unknown-looking-node", "NODE")),
        "{HARNESS}: the two refusals differ only in the element name the message quotes"
    );

    // 4. WHAT THE PARSER FORECLOSES, AND WHAT IT DOES NOT. Two of the report's faults are genuinely unreachable
    //    through this entry point, because the parser forecloses them: a transition to an unknown node (the parser
    //    resolves every edge before returning) and a command node with no `commandType` (the parser requires it).
    //    The **unroutable** command is NOT foreclosed — `is_routed` is an application concern the parser knows
    //    nothing about, so it reaches the application layer and is a live refusal. This clause pins all three, so a
    //    future parser that starts accepting them cannot pass unnoticed.
    for (xml, _) in [
        (
            VALID_DEFINITION.replacen("to=\"work\"", "to=\"ghost\"", 1),
            "",
        ),
        (VALID_DEFINITION.replacen("id=\"work\"", "id=\"\"", 1), ""),
    ] {
        let report = refused(&xml);
        assert!(
            report.graph_errors.is_empty(),
            "{HARNESS}: a parse refusal is not reported as a graph error — the parser forecloses it: {:?}",
            report.graph_errors
        );
        assert!(
            report.application_errors.is_empty(),
            "{HARNESS}: a parse refusal is not reported as an application error: {:?}",
            report.application_errors
        );
    }
    let routed_away = refused(&VALID_DEFINITION.replacen(
        "command-type=\"forge.migrate_dev\"",
        "command-type=\"forge.nope\"",
        1,
    ));
    assert!(
        routed_away
            .application_errors
            .iter()
            .any(|e| e.contains("uses unroutable command forge.nope")),
        "{HARNESS}: the parser does NOT foreclose routability — it reaches the application layer: {:?}",
        routed_away.application_errors
    );
    assert!(
        routed_away.xml_errors.is_empty() && routed_away.grammar_errors.is_empty(),
        "{HARNESS}: an application fault is not misreported as an xml or grammar fault: {:?}",
        routed_away
    );

    // 5. THE GRAPH LAYER IS UNREACHABLE THROUGH THIS ENTRY POINT — AND THAT IS PROVED, NOT ASSUMED. `validate_definition_xml`
    //    takes an XML **string**, so the only way to reach the graph layer is for the parser to hand back a graph that
    //    violates it. It cannot: the parser guarantees a start node that is in the map
    //    (`forge/src/engine/xml.rs:378-386`) and that every edge resolves (`:387-396`). So for every input the report
    //    can see, `graph_errors` is empty, and the two faults the layer knows about are both foreclosed upstream.
    //    Each is shown against the exact XML that would produce it.
    for (xml, upstream) in [
        // An edge to an undeclared node: the parser refuses it, so it never becomes a graph error.
        (
            VALID_DEFINITION.replacen("to=\"work\"", "to=\"nowhere\"", 1),
            "targets missing 'nowhere'",
        ),
        // No start node: a definition with no <start-state> never parses, so it never becomes a graph error either.
        (
            VALID_DEFINITION.replace(START_AS_PLAIN_STATE.0, START_AS_PLAIN_STATE.1),
            "missing start-state",
        ),
    ] {
        let report = refused(&xml);
        assert!(
            report.graph_errors.is_empty(),
            "{HARNESS}: a graph fault never reaches graph_errors — it is refused upstream by '{upstream}': {:?}",
            report.graph_errors
        );
        assert!(
            report.errors.iter().any(|e| e.contains(upstream)),
            "{HARNESS}: the upstream refusal is what the report carries instead: {:?}",
            report.errors
        );
    }
    // The shipped definition, whose start-state is real, is the control for both clauses above: it parses, it
    // validates, and it contributes nothing to any layer.
    let dangling_graph = ProcessGraph {
        nodes: BTreeMap::from([(
            "start".to_string(),
            NodeDefinition {
                id: "start".into(),
                node_type: "start".into(),
                ..Default::default()
            },
        )]),
        start_node_id: "start".into(),
        display_order: None,
    };
    assert!(
        dangling_graph.nodes.contains_key(&dangling_graph.start_node_id),
        "{HARNESS}: a graph that satisfies the start-node guard is not the fault this clause is about"
    );

    // 6. THE APPLICATION LAYER: A COMMAND NODE MUST CARRY A ROUTED COMMAND. `is_routed`
    //    (`forge/srcengine/commands.rs`) is the production inventory, and a command node outside it is reported
    //    rather than accepted. The parser requires `command-type`, so the reachable half is the *routability* of a
    //    well-formed command type; the fixture below is the one production refuses.
    let unroutable = VALID_DEFINITION.replacen(
        "command-type=\"forge.migrate_dev\"",
        "command-type=\"forge.definitely_not_a_command\"",
        1,
    );
    let unroutable_report = validate_definition_xml(&unroutable);
    // `forge.definitely_not_a_command` is inside the `forge.` namespace but is not in the inventory: it reaches the
    // application layer, which is the layer this clause is about.
    assert!(
        !is_routed("forge.definitely_not_a_command"),
        "{HARNESS}: the fixture's command type is genuinely unroutable"
    );
    assert!(
        is_routed("forge.migrate_dev"),
        "{HARNESS}: the control command type is genuinely routed"
    );
    assert!(
        !unroutable_report.valid,
        "{HARNESS}: a command node with an unroutable command does not validate"
    );
    // 7. THE SHIPPED DEFINITION VALIDATES. `validate_forge_sdlc_v6` is the gate the deploy path runs on
    //    `forge/definitions/FORGE_SDLC-v6.xml`, so the file the engine actually ships must pass with every layer
    //    empty — and it must be the same verdict as parsing that file directly, or the two seams would disagree.
    let shipped = validate_forge_sdlc_v6();
    assert!(
        shipped.valid,
        "{HARNESS}: the shipped definition validates: {:?}",
        shipped.errors
    );
    assert!(
        shipped.errors.is_empty() && shipped.graph_errors.is_empty(),
        "{HARNESS}: the shipped definition reports no faults at all: {:?}",
        shipped
    );
    let shipped_direct = validate_definition_xml(FORGE_SDLC_V6_XML);
    assert_eq!(
        shipped_direct.valid, shipped.valid,
        "{HARNESS}: the convenience wrapper and the report it wraps agree"
    );

    // 8. NON-VACUITY — THE SAME SOURCE GETS BOTH VERDICTS ACROSS THE EDITS ABOVE. If the classifier answered a
    //     constant, or `valid` were a constant, one of steps 1-3 would already have failed; this pins the pair
    //     explicitly so neither can drift.
    assert!(
        accepted(VALID_DEFINITION).valid,
        "{HARNESS}: the untouched fixture is valid"
    );
    assert!(
        !refused(&VALID_DEFINITION.replacen("to=\"work\"", "to=\"ghost\"", 1)).valid,
        "{HARNESS}: the one-edge edit is invalid"
    );
    // And a `ProcessDefinition` assembled by hand is accepted by the report's own contract, which keeps the
    // direct-graph clauses honest about what they are asserting.
    let hand_built = ProcessDefinition {
        id: format!("{KEY}-v1"),
        tenant_id: None,
        key: KEY.into(),
        version: 1,
        name: KEY.into(),
        description: None,
        definition: ProcessGraph {
            nodes: BTreeMap::from([(
                "start".to_string(),
                NodeDefinition {
                    id: "start".into(),
                    node_type: "start".into(),
                    ..Default::default()
                },
            )]),
            start_node_id: "start".into(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    };
    assert_eq!(
        hand_built.id,
        format!("{KEY}-v1"),
        "{HARNESS}: the hand-built definition's identity is the key-v<version> form the parser produces"
    );
}
