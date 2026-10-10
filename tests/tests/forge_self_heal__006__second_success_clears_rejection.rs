//! FORGE.SELF_HEAL — second success clears rejection (TST-FORGE-SELF-HEAL-006).
//!
//! Contract: a rejection belongs to the attempt that earned it. When a reprompted lane delivers
//! on its second attempt, the turn completes with no deliverable rejection and records no hold —
//! the first attempt's miss must not linger on the evidence and hold a story that delivered.
//!
//! Level: L1 Component — the production lifecycle (`forge::roles::lifecycle::run_forge_role_turn`)
//! with a scripted double at the `RoleHarness` port. No database, no network, no model.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_self_heal__006__second_success_clears_rejection

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use forge::engine::assay::CommandResult;
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::writer::RecordingWriter;
use forge::roles::architect::ArchitectHooks;
use forge::roles::lifecycle::{run_forge_role_turn, ForgeRoleContext};
use workflow::{Result, TaskStatus};

const INCOMPLETE: &str = "still thinking, nothing delivered";
const COMPLETE: &str = concat!(
    "plan ready\n",
    "FORGE_ARCHITECT_HANDOFF: {\"version\":1,\"baseRef\":\"base\",",
    "\"findings\":[{\"id\":\"F1\",\"required\":true,",
    "\"summary\":\"valid handoff\",\"scope\":[\"src/example.rs\"],",
    "\"proofs\":[],\"risks\":[]}]}"
);

/// Misses on the first attempt and delivers on the second, recording every directive.
struct ThenDeliversHarness {
    calls: AtomicUsize,
    seen: Mutex<Vec<Option<String>>>,
}

impl RoleHarness for ThenDeliversHarness {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen
            .lock()
            .unwrap()
            .push(self_heal.map(str::to_string));
        let raw = if call == 0 { INCOMPLETE } else { COMPLETE };
        Ok(HarnessOutput {
            raw: raw.into(),
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
fn forge_self_heal_006__second_success_clears_rejection() {
    std::env::set_var("FORGE_ENFORCE_DELIVERABLES", "1");
    std::env::set_var("FORGE_DELIVERABLE_RETRIES", "1");
    let harness = ThenDeliversHarness {
        calls: AtomicUsize::new(0),
        seen: Mutex::new(vec![]),
    };
    let writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = ActiveForgeRoleTask {
        task_id: "task-006".into(),
        process_instance_id: "proc-006".into(),
        story_id: "TST-RED-B24-006".into(),
        token_id: Some("tok-006".into()),
        node_id: Some("architect".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],

        write_surface: None,
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

        execution_id: None,
        write_surface: None,
        model_attempt_control: None,
    };
    let outcome =
        run_forge_role_turn(&ctx, "architect", &task, &ArchitectHooks).expect("turn runs");

    assert_eq!(
        harness.calls.load(Ordering::SeqCst),
        2,
        "the miss is reprompted once and the delivery ends the loop"
    );
    let seen = harness.seen.lock().unwrap().clone();
    assert!(seen[0].is_none(), "the first attempt is the plain prompt");
    assert!(
        seen[1].as_deref().unwrap_or("").contains("architect-plan"),
        "the second attempt answers the corrective directive: {seen:?}"
    );
    assert!(
        outcome.evidence.deliverable_rejection.is_none(),
        "the second attempt's delivery clears the first attempt's miss: {:?}",
        outcome.evidence.deliverable_rejection
    );
    assert_eq!(
        outcome.transition_name.as_deref(),
        Some("complete"),
        "a delivered turn completes"
    );
    // Negative: the reprompted-then-delivered turn holds nothing behind.
    assert!(
        writer.holds.lock().unwrap().is_empty(),
        "a turn that delivered records no human hold"
    );
    assert!(
        writer.opened_holds.lock().unwrap().is_empty(),
        "a turn that delivered opens no hold record"
    );
}
