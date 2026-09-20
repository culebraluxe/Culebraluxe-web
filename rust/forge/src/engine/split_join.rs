//! Port of workflow_app/forge/split-join.ts. Pure.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitOutcome {
    pub child_id: String,
    pub status: String,
    pub attempt: u32,
    pub output_keys: Vec<String>,
    pub candidate_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SplitReduction {
    pub expected: usize,
    pub accounted: usize,
    pub completed: Vec<String>,
    pub failed: Vec<String>,
    pub cancelled: Vec<String>,
    pub missing: Vec<String>,
    pub duplicates: Vec<String>,
    pub conflicts: Vec<String>,
    pub join_satisfied: bool,
    pub duplicated_terminal: Vec<String>,
}

pub fn reduce_split(expected_ids: &[String], outcomes: &[SplitOutcome]) -> SplitReduction {
    use std::collections::BTreeMap;
    let expected = expected_ids.len();
    let mut by_child: BTreeMap<String, SplitOutcome> = BTreeMap::new();
    let mut duplicates = Vec::new();
    for o in outcomes {
        if let Some(existing) = by_child.get(&o.child_id) {
            duplicates.push(o.child_id.clone());
            if o.attempt >= existing.attempt {
                by_child.insert(o.child_id.clone(), o.clone());
            }
        } else {
            by_child.insert(o.child_id.clone(), o.clone());
        }
    }
    let mut completed_count: BTreeMap<String, u32> = BTreeMap::new();
    for o in outcomes {
        if o.status == "completed" {
            *completed_count.entry(o.child_id.clone()).or_insert(0) += 1;
        }
    }
    let duplicated_terminal: Vec<String> = completed_count
        .into_iter()
        .filter(|(_, c)| *c > 1)
        .map(|(id, _)| id)
        .collect();

    let completed: Vec<String> = by_child
        .values()
        .filter(|o| o.status == "completed")
        .map(|o| o.child_id.clone())
        .collect();
    let failed: Vec<String> = by_child
        .values()
        .filter(|o| o.status == "failed")
        .map(|o| o.child_id.clone())
        .collect();
    let cancelled: Vec<String> = by_child
        .values()
        .filter(|o| o.status == "cancelled")
        .map(|o| o.child_id.clone())
        .collect();
    let missing: Vec<String> = expected_ids
        .iter()
        .filter(|id| !by_child.contains_key(*id))
        .cloned()
        .collect();

    let mut owner_by_output: BTreeMap<String, String> = BTreeMap::new();
    let mut conflicts = Vec::new();
    for o in by_child.values() {
        for key in &o.output_keys {
            if let Some(existing) = owner_by_output.get(key) {
                if existing != &o.child_id && !conflicts.contains(key) {
                    conflicts.push(key.clone());
                }
            } else {
                owner_by_output.insert(key.clone(), o.child_id.clone());
            }
        }
    }

    let join_satisfied = missing.is_empty()
        && failed.is_empty()
        && cancelled.is_empty()
        && conflicts.is_empty()
        && completed.len() == expected;

    SplitReduction {
        expected,
        accounted: by_child.len(),
        completed,
        failed,
        cancelled,
        missing,
        duplicates,
        conflicts,
        join_satisfied,
        duplicated_terminal,
    }
}

pub fn split_join_hold_reasons(
    expected_ids: &[String],
    outcomes: &[SplitOutcome],
    unrecorded_candidates: &[String],
) -> Vec<String> {
    let reduction = reduce_split(expected_ids, outcomes);
    let mut reasons = Vec::new();
    if !reduction.missing.is_empty() {
        reasons.push(format!(
            "split children never reached a terminal state: {}",
            reduction.missing.join(", ")
        ));
    }
    if !reduction.failed.is_empty() {
        reasons.push(format!(
            "split children failed: {}",
            reduction.failed.join(", ")
        ));
    }
    if !reduction.cancelled.is_empty() {
        reasons.push(format!(
            "split children cancelled/paused: {}",
            reduction.cancelled.join(", ")
        ));
    }
    if !reduction.conflicts.is_empty() {
        reasons.push(format!(
            "sibling assignments claim the same output: {}",
            reduction.conflicts.join(", ")
        ));
    }
    if !reduction.duplicated_terminal.is_empty() {
        reasons.push(format!(
            "split children reported duplicate terminal completions: {}",
            reduction.duplicated_terminal.join(", ")
        ));
    }
    if !unrecorded_candidates.is_empty() {
        reasons.push(format!(
            "split children completed with no candidate SHA recorded from their own workspace: {}",
            unrecorded_candidates.join(", ")
        ));
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;
    fn child(id: &str, status: &str, keys: &[&str]) -> SplitOutcome {
        SplitOutcome {
            child_id: id.into(),
            status: status.into(),
            attempt: 1,
            output_keys: keys.iter().map(|s| s.to_string()).collect(),
            candidate_sha: Some("deadbeef".into()),
        }
    }
    #[test]
    fn missing_is_hold() {
        let reasons = split_join_hold_reasons(
            &["a".into(), "b".into()],
            &[child("a", "completed", &[])],
            &[],
        );
        assert!(reasons[0].contains("never reached"));
    }
    #[test]
    fn sibling_conflict_is_hold() {
        let reasons = split_join_hold_reasons(
            &["a".into(), "b".into()],
            &[
                child("a", "completed", &["src/x.rs"]),
                child("b", "completed", &["src/x.rs"]),
            ],
            &[],
        );
        assert!(reasons.iter().any(|r| r.contains("same output")));
    }
}
