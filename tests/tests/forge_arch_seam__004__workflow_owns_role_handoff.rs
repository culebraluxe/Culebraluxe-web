//! ARCH-SEAM-004 — role-to-role handoff follows Workflow, never Rust branching.
//!
//! ```text
//! Architect completion → Workflow decides the next node → Lead service selected from the XML binding
//! Lead completion      → Workflow decides the next node → Smith service selected from the XML binding
//! ```
//!
//! Executed through the production durable driver over the XML definition, with the production lane composition
//! (`ForgeLaneServices`, the struct `bin/forge.rs` registers). Three executable proofs and one structural one:
//!
//!   * the FEATURE handoff happens, each turn on the service the XML binds to its node;
//!   * the same Rust with a different Lead DECISION takes a different route — the decision is read by a Workflow
//!     `<decision>`, not by a role;
//!   * the same Rust with a different DEFINITION hands Architect straight to Smith — the edge lives in the XML, so a
//!     Rust `if architect { call lead }` cannot be what moved the work;
//!   * no role module names another role's service, completes a Workflow task, or returns a transition other than
//!     its own `complete`.
//!
//! Level: L1 (production engine, in-memory store) + L0 static.

#[path = "support/forge_arch_seam.rs"]
mod support;

use std::sync::Arc;

use forge::engine::completion::MemoryLedger;
use forge::engine::job::JobService;
use forge::engine::runtime::ForgeRuntime;
use forge::engine::writer::RecordingWriter;
use forge::engine::xml::{definition_from_xml, FORGE_SDLC_V6_XML};
use forge::roles::ForgeLaneServices;
use support::*;
use test_harness::source::{workspace_root, sources_under};
use workflow::MemoryStore;

const STORY: &str = "TST-ARCH-SEAM-004";

/// Architect → Lead (SMITH) → Smith, scripted at the vendor edge only.
fn feature_script(trace: Trace, lead_decision: &str) -> ScriptedRunner {
    let decision = lead_decision.to_string();
    ScriptedRunner::new(trace)
        .answer(
            "architect",
            feature(|e| e.architecture_review_required = Some(false)),
        )
        .answer(
            "lead_pre",
            feature(move |e| e.lead_decision = Some(decision)),
        )
        .answer(
            "smith",
            feature(|e| e.candidate_sha = Some(SMITH_CANDIDATE.into())),
        )
        .answer("lead_solo_implement", feature(|_| {}))
}

fn run(rt: &ForgeRuntime<MemoryStore>, runner: &ScriptedRunner, trace: &Trace, steps: usize) {
    let lanes = ForgeLaneServices::new(runner);
    let registry = lanes.registry().expect("the production lane composition");
    let jobs = ObservedJobs::new(rt, trace.clone());
    let out = drive(rt, STORY, &jobs as &dyn JobService, &registry, steps).expect("drives");
    assert!(!out.needs_human, "a scripted happy path is never a hold");
}

#[test]
fn architect_hands_to_lead_and_lead_hands_to_smith_through_the_xml_bindings() {
    let (rt, _writer) = runtime();
    let trace = Trace::default();
    let runner = feature_script(trace.clone(), "SMITH");

    run(&rt, &runner, &trace, 3);

    assert_eq!(
        trace.with_prefix("job.enqueue"),
        vec![
            "job.enqueue forge.architect architect",
            "job.enqueue forge.lead lead_pre",
            "job.enqueue forge.smith smith",
        ],
        "each READY node became a job on the service the XML binds to it, in Workflow order"
    );
    assert_eq!(
        runner.turns(),
        vec!["turn architect", "turn lead_pre", "turn smith"],
        "and each lane ran exactly its own node"
    );
}

#[test]
fn the_lead_decision_is_read_by_a_workflow_decision_not_by_a_role() {
    let (rt, _writer) = runtime();
    let trace = Trace::default();
    // Identical Rust; only the Lead's reported decision differs.
    let runner = feature_script(trace.clone(), "SOLO");

    run(&rt, &runner, &trace, 3);

    assert_eq!(
        trace.with_prefix("job.enqueue"),
        vec![
            "job.enqueue forge.architect architect",
            "job.enqueue forge.lead lead_pre",
            "job.enqueue forge.lead lead_solo_implement",
        ],
        "`execution_shape` (an XML <decision>) routed SOLO back to the Lead lane; no Smith job exists"
    );
    assert!(
        !runner.turns().iter().any(|turn| turn == "turn smith"),
        "Smith must not run when Workflow did not route to it"
    );
}

#[test]
fn moving_an_edge_in_the_definition_moves_the_handoff_with_no_rust_change() {
    let original = r#"<transition name="proceed" to="lead_pre"/>"#;
    assert_eq!(
        FORGE_SDLC_V6_XML.matches(original).count(),
        1,
        "the architect_review → lead_pre edge this proof rewires must exist exactly once"
    );
    let rewired = FORGE_SDLC_V6_XML.replace(original, r#"<transition name="proceed" to="smith"/>"#);
    let writer = Arc::new(RecordingWriter::default());
    let rt = ForgeRuntime::from_store(
        MemoryStore::new(),
        writer,
        None,
        None,
        Arc::new(MemoryLedger::new()),
        definition_from_xml(&rewired).expect("the rewired definition parses"),
    )
    .expect("the rewired definition seeds");
    let trace = Trace::default();
    let runner = feature_script(trace.clone(), "SMITH");

    run(&rt, &runner, &trace, 2);

    assert_eq!(
        trace.with_prefix("job.enqueue"),
        vec![
            "job.enqueue forge.architect architect",
            "job.enqueue forge.smith smith",
        ],
        "the Architect's successor is whatever the definition says it is"
    );
    assert!(
        !runner.turns().iter().any(|turn| turn == "turn lead_pre"),
        "no Rust path re-inserted the Lead between Architect and Smith"
    );
}

/// Role services are siblings behind the registry. A role that names another role's service, completes a Workflow
/// task itself, or drives the story is a role deciding who runs next.
#[test]
fn no_role_module_calls_another_role_or_advances_workflow() {
    let roles_dir = workspace_root().join("forge/src/roles");
    let files = sources_under(&roles_dir);
    assert!(
        files.len() >= 10,
        "the roles tree must be read, found {}",
        files.len()
    );

    let lane_services = [
        ("scout.rs", "ScoutService"),
        ("architect.rs", "ArchitectService"),
        ("lead.rs", "LeadService"),
        ("smith.rs", "SmithService"),
        ("inspector.rs", "InspectorService"),
        ("qa.rs", "AssayService"),
        ("dev_ops.rs", "DevOpsService"),
    ];
    let routing = [
        "complete_role_task",
        "complete_task(",
        "drive_forge_story",
        "ForgeJobBridge",
        "WorkflowEngine",
        ".resolve(",
        ".execute(",
    ];
    let mut hits = Vec::new();
    for (file, own) in lane_services {
        let path = roles_dir.join(file);
        assert!(!production_code(&path).is_empty(), "{file} must be read");
        let foreign: Vec<&str> = lane_services
            .iter()
            .map(|(_, service)| *service)
            .filter(|service| service != &own)
            .collect();
        hits.extend(lines_naming(&path, &foreign));
        hits.extend(lines_naming(&path, &routing));
    }
    // A lane answers its own node with `complete` (or holds); the next node is the definition's. A lane returning a
    // transition named after another node would be Rust choosing the successor.
    for path in &files {
        for (line, code) in production_code(path) {
            if let Some(start) = code.find("transition_name: Some(\"") {
                let rest = &code[start + "transition_name: Some(\"".len()..];
                let name = rest.split('"').next().unwrap_or_default();
                if name != "complete" {
                    hits.push(format!("{}:{line}: transition `{name}`", path.display()));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "a role reaches past Workflow to hand off work:\n{}",
        hits.join("\n")
    );
}

/// The scan must be able to fail: the composition module names every lane service and calls `execute`.
#[test]
fn the_scan_detects_cross_role_naming_where_it_exists() {
    let composition = workspace_root().join("forge/src/roles/service.rs");
    let hits = lines_naming(&composition, &["SmithService", "LeadService", ".execute("]);
    assert!(hits.len() >= 3, "the scanner is blind: {hits:?}");
}
