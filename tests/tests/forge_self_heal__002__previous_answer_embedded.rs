//! FORGE.SELF_HEAL — previous answer embedded (TST-FORGE-SELF-HEAL-002).
//!
//! Contract: the corrective directive the lifecycle hands a retry must embed the attempt's own
//! previous reply (`YOUR PREVIOUS REPLY (repair this; do NOT start over)`), so the lane repairs
//! its answer instead of starting over and reproducing the same omission.
//!
//! Level: L1 Component — the production lifecycle (`forge::roles::lifecycle::run_forge_role_turn`)
//! with a recording double at the `RoleHarness` port. No database, no network, no model.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_self_heal__002__previous_answer_embedded

use std::sync::Mutex;

use forge::engine::assay::CommandResult;
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::roles::hooks::NoRoleHooks;
use forge::roles::lifecycle::{run_forge_role_turn, ForgeRoleContext};
use workflow::{Result, TaskStatus};

/// Answers every attempt with the same incomplete reply and records the directive each
/// attempt arrived with, so the retry's directive is the only thing under test.
struct PreviousAnswerHarness {
    seen: Mutex<Vec<Option<String>>>,
}

impl RoleHarness for PreviousAnswerHarness {
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
            raw: "first-draft-plan-needs-handoff-002".into(),
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
        }
    }
}

#[test]
fn forge_self_heal_002__previous_answer_embedded() {
    std::env::set_var("FORGE_ENFORCE_DELIVERABLES", "1");
    // Two retries so the second retry's directive embeds the first retry's answer too.
    std::env::set_var("FORGE_DELIVERABLE_RETRIES", "2");
    let harness = PreviousAnswerHarness {
        seen: Mutex::new(vec![]),
    };
    let current = ForgeGateEvidence::default();
    let task = ActiveForgeRoleTask {
        task_id: "task-002".into(),
        process_instance_id: "proc-002".into(),
        story_id: "TST-RED-B24-002".into(),
        token_id: Some("tok-002".into()),
        node_id: Some("architect".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],
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
    };
    run_forge_role_turn(&ctx, "architect", &task, &NoRoleHooks).expect("turn runs");

    let seen = harness.seen.lock().unwrap().clone();
    assert_eq!(
        seen.len(),
        3,
        "budget 1 + 2 retries runs three attempts: {seen:?}"
    );
    // Negative: the first attempt has no previous answer, so there is nothing to embed.
    assert!(
        seen[0].is_none(),
        "the first attempt arrives with no directive at all: {seen:?}"
    );
    let first_retry = seen[1]
        .as_deref()
        .expect("the first retry carries a directive");
    assert!(
        first_retry.contains("architect-plan"),
        "the first retry names the omission: {first_retry}"
    );
    assert!(
        !first_retry.contains("YOUR PREVIOUS REPLY"),
        "the first retry embeds nothing: no previous answer exists yet: {first_retry}"
    );
    let second_retry = seen[2]
        .as_deref()
        .expect("the second retry carries a directive");
    assert!(
        second_retry.contains("YOUR PREVIOUS REPLY"),
        "the second retry embeds the previous-answer block: {second_retry}"
    );
    assert!(
        second_retry.contains("first-draft-plan-needs-handoff-002"),
        "the second retry embeds the previous answer's own text: {second_retry}"
    );
    assert!(
        second_retry.contains("do NOT start over"),
        "the second retry orders a repair, not a restart: {second_retry}"
    );
}
