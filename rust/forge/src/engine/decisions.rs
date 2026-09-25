//! Port of lib/forge-decision.ts injection + db/forge-decision.ts active read.

use crate::engine::vendor_session::with_shared;
use db::ForgeEngineDao;

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
