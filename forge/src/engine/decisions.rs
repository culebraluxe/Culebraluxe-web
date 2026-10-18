//! Port of lib/forge-decision.ts injection + db/forge-decision.ts active read.

use crate::engine::executive_swarm::{DecisionSynthesis, ForgeDecisionHandoff};
use crate::engine::vendor_session::with_shared;
use db::{ForgeEngineDao, NewForgeDecision, NewForgeHold, NewToolArtifact};

pub const DECISION_INJECTION_CAP: usize = 20;
pub const DECISION_STORE_UNAVAILABLE: &str =
    "DECISION STORE UNAVAILABLE: the active decisions for this domain could not be read for this run. \
Proceed under the packet, the lane rules and the harness only, and SAY SO in your report — do not \
infer the project rules from memory.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionSource {
    pub key: String,
    pub statement: String,
    pub owner: Option<String>,
}

pub fn decision_domain_for_story(
    workstream: Option<&str>,
    operating_surface: Option<&str>,
) -> &'static str {
    let token = format!(
        "{} {}",
        workstream.unwrap_or(""),
        operating_surface.unwrap_or("")
    )
    .to_ascii_uppercase();
    if token.contains("CRM") || token.contains("CONTACT") || token.contains("DEAL") {
        "crm"
    } else if token.contains("WEB") || token.contains("SITE") || token.contains("MARKETING") {
        "web"
    } else if token.contains("OPS") || token.contains("OPERAT") || token.contains("ACCOUNT") {
        "ops"
    } else {
        "forge"
    }
}

pub fn lane_needs_decisions(lane: &str) -> bool {
    matches!(lane, "lead" | "smith" | "night" | "architect" | "inspector")
}

pub fn render_decision_block(decisions: &[DecisionSource]) -> String {
    if decisions.is_empty() {
        return String::new();
    }
    let mut lines = vec![
        format!(
            "ACTIVE DECISIONS ({}, newest promoted first) — these are IN FORCE for this domain.",
            decisions.len()
        ),
        "They outlive this session and are not up for re-litigation by the lane that is acting on them.".into(),
    ];
    for d in decisions {
        let owner = d
            .owner
            .as_deref()
            .map(|o| format!(" ({o})"))
            .unwrap_or_default();
        lines.push(format!("- {}: {}{owner}", d.key, d.statement));
    }
    lines.push(
        "If an active decision is wrong or stale, say so explicitly and open a learn item; do not act against it silently.".into(),
    );
    lines.join("\n")
}

pub fn with_decision_context(
    instructions: Option<&str>,
    decisions: Result<&[DecisionSource], ()>,
) -> Option<String> {
    let base = instructions.unwrap_or("").trim();
    match decisions {
        Err(()) => {
            let parts = [base, DECISION_STORE_UNAVAILABLE]
                .into_iter()
                .filter(|s| !s.is_empty());
            Some(parts.collect::<Vec<_>>().join("\n\n"))
        }
        Ok(list) => {
            let block = render_decision_block(list);
            let parts = [base, block.as_str()].into_iter().filter(|s| !s.is_empty());
            let joined = parts.collect::<Vec<_>>().join("\n\n");
            if joined.is_empty() {
                None
            } else {
                Some(joined)
            }
        }
    }
}

pub fn list_active_decisions(domain: &str, limit: usize) -> Result<Vec<DecisionSource>, String> {
    let cap = limit.clamp(1, DECISION_INJECTION_CAP) as i64;
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.active_decisions(domain, cap)
                .await
                .map(|rows| {
                    rows.into_iter()
                        .map(|row| DecisionSource {
                            key: row.key,
                            statement: row.statement,
                            owner: row.owner,
                        })
                        .collect()
                })
                .map_err(|error| error.to_string())
        })
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn store_unavailable_is_named() {
        let text = with_decision_context(Some("do the work"), Err(())).unwrap();
        assert!(text.contains("DECISION STORE UNAVAILABLE"));
        assert!(text.contains("do the work"));
    }
    #[test]
    fn lead_needs_decisions() {
        assert!(lane_needs_decisions("lead"));
        assert!(!lane_needs_decisions("qa_verify"));
    }
}

/// The durable statement shape migration 180 enforces: one sentence, at most 240 characters, no
/// sentence-ending punctuation followed by more text. The panel's recommendation may be prose; the
/// ROW must stay a single present-tense claim.
pub fn decision_statement(recommendation: &str) -> String {
    let collapsed = recommendation
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let first_sentence = collapsed
        .split(|character| matches!(character, '.' | '!' | '?'))
        .map(str::trim)
        .find(|part| !part.is_empty())
        .unwrap_or("advisory panel decision");
    let mut statement: String = first_sentence.chars().take(240).collect();
    statement = statement.trim().to_string();
    if statement.is_empty() {
        statement = "advisory panel decision".to_string();
    }
    statement
}

/// A stable decision key from the panel's id. Migration 180 wants a slug: lowercase alphanumerics
/// and single dashes, <=80 characters, opening with an alphanumeric.
pub fn decision_key(decision_id: &str) -> String {
    let mut key = String::new();
    let mut previous_dash = false;
    for character in decision_id.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            key.push(character);
            previous_dash = false;
        } else if !previous_dash && !key.is_empty() {
            key.push('-');
            previous_dash = true;
        }
        if key.len() >= 80 {
            break;
        }
    }
    let key = key.trim_end_matches('-').to_string();
    if key.is_empty() {
        "panel-decision".to_string()
    } else {
        key
    }
}

/// Persist a panel synthesis as a `candidate` decision row. It is idempotent by key, and it never
/// promotes: a panel recommends; a human/lane promotes (the table's promotion shape enforces that).
pub fn record_panel_decision(synthesis: &DecisionSynthesis) -> Result<String, String> {
    let key = decision_key(&synthesis.decision_id);
    let statement = decision_statement(&synthesis.recommendation);
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.insert_decision(&NewForgeDecision {
                key: &key,
                statement: &statement,
                status: "candidate",
                owner: None,
                evidence_sha: None,
                source: "packet",
                domain: "forge",
            })
            .await
            .map(|_| key.clone())
            .map_err(|error| error.to_string())
        })
    })?
}

/// Attach the decision handoff to a story as a durable artifact. Idempotent by decision key; returns
/// the artifact row id when a story is supplied.
pub fn record_handoff_artifact(
    handoff: &ForgeDecisionHandoff,
    story_id: &str,
    idempotency_key: &str,
) -> Result<Option<String>, String> {
    let detail = serde_json::to_value(handoff).map_err(|error| error.to_string())?;
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.record_tool_artifact(&NewToolArtifact {
                story_id: story_id.to_string(),
                story_run_id: None,
                tool: "agent-os-panel".to_string(),
                kind: "decision-handoff".to_string(),
                verdict: Some("advisory".to_string()),
                summary: Some(handoff.recommendation.clone()),
                detail: Some(detail),
                sha: None,
                idempotency_key: Some(format!("handoff:{idempotency_key}")),
            })
            .await
            .map(|row| Some(row.id.to_string()))
            .map_err(|error| error.to_string())
        })
    })?
}

/// Open a durable HOLD for an advisory decision that needs owner authority. A HOLD is story-bound
/// (its story_id/task are foreign keys), so a story-less strategic decision stays a decision row.
pub fn open_panel_hold(
    process_instance_id: &str,
    story_id: &str,
    reason: &str,
    resume_target: Option<&str>,
) -> Result<String, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.open_hold(&NewForgeHold {
                process_instance_id,
                task_id: None,
                story_id,
                reason,
                originating_node: Some("agent-os-council"),
                failure_class: Some("owner_decision"),
                resume_target,
            })
            .await
            .map_err(|error| error.to_string())
        })
    })?
}

#[cfg(test)]
mod persistence_tests {
    use super::*;

    #[test]
    fn statements_are_one_sentence_and_bounded() {
        assert_eq!(
            decision_statement("Adopt DSH as the execution harness. It already measures usage."),
            "Adopt DSH as the execution harness"
        );
        assert_eq!(decision_statement(&"x".repeat(400)).chars().count(), 240);
        assert_eq!(decision_statement("   "), "advisory panel decision");
    }

    #[test]
    fn keys_are_slugs() {
        assert_eq!(decision_key("Use DSH as Harness"), "use-dsh-as-harness");
        assert_eq!(decision_key("---"), "panel-decision");
        assert_eq!(decision_key(""), "panel-decision");
    }
}
