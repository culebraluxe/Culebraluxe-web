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
//! - cosmetic XML (a comment — before the root or between nodes, extra whitespace, the declaration, the explicit
//!   `<x></x>` form of a self-closing tag, an element whose **attributes are reordered**, and an end-state that
//!   declares the parser's default `outcome="completed"`) parses to a **structurally equal** graph, because the
//!   parser intentionally drops what is not structure; and
//! - a structural edit (a transition target or its condition/required flag, a decision condition or its
//!   `refresh-facts`, a command type, a task `priority`, a task's `form-key`/`label`/`description`, an end-state
//!   `outcome`, a dynamic-fork `count-variable`/`plan-variable`/`branch-node`/`minimum`/`maximum`, a display-order
//!   entry **or the order of two display-order entries**) parses to a **structurally unequal** graph, because those
//!   are the parts the definition intends to mean.
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
        if let Some(form_key) = &node.form_key {
            row.push_str(&format!("|formKey={form_key}"));
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
        if let Some(refresh) = node.refresh_facts {
            row.push_str(&format!("|refresh={refresh}"));
        }
        if let Some(priority) = node.priority {
            row.push_str(&format!("|priority={priority}"));
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

    // 3c. WHERE INTENDED (comments anywhere) — a comment between the root's child elements is not structure. The
    //     parser drops comments inside an element's children as well as before the root, so a comment sitting next
    //     to the definition's nodes must parse to the same graph. A parser that lost the nested-comment path would
    //     refuse this source outright; one that kept it as a pseudo-child would make it a structural difference.
    let nested_comment = FORGE_SDLC_V6_XML.replacen(
        "  <start-state",
        "  <!-- TST-WF-DEFINITION-013: a comment between nodes is not structure -->\n  <start-state",
        1,
    );
    let nested_comment_def = definition_from_xml(&nested_comment)
        .expect("a comment between the root's children still parses");
    assert!(
        graphs_equal(&def.definition, &nested_comment_def.definition),
        "{HARNESS}: a comment between nodes is not structure — parsing it must yield an equal graph"
    );

    // 3d. WHERE INTENDED (element form) — `<x/>` and `<x></x>` are the same XML element. An element written with an
    //     explicit open/close pair must parse to the same structure as its self-closing form. A parser that kept the
    //     two apart (or fabricated a child from the empty body) would be a structural lie the predicate reports.
    let explicit_close = FORGE_SDLC_V6_XML.replacen(
        BEGIN_TRANSITION,
        "<transition name=\"begin\" to=\"classify_work\"></transition>",
        1,
    );
    let explicit_close_def = definition_from_xml(&explicit_close)
        .expect("an explicitly-closed element still parses");
    assert!(
        graphs_equal(&def.definition, &explicit_close_def.definition),
        "{HARNESS}: `<x/>` and `<x></x>` are the same element — the explicit form must parse to an equal graph"
    );

    // 3e. WHERE INTENDED (attribute quoting) — XML lets an attribute value be quoted with single or double quotes,
    //     and both name the same value. An element written with single-quoted attributes must parse to the same
    //     structure; a parser that understood only `"` would refuse the definition outright.
    let single_quoted = FORGE_SDLC_V6_XML.replacen("version=\"6\"", "version='6'", 1);
    let single_quoted_def =
        definition_from_xml(&single_quoted).expect("single-quoted attributes still parse");
    assert!(
        graphs_equal(&def.definition, &single_quoted_def.definition),
        "{HARNESS}: attribute quoting style is not structure — `'` and `\"` name the same value"
    );

    // 3f. WHERE INTENDED (declaration order) — the order the node elements are declared in the source is not
    //     structure: `display-order` is the definition's explicit order, and the parser keys nodes by id, so the
    //     canonical JSON encoding must be independent of declaration order. Swapping two adjacent node elements
    //     must parse to a structurally equal graph; an encoding that followed insertion order would report a
    //     difference the definition does not mean.
    let (first, second) = (
        FORGE_SDLC_V6_XML
            .find("<task-node id=\"fast_smith\"")
            .expect("the definition declares fast_smith"),
        FORGE_SDLC_V6_XML
            .find("<task-node id=\"fast_qa_verify\"")
            .expect("the definition declares fast_qa_verify"),
    );
    let first_end = FORGE_SDLC_V6_XML[first..]
        .find("</task-node>")
        .expect("fast_smith is closed")
        + first
        + "</task-node>".len();
    let second_end = FORGE_SDLC_V6_XML[second..]
        .find("</task-node>")
        .expect("fast_qa_verify is closed")
        + second
        + "</task-node>".len();
    let swapped = format!(
        "{}{}{}{}{}",
        &FORGE_SDLC_V6_XML[..first],
        &FORGE_SDLC_V6_XML[second..second_end],
        &FORGE_SDLC_V6_XML[first_end..second],
        &FORGE_SDLC_V6_XML[first..first_end],
        &FORGE_SDLC_V6_XML[second_end..],
    );
    let swapped_def =
        definition_from_xml(&swapped).expect("swapping two adjacent node elements still parses");
    assert!(
        graphs_equal(&def.definition, &swapped_def.definition),
        "{HARNESS}: declaration order is not structure — the canonical encoding is keyed by node id"
    );

    // 3g. WHERE INTENDED (entity references) — an XML entity reference names the same value as the character it
    //     stands for, so `&apos;` inside an attribute value is the same structure as a literal `'`. The production
    //     parser decodes `&amp; &lt; &gt; &quot; &apos;` (`rust/forge/src/engine/xml.rs:219-234`); a parser that kept
    //     the raw reference instead of decoding it would read a different decision condition and fail here, even
    //     though the definition means the same thing.
    let entity_encoded = FORGE_SDLC_V6_XML.replacen(
        "workType == 'HOTFIX'",
        "workType == &apos;HOTFIX&apos;",
        1,
    );
    let entity_encoded_def = definition_from_xml(&entity_encoded)
        .expect("an attribute value written with entity references still parses");
    assert!(
        graphs_equal(&def.definition, &entity_encoded_def.definition),
        "{HARNESS}: an entity reference names the same value — `&apos;` must parse equal to `'`"
    );

    // 3h. WHERE INTENDED (default outcome) — an end-state that does not declare an outcome means Completed; the
    //     parser supplies that default (`rust/forge/src/engine/xml.rs:280-285`). Declaring the default explicitly is
    //     therefore not structure: removing `outcome="completed"` from an end-state must parse to a structurally
    //     equal graph. The anchor is asserted first so the clause cannot pass vacuously — a no-op `replacen` would
    //     leave the source byte-identical and the equality would hold for the wrong reason.
    assert!(
        FORGE_SDLC_V6_XML.contains("outcome=\"completed\""),
        "{HARNESS}: the definition declares an explicit default outcome for this clause"
    );
    let default_outcome = FORGE_SDLC_V6_XML.replacen("outcome=\"completed\"", "", 1);
    let default_outcome_def = definition_from_xml(&default_outcome)
        .expect("an end-state whose explicit default outcome is removed still parses");
    assert!(
        graphs_equal(&def.definition, &default_outcome_def.definition),
        "{HARNESS}: an end-state's default outcome is not structure — omitting `outcome=\"completed\"` must parse to an equal graph"
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

    // 4e. WHERE INTENDED (display order is ordered) — `display-order` is the definition's explicit sequence, so
    //     order is structure: swapping two adjacent entries changes the declared order even though the set of nodes
    //     is unchanged. The parse must succeed (the source stays legal) and the graphs must NOT be equal. The anchor
    //     is a two-line block, so if it were absent the `replacen` would no-op and `assert!(!graphs_equal(..))` would
    //     fail — the clause is self-proving.
    let swapped_order = FORGE_SDLC_V6_XML.replacen(
        "    <node ref=\"start\"/>\n    <node ref=\"classify_work\"/>",
        "    <node ref=\"classify_work\"/>\n    <node ref=\"start\"/>",
        1,
    );
    let swapped_order_def = definition_from_xml(&swapped_order)
        .expect("a display order with two adjacent entries swapped still parses");
    assert!(
        !graphs_equal(&def.definition, &swapped_order_def.definition),
        "{HARNESS}: display order is ordered structure — swapping two entries is a structural difference"
    );

    // 4b. WHERE INTENDED (control fields) — the remaining control fields the production parser reads and the production
    //     codec persists are each meaning, not decoration: an edge's `condition`/`required`, a decision's
    //     `refresh-facts`, a task's `priority`, an end-state's `outcome`, and a dynamic fork's `maximum`. Editing any
    //     one must parse (it stays legal) and must NOT be equal — otherwise the equality predicate would ignore a
    //     control field the engine routes on. Each anchor is the first occurrence of a field the definition declares,
    //     so `replacen(.., 1)` pins one node while the rest of the definition stays identical.
    let with_edge_condition = FORGE_SDLC_V6_XML.replacen(
        BEGIN_TRANSITION,
        "<transition name=\"begin\" to=\"classify_work\" condition=\"always\"/>",
        1,
    );
    let with_edge_condition_def = definition_from_xml(&with_edge_condition)
        .expect("an edge carrying a condition still parses");
    assert!(
        !graphs_equal(&def.definition, &with_edge_condition_def.definition),
        "{HARNESS}: an edge's condition is structure — adding one is a structural difference"
    );

    let with_edge_required = FORGE_SDLC_V6_XML.replacen(
        BEGIN_TRANSITION,
        "<transition name=\"begin\" to=\"classify_work\" required=\"true\"/>",
        1,
    );
    let with_edge_required_def = definition_from_xml(&with_edge_required)
        .expect("an edge carrying a required flag still parses");
    assert!(
        !graphs_equal(&def.definition, &with_edge_required_def.definition),
        "{HARNESS}: an edge's required flag is structure — setting it is a structural difference"
    );

    let refreshed_off = FORGE_SDLC_V6_XML.replacen("refresh-facts=\"true\"", "refresh-facts=\"false\"", 1);
    let refreshed_off_def = definition_from_xml(&refreshed_off)
        .expect("a decision with refresh-facts off still parses");
    assert!(
        !graphs_equal(&def.definition, &refreshed_off_def.definition),
        "{HARNESS}: a decision's refresh-facts is structure — flipping it is a structural difference"
    );

    let reprioritised = FORGE_SDLC_V6_XML.replacen("priority=\"100\"", "priority=\"50\"", 1);
    let reprioritised_def =
        definition_from_xml(&reprioritised).expect("a task with a different priority still parses");
    assert!(
        !graphs_equal(&def.definition, &reprioritised_def.definition),
        "{HARNESS}: a task's priority is structure — changing it is a structural difference"
    );

    let reoutcomed = FORGE_SDLC_V6_XML.replacen("outcome=\"completed\"", "outcome=\"cancelled\"", 1);
    let reoutcomed_def =
        definition_from_xml(&reoutcomed).expect("an end-state with a different outcome still parses");
    assert!(
        !graphs_equal(&def.definition, &reoutcomed_def.definition),
        "{HARNESS}: an end-state's outcome is structure — changing it is a structural difference"
    );

    let wider_fork = FORGE_SDLC_V6_XML.replacen("maximum=\"8\"", "maximum=\"4\"", 1);
    let wider_fork_def =
        definition_from_xml(&wider_fork).expect("a dynamic fork with a different bound still parses");
    assert!(
        !graphs_equal(&def.definition, &wider_fork_def.definition),
        "{HARNESS}: a dynamic fork's maximum is structure — changing it is a structural difference"
    );

    // 4c. WHERE INTENDED (node identity metadata) — the production parser reads a node's `label` into `name`, its
    //     `description` into `description`, and a task's `form-key` into `form_key`; the production codec
    //     (`rust/core/workflow/src/json_codec.rs:246-251,273-275`) represents all three, so a change to any one is
    //     structure, not decoration. Each anchor is the first occurrence the definition declares, so the rest of the
    //     definition stays identical and the parse can never silently no-op.
    let relabelled = FORGE_SDLC_V6_XML.replacen("label=\"Start Forge Story\"", "label=\"Begin\"", 1);
    let relabelled_def =
        definition_from_xml(&relabelled).expect("a node with a different label still parses");
    assert!(
        !graphs_equal(&def.definition, &relabelled_def.definition),
        "{HARNESS}: a node's label is structure — changing it is a structural difference"
    );

    let redescribed = FORGE_SDLC_V6_XML.replacen(
        "description=\"Start a Forge workflow instance for one software delivery story.\"",
        "description=\"Start.\"",
        1,
    );
    let redescribed_def =
        definition_from_xml(&redescribed).expect("a node with a different description still parses");
    assert!(
        !graphs_equal(&def.definition, &redescribed_def.definition),
        "{HARNESS}: a node's description is structure — changing it is a structural difference"
    );

    let reform_keyed = FORGE_SDLC_V6_XML.replacen("form-key=\"forge.smith\"", "form-key=\"forge.scout\"", 1);
    let reform_keyed_def =
        definition_from_xml(&reform_keyed).expect("a task with a different form-key still parses");
    assert!(
        !graphs_equal(&def.definition, &reform_keyed_def.definition),
        "{HARNESS}: a task's form-key is structure — changing it is a structural difference"
    );

    // 4d. WHERE INTENDED (dynamic-fork control) — the dynamic fork is fully control: its `count-variable`,
    //     `plan-variable` and `branch-node` name the runtime inputs and the node to spawn, and its `minimum` bounds
    //     the fan-out. The production parser reads each (`rust/forge/src/engine/xml.rs:328-337`) and the codec writes
    //     each (`rust/core/workflow/src/json_codec.rs:309-326`), so each is structure. The `maximum` clause above
    //     already pins the upper bound; these pin the rest.
    let recounted = FORGE_SDLC_V6_XML.replacen("count-variable=\"splitCount\"", "count-variable=\"splitCountX\"", 1);
    let recounted_def =
        definition_from_xml(&recounted).expect("a dynamic fork with a different count variable still parses");
    assert!(
        !graphs_equal(&def.definition, &recounted_def.definition),
        "{HARNESS}: a dynamic fork's count-variable is structure — changing it is a structural difference"
    );

    let replanned = FORGE_SDLC_V6_XML.replacen("plan-variable=\"splitPlan\"", "plan-variable=\"splitPlanX\"", 1);
    let replanned_def =
        definition_from_xml(&replanned).expect("a dynamic fork with a different plan variable still parses");
    assert!(
        !graphs_equal(&def.definition, &replanned_def.definition),
        "{HARNESS}: a dynamic fork's plan-variable is structure — changing it is a structural difference"
    );

    let rebranched = FORGE_SDLC_V6_XML.replacen("branch-node=\"smith_split_work\"", "branch-node=\"split_join\"", 1);
    let rebranched_def =
        definition_from_xml(&rebranched).expect("a dynamic fork pointing at another node still parses");
    assert!(
        !graphs_equal(&def.definition, &rebranched_def.definition),
        "{HARNESS}: a dynamic fork's branch-node is structure — changing it is a structural difference"
    );

    let narrowed_fork = FORGE_SDLC_V6_XML.replacen("minimum=\"2\"", "minimum=\"1\"", 1);
    let narrowed_fork_def =
        definition_from_xml(&narrowed_fork).expect("a dynamic fork with a different lower bound still parses");
    assert!(
        !graphs_equal(&def.definition, &narrowed_fork_def.definition),
        "{HARNESS}: a dynamic fork's minimum is structure — changing it is a structural difference"
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

    let mismatched = FORGE_SDLC_V6_XML.replacen("  </start-state>", "  </start_node>", 1);
    assert!(
        definition_from_xml(&mismatched).is_err(),
        "{HARNESS}: a mismatched close tag is refused, not repaired into an equal graph"
    );

    let duplicated =
        FORGE_SDLC_V6_XML.replacen("<start-state id=\"start\"", "<start-state id=\"hold\"", 1);
    assert!(
        definition_from_xml(&duplicated).is_err(),
        "{HARNESS}: a duplicate node id is refused"
    );

    let bad_entity = FORGE_SDLC_V6_XML.replacen(
        "workType == 'HOTFIX'",
        "workType == &bogus;HOTFIX&bogus;",
        1,
    );
    assert!(
        definition_from_xml(&bad_entity).is_err(),
        "{HARNESS}: an unknown entity reference is refused, not decoded into a different-but-equal graph"
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
