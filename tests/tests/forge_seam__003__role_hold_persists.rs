//! SEAM-003 — a legitimate role HOLD preserves Workflow UUID identities and the human Story id.

#[path = "support/forge_seam.rs"]
mod support;

use std::path::Path;
use std::sync::Arc;

use forge::engine::assay::CommandResult;
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::roles::lead::LeadService;
use forge::roles::AbstractForgeService;
use support::SeamWriter;
use uuid::Uuid;
use workflow::{Result as WorkflowResult, TaskStatus};

struct HoldHarness;

impl RoleHarness for HoldHarness {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> WorkflowResult<HarnessOutput> {
        Ok(HarnessOutput {
            raw: "Lead cannot safely choose an execution shape yet.\n".into(),
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

    fn assay_cwd(&self) -> &Path {
        Path::new(".")
    }

    fn run_command(&self, command: &str) -> CommandResult {
        CommandResult {
            command: command.into(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output: String::new(),
        }
    }
}

#[test]
fn role_hold_keeps_uuid_task_and_process_identity_and_human_story_id() {
    let writer = Arc::new(SeamWriter::default());
    let task_id = Uuid::new_v4().to_string();
    let process_instance_id = Uuid::new_v4().to_string();
    let story_id = "ENG-FORGE-SEAM-HOLD-003";
    let task = ActiveForgeRoleTask {
        task_id: task_id.clone(),
        process_instance_id: process_instance_id.clone(),
        story_id: story_id.into(),
        token_id: None,
        node_id: Some("lead_pre".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec![],
    };
    let runner = ProductionRoleRunner::new(
        Arc::new(HoldHarness),
        ForgeGateEvidence {
            work_type: Some("FEATURE".into()),
            ..Default::default()
        },
    )
    .with_writer(writer.clone())
    .with_story_run(Some("run-hold-003".into()));

    let outcome = LeadService::new(&runner)
        .execute("lead_pre", &task)
        .expect("a role verdict is an outcome, not an engine failure");
    assert!(
        outcome
            .evidence
            .deliverable_rejection
            .as_deref()
            .unwrap_or_default()
            .contains("lead-decision"),
        "the hold must be caused by the role's missing decision (the gate names it `lead-decision`)"
    );

    let holds = writer.opened_holds.lock().expect("holds");
    assert_eq!(holds.len(), 1);
    let hold = &holds[0];
    assert_eq!(hold.task_id.as_deref(), Some(task_id.as_str()));
    assert_eq!(hold.process_instance_id, process_instance_id);
    assert_eq!(hold.story_id, story_id);
    assert_eq!(hold.originating_node.as_deref(), Some("lead_pre"));
    assert_eq!(
        hold.reason,
        outcome.evidence.deliverable_rejection.clone().unwrap()
    );

    Uuid::parse_str(&hold.process_instance_id)
        .expect("process_instance_id remains UUID-compatible");
    Uuid::parse_str(hold.task_id.as_deref().expect("task id present"))
        .expect("task_id remains UUID-compatible");
    assert!(
        Uuid::parse_str(&hold.story_id).is_err(),
        "story id remains the human Story ID"
    );
}
