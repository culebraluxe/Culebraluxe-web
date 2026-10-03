//! Port of forge-execution-shaping.ts validateSmithExecutionPlan.

pub struct SmithChunk {
    pub id: i32,
    pub outcome: String,
    pub surface: Vec<String>,
    pub invariant: String,
    pub proof: String,
    pub depends_on: Vec<i32>,
}

pub struct SmithExecutionPlan {
    pub size: String,
    pub chunks: Vec<SmithChunk>,
}

pub fn validate_smith_execution_plan(plan: Option<&SmithExecutionPlan>) -> Vec<String> {
    let Some(plan) = plan else {
        return vec!["missing smith_execution_plan".into()];
    };
    let mut v = Vec::new();
    let n = plan.chunks.len();
    if n < 1 {
        v.push("plan declares no chunks".into());
    }
    if n > 3 {
        v.push(format!(
            "more than 3 chunks ({n}): a 4th chunk is HOLD, not keep working"
        ));
    }
    if !matches!(plan.size.as_str(), "SMALL" | "MEDIUM" | "LARGE") {
        v.push(format!(
            "unknown size '{}' (expected SMALL|MEDIUM|LARGE)",
            plan.size
        ));
    }
    for chunk in &plan.chunks {
        let label = format!("chunk {}: ", chunk.id);
        if chunk.outcome.trim().is_empty() {
            v.push(format!("{label}missing one outcome"));
        }
        if chunk.surface.is_empty() {
            v.push(format!("{label}missing code surface"));
        } else if chunk.surface.iter().any(|s| s.trim().is_empty()) {
            v.push(format!("{label}has a blank code surface entry"));
        }
        if chunk.invariant.trim().is_empty() {
            v.push(format!("{label}missing invariant"));
        }
        if chunk.proof.trim().is_empty() {
            v.push(format!("{label}missing runnable targeted proof"));
        }
    }
    let ids: Vec<i32> = plan.chunks.iter().map(|c| c.id).collect();
    for (i, id) in ids.iter().enumerate() {
        if *id != (i as i32) + 1 {
            v.push(format!(
                "chunk ids must be serial 1..n (got {})",
                ids.iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            break;
        }
    }
    for chunk in &plan.chunks {
        let deps: Vec<_> = chunk
            .depends_on
            .iter()
            .copied()
            .filter(|d| *d != chunk.id - 1)
            .collect();
        if !deps.is_empty() {
            v.push(format!(
                "chunk {} has non-serial dependency ({})",
                chunk.id,
                deps.iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
    }
    v
}

pub fn assess_smith_dispatch(plan: Option<&SmithExecutionPlan>) -> (String, Vec<String>) {
    let reasons = validate_smith_execution_plan(plan);
    if plan.is_none() {
        return ("HOLD".into(), vec!["missing plan".into()]);
    }
    if reasons.is_empty() {
        ("GO".into(), reasons)
    } else {
        ("HOLD".into(), reasons)
    }
}
