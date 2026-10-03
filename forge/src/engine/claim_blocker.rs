//! Port of forge-claim-blocker.ts.

pub struct ClaimBlockerRow {
    pub id: String,
    pub story_id: String,
    pub role: Option<String>,
    pub state: String,
    pub claimed_by: Option<String>,
    pub updated_at_ms: Option<i64>,
}

pub fn describe_claim_blocker(
    rows: &[ClaimBlockerRow],
    now_ms: i64,
    stale_ms: i64,
    story_id: &str,
) -> String {
    if rows.is_empty() {
        return "no work item is holding THIS story's claim right now, so the refusal was transient — retry the run".into();
    }
    let lines: Vec<String> = rows
        .iter()
        .map(|row| {
            let age_ms = row.updated_at_ms.map(|t| (now_ms - t).max(0));
            let stale = age_ms.map(|a| a >= stale_ms).unwrap_or(false);
            let age = match age_ms {
                None => "age unknown".into(),
                Some(ms) => format!("{}m old", ms / 60_000),
            };
            format!(
                "{} ({}, role={}, story={}, held by {}, {age}{})",
                row.id,
                row.state,
                row.role.as_deref().unwrap_or("?"),
                row.story_id,
                row.claimed_by.as_deref().unwrap_or("nobody"),
                if stale { ", STALE" } else { "" }
            )
        })
        .collect();
    let any_stale = rows.iter().any(|r| {
        r.updated_at_ms
            .map(|t| (now_ms - t) >= stale_ms)
            .unwrap_or(false)
    });
    let advice = if any_stale {
        "Run `pnpm forge:clean` to interrupt the stale claims, then retry."
    } else if rows.iter().any(|r| r.story_id == story_id) {
        "This story already holds a claim: reset the story and retry."
    } else {
        // Since 2026-09-29 the claim is serial per STORY, not per system: another story's live claim does not block
        // this one, so a refusal cannot be explained by a peer holding a global lock.
        "Another story holds a claim and that no longer blocks this one: retry the run, and if it still refuses, \
         this story has no claimable item."
    };
    format!(
        "this story's claim is held by {}. {advice}",
        lines.join("; ")
    )
}
