//! ARCH-SEAM-008 — a Smith HOLD is a person's decision, never a route into QA.
//!
//! CONTRACT. In the shipped `FORGE_SDLC-v6.xml`, for every node the definition binds to the Smith service
//! (`forge.smith` — `smith`, `smith_split_work`, `repair_smith`, `fast_smith`, `fast_repair_smith` today):
//!
//! ```text
//! 1. the node declares a `hold` transition;
//! 2. that transition resolves to a node of the same definition;
//! 3. its destination is a HUMAN GATE — a task-node the definition binds no service to — so no service can be
//!    entered by it;
//! 4. the AUTOMATIC closure of that destination contains no QA node (a `forge.assay` task-node) and no
//!    implementer: an automatic path is one no person chose;
//! 5. a Smith-like node added to the definition later is inside this rail without a Rust edit, because the set is
//!    the definition's own `service="forge.smith"` binding.
//! ```
//!
//! WHY AT THE DEFINITION SEAM. The property is the definition's, so the assertion is about the definition. A
//! runtime rule ("a hold may not go to QA") would be a SECOND routing authority beside the XML — exactly what
//! `ARCH-SEAM-001`/`002`/`004` forbid, and it would only fail on a run that already paid for turns. Here a
//! definition that routes a hold into QA fails the rail, before anything runs.
//!
//! WHAT THIS RAIL DOES NOT RE-PROVE (proven where the behaviour lives): that a rejected candidate is held rather
//! than frozen (`forge_launch_intent__004__hold_prevents_smith_execution`, and the pre-freeze gate in Smith's own
//! judgement); that QA cannot own Git (`arch_boundary__011`, `forge_assay__005`); that DEV_OPS alone publishes
//! (`arch_boundary__012`); that QA judges the exact frozen SHA (`forge_release__002`, `forge_release__003`). This
//! file closes the one step those cannot: that the definition cannot reach QA from a hold at all.
//!
//! The definition is read through the readers the RUNTIME uses (`forge::engine::service_binding`,
//! `forge::engine::xml`), and the constant under test is the file the binary embeds — so this is asserted of the
//! definition that executes, not of a copy of it.
//!
//! Level: L0 — the shipped definition, its own binding readers, and nothing else. No engine, no turn, no vendor
//! edge.

use std::collections::BTreeSet;

use forge::engine::role_mapping::LaneId;
use forge::engine::service_binding::{
    forge_human_gate_nodes, forge_service_bindings, is_human_gate, lane_for_node,
    nodes_for_service, service_for_node,
};
use forge::engine::xml::{definition_from_xml, FORGE_SDLC_V6_XML};
use test_harness::source::{self, read};
use workflow::ProcessDefinition;

/// The definition the runtime executes: the shipped XML, parsed by the production parser.
fn shipped_definition() -> ProcessDefinition {
    definition_from_xml(FORGE_SDLC_V6_XML)
        .expect("the shipped FORGE_SDLC-v6.xml parses — the definition the binary executes")
}

/// The nodes the definition binds to one service key, read through the production binding.
fn nodes_of_service(key: &str) -> BTreeSet<String> {
    nodes_for_service(key)
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// The implementer nodes: every node whose XML `service` is the Smith lane's own key.
fn smith_nodes() -> BTreeSet<String> {
    nodes_of_service(LaneId::Smith.service_key())
}

/// The QA nodes: every node whose XML `service` is the Assay lane's own key.
fn qa_nodes() -> BTreeSet<String> {
    nodes_of_service(LaneId::Assay.service_key())
}

/// The nodes the definition leaves without a service — the human gates a person decides.
fn gates() -> BTreeSet<String> {
    forge_human_gate_nodes()
        .expect("the definition declares its human gates")
        .clone()
}

/// Every node an automatic step can leave `id` for, exactly as the definition declares the edges.
fn successors(definition: &ProcessDefinition, id: &str) -> Result<BTreeSet<String>, String> {
    let node = definition
        .definition
        .nodes
        .get(id)
        .ok_or_else(|| format!("{id}: the definition has no such node"))?;
    let mut out: BTreeSet<String> = node
        .transitions
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|transition| transition.to.clone())
        .collect();
    // A `<dynamic-fork>` splits into live instances of `branch-node` and rejoins at `join`: two structural edges
    // the definition declares without a `<transition>` (`split_dispatch` → `smith_split_work` → `split_join`).
    out.extend(node.branch_node.iter().cloned());
    out.extend(node.join.iter().cloned());
    for target in &out {
        if !definition.definition.nodes.contains_key(target) {
            return Err(format!("{id} → {target}: the definition has no such node"));
        }
    }
    Ok(out)
}

/// The nodes an AUTOMATIC path reaches from `start`, `start` included.
///
/// Automatic means no person chose the step: the walk follows the definition's own edges and STOPS AT a human
/// gate — the task-nodes the XML leaves without a service. Their outgoing edges exist, but a person picks one, so
/// they are a choice rather than a route anything can take on its own.
fn automatic_closure(
    definition: &ProcessDefinition,
    gates: &BTreeSet<String>,
    start: &str,
) -> Result<BTreeSet<String>, String> {
    let mut reached = BTreeSet::new();
    let mut frontier = vec![start.to_string()];
    while let Some(id) = frontier.pop() {
        if !reached.insert(id.clone()) {
            continue;
        }
        if gates.contains(&id) {
            continue;
        }
        frontier.extend(successors(definition, &id)?);
    }
    Ok(reached)
}

/// The walk, with the definition's own message if it names an edge that resolves to nothing.
fn walk(definition: &ProcessDefinition, gates: &BTreeSet<String>, start: &str) -> BTreeSet<String> {
    automatic_closure(definition, gates, start).unwrap_or_else(|why| panic!("{why}"))
}

/// The node a node's own `hold` exit names — or why it has no single answer.
fn hold_exit(definition: &ProcessDefinition, id: &str) -> Result<String, String> {
    let node = definition
        .definition
        .nodes
        .get(id)
        .ok_or_else(|| format!("{id}: the definition has no such node"))?;
    let holds: Vec<String> = node
        .transitions
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|transition| transition.name == "hold")
        .map(|transition| transition.to.clone())
        .collect();
    match holds.len() {
        1 => Ok(holds.into_iter().next().expect("exactly one")),
        0 => Err(format!(
            "{id}: declares no `hold` transition, so an implementer that will not freeze has no exit a person \
             decides"
        )),
        count => Err(format!(
            "{id}: declares {count} `hold` transitions {holds:?} — which one is a person's is not the \
             definition's answer"
        )),
    }
}

/// Every edge that is not the `hold` exit: the transitions an implementer takes when the work is done or bounced.
fn non_hold_exits(definition: &ProcessDefinition, id: &str) -> Vec<(String, String)> {
    definition
        .definition
        .nodes
        .get(id)
        .and_then(|node| node.transitions.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|transition| transition.name != "hold")
        .map(|transition| (transition.name, transition.to))
        .collect()
}

#[test]
fn every_smith_node_holds_to_a_human_gate_and_never_straight_to_qa() {
    let definition = shipped_definition();
    let gates = gates();
    let smith = smith_nodes();
    let qa = qa_nodes();

    // Without these, every assertion below passes by iterating nothing.
    assert!(
        smith.len() >= 5,
        "the definition binds {} node(s) to {}; the definition of record binds five (smith, smith_split_work, \
         repair_smith, fast_smith, fast_repair_smith): {smith:?}",
        smith.len(),
        LaneId::Smith.service_key()
    );
    assert!(
        smith.contains("smith"),
        "the canonical implementer node must still be one of them: {smith:?}"
    );
    assert!(
        !qa.is_empty(),
        "the definition binds no node to {}; a hold can only be shown not to reach QA if QA exists",
        LaneId::Assay.service_key()
    );

    for node in &smith {
        let target = match hold_exit(&definition, node) {
            Ok(target) => target,
            Err(why) => panic!("{why}"),
        };
        assert!(
            gates.contains(&target),
            "{node}: hold → {target} is not a human gate. The gates are {gates:?}; a hold that lands on a service \
             is a route, not a decision"
        );
        assert!(
            !qa.contains(target.as_str()),
            "{node}: hold → {target} enters QA. A hold says the candidate was not frozen; sending it to QA asks \
             Assay to judge a candidate the implementer never stood behind"
        );
    }
}

#[test]
fn the_automatic_path_from_a_smith_hold_reaches_no_qa_node_and_no_implementer() {
    let definition = shipped_definition();
    let gates = gates();
    let smith = smith_nodes();
    let qa = qa_nodes();

    for node in &smith {
        let target = match hold_exit(&definition, node) {
            Ok(target) => target,
            Err(why) => panic!("{why}"),
        };
        let reached = walk(&definition, &gates, &target);

        let into_qa: Vec<&String> = reached.intersection(&qa).collect();
        assert!(
            into_qa.is_empty(),
            "{node}: hold → {target} reaches {into_qa:?} without a person choosing it ({} node(s) are reachable \
             automatically from {target})",
            reached.len()
        );

        let back_into_implementing: Vec<&String> = reached.intersection(&smith).collect();
        assert!(
            back_into_implementing.is_empty(),
            "{node}: hold → {target} leads automatically back to {back_into_implementing:?} ({} node(s) are \
             reachable automatically from {target}). Re-implementing a candidate nobody unfroze, without a human \
             decision, is the runaway the hold exists to stop",
            reached.len()
        );
    }
}

#[test]
fn every_edge_of_the_shipped_definition_resolves() {
    let definition = shipped_definition();
    assert!(
        definition
            .definition
            .nodes
            .contains_key(&definition.definition.start_node_id),
        "the parsed definition names a start node it does not have: {}",
        definition.definition.start_node_id
    );

    // A `hold` whose destination cannot be resolved, or a dynamic-fork branch that cannot, would make the rails
    // above pass by walking past the edge rather than along it.
    let mut broken = Vec::new();
    for id in definition.definition.nodes.keys() {
        if let Err(why) = successors(&definition, id) {
            broken.push(why);
        }
    }
    assert!(
        broken.is_empty(),
        "the definition has edges that resolve to nothing:\n{}",
        broken.join("\n")
    );
}

/// The walk above must be able to see QA where QA really is reachable, or the hold rail proves nothing.
#[test]
fn the_walk_sees_qa_where_a_smith_completion_really_reaches_it() {
    let definition = shipped_definition();
    let gates = gates();
    let qa = qa_nodes();

    let mut reaching = Vec::new();
    for node in smith_nodes() {
        for (name, to) in non_hold_exits(&definition, &node) {
            let reached = walk(&definition, &gates, &to);
            if !reached.is_disjoint(&qa) {
                reaching.push(format!("{node}.{name} → {to}"));
            }
        }
    }
    assert!(
        !reaching.is_empty(),
        "no Smith exit reaches a {} node, so the hold rail would pass even if the walk were blind to QA",
        LaneId::Assay.service_key()
    );
}

/// The definition under test is the shipped file, and the file is what the runtime embeds.
#[test]
fn the_definition_under_test_is_the_shipped_file_the_runtime_embeds() {
    // X-5: the shipped definition lives in the Forge checkout. No checkout here is a named skip.
    let Some(forge) = source::forge_root_or_skip() else {
        return;
    };
    let path = forge.join("forge/definitions/FORGE_SDLC-v6.xml");
    let on_disk = read(&path);
    assert!(!on_disk.is_empty(), "{} must be readable", path.display());
    assert_eq!(
        on_disk,
        FORGE_SDLC_V6_XML,
        "the file and the constant the runtime parses have diverged; every assertion in this file is about the \
         definition the binary executes, so they must be one document"
    );
    assert_eq!(
        shipped_definition().key,
        "FORGE_SDLC",
        "the parsed definition is the Forge SDLC definition"
    );
}

/// The sets this rail reasons over are the runtime's own reading of the XML, and a gate is never a lane node.
#[test]
fn the_gates_are_the_definitions_own_reading_and_never_a_lane_node() {
    let gates = gates();
    assert!(
        gates.len() >= 3,
        "the definition declares its human gates (hold, repair_requirements, fast_confirmation), found {}: {gates:?}",
        gates.len()
    );
    let bindings = forge_service_bindings().expect("the definition's service bindings parse");

    for gate in &gates {
        assert!(
            !bindings.contains_key(gate),
            "{gate}: a gate is a task-node the XML binds NO service to"
        );
        assert_eq!(
            service_for_node(gate).expect("the XML bindings parse"),
            None,
            "{gate}: the runtime reads no service for a gate"
        );
        assert!(
            lane_for_node(gate).is_err(),
            "{gate}: production must REFUSE to place a gate rather than default it to a lane — a gate run as an \
             agent is authority nobody granted"
        );
    }

    for node in &smith_nodes() {
        assert_eq!(
            lane_for_node(node).expect("a node the XML binds resolves to its lane"),
            LaneId::Smith,
            "{node}: the runtime's lane for a node the XML binds to {}",
            LaneId::Smith.service_key()
        );
        assert_eq!(
            service_for_node(node).expect("the XML bindings parse"),
            Some(LaneId::Smith.service_key()),
            "{node}: the XML key the runtime reads"
        );
        assert!(
            !is_human_gate(node),
            "{node}: an agent node must never be a gate — a spurious human HOLD is the failure class this \
             direction prevents"
        );
    }

    for node in &qa_nodes() {
        assert_eq!(
            lane_for_node(node).expect("a node the XML binds resolves to its lane"),
            LaneId::Assay,
            "{node}: the runtime's lane for a node the XML binds to {}",
            LaneId::Assay.service_key()
        );
    }
}
