//! Port of forge-hold-resolve.ts resume targets + unclaimed fork fence.

pub const FORGE_HOLD_RESUME_TARGETS: &[&str] = &[
    "SCOUT",
    "DIAGNOSE",
    "ARCHITECT",
    "LEAD",
    "SMITH",
    "QA",
    "DEV_OPS",
    "PUBLISH",
    "DEPLOY",
    "SMOKE",
    "CANCEL",
];

pub fn valid_resume_target(target: &str) -> bool {
    FORGE_HOLD_RESUME_TARGETS.contains(&target)
}

pub fn refuse_unclaimed_fork_work_branch(
    node_id: &str,
    claimed_at: Option<&str>,
    fork_child: bool,
    open_siblings: usize,
) -> Option<String> {
    if !fork_child {
        return None;
    }
    if node_id != "smith_split_work" {
        return None;
    }
    if claimed_at
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some()
    {
        return None;
    }
    Some(format!(
        "refusing to advance unclaimed fork work branch {node_id} while {open_siblings} siblings are open — that fabricates completion"
    ))
}
