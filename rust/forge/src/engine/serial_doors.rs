//! Port of forge-serial-doors.ts.

pub const SERIAL_SMITH_NODES: &[&str] = &["smith", "repair_smith", "fast_smith", "fast_repair_smith"];
pub const NO_ASSIGNMENT_REASON: &str = "Smith does not choose its own scope";
pub const SCOPE_MISS_PREFIX: &str = "smith-scope:";

pub fn serial_launch_door(node_id: &str, has_accepted_assignment: bool) -> Option<String> {
    if !SERIAL_SMITH_NODES.contains(&node_id) { return None; }
    if has_accepted_assignment { return None; }
    Some(format!(
        "Forge {node_id} HOLD: no accepted Lead assignment for the serial Smith lane. {NO_ASSIGNMENT_REASON} — the Lead routes the work orders before this lane may run."
    ))
}

pub fn serial_scope_miss_reasons(violations: &[String], owner: &str) -> Vec<String> {
    violations.iter().map(|p| format!("{SCOPE_MISS_PREFIX}{p} is outside {owner}")).collect()
}
