//! FORGE.SELF_HEAL — exhausted retry creates Hold (TST-FORGE-SELF-HEAL-007).
//!
//! Contract: when the attempt budget runs out with the deliverable still missing, the turn does
//! not vanish into a `complete` with an empty handoff. The lifecycle records the story's Hold —
//! a human hold against the story plus a `DELIVERABLE_REJECTED` hold record — so a person sees
//! the lane that would not deliver.
//!
//! Level: L1 Component — the production lifecycle (`forge::roles::lifecycle::run_forge_role_turn`)
//! with a never-delivering double at the `RoleHarness` port and the recording writer. No
//! database, no network, no model.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_self_heal__007__exhausted_retry_creates_hold

use std::sync::atomic::{AtomicUsize, Ordering};

use forge::engine::assay::CommandResult;
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::writer::RecordingWriter;
use forge::roles::architect::ArchitectHooks;
use forge::roles::lifecycle::{run_forge_role_turn, ForgeRoleContext};
use workflow::{Result, TaskStatus};

/// Never delivers the architect plan and counts the attempts it is asked for.
struct NeverDeliversHarness {
    calls: AtomicUsize,
}

/// Delivers the architect plan on the first attempt.
struct DeliversHarness;

fn output(raw: &str) -> HarnessOutput {
    HarnessOutput {
        raw: raw.into(),
        candidate_sha: None,
        assay_commands: vec![],
        acceptance_mapped: false,
        refusal: None,
        execution_base: None,
        usage: None,
    }
}

fn stub_command(command: &str) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: 1,
        cancelled: false,
        passed: false,
        excerpt: "not run by this harness".into(),
        unmeasurable: true,
        output: String::new(),
    }
}

impl RoleHarness for NeverDeliversHarness {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(output("still thinking, nothing delivered"))
    }

    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }

    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }

    fn run_command(&self, command: &str) -> CommandResult {
        stub_command(command)
    }
}

impl RoleHarness for DeliversHarness {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        Ok(output(concat!(
            "plan ready\n",
            "FORGE_ARCHITECT_HANDOFF: {\"version\":1,\"baseRef\":\"base\",",
            "\"findings\":[{\"id\":\"F1\",\"required\":true,",
            "\"summary\":\"valid handoff\",\"scope\":[\"src/example.rs\"],",
            "\"proofs\":[],\"risks\":[]}]}",
        )))
    }

    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }

    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }

    fn run_command(&self, command: &str) -> CommandResult {
        stub_command(command)
    }
}

fn role_task(task_id: &str, story_id: &str) -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: task_id.into(),
        process_instance_id: "proc-007".into(),
        story_id: story_id.into(),
        token_id: Some("tok-007".into()),
        node_id: Some("architect".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],
        write_surface: None,
    }
}

#[test]
fn forge_self_heal_007__exhausted_retry_creates_hold() {
    std::env::set_var("FORGE_ENFORCE_DELIVERABLES", "1");
    std::env::set_var("FORGE_DELIVERABLE_RETRIES", "1");
    let hooks = ArchitectHooks;

    // Positive: the budget runs out with nothing delivered, so the story holds.
    let harness = NeverDeliversHarness {
        calls: AtomicUsize::new(0),
    };
    let writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("task-007", "TST-RED-B24-007");
    let ctx = ForgeRoleContext {
        harness: &harness,
        execution_id: None,
        write_surface: None,
        current: &current,
        writer: Some(&writer),
        story_run_id: None,
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
        model_attempt_control: None,
    };
    let outcome = run_forge_role_turn(&ctx, "architect", &task, &hooks).expect("turn runs");

    assert_eq!(
        harness.calls.load(Ordering::SeqCst),
        2,
        "both budgeted attempts run before the hold"
    );
    let rejection = outcome
        .evidence
        .deliverable_rejection
        .as_deref()
        .expect("the exhausted turn carries a rejection");
    assert!(
        rejection.contains("no readable handoff"),
        "the rejection names the undelivered requirement: {rejection}"
    );
    let holds = writer.holds.lock().unwrap().clone();
    assert_eq!(
        holds.len(),
        1,
        "the exhausted turn marks exactly one human hold: {holds:?}"
    );
    assert_eq!(
        holds[0].0, "TST-RED-B24-007",
        "the hold is against this story"
    );
    assert!(
        holds[0].1.contains("no readable handoff"),
        "the hold reason names the undelivered requirement: {:?}",
        holds[0].1
    );
    let opened = writer.opened_holds.lock().unwrap().clone();
    assert_eq!(
        opened.len(),
        1,
        "the exhausted turn opens exactly one hold record: {opened:?}"
    );
    assert_eq!(
        opened[0].0, "TST-RED-B24-007",
        "the hold record is against this story"
    );
    assert_eq!(
        opened[0].1, "DELIVERABLE_REJECTED",
        "the hold record carries the deliverable-rejected failure class"
    );

    // Negative: a turn that delivers inside its budget holds nothing.
    let delivers = DeliversHarness;
    let clean_writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("task-007-clean", "TST-RED-B24-007-CLEAN");
    let ctx = ForgeRoleContext {
        harness: &delivers,
        execution_id: None,
        write_surface: None,
        current: &current,
        writer: Some(&clean_writer),
        story_run_id: None,
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
        model_attempt_control: None,
    };
    let outcome = run_forge_role_turn(&ctx, "architect", &task, &hooks).expect("turn runs");
    assert!(
        outcome.evidence.deliverable_rejection.is_none(),
        "a delivered turn carries no rejection"
    );
    assert!(
        clean_writer.holds.lock().unwrap().is_empty(),
        "a delivered turn marks no human hold"
    );
    assert!(
        clean_writer.opened_holds.lock().unwrap().is_empty(),
        "a delivered turn opens no hold record"
    );
}
