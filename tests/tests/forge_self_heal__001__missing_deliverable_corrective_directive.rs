//! FORGE.SELF_HEAL — missing deliverable → corrective directive (TST-FORGE-SELF-HEAL-001).
//!
//! Contract: the shared role-turn lifecycle (`forge::roles::lifecycle::run_forge_role_turn`)
//! is the production boundary every lane retries through. When an attempt leaves a required
//! deliverable missing, the next attempt must carry a corrective directive naming the omission
//! instead of repeating the plain prompt.
//!
//! Level: L1 Component — the production lifecycle with a recording double at the `RoleHarness`
//! port. No database, no network, no model.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_self_heal__001__missing_deliverable_corrective_directive

use std::sync::Mutex;

use forge::engine::assay::CommandResult;
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::roles::hooks::NoRoleHooks;
use forge::roles::lifecycle::{run_forge_role_turn, ForgeRoleContext};
use workflow::{Result, TaskStatus};

/// Records every corrective directive the lifecycle hands it and answers with the reply
/// script it was built with, so the directive is the only thing that can differ per attempt.
struct DirectiveRecordingHarness {
    seen: Mutex<Vec<Option<String>>>,
    reply: String,
}

impl DirectiveRecordingHarness {
    fn incomplete() -> Self {
        Self {
            seen: Mutex::new(vec![]),
            reply: "thinking through the plan, no handoff yet".into(),
        }
    }

    fn complete() -> Self {
        Self {
            seen: Mutex::new(vec![]),
            reply: "plan ready\nFORGE_EVIDENCE_JSON: {\"researchDisposition\":\"IMPLEMENT\"}"
                .into(),
        }
    }
}

impl RoleHarness for DirectiveRecordingHarness {
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
            raw: self.reply.clone(),
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

fn role_task(story_id: &str) -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-001".into(),
        process_instance_id: "proc-001".into(),
        story_id: story_id.into(),
        token_id: Some("tok-001".into()),
        node_id: Some("architect".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],

        write_surface: None,
    }
}

fn context<'a>(
    harness: &'a dyn RoleHarness,
    current: &'a ForgeGateEvidence,
) -> ForgeRoleContext<'a> {
    ForgeRoleContext {
        harness,
        current,
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
    }
}

#[test]
fn forge_self_heal_001__missing_deliverable_corrective_directive() {
    std::env::set_var("FORGE_ENFORCE_DELIVERABLES", "1");
    std::env::set_var("FORGE_DELIVERABLE_RETRIES", "1");
    let hooks = NoRoleHooks;

    // Positive: an attempt that misses the architect plan is retried WITH a directive.
    let harness = DirectiveRecordingHarness::incomplete();
    let current = ForgeGateEvidence::default();
    let task = role_task("TST-RED-B24-001");
    let ctx = context(&harness, &current);
    let outcome = run_forge_role_turn(&ctx, "architect", &task, &hooks).expect("turn runs");

    let seen = harness.seen.lock().unwrap().clone();
    assert_eq!(
        seen.len(),
        2,
        "a missed deliverable is attempted twice: {seen:?}"
    );
    assert!(
        seen[0].is_none(),
        "the first attempt is the plain prompt: {seen:?}"
    );
    let directive = seen[1]
        .as_deref()
        .expect("the retry carries a corrective directive");
    assert!(
        directive.contains("SELF-HEAL REPROMPT"),
        "the directive is a reprompt: {directive}"
    );
    assert!(
        directive.contains("architect-plan"),
        "the directive names the missing deliverable: {directive}"
    );
    assert!(
        outcome.evidence.deliverable_rejection.is_some(),
        "the still-missing deliverable is rejected after the budget runs out"
    );

    // Negative: a turn that delivers on the first attempt is never reprompted.
    let clean = DirectiveRecordingHarness::complete();
    let current = ForgeGateEvidence::default();
    let task = role_task("TST-RED-B24-001-CLEAN");
    let ctx = context(&clean, &current);
    let outcome = run_forge_role_turn(&ctx, "architect", &task, &hooks).expect("turn runs");

    let seen = clean.seen.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![None],
        "a delivered turn runs once, with the plain prompt and no directive: {seen:?}"
    );
    assert!(
        outcome.evidence.deliverable_rejection.is_none(),
        "a delivered turn carries no rejection"
    );
    assert_eq!(
        outcome.transition_name.as_deref(),
        Some("complete"),
        "a delivered turn completes"
    );
}
