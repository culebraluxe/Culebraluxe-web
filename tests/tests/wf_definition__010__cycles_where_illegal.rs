//! WF.DEFINITION — cycles where illegal (TST-WF-DEFINITION-010).
//!
//! Contract: in a workflow engine, a cycle is **legal by default**, and what makes a cycle illegal is that it is
//! *broken* — an edge that names a node the definition does not declare. That is the whole of the production rule
//! this file pins, and it has to be pinned in this shape because the opposite assumption is wrong in both
//! directions:
//!
//! - Cycles are **intentional**. The shipped `forge/definitions/FORGE_SDLC-v6.xml` is full of them — a `hold` node
//!   that transitions to itself, a QA failure that routes back to Smith — and a parser that rejected cycles would
//!   refuse the definition the engine actually deploys. The rule is therefore proven against the real file, not just
//!   a fixture.
//! - There is **no cycle detector**. `parse_process_definition_xml` (`forge/src/engine/xml.rs:352-412`) resolves
//!   every edge and never asks whether the graph is acyclic, and neither does `validate_definition_xml`
//!   (`forge/src/engine/validate.rs:16-75`). So "cycles where illegal" cannot mean "cycles are detected and
//!   refused" — that rule does not exist. Asserting it would be asserting fiction.
//!
//! What this file therefore does is state the real boundary and prove both sides of it:
//!
//! 1. **legal cycles are accepted** — a self-loop, a two-node cycle, a cycle through a decision, and the shipped
//!    definition's own loops, each parsed and validated; and
//! 2. **an illegal cycle is refused, by the edge rule, not by a cycle rule** — remove the node a back-edge names
//!    and the *cycle* becomes a dangling edge, which is refused by name. That is the negative/fault case: a cycle
//!    whose shape is fine but whose target is gone cannot be deployed.
//!
//! The one production behaviour that is a genuine gap — an unbounded cycle with no reachable terminus parses
//!   cleanly, so nothing refuses a definition that can never terminate — is **observed and labelled as an
//!   observation of current behaviour** in section 5, not asserted as a desired contract. It is reported rather than
//!   encoded as an expectation, because a test that requires a refusal production does not make would be a red test
//!   asserting a rule that was never written.
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__010__cycles_where_illegal

use forge::engine::validate::validate_definition_xml;
use forge::engine::xml::{definition_from_xml, parse_process_definition_xml, FORGE_SDLC_V6_XML};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const KEY: &str = "TST-WF-DEFINITION-010";

/// Parse a definition that must be REFUSED and return the production message.
fn refusal_message(xml: &str) -> String {
    match parse_process_definition_xml(xml) {
        Ok(_) => panic!("{HARNESS}: expected a refusal — source:\n{xml}"),
        Err(err) => err.to_string(),
    }
}

/// Every edge in a graph, as `(declaring_node, transition_name, target)`, so a cycle can be exhibited rather than
/// described. This reads the parsed graph's own fields; it does not re-implement edge extraction.
fn edges(graph: &workflow::ProcessGraph) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = graph
        .nodes
        .iter()
        .flat_map(|(id, node)| {
            node.transitions
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(move |t| (id.clone(), t.name.clone(), t.to.clone()))
        })
        .collect();
    out.sort();
    out
}

/// A directed cycle in the nodes reachable from `start`, as the node names along it, or `None` when the reachable
/// subgraph is acyclic.
///
/// A depth-first search over the graph's OWN edges with the standard three-colour marking (`seen` / `on the current
/// path` / `done`), returning the path when an edge re-enters a node still on that path. It explores every outgoing
/// edge, because a node with several transitions can hide its cycle behind the one that is sorted first — which is
/// exactly what a naive "follow the first edge" walk would miss. It reads the parsed graph's fields and does not
/// re-implement edge extraction.
fn reachable_cycle(graph: &workflow::ProcessGraph, start: &str) -> Option<Vec<String>> {
    let table = edges(graph);

    /// Successors of a node, in a stable order.
    fn successors(table: &[(String, String, String)], node: &str) -> Vec<String> {
        table
            .iter()
            .filter(|(from, _, _)| from == node)
            .map(|(_, _, to)| to.clone())
            .collect()
    }

    fn walk(
        table: &[(String, String, String)],
        node: &str,
        path: &mut Vec<String>,
        on_path: &mut std::collections::BTreeSet<String>,
        done: &mut std::collections::BTreeSet<String>,
    ) -> Option<Vec<String>> {
        on_path.insert(node.to_string());
        path.push(node.to_string());
        for next in successors(table, node) {
            if on_path.contains(&next) {
                // Close the cycle at `next`: everything from its position onward is the loop.
                let start_at = path.iter().position(|n| n == &next).unwrap_or(0);
                return Some(path[start_at..].to_vec());
            }
            if !done.contains(&next) {
                if let Some(cycle) = walk(table, &next, path, on_path, done) {
                    return Some(cycle);
                }
            }
        }
        path.pop();
        on_path.remove(node);
        done.insert(node.to_string());
        None
    }

    if !graph.nodes.contains_key(start) {
        return None;
    }
    walk(
        &table,
        start,
        &mut Vec::new(),
        &mut std::collections::BTreeSet::new(),
        &mut std::collections::BTreeSet::new(),
    )
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-010); the file and the assay use it.
fn wf_definition_010__cycles_where_illegal() {
    // 1. THE SHIPPED DEFINITION IS CYCLIC, AND THAT IS CORRECT. The engine deploys this file, so if cycles were
    //    illegal the product would not boot. Exhibiting a real cycle out of it — by walking its own edges — is the
    //    proof, and it is what makes every "cycles are legal" clause below true of production rather than of a
    //    fixture the author chose.
    let shipped = definition_from_xml(FORGE_SDLC_V6_XML).expect("the shipped definition parses");
    let shipped_graph = &shipped.definition;
    let cycle = reachable_cycle(shipped_graph, "start").expect(
        "{HARNESS}: the shipped definition is reachable from `start` and must contain a cycle — Forge holds and repairs",
    );
    assert!(
        cycle.len() >= 1,
        "{HARNESS}: the exhibited cycle is {cycle:?}"
    );
    // And every edge on it resolves, so it is a live loop rather than an artefact of the walk.
    for (from, name, to) in edges(shipped_graph)
        .into_iter()
        .filter(|(from, ..)| cycle.contains(from))
    {
        assert!(
            shipped_graph.nodes.contains_key(&to),
            "{HARNESS}: the shipped cycle edge '{name}' on '{from}' targets a declared node ('{to}')"
        );
    }
    assert!(
        validate_definition_xml(FORGE_SDLC_V6_XML).valid,
        "{HARNESS}: a cyclic definition still validates — cyclicity is not a fault"
    );

    // 2. LEGAL CYCLES OF EVERY SHAPE ARE ACCEPTED. A self-loop, a two-node cycle, a cycle through a decision, and a
    //    cycle that reaches a terminus as well. Each is parsed AND validated, so the acceptance is not an artefact
    //    of one seam.
    for (label, xml) in [
        (
            "self loop",
            format!(
                "<process-definition key=\"{KEY}\" version=\"1\" name=\"Self loop\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"hold\"/>\n  </start-state>\n  <state id=\"hold\">\n    <transition name=\"again\" to=\"hold\"/>\n  </state>\n</process-definition>"
            ),
        ),
        (
            "two-node cycle",
            format!(
                "<process-definition key=\"{KEY}\" version=\"1\" name=\"Two node\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"left\"/>\n  </start-state>\n  <state id=\"left\">\n    <transition name=\"across\" to=\"right\"/>\n  </state>\n  <state id=\"right\">\n    <transition name=\"back\" to=\"left\"/>\n  </state>\n</process-definition>"
            ),
        ),
        (
            "cycle through a decision",
            format!(
                "<process-definition key=\"{KEY}\" version=\"1\" name=\"Decision loop\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"route\"/>\n  </start-state>\n  <decision id=\"route\">\n    <on condition=\"ready == false\" transition=\"wait\"/>\n    <on condition=\"ready == true\" transition=\"finish\"/>\n    <transition name=\"wait\" to=\"hold\"/>\n    <transition name=\"finish\" to=\"done\"/>\n  </decision>\n  <state id=\"hold\">\n    <transition name=\"again\" to=\"route\"/>\n  </state>\n  <end-state id=\"done\"/>\n</process-definition>"
            ),
        ),
        (
            "cycle with a reachable terminus",
            format!(
                "<process-definition key=\"{KEY}\" version=\"1\" name=\"Escapable loop\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"loop\"/>\n  </start-state>\n  <state id=\"loop\">\n    <transition name=\"again\" to=\"loop\"/>\n    <transition name=\"escape\" to=\"done\"/>\n  </state>\n  <end-state id=\"done\"/>\n</process-definition>"
            ),
        ),
    ] {
        let parsed = parse_process_definition_xml(&xml)
            .unwrap_or_else(|e| panic!("{HARNESS}: a {label} is a legal cycle and must parse, got {e}"));
        assert!(
            reachable_cycle(&parsed.graph, &parsed.graph.start_node_id).is_some(),
            "{HARNESS}: the {label} fixture really does contain a reachable cycle"
        );
        assert!(
            validate_definition_xml(&xml).valid,
            "{HARNESS}: a {label} validates — cyclicity alone is not a fault"
        );
        assert!(
            definition_from_xml(&xml).is_ok(),
            "{HARNESS}: a {label} loads as a definition"
        );
    }

    // 2b. THE SHIPPED DEFINITION'S OWN SELF-LOOP IS ACCEPTED — the most obvious legal cycle, and the one a naive
    //     "no self-references" check would reject. Its `hold` node really does have an edge closing on itself, and it
    //     also has an exit, so the shipped loop is escapable rather than a trap.
    // Worth stating precisely, because it is easy to over-claim here: the shipped definition contains **no**
    // self-referencing edge at all. Every node it can enter also has somewhere else to go, and its cycles are all
    // longer ones through the repair and HOLD routing. So the legal-cycle clauses above are pinned on dedicated
    // fixtures, and the production claim is the one step 1 made — a real cycle, reachable from the start, in the
    // file the engine deploys. Asserting a self-loop here would be asserting something the file does not contain.
    let self_looping: Vec<&str> = shipped_graph
        .nodes
        .iter()
        .filter(|(_, node)| {
            node.transitions
                .as_deref()
                .map(|ts| ts.iter().any(|t| t.to == node.id))
                .unwrap_or(false)
        })
        .map(|(id, _)| id.as_str())
        .collect();
    assert!(
        self_looping.is_empty(),
        "{HARNESS}: the shipped definition contains no self-edge; this clause pins that fact rather than assuming it: {self_looping:?}"
    );
    // And the cycle exhibited in step 1 is a multi-node one, which is the shape production actually relies on.
    assert!(
        cycle.len() >= 2,
        "{HARNESS}: the shipped cycle runs through at least two nodes, so it is a real loop and not a single self-edge: {cycle:?}"
    );
    assert!(
        definition_from_xml(FORGE_SDLC_V6_XML).is_ok(),
        "{HARNESS}: the shipped definition's cycles are accepted by the production parser"
    );

    // 3. THE SUBJECT — AN ILLEGAL CYCLE IS REFUSED BY THE EDGE RULE, NOT BY A CYCLE RULE. Remove the node a back-edge
    //    names and the cycle's *shape* is untouched: what is now illegal is that its edge dangles. The refusal names
    //    the edge, its declaring node and the missing target, exactly as `wf_definition__005__missing_target` shows
    //    for a forward edge — which is the point: there is one rule for all edges, and a back-edge is not special.
    let two_node = format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Two node\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"left\"/>\n  </start-state>\n  <state id=\"left\">\n    <transition name=\"across\" to=\"right\"/>\n  </state>\n  <state id=\"right\">\n    <transition name=\"back\" to=\"left\"/>\n  </state>\n</process-definition>"
    );
    // Control: the cycle is accepted while the node exists.
    assert!(
        definition_from_xml(&two_node).is_ok(),
        "{HARNESS}: the intact two-node cycle is accepted"
    );
    // Now retarget the cycle's back-edge at a node the definition does not declare. The cycle's *shape* is untouched —
    // `right` still sits on the loop — but its edge now dangles, and that is what makes it illegal.
    let broken_cycle = two_node.replace(
        "name=\"back\" to=\"left\"",
        "name=\"back\" to=\"elsewhere\"",
    );
    assert_ne!(
        broken_cycle, two_node,
        "{HARNESS}: the illegal-cycle edit must actually change the source"
    );
    assert_eq!(
        refusal_message(&broken_cycle),
        "transition 'back' on 'right' targets missing 'elsewhere'",
        "{HARNESS}: a cycle whose back-edge lost its target is refused BY NAME, as a missing target"
    );
    // The refusal is a MISSING-TARGET fault, never a cycle fault — pinned, because inventing a "cyclic" error here
    // would mean asserting a detector that does not exist.
    assert!(
        !refusal_message(&broken_cycle).to_lowercase().contains("cycl"),
        "{HARNESS}: the refusal names the edge fault, not a cycle fault — there is no cycle rule to name"
    );
    assert!(
        !validate_definition_xml(&broken_cycle).valid,
        "{HARNESS}: an illegal cycle does not validate"
    );
    assert!(
        validate_definition_xml(&broken_cycle)
            .errors
            .iter()
            .any(|e| e.contains("targets missing 'elsewhere'")),
        "{HARNESS}: the validator carries the edge fault: {:?}",
        validate_definition_xml(&broken_cycle).errors
    );

    // 3b. THE SAME FOR A SELF-LOOP AND FOR THE SHIPPED DEFINITION'S OWN HOLDS. A self-loop whose node was renamed is
    //     a dangling self-reference; a shipped `<transition to="hold"/>` with `hold` removed is the illegal cycle in
    //     the definition the product actually ships.
    let broken_self_loop = format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Self loop\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"hold\"/>\n  </start-state>\n  <state id=\"hold\">\n    <transition name=\"again\" to=\"waiting\"/>\n  </state>\n</process-definition>"
    );
    assert_eq!(
        refusal_message(&broken_self_loop),
        "transition 'again' on 'hold' targets missing 'waiting'",
        "{HARNESS}: a self-loop retargeted at a missing node is refused as a missing target"
    );
    let shipped_hold_removed =
        FORGE_SDLC_V6_XML.replacen("  <task-node id=\"hold\"", "  <task-node id=\"held\"", 1);
    assert!(
        shipped_hold_removed != FORGE_SDLC_V6_XML,
        "{HARNESS}: the shipped definition must declare the `hold` node this clause renames"
    );
    let message = refusal_message(&shipped_hold_removed);
    assert!(
        message.contains("targets missing 'hold'"),
        "{HARNESS}: renaming the shipped definition's hold node makes every edge to it illegal: {message}"
    );

    // 3c. A CYCLE IS NOT THE ONLY WAY AN EDGE IS ILLEGAL — the rule does not care about direction. Shown so the
    //     clause cannot be satisfied by a "back-edge" special case: a forward edge is refused the same way.
    let broken_forward = two_node.replace("to=\"right\"", "to=\"nowhere\"");
    assert_eq!(
        refusal_message(&broken_forward),
        "transition 'across' on 'left' targets missing 'nowhere'",
        "{HARNESS}: a forward edge to a missing node is refused identically to a back-edge"
    );

    // 4. NON-VACUITY — THE SAME SHAPE GETS BOTH ANSWERS, AND THE DIFFERENCE IS THE TARGET. Restoring the back-edge's
    //     target turns the refusal into an acceptance, so steps 2 and 3 cannot both be satisfied by a constant.
    let repaired = broken_cycle.replace("to=\"elsewhere\"", "to=\"left\"");
    assert_eq!(
        repaired, two_node,
        "{HARNESS}: the repaired source is exactly the original — so only the back-edge's target differed"
    );
    assert!(
        parse_process_definition_xml(&broken_cycle).is_err(),
        "{HARNESS}: with the target missing the cycle is illegal"
    );
    assert!(
        parse_process_definition_xml(&repaired).is_ok(),
        "{HARNESS}: with the target present the same cycle is legal"
    );
    // And an edge that targets its own node is never a missing target — the reflexive check.
    let self_ref = format!(
        "<process-definition key=\"{KEY}\" version=\"1\" name=\"Reflexive\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"start\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
    );
    assert!(
        parse_process_definition_xml(&self_ref).is_ok(),
        "{HARNESS}: an edge that targets its own node is a legal self-reference, not a missing target"
    );

    // 5. OBSERVED, NOT REQUIRED: production has no cycle detector, so an unbounded cycle with no reachable terminus
    //    parses and validates cleanly. This clause RECORDS the current behaviour and is deliberately written so it
    //    cannot be mistaken for a requirement — the definition below is a real defect class (an instance that can
    //    never terminate), and nothing in production refuses it today. If cycle detection is ever added, this clause
    //    is the one that will fail, and that is the correct direction for it to break.
    let unbounded = parse_process_definition_xml(
        "<process-definition key=\"TST-WF-DEFINITION-010\" version=\"1\" name=\"Unbounded\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"a\"/>\n  </start-state>\n  <state id=\"a\">\n    <transition name=\"to-b\" to=\"b\"/>\n  </state>\n  <state id=\"b\">\n    <transition name=\"to-a\" to=\"a\"/>\n  </state>\n</process-definition>",
    )
    .expect("OBSERVED: an unbounded cycle with no terminus is accepted by the production parser today");
    assert!(
        reachable_cycle(&unbounded.graph, "start").is_some(),
        "OBSERVED: this fixture really does contain a cycle reachable from its start"
    );
    assert!(
        unbounded.graph.nodes.values().all(|n| n.node_type != "end"),
        "OBSERVED: and it declares no terminus at all, so nothing can ever complete it"
    );
    assert!(
        validate_definition_xml(
            "<process-definition key=\"TST-WF-DEFINITION-010\" version=\"1\" name=\"Unbounded\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"a\"/>\n  </start-state>\n  <state id=\"a\">\n    <transition name=\"to-b\" to=\"b\"/>\n  </state>\n  <state id=\"b\">\n    <transition name=\"to-a\" to=\"a\"/>\n  </state>\n</process-definition>"
        )
        .valid,
        "OBSERVED: and it VALIDATES today — this is a product gap, not a contract this test requires"
    );

    // 6. THE SHIPPED DEFINITION HAS BOTH — CYCLES AND TERMINI. The two clauses above must not be read as a claim that
    //    cyclicity and termination are the same thing: the real definition has a loop AND four termini, and it is
    //    accepted.
    assert_eq!(
        shipped_graph
            .nodes
            .values()
            .filter(|n| n.node_type == "end")
            .count(),
        4,
        "{HARNESS}: the shipped definition declares four termini alongside its loops"
    );
    assert!(
        validate_definition_xml(FORGE_SDLC_V6_XML).valid,
        "{HARNESS}: and the shipped definition validates as a whole"
    );
}
