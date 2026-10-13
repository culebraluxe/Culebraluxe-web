//! Trajectory telemetry: what an agent execution DID, as observable events, summarized per run.
//!
//! This is measurement, not control. The workflow XML still decides what happens next; nothing here changes how an agent
//! is driven. It records enough to answer, from real runs, questions such as "how long until the first edit" and "how many
//! verification calls came after the last edit", which total token counts cannot answer.
//!
//! THE EVENTS come from the vendor's event stream, which already reports every tool call with its name and a timestamp.
//! Nothing here reads hidden reasoning; only the calls and their order are kept.
//!
//! PROFILES. Each execution is tagged with a harness profile id. Phase 1 changes no behavior, so every role is recorded
//! under the profile that describes how it already runs today. The ids are versioned: an id is never reused for a
//! different meaning, and a changed meaning gets a new version.

use serde::{Deserialize, Serialize};

/// The harness profile a role runs under. Phase 1 names the current behavior; later phases add alternatives beside it.
pub const SMITH_STRUCTURED_V1: &str = "smith-structured-v1";
pub const SCOUT_READ_V1: &str = "scout-read-v1";
pub const ARCHITECT_ANALYSIS_V1: &str = "architect-analysis-v1";
pub const ASSAY_DETERMINISTIC_V1: &str = "assay-deterministic-v1";
pub const DEVOPS_RELEASE_V1: &str = "devops-release-v1";
/// Any role not named above runs under this generic profile, so every execution still carries a profile id.
pub const GENERIC_V1: &str = "generic-v1";

/// The profile for a workflow node. Nodes name their role (`smith`, `feature_scout`, `qa_verify`, `architect`,
/// `dev_ops_release`); the first role word found decides. A node that names no role is generic, never guessed.
pub fn profile_id_for_node(node_id: &str) -> &'static str {
    let lower = node_id.trim().to_ascii_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_ascii_alphanumeric()).collect();
    for word in words {
        let profile = match word {
            "smith" => SMITH_STRUCTURED_V1,
            "scout" => SCOUT_READ_V1,
            "architect" => ARCHITECT_ANALYSIS_V1,
            "assay" | "qa" => ASSAY_DETERMINISTIC_V1,
            "dev" | "devops" | "release" => DEVOPS_RELEASE_V1,
            _ => continue,
        };
        return profile;
    }
    GENERIC_V1
}

/// The profile id for a role, by its name. Deterministic: the same role always gets the same id.
pub fn profile_id_for_role(role: &str) -> &'static str {
    match role.trim().to_ascii_lowercase().as_str() {
        "smith" => SMITH_STRUCTURED_V1,
        "scout" => SCOUT_READ_V1,
        "architect" => ARCHITECT_ANALYSIS_V1,
        "assay" | "qa" => ASSAY_DETERMINISTIC_V1,
        "dev_ops" | "devops" | "release" => DEVOPS_RELEASE_V1,
        _ => GENERIC_V1,
    }
}

/// What kind of action a tool call was. Read from the tool's name only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolKind {
    /// A file mutation (an edit, a write, a patch).
    Edit,
    /// A shell command. Verification (`cargo check`, `cargo test`) runs here.
    Shell,
    /// Anything else: reads, searches, lists, structured operations.
    Structured,
}

/// Classify a vendor tool name. Matching is case-insensitive and on the name's tokens, so `apply_patch` is an edit and
/// `shell` is a shell call even when the vendor adds a suffix.
pub fn classify_tool(name: &str) -> ToolKind {
    let lower = name.trim().to_ascii_lowercase();
    if lower.contains("edit") || lower.contains("write") || lower.contains("patch") {
        ToolKind::Edit
    } else if lower == "shell" || lower == "bash" || lower.contains("shell") {
        ToolKind::Shell
    } else {
        ToolKind::Structured
    }
}

/// One tool call, in the order the vendor reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// The vendor's own timestamp for the call, in milliseconds since the epoch.
    pub at_ms: i64,
    pub tool: String,
}

/// The behavioral summary of one execution. Every count is measured; an absent edit is `None`, never a fabricated zero.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrajectoryMetrics {
    pub tool_call_count: usize,
    pub edit_call_count: usize,
    pub shell_call_count: usize,
    pub structured_call_count: usize,
    pub first_edit_at_ms: Option<i64>,
    pub last_edit_at_ms: Option<i64>,
    /// Shell calls made after the last edit. A run that edits and then verifies shows its verification cost here.
    pub verification_calls_after_last_edit: usize,
}

/// Summarize a run's tool calls. The calls are summarized in the order they were reported, not re-sorted, so the sequence
/// the agent actually followed is what is measured.
pub fn summarize(calls: &[ToolCall]) -> TrajectoryMetrics {
    let mut metrics = TrajectoryMetrics {
        tool_call_count: calls.len(),
        ..TrajectoryMetrics::default()
    };
    for call in calls {
        match classify_tool(&call.tool) {
            ToolKind::Edit => {
                metrics.edit_call_count += 1;
                metrics.first_edit_at_ms.get_or_insert(call.at_ms);
                metrics.last_edit_at_ms = Some(call.at_ms);
            }
            ToolKind::Shell => metrics.shell_call_count += 1,
            ToolKind::Structured => metrics.structured_call_count += 1,
        }
    }
    if let Some(last_edit) = metrics.last_edit_at_ms {
        metrics.verification_calls_after_last_edit = calls
            .iter()
            .filter(|call| call.at_ms > last_edit && classify_tool(&call.tool) == ToolKind::Shell)
            .count();
    }
    metrics
}

/// Milliseconds from the start of the execution to its first edit. `None` when the run never edited.
pub fn time_to_first_edit_ms(started_at_ms: i64, metrics: &TrajectoryMetrics) -> Option<i64> {
    metrics
        .first_edit_at_ms
        .map(|first| (first - started_at_ms).max(0))
}

/// The turn identity a trajectory row is keyed by within its run.
///
/// The key is COMPOSED, never clocked: a repeat of the same settlement lands on the same row, and nothing else can
/// land on it. It mirrors the attempt ledger's own identity — `forge_model_generation_attempt` is unique on
/// `(generation_id, task_id, role_attempt)` — so a REPLAN, which mints a new generation for the same task and role
/// attempt, produces a new key and keeps the previous turn's row instead of overwriting it.
///
/// An uncontrolled lane (hand-run, no attempt controller) has no generation to tell a re-dispatch from a retry, so
/// its key is the task/attempt pair alone; re-dispatching that same task and attempt index then replaces the row,
/// which is the same collapse every pre-293 row had. The format is OPAQUE — a reader must not parse it, and nothing
/// outside this module may compose one.
pub fn trajectory_attempt_key(attempt_key: &str, generation_id: Option<&str>) -> String {
    match generation_id {
        Some(generation) => format!("{attempt_key}@{generation}"),
        None => attempt_key.to_string(),
    }
}

/// The profile's declared version, read from the profile id's trailing `-vN`.
///
/// Profiles carry their version in their id (`smith-structured-v1`), so a comparison can group by version without a
/// second column to keep in step. An id that declares no version has no version — `None`, never a fabricated `v1`.
pub fn profile_version(profile_id: &str) -> Option<i32> {
    let (_, version) = profile_id.rsplit_once("-v")?;
    if version.is_empty() || !version.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    version.parse().ok()
}

/// One turn's trajectory as the lane measured it: the measurement, the identity that makes it findable, and the
/// clock readings taken around the harness call.
///
/// Every optional field means "not known at this seam". `provider` is the backend that ran the turn and `model` is
/// the selection Forge handed that backend; both come from the RESOLVED profile, because that is the configuration the
/// turn executed under. It is not a claim about the vendor's own reading of what it billed: the harness does not
/// report that back to the lane, so it is recorded nowhere rather than guessed.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnTrajectory {
    pub profile_id: String,
    pub profile_version: Option<i32>,
    /// Opaque, stable within the run. See [`trajectory_attempt_key`].
    pub attempt_key: String,
    pub generation_id: Option<String>,
    pub task_id: Option<String>,
    /// The attempt index within the role: 0 is the first turn, N>0 a repair/reprompt turn.
    pub role_attempt: Option<i32>,
    pub workflow_node: Option<String>,
    /// The lane's service key — the identity a lane is registered under.
    pub role: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub turn_started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub turn_ended_at: Option<chrono::DateTime<chrono::Utc>>,
    /// How the turn ended: `completed`, `failed`, `cancelled` (interrupted) or `uncertain` (engine fault).
    pub outcome: Option<String>,
    pub first_edit_latency_ms: Option<i64>,
    pub candidate_sha: Option<String>,
    pub metrics: TrajectoryMetrics,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(at_ms: i64, tool: &str) -> ToolCall {
        ToolCall {
            at_ms,
            tool: tool.to_string(),
        }
    }

    #[test]
    fn tool_names_are_classified_by_what_they_do() {
        assert_eq!(classify_tool("shell"), ToolKind::Shell);
        assert_eq!(classify_tool("Bash"), ToolKind::Shell);
        assert_eq!(classify_tool("edit"), ToolKind::Edit);
        assert_eq!(classify_tool("apply_patch"), ToolKind::Edit);
        assert_eq!(classify_tool("write_file"), ToolKind::Edit);
        assert_eq!(classify_tool("read"), ToolKind::Structured);
        assert_eq!(classify_tool("grep"), ToolKind::Structured);
    }

    #[test]
    fn a_run_with_no_edit_has_no_first_edit_and_no_verification_after_one() {
        let metrics = summarize(&[call(10, "read"), call(20, "shell")]);
        assert_eq!(metrics.tool_call_count, 2);
        assert_eq!(metrics.edit_call_count, 0);
        assert_eq!(metrics.first_edit_at_ms, None);
        assert_eq!(metrics.last_edit_at_ms, None);
        assert_eq!(metrics.verification_calls_after_last_edit, 0);
        assert_eq!(time_to_first_edit_ms(0, &metrics), None);
    }

    #[test]
    fn edits_and_the_verification_after_the_last_edit_are_counted() {
        let metrics = summarize(&[
            call(100, "read"),
            call(200, "edit"),
            call(300, "shell"), // cargo check, before the final edit
            call(400, "edit"),
            call(500, "shell"), // cargo test after the final edit
            call(600, "shell"),
        ]);
        assert_eq!(metrics.tool_call_count, 6);
        assert_eq!(metrics.edit_call_count, 2);
        assert_eq!(metrics.shell_call_count, 3);
        assert_eq!(metrics.structured_call_count, 1);
        assert_eq!(metrics.first_edit_at_ms, Some(200));
        assert_eq!(metrics.last_edit_at_ms, Some(400));
        assert_eq!(metrics.verification_calls_after_last_edit, 2);
        assert_eq!(time_to_first_edit_ms(50, &metrics), Some(150));
    }

    #[test]
    fn the_calls_are_summarized_in_the_order_reported() {
        // An out-of-order timestamp is not silently re-sorted: the first edit is the first one the agent made.
        let metrics = summarize(&[call(500, "edit"), call(100, "edit")]);
        assert_eq!(metrics.first_edit_at_ms, Some(500));
        assert_eq!(metrics.last_edit_at_ms, Some(100));
    }

    #[test]
    fn a_node_is_profiled_by_the_role_word_it_names() {
        assert_eq!(profile_id_for_node("smith"), SMITH_STRUCTURED_V1);
        assert_eq!(profile_id_for_node("feature_scout"), SCOUT_READ_V1);
        assert_eq!(profile_id_for_node("qa_verify"), ASSAY_DETERMINISTIC_V1);
        assert_eq!(profile_id_for_node("architect"), ARCHITECT_ANALYSIS_V1);
        assert_eq!(profile_id_for_node("start"), GENERIC_V1);
    }

    #[test]
    fn the_same_role_always_gets_the_same_profile_and_unknown_roles_get_the_generic_one() {
        assert_eq!(profile_id_for_role("Smith"), SMITH_STRUCTURED_V1);
        assert_eq!(profile_id_for_role("smith"), profile_id_for_role("SMITH"));
        assert_eq!(profile_id_for_role("assay"), ASSAY_DETERMINISTIC_V1);
        assert_eq!(profile_id_for_role("dev_ops"), DEVOPS_RELEASE_V1);
        assert_eq!(profile_id_for_role("something-new"), GENERIC_V1);
    }

    #[test]
    fn a_replan_gets_a_new_key_and_a_retried_settlement_reuses_its_own() {
        let attempt = "task-1:role-attempt:0";
        // A retry of the same settlement: same generation, so the same row.
        assert_eq!(
            trajectory_attempt_key(attempt, Some("gen-1")),
            trajectory_attempt_key(attempt, Some("gen-1"))
        );
        // A repair turn is a different attempt, and a REPLAN mints a new generation: neither overwrites the first.
        assert_ne!(
            trajectory_attempt_key(attempt, Some("gen-1")),
            trajectory_attempt_key("task-1:role-attempt:1", Some("gen-1"))
        );
        assert_ne!(
            trajectory_attempt_key(attempt, Some("gen-1")),
            trajectory_attempt_key(attempt, Some("gen-2"))
        );
        // No controller at all: the key is the attempt identity alone, never a clock reading.
        assert_eq!(trajectory_attempt_key(attempt, None), attempt);
    }

    #[test]
    fn a_profile_version_is_read_from_the_id_and_never_invented() {
        assert_eq!(profile_version(SMITH_STRUCTURED_V1), Some(1));
        assert_eq!(profile_version("smith-structured-v12"), Some(12));
        assert_eq!(profile_version("generic"), None);
        assert_eq!(profile_version("thing-v"), None);
        assert_eq!(profile_version("thing-vNext"), None);
    }
}
