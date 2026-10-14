//! WF.DEFINITION — invalid fork/join topology (TST-WF-DEFINITION-008).
//!
//! Contract: parsing a definition is not the same as being able to run it. `topology_from_graph` projects a parsed
//! graph into the shape the Forge runtime actually executes
//! (`forge/src/engine/topology.rs:27-62`) — node ids, the task/command shapes with their positions, the end ids, and
//! **the** dynamic fork — and `structural_problems` (`:64-116`) / `ensure_topology` (`:118-132`) state the invariants
//! `ForgeRuntime::from_store` demands before it will run anything. A definition that violates them cannot be
//! deployed into a running engine, whatever the parser accepted.
//!
//! The invariants are checked one fault at a time here, each produced by removing or renaming exactly one thing in
//! the shipped v6 topology, so a clause cannot be satisfied by a broader "something is wrong". The set is:
//!
//! - the key and version must be the ones the runtime asks for (`:66-77`);
//! - five required superset nodes and four required termini must be present (`:78-93`);
//! - there must be a dynamic SPLIT fork, and it must not also be a task/command (`:94-104`); and
//! - every task/command must carry a Forge position from `POSITIONS` (`:105-114`).
//!
//! The parse side of the same contract is pinned too: `fork`, `join` and `dynamic-fork` are real node kinds with
//! their own required attributes, and a `dynamic-fork` missing its `count-variable`, `branch-command-type` or `join`
//! is refused at parse time — before the topology layer ever sees it.
//!
//! **One honest note about reachability.** The "dynamic fork must not be a task/command" arm is unreachable through
//! `topology_from_graph`, because it derives the fork id from `node_type == "dynamic-fork"` and the tasks from
//! `node_type == "task" | "command"`, so no single node can be both. `ForgeSdlcTopology`'s fields are public, so
//! that arm is pinned here on a topology built directly rather than left untested.
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__008__invalid_fork_join_topology

use forge::engine::topology::{
    ensure_topology, structural_problems, topology_from_graph, ForgeSdlcTopology, POSITIONS,
};
use forge::engine::xml::{definition_from_xml, parse_process_definition_xml, FORGE_SDLC_V6_XML};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
/// The node ids the runtime requires as a superset (`forge/src/engine/topology.rs:78-88`).
const REQUIRED_NODES: &[&str] = &[
    "start",
    "classify_work",
    "execution_shape",
    "qa_result",
    "hold",
];
/// The terminus ids the runtime requires (`forge/src/engine/topology.rs:89-93`).
const REQUIRED_TERMINI: &[&str] = &["complete", "cancelled", "failed", "archive_research"];

/// Parse a definition that must be REFUSED and return the production message.
fn refusal_message(xml: &str) -> String {
    match parse_process_definition_xml(xml) {
        Ok(_) => panic!("{HARNESS}: expected a parse refusal — source:\n{xml}"),
        Err(err) => err.to_string(),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-008); the file and the assay use it.
fn wf_definition_008__invalid_fork_join_topology() {
    // 1. THE SHIPPED TOPOLOGY SATISFIES EVERY INVARIANT. This is the positive control and the baseline every fault
    //    below is subtracted from.
    let shipped = definition_from_xml(FORGE_SDLC_V6_XML).expect("the shipped definition parses");
    let topology = topology_from_graph(&shipped.key, shipped.version, &shipped.definition);
    assert!(
        structural_problems(&topology).is_empty(),
        "{HARNESS}: the shipped topology has no structural problems"
    );
    assert!(
        ensure_topology(&topology).is_ok(),
        "{HARNESS}: the runtime would accept the shipped topology"
    );
    for id in REQUIRED_NODES {
        assert!(
            topology.node_ids.contains(*id),
            "{HARNESS}: the shipped topology declares the required node '{id}'"
        );
    }
    for id in REQUIRED_TERMINI {
        assert!(
            topology.end_ids.contains(*id),
            "{HARNESS}: the shipped topology declares the terminus '{id}'"
        );
    }
    let fork_id = topology
        .dynamic_fork_id
        .clone()
        .expect("the shipped definition declares a dynamic fork");
    assert!(
        topology.node_ids.contains(&fork_id),
        "{HARNESS}: the dynamic fork '{fork_id}' is a declared node"
    );
    assert!(
        !topology.tasks.contains_key(&fork_id),
        "{HARNESS}: the dynamic fork is not also a task/command node"
    );
    // And every task/command carries a real Forge position — the invariant each task clause below attacks.
    assert!(
        !topology.tasks.is_empty(),
        "{HARNESS}: the shipped topology has task/command nodes"
    );
    for (id, task) in &topology.tasks {
        assert!(
            POSITIONS.contains(&task.responsibility.as_deref().unwrap_or("")),
            "{HARNESS}: node '{id}' carries the Forge position '{:?}'",
            task.responsibility
        );
    }

    // 2. THE PROJECTION IS A PROJECTION, NOT A COPY. The two `end`-kind facts and the dynamic fork are found by
    //    node type, and the task shape's position falls back to the first candidate group when no `responsibility`
    //    attribute was declared — both load-bearing for the faults below.
    let parsed =
        parse_process_definition_xml(FORGE_SDLC_V6_XML).expect("the shipped definition parses");
    assert_eq!(
        topology.key, parsed.key,
        "{HARNESS}: the projection carries the definition's own key"
    );
    assert_eq!(
        topology.version, parsed.version,
        "{HARNESS}: the projection carries the definition's own version"
    );
    assert_eq!(
        topology.node_ids.len(),
        parsed.graph.nodes.len(),
        "{HARNESS}: every declared node is in the projection's id set"
    );
    assert_eq!(
        topology.end_ids.len(),
        parsed
            .graph
            .nodes
            .values()
            .filter(|n| n.node_type == "end")
            .count(),
        "{HARNESS}: the terminus set is exactly the end-typed nodes"
    );
    // The candidate-group fallback: a task with candidate_groups but no responsibility still gets a position.
    let mut projected = topology.clone();
    let sample = projected
        .tasks
        .keys()
        .next()
        .cloned()
        .expect("a task exists");
    projected
        .tasks
        .get_mut(&sample)
        .expect("the sampled task exists")
        .responsibility = None;
    let problems = structural_problems(&projected);
    assert!(
        problems
            .iter()
            .any(|p| p.contains(&format!("node '{sample}' has responsibility '(none)'"))),
        "{HARNESS}: a task with no position is reported as having none — the check reads the projected value: {problems:?}"
    );

    // 3. FAULT BY FAULT — each clause removes or renames exactly one thing and demands the production message, so no
    //    clause can be satisfied by a broader complaint.
    for (label, broken, expected) in [
        (
            "key",
            {
                let mut t = topology.clone();
                t.key = "NOT_FORGE_SDLC".into();
                t
            },
            "FORGE_SDLC key is 'NOT_FORGE_SDLC', expected 'FORGE_SDLC'",
        ),
        (
            "version",
            {
                let mut t = topology.clone();
                t.version = 999;
                t
            },
            "FORGE_SDLC version is '999', expected 7",
        ),
        (
            "dynamic fork absent",
            {
                let mut t = topology.clone();
                t.dynamic_fork_id = None;
                t
            },
            "FORGE_SDLC must declare a dynamic SPLIT fork (split_dispatch)",
        ),
    ] {
        let problems = structural_problems(&broken);
        assert!(
            problems.iter().any(|p| p == expected),
            "{HARNESS}: a wrong {label} must be reported: {expected} — got {problems:?}"
        );
        let error = ensure_topology(&broken)
            .expect_err(&format!("{HARNESS}: a wrong {label} must be refused"));
        assert!(
            error.contains(expected),
            "{HARNESS}: ensure_topology carries the {label} fault: {error}"
        );
        assert!(
            error.starts_with("FORGE_SDLC topology invariant violated"),
            "{HARNESS}: the refusal is the production envelope: {error}"
        );
    }

    // 3b. EVERY REQUIRED NODE, ONE AT A TIME. Each is removed on its own, so a check that only looked at one node
    //     would fail the other four.
    for required in REQUIRED_NODES {
        let mut broken = topology.clone();
        broken.node_ids.remove(*required);
        let problems = structural_problems(&broken);
        assert!(
            problems
                .iter()
                .any(|p| p.contains(&format!("required superset node '{required}' missing"))),
            "{HARNESS}: removing '{required}' must be reported by name: {problems:?}"
        );
        // Reported EXACTLY ONCE. Several required ids are also tasks, so a checker that walked the task map for
        // node presence would report some of these twice — and an operator reading a doubled list cannot tell one
        // fault from two.
        let reports = problems
            .iter()
            .filter(|p| p.contains(&format!("required superset node '{required}' missing")))
            .count();
        assert_eq!(
            reports, 1,
            "{HARNESS}: the missing node '{required}' is one fault, not several: {problems:?}"
        );
    }
    // And each terminus, likewise — a terminus is checked by the END-id set, which is a different set from the node
    // ids, so removing an end node from `node_ids` alone must not satisfy the terminus requirement.
    for terminus in REQUIRED_TERMINI {
        let mut broken = topology.clone();
        broken.end_ids.remove(*terminus);
        let problems = structural_problems(&broken);
        assert!(
            problems
                .iter()
                .any(|p| p == &format!("required terminus '{terminus}' missing")),
            "{HARNESS}: removing the terminus '{terminus}' must be reported: {problems:?}"
        );
    }

    // 3c. THE TWO SETS ARE DISTINCT, AND A CHECK THAT READ THE WRONG ONE WOULD BE CAUGHT HERE. `hold` is a required
    //     NODE but not a terminus, and `complete` is a required TERMINUS — so moving a fault between the sets must
    //     change which message appears.
    let mut moved = topology.clone();
    moved.node_ids.remove("complete");
    let problems = structural_problems(&moved);
    assert!(
        !problems
            .iter()
            .any(|p| p.contains("required terminus 'complete' missing")),
        "{HARNESS}: removing a node id does not remove the terminus from the END set: {problems:?}"
    );
    let mut unended = topology.clone();
    unended.end_ids.remove("hold");
    let problems = structural_problems(&unended);
    assert!(
        !problems
            .iter()
            .any(|p| p.contains("required superset node 'hold' missing")),
        "{HARNESS}: removing an end id does not remove the node from the NODE set: {problems:?}"
    );

    // 3d. A POSITION OUTSIDE `POSITIONS` IS REFUSED, AND AN EMPTY ONE IS TOO. Every position the shipped topology
    //     actually uses is a legal one, so each illegal value here is new.
    for illegal in ["architect2", "SCOUT", "devops", "", " forge"] {
        let mut broken = topology.clone();
        let victim = broken.tasks.keys().next().cloned().expect("a task exists");
        broken
            .tasks
            .get_mut(&victim)
            .expect("the sampled task exists")
            .responsibility = if illegal.is_empty() {
            None
        } else {
            Some(illegal.into())
        };
        let problems = structural_problems(&broken);
        assert!(
            problems
                .iter()
                .any(|p| p.contains(&format!("node '{victim}' has responsibility"))),
            "{HARNESS}: responsibility '{illegal}' must be refused: {problems:?}"
        );
        assert!(
            ensure_topology(&broken).is_err(),
            "{HARNESS}: an out-of-set position must not pass ensure_topology"
        );
    }
    // And every legal position is accepted, so the check is against the inventory rather than a whitelist of one.
    for legal in POSITIONS {
        let mut ok = topology.clone();
        let victim = ok.tasks.keys().next().cloned().expect("a task exists");
        ok.tasks
            .get_mut(&victim)
            .expect("the sampled task exists")
            .responsibility = Some((*legal).into());
        assert!(
            structural_problems(&ok).is_empty(),
            "{HARNESS}: '{legal}' is a Forge position and must be accepted"
        );
    }

    // 4. THE DYNAMIC-FORK-MUST-NOT-BE-A-TASK ARM, PINNED ON A TOPOLOGY BUILT DIRECTLY. `topology_from_graph` cannot
    //    produce it (a node's single `node_type` decides both facts), so the arm is reached through the production
    //    type's own public fields — the same fields the runtime reads.
    let clashing_fork = ForgeSdlcTopology {
        key: topology.key.clone(),
        version: topology.version,
        node_ids: topology.node_ids.clone(),
        tasks: {
            let mut tasks = topology.tasks.clone();
            tasks.insert(
                fork_id.clone(),
                forge::engine::topology::ForgeTaskShape {
                    id: fork_id.clone(),
                    label: None,
                    responsibility: Some("smith".into()),
                },
            );
            tasks
        },
        end_ids: topology.end_ids.clone(),
        dynamic_fork_id: Some(fork_id.clone()),
    };
    let problems = structural_problems(&clashing_fork);
    assert!(
        problems
            .iter()
            .any(|p| p == &format!("dynamic-fork '{fork_id}' must not be a task/command node")),
        "{HARNESS}: a dynamic fork that is also a task must be reported: {problems:?}"
    );
    assert!(
        ensure_topology(&clashing_fork).is_err(),
        "{HARNESS}: that clash must be refused by ensure_topology"
    );

    // 5. MULTIPLE FAULTS ARE ALL REPORTED — the function is a collector, not a first-error stop. Without this, a
    //    caller could fix one problem at a time against a runtime that names only the first.
    let mut several = topology.clone();
    several.key = "WRONG".into();
    several.node_ids.remove("start");
    several.end_ids.remove("complete");
    several.dynamic_fork_id = None;
    let problems = structural_problems(&several);
    assert!(
        problems.len() >= 4,
        "{HARNESS}: four independent faults must all be reported, got {problems:?}"
    );
    for expected in [
        "FORGE_SDLC key is 'WRONG', expected 'FORGE_SDLC'",
        "required superset node 'start' missing",
        "required terminus 'complete' missing",
        "FORGE_SDLC must declare a dynamic SPLIT fork (split_dispatch)",
    ] {
        assert!(
            problems.iter().any(|p| p == expected),
            "{HARNESS}: the collector must include '{expected}': {problems:?}"
        );
    }
    assert_eq!(
        problems.len(),
        structural_problems(&several).len(),
        "{HARNESS}: the fault count is stable for one input — the projection is pure"
    );

    // 6. THE PARSE SIDE: FORK, JOIN AND DYNAMIC-FORK ARE REAL KINDS WITH REQUIRED ATTRIBUTES. A topology cannot be
    //    built from a dynamic fork the parser refused, so these refusals are the same contract one layer earlier.
    let fork_source = parse_process_definition_xml(&format!(
        "<process-definition key=\"fork-join-kinds-under-test\" version=\"1\" name=\"Kinds\">\n  <fork id=\"fan\">\n    <transition name=\"left\" to=\"waits\"/>\n  </fork>\n  <join id=\"joined\">\n    <transition name=\"join-go\" to=\"waits\"/>\n  </join>\n  <timer id=\"waits\" on-fire=\"done\"/>\n  <dynamic-fork id=\"split\" count-variable=\"n\" plan-variable=\"p\" branch-command-type=\"forge.launch_builder\" branch-node=\"work\" join=\"joined\" minimum=\"2\" maximum=\"8\">\n    <transition name=\"split-go\" to=\"done\"/>\n  </dynamic-fork>\n  <task-node id=\"work\" responsibility=\"smith\">\n    <transition name=\"finish\" to=\"joined\"/>\n  </task-node>\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"fan\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
    ))
    .expect("a definition declaring fork, join and dynamic-fork parses");
    assert_eq!(
        fork_source.graph.nodes["fan"].node_type, "fork",
        "{HARNESS}: <fork> reads as a fork node"
    );
    assert_eq!(
        fork_source.graph.nodes["joined"].node_type, "join",
        "{HARNESS}: <join> reads as a join node"
    );
    assert_eq!(
        fork_source.graph.nodes["split"].node_type, "dynamic-fork",
        "{HARNESS}: <dynamic-fork> reads as a dynamic-fork node"
    );
    assert_eq!(
        fork_source.graph.nodes["split"].join.as_deref(),
        Some("joined"),
        "{HARNESS}: the dynamic fork records which join it rejoins at — the fork/join pairing is in the parse"
    );
    // A dynamic fork missing any of its three required attributes is refused, so a half-declared split can never
    // reach the topology layer.
    for (missing, attribute) in [
        (
            "<dynamic-fork id=\"split\" plan-variable=\"p\" branch-command-type=\"forge.launch_builder\" join=\"joined\"/>",
            "count-variable",
        ),
        (
            "<dynamic-fork id=\"split\" count-variable=\"n\" plan-variable=\"p\" join=\"joined\"/>",
            "branch-command-type",
        ),
        (
            "<dynamic-fork id=\"split\" count-variable=\"n\" plan-variable=\"p\" branch-command-type=\"forge.launch_builder\"/>",
            "join",
        ),
    ] {
        let message = refusal_message(&format!(
            "<process-definition key=\"fork-join-kinds-under-test\" version=\"1\" name=\"Broken split\">\n  {missing}\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
        ));
        assert!(
            message.contains(&format!("missing {attribute}")),
            "{HARNESS}: a dynamic fork without {attribute} is refused by name: {message}"
        );
    }
    // A JOIN THAT NAMED NO DYNAMIC FORK IS FINE — joins are ordinary nodes, and only the DYNAMIC fork is required by
    // the FORGE_SDLC invariant. A parser that demanded a fork beside every join would refuse honest definitions.
    let forkless = definition_from_xml(
        "<process-definition key=\"forkless-under-test\" version=\"1\" name=\"No fork\">\n  <join id=\"joined\">\n    <transition name=\"go\" to=\"done\"/>\n  </join>\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"joined\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>",
    )
    .expect("a definition with a join but no dynamic fork parses");
    let forkless_topology =
        topology_from_graph(&forkless.key, forkless.version, &forkless.definition);
    assert!(
        forkless_topology.dynamic_fork_id.is_none(),
        "{HARNESS}: no dynamic fork means the projection records none"
    );
    let problems = structural_problems(&forkless_topology);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("must declare a dynamic SPLIT fork")),
        "{HARNESS}: the FORGE_SDLC invariant still demands one, even though the parse accepted the definition: {problems:?}"
    );

    // 7. NON-VACUITY. The shipped topology passes and the same topology minus one node does not, so the projection
    //    and the checker cannot both be constant.
    assert!(
        structural_problems(&topology).is_empty(),
        "{HARNESS}: the shipped topology passes"
    );
    let mut one_short = topology.clone();
    one_short.node_ids.remove("hold");
    assert!(
        !structural_problems(&one_short).is_empty(),
        "{HARNESS}: the same topology minus one node does not pass"
    );
}
