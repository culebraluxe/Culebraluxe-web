//! ARCH-SEAM-005 — the candidate is one durable fact across every role boundary.
//!
//! CONTRACT.
//!
//! ```text
//! Smith candidate → evidence/state writer → Workflow → Inspector → Assay → release decision
//! the SHA being reviewed == the SHA Assay measures == the SHA eligible for publish
//! ```
//!
//! No consumer may silently substitute the worktree's current HEAD.
//!
//!   * EXECUTABLE, through the production driver: the candidate Smith reports is the Workflow fact every later turn
//!     is dispatched under — Lead integration, Inspector and Assay each start from exactly that SHA — and it is the
//!     fact the generation ends with.
//!   * STRUCTURAL: the release decision publishes the stored candidate (`evidence.candidate_sha`), never a HEAD it
//!     resolves itself, and the publish path refuses to publish with no candidate.
//!   * EXECUTABLE, the measuring road: Assay's model-free RUST_CONTRACT turn, handed evidence whose candidate is the
//!     reviewed SHA and a workspace whose HEAD is another commit, must measure — and report — the reviewed SHA, or
//!     refuse. See the RED test below.
//!
//! Level: L1 executable; L0 structural.

#[path = "support/forge_arch_chain.rs"]
mod arch;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use arch::*;
use forge::engine::assay::CommandResult;
use forge::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::WorkflowJobService;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::roles::architect::ArchitectService;
use forge::roles::dev_ops::DevOpsService;
use forge::roles::hooks::ForgeRoleHooks;
use forge::roles::inspector::InspectorService;
use forge::roles::lead::LeadService;
use forge::roles::lifecycle::ForgeRoleContext;
use forge::roles::qa::{AssayHooks, AssayService};
use forge::roles::scout::ScoutService;
use forge::roles::smith::SmithService;
use forge::roles::{AbstractForgeService, ForgeServiceRegistry};
use workflow::{MemoryStore, Value};

/// A scripted turn that first records the candidate the Workflow holds at the moment the turn is dispatched.
struct Observing {
    memory: MemoryStore,
    seen: Arc<Mutex<Vec<(String, Option<String>)>>>,
}

impl ForgeRoleRunner for Observing {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> workflow::Result<ForgeRoleOutcome> {
        let candidate = self
            .memory
            .with_tx(|tx| tx.get_instance(&task.process_instance_id))
            .ok()
            .and_then(|instance| {
                instance
                    .variables
                    .get("candidateSha")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
        self.seen
            .lock()
            .expect("seen")
            .push((node_id.to_string(), candidate));
        let evidence = feature_script()
            .remove(node_id)
            .unwrap_or_else(|| panic!("no turn scripted for {node_id}"));
        Ok(ForgeRoleOutcome {
            transition_name: Some("complete".into()),
            evidence,
        })
    }
}

#[test]
fn every_turn_after_smith_is_dispatched_under_smiths_candidate() {
    let (fixture, memory) = fixture();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let runner = Observing {
        memory: memory.clone(),
        seen: seen.clone(),
    };
    let scout = ScoutService::new(&runner);
    let architect = ArchitectService::new(&runner);
    let lead = LeadService::new(&runner);
    let smith = SmithService::new(&runner);
    let inspector = InspectorService::new(&runner);
    let assay = AssayService::new(&runner);
    let devops = DevOpsService::new(&runner);
    let mut registry = ForgeServiceRegistry::new();
    for service in [
        &scout as &dyn AbstractForgeService,
        &architect,
        &lead,
        &smith,
        &inspector,
        &assay,
        &devops,
    ] {
        registry.register(service).expect("register");
    }
    let jobs = WorkflowJobService::new(fixture.rt.engine());
    let out = drive(&fixture, &jobs, &registry, 40).expect("the FEATURE generation drives");

    let seen = seen.lock().expect("seen").clone();
    let candidate_at = |node: &str| -> Option<String> {
        seen.iter()
            .find(|(seen_node, _)| seen_node == node)
            .unwrap_or_else(|| panic!("{node} never ran: {seen:?}"))
            .1
            .clone()
    };
    assert_eq!(
        candidate_at("smith"),
        None,
        "nothing is a candidate before Smith delivers one"
    );
    for consumer in ["lead_post", "qa_review", "qa_verify"] {
        assert_eq!(
            candidate_at(consumer).as_deref(),
            Some(CANDIDATE),
            "{consumer} was dispatched under another candidate than the one Smith delivered"
        );
    }
    let final_candidate = memory
        .with_tx(|tx| tx.get_instance(&out.instance_id))
        .expect("instance")
        .variables
        .get("candidateSha")
        .and_then(Value::as_str)
        .map(str::to_string);
    assert_eq!(
        final_candidate.as_deref(),
        Some(CANDIDATE),
        "the generation ends on Smith's candidate"
    );
}

#[test]
fn the_release_decision_publishes_the_stored_candidate_and_refuses_none() {
    let release = production_code(&workspace_root().join("forge/src/engine/release.rs"));
    assert!(
        release.contains(".publish(evidence.candidate_sha.as_deref()"),
        "the release publishes the candidate the evidence carries"
    );
    let publish = production_code(&workspace_root().join("forge/src/engine/git_publish.rs"));
    assert!(
        publish.contains("let Some(sha) = candidate_sha else"),
        "the publish path refuses to publish without a candidate"
    );
    for (file, code) in [("release.rs", &release), ("git_publish.rs", &publish)] {
        assert!(
            !code.contains("rev-parse HEAD") && !code.contains("\"HEAD\""),
            "{file} resolves HEAD itself instead of publishing the reviewed candidate"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The measuring road.
// ---------------------------------------------------------------------------------------------------------------

/// The SHA the workspace's HEAD points at in the probe — a commit nobody reviewed.
const WORKSPACE_HEAD: &str = "badbadbadbadbadbadbadbadbadbadbadbadbad0";

/// A harness with no model: every command passes and `git rev-parse HEAD` answers `WORKSPACE_HEAD`.
struct Workspace {
    cwd: PathBuf,
}

impl RoleHarness for Workspace {
    fn run_role(
        &self,
        node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        panic!("Assay's RUST_CONTRACT road asked the harness for a model turn on {node_id}")
    }
    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        false
    }
    fn assay_cwd(&self) -> &Path {
        &self.cwd
    }
    fn execution_base_commit(&self) -> Option<&str> {
        Some("0000000000000000000000000000000000000000")
    }
    fn run_command(&self, command: &str) -> CommandResult {
        let output = if command.trim() == "git rev-parse HEAD" {
            format!("{WORKSPACE_HEAD}\n")
        } else {
            String::new()
        };
        CommandResult {
            command: command.to_string(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output,

            cancelled: false,
        }
    }
}

/// RED — Assay's RUST_CONTRACT turn measures the workspace HEAD and REPLACES the reviewed candidate with it.
///
/// Mechanism (2026-10-03): `roles/qa.rs::run_rust_contract_qa` runs `git rev-parse HEAD` and assigns the answer to
/// `current.candidate_sha` (or `None`), discarding the candidate the evidence already carried — the one Smith
/// delivered and Inspector reviewed. That evidence is what the Workflow keeps, and `engine/release.rs` publishes
/// `evidence.candidate_sha`. So whenever the workspace HEAD is not the reviewed candidate, the SHA Assay measures and
/// the SHA eligible for publish are silently not the SHA that was reviewed. Owner: the Assay lane (`roles/qa.rs`).
/// The contract allows either answer the test accepts: keep the reviewed candidate, or refuse the measurement.
#[test]
fn assay_measures_the_reviewed_candidate_not_the_workspace_head() {
    let workspace = Workspace {
        cwd: std::env::temp_dir(),
    };
    let reviewed = ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        qa_review_passed: Some(true),
        ..Default::default()
    };
    let commands = vec!["cargo test -p forge".to_string()];
    let ctx = ForgeRoleContext {
        harness: &workspace,
        current: &reviewed,
        writer: None,
        story_run_id: None,
        bench_intent: None,
        test_mode: Some("RUST_CONTRACT"),
        contract_assay_commands: &commands,
        contract_acceptance_mapped: true,
        require_prod: false,

        execution_id: None,
        write_surface: None,
        model_attempt_control: None,
    };
    let task = ActiveForgeRoleTask {
        task_id: "t-assay".into(),
        process_instance_id: "p-assay".into(),
        story_id: STORY.into(),
        token_id: None,
        node_id: Some("qa_verify".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec![],

        write_surface: None,
    };
    let turn = AssayHooks
        .turn_without_model(&ctx, "qa_verify", &task)
        .expect("RUST_CONTRACT Assay takes the whole turn without a model");
    match turn {
        // Refusing to measure a workspace that is not the reviewed candidate satisfies the contract.
        Err(_) => {}
        Ok(outcome) => assert_eq!(
            outcome.evidence.candidate_sha.as_deref(),
            Some(CANDIDATE),
            "Assay measured and reported the workspace HEAD instead of the reviewed candidate; release publishes \
             this value"
        ),
    }
}
