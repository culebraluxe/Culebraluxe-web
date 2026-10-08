//! WF.DEFINITION — duplicate node IDs (TST-WF-DEFINITION-004).
//!
//! Contract: node ids are the graph's primary key, so a definition that declares the same id twice is refused
//! outright. The check is in `parse_process_definition_xml` (`forge/src/engine/xml.rs:381-384`):
//!
//! ```text
//! if nodes.contains_key(&node.id) { return Err(XmlError(format!("duplicate node {}", node.id))); }
//! nodes.insert(node.id.clone(), node);
//! ```
//!
//! It is deliberately **before** the insert, so the first declaration is the one that survives in a map and the
//! second is the fault — and the refusal names the id, so an operator can find it without guessing. The nodes live
//! in a `BTreeMap` keyed by id, so without this check the second declaration would silently *overwrite* the first:
//! a definition author would get a workflow with a node quietly missing. That is the failure this file exists to
//! close, and it is why "the second one just wins" is the wrong behaviour rather than a harmless one.
//!
//! The clause is proved in both directions and on every axis that could weaken it:
//!
//! - **across kinds** — a `<task-node>` and a `<state>` sharing an id is the same fault as two of the same kind,
//!   because the check is on the id and runs before `parse_node`'s per-kind fields are even consulted;
//! - **the start node is not exempt** — a second `<start-state>` reusing the start id is refused;
//! - **a differing body does not excuse it** — two nodes with the same id but different labels, kinds or
//!   transitions are still a duplicate; and
//! - **the match is exact** — `Work`, ` work `, `work ` and `wor` are four distinct ids, so a definition cannot smuggle
//!   a shadow copy past the check through case or whitespace. (That is the bypass this clause rules out, and it is
//!   also why the check cannot be "fuzzy".)
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__004__duplicate_node_ids

use forge::engine::validate::validate_definition_xml;
use forge::engine::xml::{definition_from_xml, parse_process_definition_xml, FORGE_SDLC_V6_XML};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const KEY: &str = "TST-WF-DEFINITION-004";
/// The id the duplicated node carries; the refusal must name it.
const CLASH: &str = "work";

/// An honest definition with three distinct ids — the positive control. Every refusal below is the check reacting
/// to a real clash, not to the fixture.
const VALID_DEFINITION: &str = r#"<process-definition key="TST-WF-DEFINITION-004" version="1" name="Unique ids">
  <start-state id="start">
    <transition name="begin" to="work"/>
  </start-state>
  <task-node id="work" label="Work" responsibility="smith">
    <transition name="finish" to="done"/>
  </task-node>
  <end-state id="done"/>
</process-definition>"#;

/// `<start-state id="start"/> <end-state id="done"/>` plus the given nodes, in the order given. Declaration order is
/// the axis under test, so it is a parameter rather than fixed.
fn definition_with(nodes: &str) -> String {
    format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Fixture\">\n{nodes}\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
    )
}

/// Parse a definition that must be REFUSED and return the production message.
fn refusal_message(xml: &str) -> String {
    match parse_process_definition_xml(xml) {
        Ok(d) => panic!(
            "{HARNESS}: expected a duplicate-id refusal, but it parsed key '{}' — source:\n{xml}",
            d.key
        ),
        Err(err) => err.to_string(),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-004); the file and the assay use it.
fn wf_definition_004__duplicate_node_ids() {
    // 1. POSITIVE CONTROL — a definition with distinct ids parses, and the map is keyed by id with the key equal to
    //    the node's own id. Without this, a parser that refused every definition would satisfy every clause below.
    let honest =
        parse_process_definition_xml(VALID_DEFINITION).expect("the honest definition must parse");
    assert_eq!(
        honest.graph.nodes.len(),
        3,
        "{HARNESS}: three distinct ids reach the map"
    );
    for (id, node) in &honest.graph.nodes {
        assert_eq!(
            node.id, *id,
            "{HARNESS}: the map key and the node's own id are the same id"
        );
    }
    assert!(
        definition_from_xml(VALID_DEFINITION).is_ok(),
        "{HARNESS}: the honest definition loads as a definition"
    );

    // 2. THE SUBJECT — the same id twice is REFUSED, and the refusal names it. The two nodes differ in every other
    //    respect (kind, label, outgoing edge), so nothing but the id can be the reason.
    let clashing = definition_with(&format!(
        "  <state id=\"{CLASH}\" label=\"First\"/>\n  <task-node id=\"{CLASH}\" label=\"Second\" responsibility=\"qa\"/>"
    ));
    assert_ne!(
        clashing,
        definition_with(&format!("  <state id=\"{CLASH}\" label=\"First\"/>")),
        "{HARNESS}: the duplicate declaration must actually be present in the source"
    );
    assert_eq!(
        refusal_message(&clashing),
        format!("duplicate node {CLASH}"),
        "{HARNESS}: a repeated id is refused and the refusal names it"
    );

    // 3. ORDER INDEPENDENCE — the fault is the pair, not the position. Swapping the two declarations produces the
    //    identical refusal, because the check is symmetric in what it compares: a key already present.
    let reversed = definition_with(&format!(
        "  <task-node id=\"{CLASH}\" label=\"Second\" responsibility=\"qa\"/>\n  <state id=\"{CLASH}\" label=\"First\"/>"
    ));
    assert_eq!(
        refusal_message(&reversed),
        format!("duplicate node {CLASH}"),
        "{HARNESS}: declaration order does not change the refusal"
    );

    // 4. ACROSS KINDS — the check is on the id and runs before `parse_node`'s per-kind work is consulted, so a
    //    clash between two DIFFERENT kinds is the same fault. Each pair below is a distinct kind combination, so a
    //    check that only compared same-kind siblings would pass step 2 and fail here.
    for (first, second) in [
        (
            format!("  <task-node id=\"{CLASH}\" responsibility=\"smith\"/>"),
            format!("  <state id=\"{CLASH}\"/>"),
        ),
        (
            format!("  <state id=\"{CLASH}\"/>"),
            format!("  <end-state id=\"{CLASH}\"/>"),
        ),
        (
            format!("  <end-state id=\"{CLASH}\"/>"),
            format!("  <task-node id=\"{CLASH}\" responsibility=\"qa\"/>"),
        ),
        (
            format!("  <fork id=\"{CLASH}\"/>"),
            format!("  <join id=\"{CLASH}\"/>"),
        ),
        (
            format!("  <decision id=\"{CLASH}\"/>"),
            format!("  <timer id=\"{CLASH}\"/>"),
        ),
        (
            format!("  <command-node id=\"{CLASH}\" command-type=\"forge.migrate_dev\"/>"),
            format!("  <state id=\"{CLASH}\"/>"),
        ),
        (
            format!(
                "  <dynamic-fork id=\"{CLASH}\" count-variable=\"n\" branch-command-type=\"forge.launch_builder\" join=\"j\"/>"
            ),
            format!("  <start-state id=\"{CLASH}\"/>"),
        ),
    ] {
        let xml = definition_with(&format!("{first}\n{second}"));
        assert_eq!(
            refusal_message(&xml),
            format!("duplicate node {CLASH}"),
            "{HARNESS}: a cross-kind id clash is refused too — first={first:?}"
        );
    }

    // 5. THE START NODE IS NOT EXEMPT, IN EITHER POSITION. `start_id` is captured before the map insert
    //    (`forge/src/engine/xml.rs:378-384`), so a clash on the start id could in principle slip past — it does not.
    let duplicate_start_first = definition_with("  <state id=\"start\" label=\"Impostor\"/>");
    assert_eq!(
        refusal_message(&duplicate_start_first),
        "duplicate node start",
        "{HARNESS}: a node declared before the start-state may not reuse the start id"
    );
    let duplicate_start_second = format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Two starts\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <start-state id=\"start\" label=\"Second start\"/>\n  <end-state id=\"done\"/>\n</process-definition>"
    );
    assert_eq!(
        refusal_message(&duplicate_start_second),
        "duplicate node start",
        "{HARNESS}: a second <start-state> reusing the start id is refused"
    );

    // 5b. TWO START-STATES WITH DIFFERENT IDS ARE **NOT** A DUPLICATE — and that is a distinct, honest finding rather
    //     than a gap this file papers over. Nothing in the parser objects; `start_id` is simply overwritten by the
    //     later declaration (`forge/src/engine/xml.rs:378-380`), so the graph starts at the LAST `<start-state>`. The
    //     duplicate rule does not apply because the ids differ, and this clause records that boundary explicitly so
    //     a future change to start-node selection cannot alter it silently.
    let two_starts = parse_process_definition_xml(&format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Two starts\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <start-state id=\"begin_here\">\n    <transition name=\"again\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
    ))
    .expect("two start-states with distinct ids are not a duplicate");
    assert_eq!(
        two_starts.graph.start_node_id, "begin_here",
        "{HARNESS}: with two distinct start ids the later declaration wins — the duplicate rule does not apply"
    );

    // 6. THE IDENTITY IS EXACT — NEAR MISSES ARE NOT DUPLICATES. Case, padding and truncation all produce distinct
    //    ids, so a definition cannot hide a shadow node behind a lookalike. This is the bypass that a
    //    case-folding or trimming implementation would let through, so it is pinned from both sides: the lookalikes
    //    coexist as separate nodes, and the true duplicate is still refused.
    let lookalikes = definition_with(&format!(
        "  <state id=\"{CLASH}\"/>\n  <state id=\"{CLASH}\"/>\n  <state id=\"WORK\"/>\n  <state id=\" work \"/>"
    ));
    // The true duplicate in this source still wins: the refusal comes from the pair, not from the lookalikes.
    assert_eq!(
        refusal_message(&lookalikes),
        format!("duplicate node {CLASH}"),
        "{HARNESS}: the exact pair is refused even when lookalike ids are present"
    );
    // With the exact pair removed, the lookalikes are three DIFFERENT nodes and the definition parses.
    let distinct_lookalikes = definition_with(&format!(
        "  <state id=\"{CLASH}\"/>\n  <state id=\"WORK\"/>\n  <state id=\" work \"/>\n  <state id=\"wor\"/>"
    ));
    let parsed = parse_process_definition_xml(&distinct_lookalikes)
        .expect("case, padding and truncation are distinct ids, so nothing is duplicated");
    assert_eq!(
        parsed.graph.nodes.len(),
        6,
        "{HARNESS}: four lookalike ids plus start and done are six distinct nodes: {:?}",
        parsed.graph.nodes.keys().collect::<Vec<_>>()
    );
    for near in ["WORK", " work ", "wor"] {
        assert!(
            parsed.graph.nodes.contains_key(near),
            "{HARNESS}: '{near}' is its own id, not a duplicate of '{CLASH}'"
        );
    }

    // 6b. `<display-order>` REFERENCES ARE NOT NODES. The parser reads `<node ref="…"/>` without declaring a node
    //     (`forge/src/engine/xml.rs:368-375`), so a ref repeating an existing id is an ordering choice, not a
    //     duplicate declaration. A parser that folded refs into the same check would refuse every definition whose
    //     display order names a node it also declares — which is all of them.
    let display_repeats = parse_process_definition_xml(&format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Display\">\n  <display-order>\n    <node ref=\"start\"/>\n    <node ref=\"start\"/>\n    <node ref=\"work\"/>\n  </display-order>\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"{CLASH}\"/>\n  </start-state>\n  <task-node id=\"{CLASH}\" responsibility=\"smith\"/>\n  <end-state id=\"done\"/>\n</process-definition>"
    ))
    .expect("a repeated display-order ref is not a duplicate node");
    assert_eq!(
        display_repeats.graph.nodes.len(),
        3,
        "{HARNESS}: display-order contributes no nodes"
    );

    // 7. A DUPLICATE IS STILL A DUPLICATE WHOSE FIRST DECLARATION WAS UNREACHABLE. The check walks every declared
    //     node, not only those reachable from the start, so a shadow id parked behind an orphan node cannot escape
    //     it — the same property `wf_definition__005__missing_target` relies on for edges.
    let orphaned = definition_with(&format!(
        "  <state id=\"lonely\"/>\n  <state id=\"{CLASH}\"/>\n  <state id=\"{CLASH}\" label=\"Shadow\"/>"
    ));
    assert_eq!(
        refusal_message(&orphaned),
        format!("duplicate node {CLASH}"),
        "{HARNESS}: a duplicate behind an unreachable node is still refused"
    );

    // 8. THE OTHER SEAMS AGREE. `definition_from_xml` (what deploy calls) and `validate_definition_xml` (what the
    //     validator reports) consume the same parser, so neither may report a duplicate as acceptable.
    let via_definition = definition_from_xml(&clashing)
        .expect_err("{HARNESS}: definition_from_xml must refuse a duplicate too");
    assert_eq!(
        via_definition.to_string(),
        format!("duplicate node {CLASH}"),
        "{HARNESS}: the refusal is the same one, with the same text"
    );
    let report = validate_definition_xml(&clashing);
    assert!(!report.valid, "{HARNESS}: a duplicate id does not validate");
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains(&format!("duplicate node {CLASH}"))),
        "{HARNESS}: the validator's structured errors carry the duplicate: {:?}",
        report.errors
    );

    // 9. THE SHIPPED DEFINITION HAS NO DUPLICATE, AND ITS OWN IDS ARE THE PROOF THE RULE APPLIES TO IT. Every id in
    //    the deployed definition is unique, so the check is not vacuous there — and renaming one id to another's
    //    makes the shipped file itself refused.
    let shipped = definition_from_xml(FORGE_SDLC_V6_XML).expect("the shipped definition parses");
    let mut ids: Vec<&String> = shipped.definition.nodes.keys().collect();
    let count = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(
        ids.len(),
        count,
        "{HARNESS}: the shipped definition declares {} distinct ids",
        count
    );
    let shipped_clash = FORGE_SDLC_V6_XML.replacen(
        "  <end-state id=\"cancelled\"",
        "  <end-state id=\"complete\"",
        1,
    );
    assert!(
        shipped_clash != FORGE_SDLC_V6_XML,
        "{HARNESS}: the shipped definition must contain the anchor this clause edits"
    );
    assert_eq!(
        refusal_message(&shipped_clash),
        "duplicate node complete",
        "{HARNESS}: the shipped definition is refused when one terminus reuses another's id"
    );

    // 10. NON-VACUITY. If the parser refused every definition, step 1 would have failed; if it accepted every
    //     definition, steps 2-9 would have. The pair is asserted together so neither can drift.
    assert!(
        parse_process_definition_xml(VALID_DEFINITION).is_ok(),
        "{HARNESS}: distinct ids are accepted"
    );
    assert!(
        parse_process_definition_xml(&clashing).is_err(),
        "{HARNESS}: a repeated id is refused — the rule has both answers"
    );
    // And the rule is specific to a REPEATED id: renaming the second node rather than duplicating it is accepted.
    let renamed = definition_with(&format!(
        "  <state id=\"{CLASH}\" label=\"First\"/>\n  <task-node id=\"work_two\" label=\"Second\" responsibility=\"qa\"/>"
    ));
    assert!(
        parse_process_definition_xml(&renamed).is_ok(),
        "{HARNESS}: two similar-but-distinct ids are not a duplicate"
    );
}
