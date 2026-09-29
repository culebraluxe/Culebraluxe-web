//! The self-heal retry must carry the corrective directive — the port built it and threw it away.

use std::sync::Mutex;

use forge::engine::*;
use workflow::Result;

/// A harness that records every directive the runner hands it. It answers with the same incomplete architect
/// output every time, so the directive is the only thing that can differ between the two calls.
struct DirectiveRecordingHarness {
    seen: Mutex<Vec<Option<String>>>,
}

impl runner::RoleHarness for DirectiveRecordingHarness {
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }

    fn run_role(
        &self,
        _n: &str,
        _t: &runtime::ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<runner::HarnessOutput> {
        self.seen
            .lock()
            .unwrap()
            .push(self_heal.map(str::to_string));
        Ok(runner::HarnessOutput {
            raw: "I thought about the plan".into(),
            candidate_sha: None,
            assay_commands: vec![],
            acceptance_mapped: false,
        })
    }

    fn exists_on_base_ref(&self, _b: &str, _p: &str) -> bool {
        true
    }

    fn run_command(&self, command: &str) -> assay::CommandResult {
        assay::CommandResult {
            command: command.into(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output: String::new(),
        }
    }
}

/// The retry carries the directive naming what the previous attempt missed.
///
/// 2026-09-29: `ProductionRoleRunner` computed this directive and discarded it (`let _directive = …`), so a
/// "bounded corrective retry" re-sent the same prompt and could only reproduce the same omission. This test
/// fails on the old code — the second call arrives with `self_heal == None`.
///
/// It lives in its own test binary on purpose: it sets `FORGE_DELIVERABLE_RETRIES` in the process environment and
/// must not leak that into another test's attempt budget.
#[test]
fn the_retry_carries_the_corrective_directive() {
    std::env::set_var("FORGE_DELIVERABLE_RETRIES", "1");
    let harness = DirectiveRecordingHarness {
        seen: Mutex::new(vec![]),
    };
    let role = runner::ProductionRoleRunner::new(&harness, ForgeGateEvidence::default());
    let task = runtime::ActiveForgeRoleTask {
        task_id: "t".into(),
        process_instance_id: "p".into(),
        story_id: "ENG-GUARD-REPO-RUST-01".into(),
        token_id: Some("k".into()),
        node_id: Some("architect".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],
    };
    let out = executor::ForgeRoleRunner::run(&role, "architect", &task).unwrap();

    let seen = harness.seen.lock().unwrap().clone();
    assert_eq!(
        seen.len(),
        2,
        "an architect with no handoff is attempted twice: {seen:?}"
    );
    assert!(seen[0].is_none(), "the first attempt is the plain prompt");
    let directive = seen[1].as_deref().expect("the retry carries a directive");
    assert!(directive.contains("SELF-HEAL REPROMPT"), "{directive}");
    assert!(
        directive.contains("ARCHITECT"),
        "the directive names the omission: {directive}"
    );
    assert!(out.evidence.deliverable_rejection.is_some());
}
