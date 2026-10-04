//! Pianola escalation rules: the ten guard conditions that surface to the captain.
//!
//! This is the third Pianola slice. It answers one question per story packet:
//! does any guard condition require a human before the worker continues? When
//! yes, the answer is an [`EscalationReason`] the captain rules on; when no,
//! the supervisor's green-path logic owns the lane.
//!
//! System-of-record invariant: Forge stays authoritative. This module creates
//! no table, no trigger, and no queue. The only durable writes are the two
//! existing DAO paths documented on [`record_escalation`]:
//! `ForgeEngineDao::record_tool_artifact` (the `forge_tool_artifact` row) and
//! `ForgeEngineDao::append_run_detail` (the `storyboard_story_run` evidence
//! line). The captain postcard is a file under `docs/agent/postcards/`, never
//! a row anywhere.

use std::path::{Path, PathBuf};

use db::{DbResult, ForgeEngineDao, NewToolArtifact, StoryPacketRow};

use super::supervisor::overlapping_pairs;
use crate::engine::assay::is_rust_contract_production_path;

// ---------------------------------------------------------------------------
// Reason and context
// ---------------------------------------------------------------------------

/// The ten guard conditions. Each one parks the story until the captain
/// rules; none of them may be answered automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EscalationReason {
    /// Goal or acceptance criteria missing, blank, or self-contradictory.
    AmbiguousSpec,
    /// Story touches service boundaries, Abstract Service / MVI patterns, or
    /// architecture decision records.
    ArchConflict,
    /// Two assigned stories claim the same target file.
    OverlappingTarget,
    /// Worker proposes a production file edit outside the test taxonomy.
    ProdChangeNeeded,
    /// Story mentions schema/migration work the packet does not authorize.
    MigrationNeeded,
    /// Assay output touches the security/entitlement surface.
    SecurityEntitlement,
    /// Worker branch attempts push, merge, or publish.
    PublishRequested,
    /// Model output proposes changing the Forge queue, assignment,
    /// deduplication, or release/publish authority.
    ForgeWorkflowMutationProposed,
    /// Same story failed twice with different reasons; the pattern is unclear.
    DoubleFailUnclear,
    /// `origin/main` moved past the worker's base commit.
    MainlineMovementInvalidatesBase,
}

impl EscalationReason {
    /// Stable machine word used in artifacts, logs, and postcard filenames.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AmbiguousSpec => "AmbiguousSpec",
            Self::ArchConflict => "ArchConflict",
            Self::OverlappingTarget => "OverlappingTarget",
            Self::ProdChangeNeeded => "ProdChangeNeeded",
            Self::MigrationNeeded => "MigrationNeeded",
            Self::SecurityEntitlement => "SecurityEntitlement",
            Self::PublishRequested => "PublishRequested",
            Self::ForgeWorkflowMutationProposed => "ForgeWorkflowMutationProposed",
            Self::DoubleFailUnclear => "DoubleFailUnclear",
            Self::MainlineMovementInvalidatesBase => "MainlineMovementInvalidatesBase",
        }
    }

    /// One-line human meaning, kept beside the word so logs read plainly.
    pub fn describe(self) -> &'static str {
        match self {
            Self::AmbiguousSpec => "story spec is missing, blank, or self-contradictory",
            Self::ArchConflict => "story touches service boundaries or architecture records",
            Self::OverlappingTarget => "two workers claim the same target file",
            Self::ProdChangeNeeded => "worker proposes a production file edit",
            Self::MigrationNeeded => "story mentions schema/migration work without authorization",
            Self::SecurityEntitlement => "assay touches the security/entitlement surface",
            Self::PublishRequested => "worker attempts push, merge, or publish",
            Self::ForgeWorkflowMutationProposed => {
                "model output proposes changing Forge queue/assignment/dedup/release authority"
            }
            Self::DoubleFailUnclear => "same story failed twice with different reasons",
            Self::MainlineMovementInvalidatesBase => "mainline moved past the worker base commit",
        }
    }
}

/// Everything `should_escalate` needs beyond the packet itself. All fields
/// are observations the caller already holds (assay text, git actions, commit
/// ids); nothing here is fetched, queued, or invented by this module.
#[derive(Debug, Clone, Default)]
pub struct SupervisorContext {
    /// Every story assigned in this experiment batch (for overlap checks).
    pub assigned_stories: Vec<StoryPacketRow>,
    /// Combined assay command output for this story (security/prod scan).
    pub assay_text: String,
    /// File paths the worker proposes to edit.
    pub proposed_paths: Vec<String>,
    /// True when the packet explicitly authorizes database work.
    pub db_work_authorized: bool,
    /// Git actions the worker attempted (`git push`, `merge`, ...).
    pub worker_git_actions: Vec<String>,
    /// Model output text (Forge-mutation scan).
    pub model_output: String,
    /// Prior failure reasons for this same story, oldest first.
    pub failure_reasons: Vec<String>,
    /// Commit the worker branched from.
    pub worker_base_commit: Option<String>,
    /// Current `origin/main` commit.
    pub main_commit: Option<String>,
}

// ---------------------------------------------------------------------------
// Decision
// ---------------------------------------------------------------------------

/// First guard condition that fires for this packet, in evaluation order, or
/// `None` when no guard fires and the lane stays on the green path.
///
/// Order is deliberate: spec validity first (nothing else is answerable on an
/// ambiguous spec), then batch integrity (overlap), then blast-radius guards
/// (prod, migration, security, publish, Forge mutation), then history guards
/// (double-fail, mainline movement).
pub fn should_escalate(
    packet: &StoryPacketRow,
    context: &SupervisorContext,
) -> Option<EscalationReason> {
    if is_ambiguous(packet) {
        return Some(EscalationReason::AmbiguousSpec);
    }
    if touches_architecture(packet) {
        return Some(EscalationReason::ArchConflict);
    }
    if overlaps_batch(packet, &context.assigned_stories) {
        return Some(EscalationReason::OverlappingTarget);
    }
    if proposes_prod_change(&context.proposed_paths, &context.assay_text) {
        return Some(EscalationReason::ProdChangeNeeded);
    }
    if needs_migration(packet, context.db_work_authorized) {
        return Some(EscalationReason::MigrationNeeded);
    }
    if touches_security(&context.assay_text) {
        return Some(EscalationReason::SecurityEntitlement);
    }
    if requests_publish(&context.worker_git_actions) {
        return Some(EscalationReason::PublishRequested);
    }
    if proposes_forge_mutation(&context.model_output) {
        return Some(EscalationReason::ForgeWorkflowMutationProposed);
    }
    if is_double_fail_unclear(&context.failure_reasons) {
        return Some(EscalationReason::DoubleFailUnclear);
    }
    if mainline_moved(
        context.worker_base_commit.as_deref(),
        context.main_commit.as_deref(),
    ) {
        return Some(EscalationReason::MainlineMovementInvalidatesBase);
    }
    None
}

fn packet_text(packet: &StoryPacketRow) -> String {
    [
        packet.goal.as_deref().unwrap_or(""),
        packet.architect_brief.as_deref().unwrap_or(""),
        packet.acceptance_criteria.as_deref().unwrap_or(""),
        packet.assay_commands.as_deref().unwrap_or(""),
    ]
    .join("\n")
}

/// Goal or acceptance criteria missing/blank, or carrying a contradiction
/// marker. Contradiction is checked narrowly (an explicit `contradict` token,
/// or a must/must-not pair) so ordinary hedging prose does not escalate.
fn is_ambiguous(packet: &StoryPacketRow) -> bool {
    let goal_blank = packet
        .goal
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty();
    let criteria_blank = packet
        .acceptance_criteria
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty();
    if goal_blank || criteria_blank {
        return true;
    }
    let lower = packet_text(packet).to_lowercase();
    if lower.contains("contradict") {
        return true;
    }
    lower.contains("must not") && lower.contains("must ")
}

/// Service-boundary, Abstract Service / MVI, or architecture-record touch.
fn touches_architecture(packet: &StoryPacketRow) -> bool {
    const MARKERS: [&str; 7] = [
        "rust/core/service",
        "abstract service",
        "abstract_service",
        "mvi",
        "architecture decision",
        "architecture-decision",
        "adr-",
    ];
    let lower = packet_text(packet).to_lowercase();
    MARKERS.iter().any(|marker| lower.contains(marker))
}

/// True when this story is one half of an overlapping pair in the batch.
fn overlaps_batch(packet: &StoryPacketRow, assigned: &[StoryPacketRow]) -> bool {
    overlapping_pairs(assigned)
        .iter()
        .any(|(_, a, b)| a == &packet.id || b == &packet.id)
}

/// True when any proposed path (or any path token in the assay text) sits
/// under a production root.
fn proposes_prod_change(proposed: &[String], assay_text: &str) -> bool {
    if proposed
        .iter()
        .any(|path| is_rust_contract_production_path(path.trim()))
    {
        return true;
    }
    super::extract_path_tokens(assay_text)
        .iter()
        .any(|path| is_rust_contract_production_path(path))
}

/// True when the story text names schema/migration work the packet does not
/// authorize. Authorization is the packet's `test_mode` naming database work
/// or the caller setting `db_work_authorized`.
fn needs_migration(packet: &StoryPacketRow, db_work_authorized: bool) -> bool {
    if db_work_authorized {
        return false;
    }
    if packet
        .test_mode
        .as_deref()
        .map(|mode| {
            let lower = mode.to_lowercase();
            lower.contains("db") || lower.contains("migration") || lower.contains("database")
        })
        .unwrap_or(false)
    {
        return false;
    }
    const MARKERS: [&str; 4] = ["migration", "migrate", "schema change", "db schema"];
    let lower = packet_text(packet).to_lowercase();
    MARKERS.iter().any(|marker| lower.contains(marker))
}

/// Narrow token scan mirroring the supervisor's green-path gate.
fn touches_security(assay_text: &str) -> bool {
    const TOKENS: [&str; 3] = ["casbin", "authz", "entitlement"];
    let lower = assay_text.to_lowercase();
    TOKENS.iter().any(|token| lower.contains(token))
}

/// Worker-side push/merge/publish attempt.
fn requests_publish(git_actions: &[String]) -> bool {
    git_actions.iter().any(|action| {
        let lower = action.to_lowercase();
        lower.contains("push")
            || lower.contains("merge")
            || lower.contains("publish")
            || lower.contains("gh pr")
            || lower.contains("release")
    })
}

/// Model output proposing to change the Forge control plane. Matched on
/// distinctive Forge nouns so ordinary "assign this variable" prose cannot
/// fire it; at least one Forge noun plus one mutation verb is required.
fn proposes_forge_mutation(model_output: &str) -> bool {
    const FORGE_NOUNS: [&str; 6] = [
        "agent_work_item",
        "forge queue",
        "deduplication",
        "dedup",
        "release authority",
        "publish authority",
    ];
    const MUTATION_VERBS: [&str; 8] = [
        "change",
        "mutat",
        "modif",
        "bypass",
        "skip",
        "rewrite",
        "reassign",
        "re-dispatch",
    ];
    // A bare distinctive noun is enough for the two that never appear in
    // innocent prose; the rest need a mutation verb beside them.
    const BARE_NOUNS: [&str; 2] = ["agent_work_item", "release authority"];
    let lower = model_output.to_lowercase();
    if BARE_NOUNS.iter().any(|noun| lower.contains(noun)) {
        return true;
    }
    let has_noun = FORGE_NOUNS.iter().any(|noun| lower.contains(noun));
    let has_verb = MUTATION_VERBS.iter().any(|verb| lower.contains(verb));
    has_noun && has_verb
}

/// Two or more prior failures with at least two distinct reasons.
fn is_double_fail_unclear(failure_reasons: &[String]) -> bool {
    if failure_reasons.len() < 2 {
        return false;
    }
    let mut seen = std::collections::HashSet::new();
    for reason in failure_reasons {
        seen.insert(reason.trim().to_lowercase());
    }
    seen.len() >= 2
}

/// Both commits known and different: the ground moved under the worker.
fn mainline_moved(base: Option<&str>, main: Option<&str>) -> bool {
    match (base, main) {
        (Some(base), Some(main)) => {
            let base = base.trim();
            let main = main.trim();
            !base.is_empty() && !main.is_empty() && base != main
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Captain action: literal steps per reason
// ---------------------------------------------------------------------------

/// Literal steps the captain can act on without a follow-up question. Never
/// prose; the postcard renders these numbered.
pub fn captain_action(reason: EscalationReason) -> Vec<String> {
    match reason {
        EscalationReason::AmbiguousSpec => vec![
            "Open the story packet and read goal plus acceptance_criteria end to end.".to_string(),
            "Fill in the missing field or resolve the contradiction in the packet text.".to_string(),
            "Re-dispatch the story with `pnpm forge:story:reset <story> reset --force`, then confirm the worker resumes.".to_string(),
        ],
        EscalationReason::ArchConflict => vec![
            "Read the packet section that names the service boundary or architecture record.".to_string(),
            "Rule whether the story may touch that surface; if yes, name the allowed files in the packet.".to_string(),
            "If no, narrow the packet scope and re-dispatch the worker on the reduced scope.".to_string(),
        ],
        EscalationReason::OverlappingTarget => vec![
            "Run `pnpm forge:batch:status` and note which two stories claim the same file.".to_string(),
            "Keep one story on that file; move the other story to a disjoint target in its packet.".to_string(),
            "Re-dispatch the moved story, then confirm `check_overlapping_targets` passes.".to_string(),
        ],
        EscalationReason::ProdChangeNeeded => vec![
            "Inspect the worker-proposed production paths in the escalation evidence.".to_string(),
            "Approve the production edit explicitly, or direct the worker to stay inside test taxonomy.".to_string(),
            "If approved, authorize the story's production scope in writing before the worker continues.".to_string(),
        ],
        EscalationReason::MigrationNeeded => vec![
            "Confirm whether this story is allowed to touch schema or migrations.".to_string(),
            "If yes, record the authorization and the numbered migration the story must use.".to_string(),
            "If no, remove the schema/migration language from the packet and re-dispatch.".to_string(),
        ],
        EscalationReason::SecurityEntitlement => vec![
            "Review the assay lines that touch casbin, authz, or entitlement evaluation.".to_string(),
            "Decide whether the story is permitted on the security surface; most stories are not.".to_string(),
            "If permitted, name the reviewer who must approve the resulting diff.".to_string(),
        ],
        EscalationReason::PublishRequested => vec![
            "Check the worker branch state; nothing in this experiment publishes.".to_string(),
            "Tell the worker to hold the branch unpushed, or take the push yourself.".to_string(),
            "Confirm no merge or release ran before the worker resumes.".to_string(),
        ],
        EscalationReason::ForgeWorkflowMutationProposed => vec![
            "Read the quoted model output in the escalation evidence.".to_string(),
            "Refuse the Forge control-plane change in the worker chat; the queue, assignment, dedup, and release authority are not worker-editable.".to_string(),
            "If the proposal names a real Forge defect, file it as its own story instead.".to_string(),
        ],
        EscalationReason::DoubleFailUnclear => vec![
            "Read both failure reasons in the escalation evidence.".to_string(),
            "Decide whether the story needs a spec fix, a scope cut, or a third attempt as-is.".to_string(),
            "Reset the story (`pnpm forge:story:reset <story> reset --force`) only after ruling.".to_string(),
        ],
        EscalationReason::MainlineMovementInvalidatesBase => vec![
            "Compare the worker base commit against current `origin/main`.".to_string(),
            "Rebase the worker scope onto the new mainline, or hold the story if the move invalidates its premise.".to_string(),
            "Re-dispatch only after the base is current.".to_string(),
        ],
    }
}

// ---------------------------------------------------------------------------
// Rendering: captain postcard
// ---------------------------------------------------------------------------

/// Render the captain postcard markdown for one escalation. Pure: no I/O, so
/// the exact front matter and fields are unit-assertable.
pub fn format_escalation_for_captain(
    reason: EscalationReason,
    story_id: &str,
    evidence_paths: &[String],
) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("type: note\n");
    out.push_str(&format!("title: Escalation {story_id}\n"));
    out.push_str("tags: [pianola, escalation]\n");
    out.push_str("---\n\n");
    out.push_str(&format!("# Escalation {story_id}\n\n"));
    out.push_str(&format!("- story: {story_id}\n"));
    out.push_str(&format!(
        "- reason: {} ({})\n",
        reason.as_str(),
        reason.describe()
    ));
    out.push_str("- evidence:\n");
    if evidence_paths.is_empty() {
        out.push_str("  - (none recorded)\n");
    } else {
        for path in evidence_paths {
            out.push_str(&format!("  - {path}\n"));
        }
    }
    out.push_str("- required captain action:\n");
    for (index, step) in captain_action(reason).iter().enumerate() {
        out.push_str(&format!("  {}. {}\n", index + 1, step));
    }
    out
}

/// Postcard filename for a story and date string (`YYYY-MM-DD`).
pub fn postcard_filename(story_id: &str, date_str: &str) -> String {
    format!("pianola-escalation-{story_id}-{date_str}.md")
}

/// Write the postcard under `<repo_root>/docs/agent/postcards/`. The only
/// filesystem write in this module; durable Forge state still goes through
/// [`record_escalation`] below, never through a file.
pub fn write_escalation_postcard(
    repo_root: &Path,
    reason: EscalationReason,
    story_id: &str,
    evidence_paths: &[String],
    date_str: &str,
) -> std::io::Result<PathBuf> {
    let dir = repo_root.join("docs/agent/postcards");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(postcard_filename(story_id, date_str));
    std::fs::write(
        &path,
        format_escalation_for_captain(reason, story_id, evidence_paths),
    )?;
    tracing::info!(
        target: "pianola::supervisor",
        story = %story_id,
        reason = %reason.as_str(),
        postcard = %path.display(),
        "pianola escalation postcard written"
    );
    Ok(path)
}

// ---------------------------------------------------------------------------
// Durable recording: existing DAO paths only
// ---------------------------------------------------------------------------

/// Build the `forge_tool_artifact` input for an escalation. `tool` is
/// `pianola`, `kind` is `escalation`, and the verdict is `escalate`: the
/// ruling stays readable from the run row, never taken from a model.
pub fn escalation_artifact(
    reason: EscalationReason,
    story_id: &str,
    story_run_id: Option<&str>,
    evidence_paths: &[String],
) -> NewToolArtifact {
    let detail = serde_json::json!({
        "reason": reason.as_str(),
        "description": reason.describe(),
        "evidence": evidence_paths,
        "captain_action": captain_action(reason),
    });
    NewToolArtifact {
        story_id: story_id.to_string(),
        story_run_id: story_run_id.map(str::to_string),
        tool: "pianola".to_string(),
        kind: "escalation".to_string(),
        verdict: Some("escalate".to_string()),
        summary: Some(format!(
            "{}: {} for {story_id}",
            reason.as_str(),
            reason.describe()
        )),
        detail: Some(detail),
        sha: None,
    }
}

/// Record an escalation through the existing engine paths and nothing else:
///
/// 1. `ForgeEngineDao::record_tool_artifact` — the one sanctioned write of
///    `forge_tool_artifact`.
/// 2. `ForgeEngineDao::append_run_detail` — the sanctioned evidence line on
///    `storyboard_story_run` (skipped when no run id is known).
///
/// No `CREATE TABLE`, no new writer, no second door onto either table.
pub async fn record_escalation(
    engine: &ForgeEngineDao,
    reason: EscalationReason,
    story_id: &str,
    story_run_id: Option<&str>,
    evidence_paths: &[String],
) -> DbResult<()> {
    let input = escalation_artifact(reason, story_id, story_run_id, evidence_paths);
    engine.record_tool_artifact(&input).await?;
    if let Some(run_id) = story_run_id {
        engine
            .append_run_detail(
                run_id,
                &format!("pianola escalation {} for {story_id}", reason.as_str()),
            )
            .await?;
    }
    tracing::info!(
        target: "pianola::supervisor",
        story = %story_id,
        reason = %reason.as_str(),
        "pianola escalation recorded via existing Forge DAO path"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
        second.assay_commands =
            Some("cover forge/src/pianola/escalation.rs with tests".to_string());
        let mut first_overlap = clean_packet("TST-1");
        first_overlap.goal =
            Some("refactor forge/src/pianola/escalation.rs for clarity".to_string());
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
        let artifact =
            escalation_artifact(EscalationReason::AmbiguousSpec, "TST-1", None, &evidence);
        assert_eq!(artifact.tool, "pianola");
        assert_eq!(artifact.kind, "escalation");
        assert_eq!(artifact.story_id, "TST-1");
        assert!(artifact
            .summary
            .as_deref()
            .unwrap_or("")
            .contains("AmbiguousSpec"));
    }
}
