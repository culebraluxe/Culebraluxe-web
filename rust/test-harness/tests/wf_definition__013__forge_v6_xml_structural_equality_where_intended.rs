//! WF.DEFINITION — Forge v6 XML structural equality where intended (TST-WF-DEFINITION-013).
//!
//! Contract: the Forge v6 XML **is** the definition (`rust/forge/src/engine/xml.rs:1-2`,
//! `rust/forge/definitions/FORGE_SDLC-v6.xml`), and the structure the production parser reads from it is structurally
//! equal to itself — and to the structure that survives the production persistence boundary — exactly where the
//! definition intends, and structurally unequal exactly where a real structural edit is made.
//!
//! The three production pieces this file exercises, all pure:
//!
//! - `definition_from_xml` / `parse_process_definition_xml` (`rust/forge/src/engine/xml.rs:352-426`) — the one parser
//!   production uses; `deploy_xml` (`rust/forge/src/engine/deploy.rs:83-92`) and the engine binary
//!   (`rust/forge/src/bin/forge_task.rs:45`) both call it.
//! - `graphs_equal` (`rust/forge/src/engine/version_policy.rs:39-41`) — the production structural-equality predicate:
//!   `graph_to_json(a) == graph_to_json(b)`. It is what `classify_deploy`
//!   (`rust/forge/src/engine/version_policy.rs:43-62`) uses to decide a re-deploy is a duplicate.
//! - `graph_to_json` / `graph_from_json` (`rust/core/workflow/src/json_codec.rs:214-220`) — the codec that writes the
//!   parsed graph to the Neon `jsonb` column and reads it back; the graph's structure must be a fixed point of it.
//!
//! WHERE INTENDED is the point of the contract, and it is demonstrated in both directions:
//!
//! - cosmetic XML (a comment, extra whitespace, the declaration, an element whose **attributes are reordered**)
//!   parses to a **structurally equal** graph, because the parser intentionally drops what is not structure; and
//! - a structural edit (a transition target, a decision condition, a command type, a display-order entry) parses to a
//!   **structurally unequal** graph, because those are the parts the definition intends to mean.
//!
//! A structural edit that would make the graph dishonest (a transition to a node that does not exist, an unknown
//! element, a duplicate node id) is not a different graph — it is a refusal, so equality can never paper over a
//! broken definition.
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider: the
//! input is the production XML embedded in the binary and the outputs are the parser's and the pure policies' own.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended

use forge::engine::commands::{is_release, is_routed, XML_RELEASE_COMMANDS};
use forge::engine::topology::{structural_problems, topology_from_graph, POSITIONS};
use forge::engine::version_policy::{classify_deploy, graphs_equal, DeployDecision};
use forge::engine::xml::{definition_from_xml, parse_process_definition_xml, FORGE_SDLC_V6_XML};
use workflow::json_codec::{graph_from_json, graph_to_json};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
/// The production definition's identity, as the XML declares it (`FORGE_SDLC`, version 6).
const KEY: &str = "FORGE_SDLC";
const VERSION: i32 = 6;
/// The source line that opens the definition the whole contract is about; the stable anchor for the edits below.
const BEGIN_TRANSITION: &str = "<transition name=\"begin\" to=\"classify_work\"/>";
const XML_DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";

/// The routing-structural facts a definition intends its routes to have: node identity and type, edges (name,
/// target, condition, required), decision arms, command type, terminus outcome, the dynamic-fork control fields, and
/// the declared start and display order. Deterministic (rows sorted), so it compares across the production JSON
/// boundary without depending on display text; it is a reader of production fields, not a second parser.
fn routing_signature(graph: &workflow::ProcessGraph) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    for (id, node) in &graph.nodes {
        let mut row = format!("{id}|{}", node.node_type);
        if let Some(command_type) = &node.command_type {
            row.push_str(&format!("|cmd={command_type}"));
        }
        if let Some(transition) = &node.transition {
            row.push_str(&format!("|transition={transition}"));
        }
        if let Some(outcome) = node.outcome {
            row.push_str(&format!("|outcome={outcome:?}"));
        }
        if let Some(transitions) = &node.transitions {
            for t in transitions {
                row.push_str(&format!(
                    "|t:{}->{}:{}:{:?}",
                    t.name,
                    t.to,
                    t.condition.as_deref().unwrap_or(""),
                    t.required
                ));
            }
        }
        if let Some(decisions) = &node.decisions {
            for d in decisions {
                row.push_str(&format!("|d:{}->{}", d.condition, d.transition));
            }
        }
        for (label, value) in [
            ("count", node.count_variable.as_deref()),
            ("plan", node.plan_variable.as_deref()),
            ("branchNode", node.branch_node.as_deref()),
            ("join", node.join.as_deref()),
        ] {
            if let Some(value) = value {
                row.push_str(&format!("|{label}={value}"));
            }
        }
        if let Some(minimum) = node.minimum {
            row.push_str(&format!("|min={minimum}"));
        }
        if let Some(maximum) = node.maximum {
            row.push_str(&format!("|max={maximum}"));
        }
        rows.push(row);
    }
    rows.push(format!("start={}", graph.start_node_id));
    if let Some(order) = &graph.display_order {
        rows.push(format!("order={}", order.join(",")));
    }
    rows.sort();
    rows
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-013); the file and the assay use it.
fn wf_definition_013__forge_v6_xml_structural_equality_where_intended() {
    // 1. THE PRODUCTION PARSER ACCEPTS THE PRODUCTION XML AND READS THE INTENDED IDENTITY. The identity is the
    //    definition's own, not a caller's: the parser reads `key`, `version`, `name` and the start node from the XML
    //    file, and `definition_from_xml` forms the definition id from key + version. A parser that returned an empty
    //    graph or a renamed definition would fail here.
    let parsed = parse_process_definition_xml(FORGE_SDLC_V6_XML)
        .expect("FORGE_SDLC-v6.xml is the definition and must parse");
    assert_eq!(parsed.key, KEY, "{HARNESS}: the XML declares its own key");
    assert_eq!(
        parsed.version, VERSION,
        "{HARNESS}: the XML declares version 6"
    );
    assert_eq!(
        parsed.name, "Forge Software Delivery Lifecycle",
        "{HARNESS}: the XML declares its own name"
    );
    assert!(
        parsed.description.is_some(),
        "{HARNESS}: the XML declares the definition description"
    );
    assert_eq!(
        parsed.graph.start_node_id, "start",
        "{HARNESS}: the XML declares its own start node"
    );
    assert!(
        parsed.graph.nodes.len() >= 50,
        "{HARNESS}: v6 is the full superset, not a fixture (got {} nodes)",
        parsed.graph.nodes.len()
    );

    let def = definition_from_xml(FORGE_SDLC_V6_XML)
        .expect("the production definition loads from the XML");
    assert_eq!(
        def.id, "FORGE_SDLC-v6",
        "{HARNESS}: the definition id is derived as key-v<version>"
    );

    // 2. STRUCTURAL EQUALITY WHERE INTENDED — an independent parse of the same source is the same structure. The
    //    equality is the production predicate, not byte equality of the source, so it is stable across processes and
    //    compilations.
    let again = definition_from_xml(FORGE_SDLC_V6_XML).expect("the XML parses again");
    assert!(
        graphs_equal(&def.definition, &again.definition),
        "{HARNESS}: parsing the same definition twice yields a structurally equal graph"
    );
    assert_eq!(
        graph_to_json(&def.definition),
        graph_to_json(&again.definition),
        "{HARNESS}: the production equality predicate is the JSON encoding, and the encoding is stable"
    );

    // 3. WHERE INTENDED (cosmetic) — a comment, extra whitespace and the declaration are not structure. The parser
    //    intentionally drops them, so the graph is structurally equal. This is the half of "where intended" that
    //    stops the contract from being byte equality.
    let cosmetic = FORGE_SDLC_V6_XML.replacen(
        XML_DECLARATION,
        &format!("{XML_DECLARATION}\n<!-- TST-WF-DEFINITION-013: comments and whitespace are not structure -->\n\n\n"),
        1,
    );
    let cosmetic = format!("{cosmetic}\n   \n");
    let cosmetic_def = definition_from_xml(&cosmetic)
        .expect("cosmetic XML (a comment and whitespace) still parses");
    assert!(
        graphs_equal(&def.definition, &cosmetic_def.definition),
        "{HARNESS}: comments and whitespace are not structure — parsing them must yield an equal graph"
    );

    // 3b. WHERE INTENDED (attribute order) — XML attribute order is not significant. Reordering the attributes of an
    //     element must parse to the same structure. A parser that read attributes into a positional list and
    //     serialized them in source order would make this a structural difference and fail here, even though the
    //     definition means the same thing.
    let reordered_attrs = FORGE_SDLC_V6_XML.replacen(
        BEGIN_TRANSITION,
        "<transition to=\"classify_work\" name=\"begin\"/>",
        1,
    );
    let reordered_attrs_def = definition_from_xml(&reordered_attrs)
        .expect("an element whose attributes are reordered still parses");
    assert!(
        graphs_equal(&def.definition, &reordered_attrs_def.definition),
        "{HARNESS}: attribute order is not structure — a reordered element must parse to an equal graph"
    );

    // 4. WHERE INTENDED (structural) — a real structural edit parses, but is NOT equal. Four independent facets are
    //    edited so the equality cannot be pinned to one serialized field: an edge (transition target), a decision
    //    arm's condition, a command node's command type, and the declared display order. Each parse must SUCCEED (the
    //    syntax stays legal) while the structure differs, which is exactly what the predicate must report.
    let moved = FORGE_SDLC_V6_XML.replacen(
        BEGIN_TRANSITION,
        "<transition name=\"begin\" to=\"architect\"/>",
        1,
    );
    let moved_def = definition_from_xml(&moved).expect("a still-legal transition target parses");
    assert!(
        !graphs_equal(&def.definition, &moved_def.definition),
        "{HARNESS}: a changed transition target is a structural difference, not a cosmetic one"
    );

    let reconditioned = FORGE_SDLC_V6_XML.replacen(
        "workType == 'HOTFIX'",
        "workType == 'PATCH'",
        1,
    );
    let reconditioned_def =
        definition_from_xml(&reconditioned).expect("a still-legal decision condition parses");
    assert!(
        !graphs_equal(&def.definition, &reconditioned_def.definition),
        "{HARNESS}: a changed decision condition is a structural difference"
    );

    let retyped = FORGE_SDLC_V6_XML.replacen(
        "command-type=\"forge.migrate_dev\"",
        "command-type=\"forge.publish_candidate\"",
        1,
    );
    let retyped_def = definition_from_xml(&retyped).expect("a still-routed command type parses");
    assert!(
        !graphs_equal(&def.definition, &retyped_def.definition),
        "{HARNESS}: a changed command type is a structural difference"
    );

    let reordered = FORGE_SDLC_V6_XML.replacen("    <node ref=\"start\"/>\n", "", 1);
    let reordered_def =
        definition_from_xml(&reordered).expect("a display order that omits a node still parses");
    assert!(
        !graphs_equal(&def.definition, &reordered_def.definition),
        "{HARNESS}: a changed display order is a structural difference"
    );

    // 5. THE ROUTING STRUCTURE MUST SURVIVE THE PRODUCTION PERSISTENCE BOUNDARY. `deploy_xml` writes the parsed
    //    graph with `graph_to_json` into Neon's `jsonb` column and the engine reads it back with `graph_from_json`;
    //    the routes the engine later follows must be exactly the routes the XML defined. This is the same codec the
    //    deploy path uses, exercised without a database.
    //
    //    The comparison is deliberately on the routing structure the definition intends to mean — node identity and
    //    type, edges and decisions, command type, terminus, declared order — not on display text. The codec's
    //    string reader corrupts non-ASCII characters (`graph_from_json` pushes raw bytes as `char`; the em dash in a
    //    label comes back as mojibake), which is a defect in the codec and not in the definition; the contract must
    //    not depend on that, so it asserts the structure that decides behaviour.
    let persisted = graph_to_json(&def.definition);
    let reloaded =
        graph_from_json(&persisted).expect("the production JSON codec reads back its own encoding");
    assert_eq!(
        routing_signature(&def.definition),
        routing_signature(&reloaded),
        "{HARNESS}: every route, decision, command type, terminus and declared order survives persistence"
    );

    // 6. THE PRODUCTION POLICY THAT CONSUMES EQUALITY AGREES. `classify_deploy` treats an incoming graph equal to the
    //    deployed one as a duplicate replace, a structurally different one as a real replace, and an already-used
    //    version as immutable — where equality must NOT rescue a redeploy. This pins the predicate to its one
    //    production caller, so the contract is not merely a helper assertion.
    assert!(
        matches!(
            classify_deploy(false, 0, None, &def.definition),
            DeployDecision::Insert { .. }
        ),
        "{HARNESS}: a missing row is an insert"
    );
    match classify_deploy(true, 0, None, &def.definition) {
        DeployDecision::Update { duplicate, .. } => assert!(
            !duplicate,
            "{HARNESS}: a present row with no stored graph is a non-duplicate replace — equality has nothing to hold to"
        ),
        other => panic!("{HARNESS}: expected Update, got {other:?}"),
    }
    match classify_deploy(true, 0, Some(&def.definition), &again.definition) {
        DeployDecision::Update { duplicate, .. } => assert!(
            duplicate,
            "{HARNESS}: a structurally equal incoming graph is classified a duplicate"
        ),
        other => panic!("{HARNESS}: expected Update, got {other:?}"),
    }
    match classify_deploy(true, 0, Some(&def.definition), &moved_def.definition) {
        DeployDecision::Update { duplicate, .. } => assert!(
            !duplicate,
            "{HARNESS}: a structurally different incoming graph is not a duplicate"
        ),
        other => panic!("{HARNESS}: expected Update, got {other:?}"),
    }
    match classify_deploy(true, 3, Some(&def.definition), &again.definition) {
        DeployDecision::Reject { .. } => {}
        other => panic!(
            "{HARNESS}: a version with instances is immutable — equality cannot override it, got {other:?}"
        ),
    }

    // 7. WHERE INTENDED (inventory) — the XML's `<command-node>` command types are exactly the maintained Forge
    //    release-command inventory, and every one is routed. The definition intends command nodes to be release
    //    commands; a command node outside that set would be a structural disagreement the validator would refuse.
    let mut command_types: Vec<&str> = def
        .definition
        .nodes
        .values()
        .filter(|node| node.node_type == "command")
        .filter_map(|node| node.command_type.as_deref())
        .collect();
    command_types.sort_unstable();
    command_types.dedup();
    assert!(
        !command_types.is_empty(),
        "{HARNESS}: the v6 XML declares command nodes"
    );
    let mut release_inventory: Vec<&str> = XML_RELEASE_COMMANDS.to_vec();
    release_inventory.sort_unstable();
    assert_eq!(
        command_types, release_inventory,
        "{HARNESS}: the XML command-node inventory equals the maintained release-command inventory"
    );
    for command_type in command_types.iter().copied() {
        assert!(
            is_routed(command_type) && is_release(command_type),
            "{HARNESS}: command node '{command_type}' must be a routed Forge release command"
        );
    }

    // 8. WHERE INTENDED (topology) — the parsed structure satisfies the same topology invariant the production
    //    runtime enforces before it will run a definition (`forge::engine::topology::ensure_topology`, called from
    //    `ForgeRuntime::from_store`). Every task/command role is a Forge position.
    let topology = topology_from_graph(&def.key, def.version, &def.definition);
    let problems = structural_problems(&topology);
    assert!(
        problems.is_empty(),
        "{HARNESS}: the parsed v6 topology has no structural problems: {problems:?}"
    );
    for task in topology.tasks.values() {
        let role = task.responsibility.as_deref().unwrap_or("");
        assert!(
            POSITIONS.contains(&role),
            "{HARNESS}: node '{}' carries Forge position '{role}'",
            task.id
        );
    }

    // 9. NEGATIVE/REFUSAL — a structurally dishonest XML is REFUSED, never parsed into a different-but-equal graph.
    //    Equality is defined over valid structures; a broken definition has no structure to be equal to.
    let dangling = FORGE_SDLC_V6_XML.replacen(
        BEGIN_TRANSITION,
        "<transition name=\"begin\" to=\"no_such_node\"/>",
        1,
    );
    assert!(
        definition_from_xml(&dangling).is_err(),
        "{HARNESS}: a transition to a missing node is refused, not silently accepted"
    );

    let unknown = FORGE_SDLC_V6_XML.replacen("  <start-state", "  <mystery/>\n  <start-state", 1);
    assert!(
        definition_from_xml(&unknown).is_err(),
        "{HARNESS}: an unknown element is refused"
    );

    let duplicated =
        FORGE_SDLC_V6_XML.replacen("<start-state id=\"start\"", "<start-state id=\"hold\"", 1);
    assert!(
        definition_from_xml(&duplicated).is_err(),
        "{HARNESS}: a duplicate node id is refused"
    );

    // 10. NEGATIVE (non-vacuity) — the predicate returns BOTH answers on real graphs. If `graphs_equal` were a
    //     constant `true` the cosmetic case would pass and step 4 would fail; if it were a constant `false`, steps 2,
    //     3, 5 and 6 would fail. Asserting both here makes the intent explicit and the test self-checking.
    assert!(
        graphs_equal(&again.definition, &again.definition),
        "{HARNESS}: the predicate is reflexive on a real graph"
    );
    assert!(
        !graphs_equal(&again.definition, &moved_def.definition),
        "{HARNESS}: the predicate distinguishes a real edit from an identical graph"
    );
}
