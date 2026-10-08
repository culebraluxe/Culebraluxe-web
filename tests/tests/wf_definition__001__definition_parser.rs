//! WF.DEFINITION — definition parser (TST-WF-DEFINITION-001).
//!
//! Contract: the production XML parser reads a definition **as the definition means it**, element by element —
//! every element name maps to its node type, every declared attribute is read into the field the engine routes on,
//! and a required attribute that is absent is a refusal rather than a default. The parser is the one production
//! uses: `deploy_xml` (`forge/src/engine/deploy.rs:83-92`), the engine binary (`forge/src/bin/forge_task.rs:45`)
//! and `validate_definition_xml` (`forge/src/engine/validate.rs:16-75`) all call
//! `parse_process_definition_xml` / `definition_from_xml` (`forge/src/engine/xml.rs:352-412`, `:468-480`).
//!
//! This file is about **coverage of the parse, not about validity**. Whether a parsed graph is a well-formed workflow
//! is a different boundary and a different story (`wf_definition__005__missing_target` owns the edge check,
//! `wf_definition__002__definition_validation` owns the report). What is proved here is narrower and exact:
//!
//! - **every element name the parser supports** produces its production `node_type`, and the attributes each kind
//!   declares land in the production field the engine reads — the full read of the XML, because a parser that
//!   silently dropped `form-key`, `priority`, `count-variable`, `minimum`, a decision's `refresh-facts` or a
//!   command node's `transition` would return a plausible graph that routes wrongly;
//! - **every required attribute** is required: `req` (`forge/src/engine/xml.rs:236-242`) treats an absent **or empty**
//!   value as absent, and each kind's required attributes are each refused when removed, one at a time; and
//! - **the refusal names what is missing**, so an operator can repair the definition from the error alone.
//!
//! The distinguishing negatives are what stop this passing vacuously: an attribute the XML does not declare must
//! read as `None` rather than an empty string or a default (a `form-key=""` is *refused*, not `Some("")`); the
//! `end-state` outcome default is `completed` and only the three declared outcomes change it; and an unparsable
//! priority degrades to `None` rather than aborting the parse — the one attribute whose loss is silent, pinned here
//! because a silent loss is the failure mode this file exists to catch.
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider: the
//! inputs are literal XML strings and the outputs are the production parser's own.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__001__definition_parser

use forge::engine::xml::{
    definition_from_xml, parse_process_definition_xml, ParsedDefinition, XmlError,
};
use workflow::ProcessOutcome;

const HARNESS: &str = "WorkflowHarness/L0 Pure";
/// The identity of the fixture below, as the XML declares it.
const KEY: &str = "TST-WF-DEFINITION-001";
const VERSION: i32 = 3;
/// The one node per element kind, so every arm of `parse_node`'s match is covered by the same fixture.
const ALL_KINDS: &str = r#"<process-definition key="TST-WF-DEFINITION-001" version="3" name="All kinds" description="Every node kind.">
  <start-state id="start" label="Start">
    <transition name="begin" to="plain"/>
  </start-state>
  <state id="plain" label="Plain"/>
  <task-node id="task" label="Task" description="A human gate." responsibility="smith" form-key="forge.smith" priority="70">
    <transition name="submit" to="emitted" condition="always" required="true"/>
  </task-node>
  <command-node id="emitted" label="Emit" responsibility="dev_ops" command-type="forge.migrate_dev" transition="complete"/>
  <decision id="route" label="Route" refresh-facts="true">
    <on condition="workType == 'BUG'" transition="bug"/>
    <on condition="workType == 'HOTFIX'" transition="hotfix"/>
    <transition name="bug" to="fan"/>
    <transition name="hotfix" to="waits"/>
  </decision>
  <fork id="fan" label="Fan">
    <transition name="left" to="waits"/>
    <transition name="right" to="waits"/>
  </fork>
  <join id="joined" label="Joined">
    <transition name="joined-go" to="waits"/>
  </join>
  <timer id="waits" label="Waits" due-at="2026-01-01T00:00:00Z" due-at-variable="slaDue" on-fire="done"/>
  <dynamic-fork id="split" label="Split" count-variable="splitCount" plan-variable="splitPlan" branch-command-type="forge.launch_builder" branch-node="split_work" join="split_join" minimum="2" maximum="8">
    <transition name="split-go" to="waits"/>
  </dynamic-fork>
  <end-state id="done" label="Done" outcome="cancelled"/>
</process-definition>"#;

/// Parse a definition that must be accepted. A refusal here is a failure of the fixture, not of the parser.
fn parsed(xml: &str) -> ParsedDefinition {
    parse_process_definition_xml(xml)
        .unwrap_or_else(|e| panic!("{HARNESS}: this fixture must parse, got: {e}"))
}

/// Parse a definition that must be REFUSED and return the production message, so the reason can be asserted rather
/// than swallowed. A definition that parses is a failure of the clause itself.
fn refusal_message(xml: &str) -> String {
    match parse_process_definition_xml(xml) {
        Ok(d) => panic!(
            "{HARNESS}: expected the parser to refuse this definition, but it parsed key '{}' — source:\n{xml}",
            d.key
        ),
        Err(err) => err.to_string(),
    }
}

/// `<start-state id="start"/>` plus the given extra root-level elements, so a clause can vary one node at a time
/// without disturbing the rest.
fn definition_with(extra: &str) -> String {
    format!(
        "<process-definition key=\"{KEY}\" version=\"{VERSION}\" name=\"Fixture\">\n{extra}\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"done\"/>\n  </start-state>\n  <end-state id=\"done\"/>\n</process-definition>"
    )
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-001); the file and the assay use it.
fn wf_definition_001__definition_parser() {
    // 1. THE ROOT IS READ, NOT GUESSED. Key, version, name and description come from the XML itself, and the
    //    definition id is derived as `key-v<version>` — not from a caller's guess.
    let all = parsed(ALL_KINDS);
    assert_eq!(all.key, KEY, "{HARNESS}: the XML declares its own key");
    assert_eq!(
        all.version, VERSION,
        "{HARNESS}: the XML declares version 3, parsed as a number"
    );
    assert_eq!(
        all.name, "All kinds",
        "{HARNESS}: the XML declares its own name"
    );
    assert_eq!(
        all.description.as_deref(),
        Some("Every node kind."),
        "{HARNESS}: the root's description is read"
    );
    assert_eq!(
        all.graph.start_node_id, "start",
        "{HARNESS}: the start node is the one <start-state> the XML declares"
    );
    assert_eq!(
        all.graph.nodes.len(),
        10,
        "{HARNESS}: every declared node reaches the map: {:?}",
        all.graph.nodes.keys().collect::<Vec<_>>()
    );
    let def = definition_from_xml(ALL_KINDS).expect("the same source loads as a definition");
    assert_eq!(
        def.id,
        format!("{KEY}-v{VERSION}"),
        "{HARNESS}: the definition id is derived as key-v<version> from the XML alone"
    );
    assert_eq!(
        def.status,
        workflow::DefinitionStatus::Active,
        "{HARNESS}: a parsed definition is active"
    );

    // 2. EVERY ELEMENT NAME MAPS TO ITS NODE TYPE. `parse_node`'s match arms are the whole vocabulary; one kind
    //    reading as another (or as a default) would send the engine down the wrong handler.
    for (id, expected) in [
        ("start", "start"),
        ("plain", "state"),
        ("task", "task"),
        ("emitted", "command"),
        ("route", "decision"),
        ("fan", "fork"),
        ("joined", "join"),
        ("waits", "timer"),
        ("split", "dynamic-fork"),
        ("done", "end"),
    ] {
        let node = &all.graph.nodes[id];
        assert_eq!(
            node.node_type, expected,
            "{HARNESS}: <{id}> must read as node_type '{expected}'"
        );
        assert_eq!(
            node.id, id,
            "{HARNESS}: the node's own id is the one the XML declared, and the map is keyed by it"
        );
    }

    // 3. EVERY DECLARED ATTRIBUTE LANDS IN THE FIELD THE ENGINE ROUTES ON. A dropped attribute would leave the node
    //    looking valid and routing wrongly, which is the failure this clause exists to catch.
    let task = &all.graph.nodes["task"];
    assert_eq!(
        task.name.as_deref(),
        Some("Task"),
        "{HARNESS}: a task's label is read into its name"
    );
    assert_eq!(
        task.description.as_deref(),
        Some("A human gate."),
        "{HARNESS}: a task's description is read"
    );
    assert_eq!(
        task.responsibility.as_deref(),
        Some("smith"),
        "{HARNESS}: a task's responsibility is read"
    );
    assert_eq!(
        task.candidate_groups.as_deref(),
        Some(["smith".to_string()].as_slice()),
        "{HARNESS}: responsibility becomes the single candidate group a task is claimed through"
    );
    assert_eq!(
        task.form_key.as_deref(),
        Some("forge.smith"),
        "{HARNESS}: a task's form-key is read"
    );
    assert_eq!(
        task.priority,
        Some(70),
        "{HARNESS}: a task's priority is read as a number"
    );

    let command = &all.graph.nodes["emitted"];
    assert_eq!(
        command.command_type.as_deref(),
        Some("forge.migrate_dev"),
        "{HARNESS}: a command node's command-type is read"
    );
    assert_eq!(
        command.transition.as_deref(),
        Some("complete"),
        "{HARNESS}: a command node's transition is read"
    );

    let decision = &all.graph.nodes["route"];
    assert_eq!(
        decision.refresh_facts,
        Some(true),
        "{HARNESS}: a decision's refresh-facts is read as a bool"
    );
    let arms = decision
        .decisions
        .as_ref()
        .expect("a decision's <on> children become its arms");
    assert_eq!(arms.len(), 2, "{HARNESS}: the decision declares two arms");
    assert_eq!(arms[0].condition, "workType == 'BUG'");
    assert_eq!(arms[0].transition, "bug");
    assert_eq!(arms[1].condition, "workType == 'HOTFIX'");
    assert_eq!(arms[1].transition, "hotfix");

    let timer = &all.graph.nodes["waits"]
        .timer
        .clone()
        .expect("a timer node reads a spec");
    assert_eq!(
        timer.due_at.as_deref(),
        Some("2026-01-01T00:00:00Z"),
        "{HARNESS}: a timer's due-at is read"
    );
    assert_eq!(
        timer.due_at_variable.as_deref(),
        Some("slaDue"),
        "{HARNESS}: a timer's due-at-variable is read"
    );
    assert_eq!(
        timer.transition.as_deref(),
        Some("done"),
        "{HARNESS}: a timer's on-fire transition is read"
    );

    let split = &all.graph.nodes["split"];
    assert_eq!(
        split.count_variable.as_deref(),
        Some("splitCount"),
        "{HARNESS}: a dynamic fork's count-variable is read"
    );
    assert_eq!(
        split.plan_variable.as_deref(),
        Some("splitPlan"),
        "{HARNESS}: a dynamic fork's plan-variable is read"
    );
    assert_eq!(
        split.branch_command_type.as_deref(),
        Some("forge.launch_builder"),
        "{HARNESS}: a dynamic fork's branch-command-type is read"
    );
    assert_eq!(
        split.branch_node.as_deref(),
        Some("split_work"),
        "{HARNESS}: a dynamic fork's branch-node is read"
    );
    assert_eq!(
        split.join.as_deref(),
        Some("split_join"),
        "{HARNESS}: a dynamic fork's join is read"
    );
    assert_eq!(
        split.minimum,
        Some(2),
        "{HARNESS}: the fork's minimum is read"
    );
    assert_eq!(
        split.maximum,
        Some(8),
        "{HARNESS}: the fork's maximum is read"
    );

    // 4. EDGES AND THEIR GUARDS. `collect_transitions` reads `name`/`to` as required and `condition`/`required` as
    //    optional, with `required` defaulting to false unless the attribute says "true".
    let fork_edges = all.graph.nodes["fan"].transitions.as_deref().unwrap();
    assert_eq!(
        fork_edges.len(),
        2,
        "{HARNESS}: the fork declares two branches"
    );
    let task_edge = &all.graph.nodes["task"].transitions.as_deref().unwrap()[0];
    assert_eq!(task_edge.name, "submit");
    assert_eq!(task_edge.to, "emitted");
    assert_eq!(
        task_edge.condition.as_deref(),
        Some("always"),
        "{HARNESS}: an edge's condition is read"
    );
    assert_eq!(
        task_edge.required,
        Some(true),
        "{HARNESS}: an edge's required flag is read"
    );
    let fork_edge = &fork_edges[0];
    assert_eq!(
        fork_edge.condition, None,
        "{HARNESS}: an undeclared edge condition is None, not an empty string"
    );
    assert_eq!(
        fork_edge.required, None,
        "{HARNESS}: an edge that does not declare `required` has None — the three-valued flag is never coerced"
    );
    // The declared values, so the clause above cannot be satisfied by a parser that always read None.
    for (declared, expected) in [
        ("true", Some(true)),
        ("false", Some(false)),
        ("yes", Some(false)),
    ] {
        let edge = parsed(&definition_with(&format!(
            "  <state id=\"work\"><transition name=\"go\" to=\"done\" required=\"{declared}\"/></state>"
        )));
        assert_eq!(
            edge.graph.nodes["work"].transitions.as_deref().unwrap()[0].required,
            expected,
            "{HARNESS}: required=\"{declared}\" reads as {expected:?} — only the literal \"true\" is true"
        );
    }
    assert!(
        all.graph.nodes["plain"].transitions.is_none(),
        "{HARNESS}: a node with no <transition> children has no transitions at all, not an empty list"
    );

    // 5. THE END-STATE OUTCOME MAPPING, INCLUDING ITS DEFAULT. Only the three declared outcomes change the parse;
    //    anything else — including an undeclared spelling — is Completed, because that is the parser's documented
    //    default (`forge/src/engine/xml.rs:280-285`).
    for (declared, expected) in [
        ("cancelled", ProcessOutcome::Cancelled),
        ("failed", ProcessOutcome::Failed),
        ("conflict", ProcessOutcome::Conflict),
        ("completed", ProcessOutcome::Completed),
        ("", ProcessOutcome::Completed),
    ] {
        let xml = if declared.is_empty() {
            r#"<process-definition key="TST-WF-DEFINITION-001-OUTCOME" version="1" name="Outcome">
  <start-state id="start"/>
  <end-state id="done" outcome=""/>
</process-definition>"#
        } else {
            &format!(
                "<process-definition key=\"TST-WF-DEFINITION-001-OUTCOME\" version=\"1\" name=\"Outcome\">\n  <start-state id=\"start\"/>\n  <end-state id=\"done\" outcome=\"{declared}\"/>\n</process-definition>"
            )
        };
        assert_eq!(
            parsed(xml).graph.nodes["done"].outcome,
            Some(expected),
            "{HARNESS}: an end-state declaring outcome=\"{declared}\" reads as {expected:?}"
        );
    }
    assert_eq!(
        parsed(ALL_KINDS).graph.nodes["done"].outcome,
        Some(ProcessOutcome::Cancelled),
        "{HARNESS}: the fixture's own declared outcome is read, not defaulted"
    );

    // 6. DISPLAY-ORDER IS OPTIONAL AND ORDERED. Absent, it is `None` (not an empty list); declared, it is the exact
    //    sequence of `ref`s the XML declares, in declaration order.
    let ordered = parsed(
        r#"<process-definition key="TST-WF-DEFINITION-001-ORDER" version="1" name="Order">
  <display-order>
    <node ref="start"/>
    <node ref="done"/>
  </display-order>
  <start-state id="start"/>
  <end-state id="done"/>
</process-definition>"#,
    );
    assert_eq!(
        ordered.graph.display_order.as_deref(),
        Some(["start".to_string(), "done".to_string()].as_slice()),
        "{HARNESS}: display-order is the declared sequence, in order"
    );
    assert_eq!(
        parsed(
            r#"<process-definition key="TST-WF-DEFINITION-001-NOORDER" version="1" name="No order">
  <start-state id="start"/>
  <end-state id="done"/>
</process-definition>"#
        )
        .graph
        .display_order,
        None,
        "{HARNESS}: a definition with no display-order has none, not an empty one"
    );

    // 7. AN UNDECLARED ATTRIBUTE IS ABSENT, NOT EMPTY. This is the negative that stops "every attribute is read"
    //    from being satisfied by a parser that fills in blanks: each optional attribute the XML does not declare
    //    must read as `None`, because the engine distinguishes "declared empty" from "not declared".
    let bare = parsed(&definition_with(
        "  <task-node id=\"work\" label=\"Work\"/>\n  <command-node id=\"run\" command-type=\"forge.migrate_dev\"/>\n  <timer id=\"clock\"/>",
    ));
    let work = &bare.graph.nodes["work"];
    assert_eq!(
        work.name.as_deref(),
        Some("Work"),
        "{HARNESS}: the declared label is read"
    );
    for (field, value) in [
        ("description", &work.description),
        ("responsibility", &work.responsibility),
        ("form_key", &work.form_key),
    ] {
        assert_eq!(
            *value, None,
            "{HARNESS}: an undeclared task {field} is None, not an empty string"
        );
    }
    assert_eq!(
        work.candidate_groups, None,
        "{HARNESS}: a task with no responsibility has no candidate groups"
    );
    assert_eq!(
        work.priority, None,
        "{HARNESS}: a task with no priority has none, not zero"
    );
    let run = &bare.graph.nodes["run"];
    assert_eq!(
        run.transition, None,
        "{HARNESS}: a command node with no transition has none"
    );
    let timer = &bare.graph.nodes["clock"]
        .timer
        .clone()
        .expect("a timer always reads a spec, defaulting to empty");
    assert_eq!(
        timer.due_at, None,
        "{HARNESS}: a timer with no due-at has none"
    );
    assert_eq!(
        timer.transition, None,
        "{HARNESS}: a timer with no on-fire has none"
    );
    // A task's name DEFAULTS to its id when no label is declared — the one attribute the parser does substitute.
    assert_eq!(
        bare.graph.nodes["run"].name.as_deref(),
        Some("run"),
        "{HARNESS}: a node with no label is named after its own id"
    );

    // 8. EVERY REQUIRED ATTRIBUTE IS REQUIRED, AND ITS ABSENCE IS NAMED. `req` refuses an absent OR EMPTY value, so
    //    each clause below removes one required attribute and demands the refusal name it. A parser that defaulted
    //    any of these would parse a definition no runtime could honour.
    for (declaration, missing) in [
        (
            "  <task-node label=\"Work\"/>",
            "id",
        ),
        (
            "  <task-node id=\"\"/>",
            "id",
        ),
        (
            "  <state id=\"\"><transition name=\"go\" to=\"done\"/></state>",
            "id",
        ),
        (
            "  <task-node id=\"work\"><transition to=\"done\"/></task-node>",
            "name",
        ),
        (
            "  <state id=\"work\"><transition name=\"\" to=\"done\"/></state>",
            "name",
        ),
        (
            "  <state id=\"work\"><transition name=\"go\" to=\"\"/></state>",
            "to",
        ),
        (
            "  <command-node id=\"work\"/>",
            "command-type",
        ),
        (
            "  <command-node id=\"work\" command-type=\"\"/>",
            "command-type",
        ),
        (
            "  <decision id=\"work\"><on transition=\"go\"/></decision>",
            "condition",
        ),
        (
            "  <decision id=\"work\"><on condition=\"x == 'y'\"/></decision>",
            "transition",
        ),
        (
            "  <dynamic-fork id=\"work\"/>",
            "count-variable",
        ),
        (
            "  <dynamic-fork id=\"work\" count-variable=\"n\"/>",
            "branch-command-type",
        ),
        (
            "  <dynamic-fork id=\"work\" count-variable=\"n\" branch-command-type=\"forge.launch_builder\"/>",
            "join",
        ),
    ] {
        let message = refusal_message(&definition_with(declaration));
        assert!(
            message.contains(&format!("missing {missing}")),
            "{HARNESS}: removing '{missing}' from {declaration:?} must be refused by name: {message}"
        );
    }

    // 8b. THE SAME FOR THE ROOT ITSELF. The key, version and name are required of every definition, and a version
    //    that is not an integer is refused rather than coerced to zero.
    for (attribute, value) in [
        ("key", KEY.to_string()),
        ("version", VERSION.to_string()),
        ("name", "Fixture".to_string()),
    ] {
        let original = format!("{attribute}=\"{value}\"");
        assert!(
            definition_with("  <state id=\"work\"/>").contains(&original),
            "{HARNESS}: the root declares {original}, so the empty-value clause below is not a no-op"
        );
        let xml = definition_with("  <state id=\"work\"/>").replacen(
            &original,
            &format!("{attribute}=\"\""),
            1,
        );
        let message = refusal_message(&xml);
        assert!(
            message.contains(&format!("missing {attribute}")),
            "{HARNESS}: an empty root {attribute} is refused by name: {message}"
        );
    }
    let non_numeric = definition_with("  <state id=\"work\"/>").replacen(
        &format!("version=\"{VERSION}\""),
        "version=\"six\"",
        1,
    );
    assert_eq!(
        refusal_message(&non_numeric),
        "bad version",
        "{HARNESS}: a non-numeric version is refused as a bad version, not parsed as 0"
    );

    // 9. THE ONE SILENT LOSS, PINNED. A priority that is present but not an integer reads as `None` and the rest of
    //    the node parses (`p.parse().ok()`, `forge/src/engine/xml.rs:293-295`). That is the only declared attribute
    //    whose loss is silent, so it is asserted explicitly: a definition that must still parse, and a node whose
    //    priority is gone.
    let bad_priority = parsed(&definition_with(
        "  <task-node id=\"work\" priority=\"high\"/>",
    ));
    assert_eq!(
        bad_priority.graph.nodes["work"].priority, None,
        "{HARNESS}: an unparsable priority reads as None rather than aborting the parse"
    );
    assert!(
        bad_priority.graph.nodes.contains_key("work"),
        "{HARNESS}: the node itself survives a bad priority — only the field is lost"
    );
    // And the positive half, so the clause cannot be satisfied by a parser that always dropped priority.
    assert_eq!(
        all.graph.nodes["task"].priority,
        Some(70),
        "{HARNESS}: a well-formed priority IS read, so the clause above is about the bad value"
    );

    // 10. NON-VACUITY — THE SAME SOURCE PARSES THE SAME WAY TWICE, AND A CHANGE MOVES THE READ. If the parser were
    //     constant, step 2 would fail; if it ignored the source, this pair could not differ.
    let again = parsed(ALL_KINDS);
    assert_eq!(
        again.graph.nodes["task"].form_key, all.graph.nodes["task"].form_key,
        "{HARNESS}: the parse is deterministic for one source"
    );
    let rekeyed =
        parsed(&ALL_KINDS.replacen("form-key=\"forge.smith\"", "form-key=\"forge.qa\"", 1));
    assert_ne!(
        rekeyed.graph.nodes["task"].form_key, all.graph.nodes["task"].form_key,
        "{HARNESS}: the parse reads the source — changing the form-key moves the read"
    );

    // 11. THE REFUSAL TYPE IS THE PRODUCTION ONE. Every refusal above is an `XmlError`, the single error type
    //     `definition_from_xml` propagates and `validate_definition_xml` classifies — not a panic and not a
    //     silently-empty graph.
    let err: XmlError = parse_process_definition_xml("not xml at all")
        .expect_err("{HARNESS}: text that is not XML is refused");
    assert!(
        !err.0.is_empty(),
        "{HARNESS}: the production error carries its reason: {err}"
    );
    assert!(
        definition_from_xml(&definition_with("  <mystery-node id=\"x\"/>")).is_err(),
        "{HARNESS}: the same refusal reaches definition_from_xml, which is what deploy_xml calls"
    );
}
