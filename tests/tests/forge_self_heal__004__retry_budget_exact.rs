//! FORGE.SELF_HEAL — retry budget exact (TST-FORGE-SELF-HEAL-004).
//!
//! Contract: the bounded attempt loop runs exactly its budget — one initial attempt plus
//! exactly `FORGE_DELIVERABLE_RETRIES` reprompts when enforcement is on, and exactly one
//! attempt when it is off. An off-by-one here either pays for turns nobody asked for or
//! abandons a lane that one more reprompt would have saved.
//!
//! Level: L1 Component — the pure budget function (`forge::engine::self_heal::attempt_budget`)
//! plus the production lifecycle (`forge::roles::lifecycle::run_forge_role_turn`) that reads it,
//! with a counting double at the `RoleHarness` port. No database, no network, no model.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_self_heal__004__retry_budget_exact

use std::sync::atomic::{AtomicUsize, Ordering};

use forge::engine::assay::CommandResult;
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::self_heal::attempt_budget;
use forge::roles::hooks::NoRoleHooks;
use forge::roles::lifecycle::{run_forge_role_turn, ForgeRoleContext};
use workflow::{Result, TaskStatus};

/// Misses the architect plan every time and counts the attempts it is asked for.
struct CountingHarness {
    turns: AtomicUsize,
}

impl RoleHarness for CountingHarness {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        self.turns.fetch_add(1, Ordering::SeqCst);
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

fn role_task(story_id: &str) -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-004".into(),
        process_instance_id: "proc-004".into(),
        story_id: story_id.into(),
        token_id: Some("tok-004".into()),
        node_id: Some("architect".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],

        write_surface: None,
    }
}

/// Runs one architect turn that always misses, under the given enforcement and retry
/// settings, and returns the number of harness attempts the lifecycle asked for.
fn attempts_for(enforce: &str, retries: &str, story_id: &str) -> usize {
    std::env::set_var("FORGE_ENFORCE_DELIVERABLES", enforce);
    std::env::set_var("FORGE_DELIVERABLE_RETRIES", retries);
    let harness = CountingHarness {
        turns: AtomicUsize::new(0),
    };
    let current = ForgeGateEvidence::default();
    let task = role_task(story_id);
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
    harness.turns.load(Ordering::SeqCst)
}

#[test]
fn forge_self_heal_004__retry_budget_exact() {
    // The pure budget: one initial attempt plus exactly the configured retries, or one
    // attempt flat when enforcement is off.
    assert_eq!(
        attempt_budget(true, 0),
        1,
        "zero retries is the single attempt"
    );
    assert_eq!(attempt_budget(true, 1), 2, "one retry is two attempts");
    assert_eq!(attempt_budget(true, 2), 3, "two retries is three attempts");
    assert_eq!(
        attempt_budget(false, 5),
        1,
        "enforcement off ignores the retry count"
    );
    assert_eq!(
        attempt_budget(false, 0),
        1,
        "enforcement off with no retries is one attempt"
    );

    // The lifecycle honors the budget exactly, end to end.
    assert_eq!(
        attempts_for("1", "2", "TST-RED-B24-004-A"),
        3,
        "two retries run exactly three attempts — no more, no fewer"
    );
    assert_eq!(
        attempts_for("1", "0", "TST-RED-B24-004-B"),
        1,
        "zero retries run exactly one attempt"
    );
    assert_eq!(
        attempts_for("0", "5", "TST-RED-B24-004-C"),
        1,
        "enforcement off runs exactly one attempt whatever the retry count"
    );
}
