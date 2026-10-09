//! FORGE.SELF_HEAL — only missing requirements named (TST-FORGE-SELF-HEAL-003).
//!
//! Contract: the corrective directive names exactly the deliverables the attempt missed —
//! and nothing it already delivered. A directive that also names satisfied requirements sends
//! the lane to re-do delivered work, which is how a bounded retry burns its whole budget
//! without converging.
//!
//! Level: L1 Component — the production lifecycle (`forge::roles::lifecycle::run_forge_role_turn`)
//! with a recording double at the `RoleHarness` port. No database, no network, no model.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_self_heal__003__only_missing_requirements_named

use std::sync::Mutex;

use forge::engine::assay::CommandResult;
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::roles::hooks::NoRoleHooks;
use forge::roles::lifecycle::{run_forge_role_turn, ForgeRoleContext};
use workflow::{Result, TaskStatus};

/// Always misses the architect plan and records the directive, with a reply worded so no
/// other requirement name can leak into the directive through the echoed previous answer.
struct OnlyMissingHarness {
    seen: Mutex<Vec<Option<String>>>,
}

impl RoleHarness for OnlyMissingHarness {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        self.seen
            .lock()
            .unwrap()
            .push(self_heal.map(str::to_string));
        Ok(HarnessOutput {
            raw: "still thinking, nothing delivered".into(),
            candidate_sha: None,
            assay_commands: vec![],
            acceptance_mapped: false,
            refusal: None,
            execution_base: None,
            usage: None,
        })
    }

    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }

    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }

    fn run_command(&self, command: &str) -> CommandResult {
        CommandResult {
            command: command.into(),
            exit_code: 1,
            passed: false,
            excerpt: "not run by this harness".into(),
            unmeasurable: true,
            output: String::new(),

            cancelled: false,
        }
    }
}

#[test]
fn forge_self_heal_003__only_missing_requirements_named() {
    std::env::set_var("FORGE_ENFORCE_DELIVERABLES", "1");
    std::env::set_var("FORGE_DELIVERABLE_RETRIES", "1");
    let harness = OnlyMissingHarness {
        seen: Mutex::new(vec![]),
    };
    let current = ForgeGateEvidence::default();
    let task = ActiveForgeRoleTask {
        task_id: "task-003".into(),
        process_instance_id: "proc-003".into(),
        story_id: "TST-RED-B24-003".into(),
        token_id: Some("tok-003".into()),
        node_id: Some("architect".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],

        write_surface: None,
    };
    let ctx = ForgeRoleContext {
        harness: &harness,
        current: &current,
        writer: None,
        story_run_id: None,
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,

        execution_id: None,
        write_surface: None,
        model_attempt_control: None,
    };
    run_forge_role_turn(&ctx, "architect", &task, &NoRoleHooks).expect("turn runs");

    let seen = harness.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2, "a missed deliverable is retried: {seen:?}");
    let directive = seen[1].as_deref().expect("the retry carries a directive");
    assert!(
        directive.contains("architect-plan"),
        "the directive names the missing requirement: {directive}"
    );
    // Every other requirement the gate knows must stay out of the directive: none of them
    // is missing on an architect turn, so naming one would re-ask delivered work.
    for satisfied in [
        "smith-candidate",
        "qa-verdict",
        "lead-decision",
        "devops-receipt",
        "scout-packet",
        "failure-class",
    ] {
        assert!(
            !directive.contains(satisfied),
            "the directive must not name satisfied requirement {satisfied}: {directive}"
        );
    }
}
