// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;

fn packet(
    id: &str,
    goal: Option<&str>,
    criteria: Option<&str>,
    brief: Option<&str>,
) -> StoryPacketRow {
    StoryPacketRow {
        id: id.to_string(),
        title: format!("story {id}"),
        goal: goal.map(str::to_string),
        architect_brief: brief.map(str::to_string),
        acceptance_criteria: criteria.map(str::to_string),
        test_mode: None,
        assay_commands: None,
    }
}

fn clean_packet(id: &str) -> StoryPacketRow {
    let mut row = packet(
        id,
        Some("add contract tests for the assay gate"),
        Some("tests compile and the gate stays green"),
        Some("plain brief with no markers"),
    );
    row.assay_commands = Some("cargo test -p forge --lib pianola".to_string());
    row
}

fn clean_context() -> SupervisorContext {
    SupervisorContext {
        assigned_stories: vec![clean_packet("TST-1")],
        ..Default::default()
    }
}

#[test]
fn no_guard_fires_on_clean_packet() {
    let row = clean_packet("TST-1");
    assert_eq!(should_escalate(&row, &clean_context()), None);
}

#[test]
fn empty_goal_is_ambiguous() {
    let row = packet("TST-1", None, Some("something"), None);
    assert_eq!(
        should_escalate(&row, &clean_context()),
        Some(EscalationReason::AmbiguousSpec)
    );
}

#[test]
fn empty_criteria_is_ambiguous() {
    let row = packet("TST-1", Some("goal"), Some("   "), None);
    assert_eq!(
        should_escalate(&row, &clean_context()),
        Some(EscalationReason::AmbiguousSpec)
    );
}

#[test]
fn contradictory_spec_is_ambiguous() {
    let row = packet(
        "TST-1",
        Some("goal text here"),
        Some("acceptance text here"),
        Some("this brief contradicts the goal above"),
    );
    assert_eq!(
        should_escalate(&row, &clean_context()),
        Some(EscalationReason::AmbiguousSpec)
    );
}

#[test]
fn architecture_surface_escalates() {
    let row = packet(
        "TST-1",
        Some("refactor the Abstract Service boundary"),
        Some("service seam stays covered"),
        None,
    );
    assert_eq!(
        should_escalate(&row, &clean_context()),
        Some(EscalationReason::ArchConflict)
    );
}

#[test]
fn overlapping_batch_escalates() {
    let first = clean_packet("TST-1");
    let mut second = clean_packet("TST-2");
    second.goal = Some("cover forge/src/pianola/escalation.rs with tests".to_string());
    second.assay_commands = Some("cover forge/src/pianola/escalation.rs with tests".to_string());
    let mut first_overlap = clean_packet("TST-1");
    first_overlap.goal = Some("refactor forge/src/pianola/escalation.rs for clarity".to_string());
    first_overlap.assay_commands =
        Some("refactor forge/src/pianola/escalation.rs for clarity".to_string());
    let context = SupervisorContext {
        assigned_stories: vec![first_overlap.clone(), second.clone()],
        ..Default::default()
    };
    assert_eq!(
        should_escalate(&first_overlap, &context),
        Some(EscalationReason::OverlappingTarget)
    );
    let _ = first;
}

#[test]
fn prod_path_proposal_escalates() {
    let row = clean_packet("TST-1");
    let context = SupervisorContext {
        proposed_paths: vec!["middle/workflow/src/engine/engine_options.rs".to_string()],
        ..clean_context()
    };
    assert_eq!(
        should_escalate(&row, &context),
        Some(EscalationReason::ProdChangeNeeded)
    );
}

#[test]
fn unauthorized_migration_escalates_but_authorized_does_not() {
    let mut row = clean_packet("TST-1");
    row.goal = Some("add the migration for the new ledger table".to_string());
    let denied = SupervisorContext {
        db_work_authorized: false,
        ..clean_context()
    };
    assert_eq!(
        should_escalate(&row, &denied),
        Some(EscalationReason::MigrationNeeded)
    );
    let allowed = SupervisorContext {
        db_work_authorized: true,
        ..clean_context()
    };
    assert_eq!(should_escalate(&row, &allowed), None);
}

#[test]
fn security_touch_escalates() {
    let row = clean_packet("TST-1");
    let context = SupervisorContext {
        assay_text: "touches casbin policy evaluation".to_string(),
        ..clean_context()
    };
    assert_eq!(
        should_escalate(&row, &context),
        Some(EscalationReason::SecurityEntitlement)
    );
}

#[test]
fn publish_attempt_escalates() {
    let row = clean_packet("TST-1");
    let context = SupervisorContext {
        worker_git_actions: vec!["git push origin lane/tst-1".to_string()],
        ..clean_context()
    };
    assert_eq!(
        should_escalate(&row, &context),
        Some(EscalationReason::PublishRequested)
    );
}

#[test]
fn forge_mutation_proposal_escalates() {
    let row = clean_packet("TST-1");
    let context = SupervisorContext {
        model_output: "we should change the forge queue assignment logic".to_string(),
        ..clean_context()
    };
    assert_eq!(
        should_escalate(&row, &context),
        Some(EscalationReason::ForgeWorkflowMutationProposed)
    );
}

#[test]
fn double_fail_with_distinct_reasons_escalates() {
    let row = clean_packet("TST-1");
    let distinct = SupervisorContext {
        failure_reasons: vec!["timeout".to_string(), "assertion failed".to_string()],
        ..clean_context()
    };
    assert_eq!(
        should_escalate(&row, &distinct),
        Some(EscalationReason::DoubleFailUnclear)
    );
    let repeated = SupervisorContext {
        failure_reasons: vec!["timeout".to_string(), "timeout".to_string()],
        ..clean_context()
    };
    assert_eq!(should_escalate(&row, &repeated), None);
    let single = SupervisorContext {
        failure_reasons: vec!["timeout".to_string()],
        ..clean_context()
    };
    assert_eq!(should_escalate(&row, &single), None);
}

#[test]
fn mainline_movement_escalates() {
    let row = clean_packet("TST-1");
    let moved = SupervisorContext {
        worker_base_commit: Some("aaa".to_string()),
        main_commit: Some("bbb".to_string()),
        ..clean_context()
    };
    assert_eq!(
        should_escalate(&row, &moved),
        Some(EscalationReason::MainlineMovementInvalidatesBase)
    );
    let steady = SupervisorContext {
        worker_base_commit: Some("aaa".to_string()),
        main_commit: Some("aaa".to_string()),
        ..clean_context()
    };
    assert_eq!(should_escalate(&row, &steady), None);
}

#[test]
fn postcard_carries_front_matter_and_literal_steps() {
    let evidence = vec!["forge/src/pianola/escalation.rs:10-20".to_string()];
    let body =
        format_escalation_for_captain(EscalationReason::OverlappingTarget, "TST-2", &evidence);
    assert!(body.contains("type: note"));
    assert!(body.contains("title: Escalation TST-2"));
    assert!(body.contains("- story: TST-2"));
    assert!(body.contains("OverlappingTarget"));
    assert!(body.contains("forge/src/pianola/escalation.rs:10-20"));
    assert!(body.contains("required captain action"));
    assert!(body.contains("pnpm forge:batch:status"));
}

#[test]
fn artifact_uses_existing_tool_kind_shape() {
    let evidence = vec!["forge/src/pianola/escalation.rs:10-20".to_string()];
    let artifact = escalation_artifact(EscalationReason::AmbiguousSpec, "TST-1", None, &evidence);
    assert_eq!(artifact.tool, "pianola");
    assert_eq!(artifact.kind, "escalation");
    assert_eq!(artifact.story_id, "TST-1");
    assert!(artifact
        .summary
        .as_deref()
        .unwrap_or("")
        .contains("AmbiguousSpec"));
}
