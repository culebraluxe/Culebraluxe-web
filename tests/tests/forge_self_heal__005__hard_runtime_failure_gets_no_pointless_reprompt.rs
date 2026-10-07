//! FORGE.SELF_HEAL — hard runtime failure gets no pointless reprompt (TST-FORGE-SELF-HEAL-005).
//!
//! Contract: the self-heal loop reprompts a lane that *answered* incompletely. A harness that
//! fails outright — a crash, a transport error, a dead subprocess — produced no answer to
//! repair, so the lifecycle must surface the error immediately instead of spending a reprompt
//! (and model money) on a retry that has no previous answer to embed.
//!
//! Level: L1 Component — the production lifecycle (`forge::roles::lifecycle::run_forge_role_turn`)
//! with a failing double at the `RoleHarness` port. No database, no network, no model.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_self_heal__005__hard_runtime_failure_gets_no_pointless_reprompt

use std::sync::atomic::{AtomicUsize, Ordering};

use forge::engine::assay::CommandResult;
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::writer::RecordingWriter;
use forge::roles::hooks::NoRoleHooks;
use forge::roles::lifecycle::{run_forge_role_turn, ForgeRoleContext};
use workflow::{Result, TaskStatus, WorkflowError};

/// Fails every turn the way a crashed harness does, and counts how many turns it was asked for.
struct CrashingHarness {
    turns: AtomicUsize,
}

impl RoleHarness for CrashingHarness {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        self.turns.fetch_add(1, Ordering::SeqCst);
        assert!(
            self_heal.is_none(),
            "a reprompt after a crash would arrive with a directive built from no answer"
        );
        Err(WorkflowError::generic("harness subprocess died: exit 137"))
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
fn forge_self_heal_005__hard_runtime_failure_gets_no_pointless_reprompt() {
    std::env::set_var("FORGE_ENFORCE_DELIVERABLES", "1");
    std::env::set_var("FORGE_DELIVERABLE_RETRIES", "3");
    let harness = CrashingHarness {
        turns: AtomicUsize::new(0),
    };
    let writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = ActiveForgeRoleTask {
        task_id: "task-005".into(),
        process_instance_id: "proc-005".into(),
        story_id: "TST-RED-B24-005".into(),
        token_id: Some("tok-005".into()),
        node_id: Some("architect".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],
    };
    let ctx = ForgeRoleContext {
        harness: &harness,
        current: &current,
        writer: Some(&writer),
        story_run_id: None,
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
    };
    let outcome = run_forge_role_turn(&ctx, "architect", &task, &NoRoleHooks);

    assert!(
        outcome.is_err(),
        "a hard harness failure surfaces as a failed turn, not a silent retry loop"
    );
    assert_eq!(
        harness.turns.load(Ordering::SeqCst),
        1,
        "the crash is surfaced after exactly one attempt: no pointless reprompt"
    );
    // A crash is not a rejected deliverable, so it must not be recorded as one.
    assert!(
        writer.holds.lock().unwrap().is_empty(),
        "a hard failure records no deliverable hold"
    );
    assert!(
        writer.opened_holds.lock().unwrap().is_empty(),
        "a hard failure opens no hold record"
    );
}
