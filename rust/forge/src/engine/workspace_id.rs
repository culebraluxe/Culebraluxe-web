//! Port of agent-runtime/invoker.ts workspace lineage. Pure.

pub fn sanitize_execution_segment(input: &str) -> String {
    let mut out = String::new();
    for ch in input.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

pub fn resolve_workspace_run_id(execution_id: &str, split_child_id: Option<&str>) -> Result<String, String> {
    let execution = execution_id.trim();
    if execution.is_empty() {
        return Err("Forge workspace executionId is required".into());
    }
    let split = split_child_id.unwrap_or("").trim();
    Ok(if split.is_empty() {
        execution.to_string()
    } else {
        format!("{execution}-split-{split}")
    })
}

pub fn forge_execution_generation_key(process_instance_id: &str, replan_attempts: i32) -> Result<String, String> {
    let pid = sanitize_execution_segment(process_instance_id);
    if pid.is_empty() {
        return Err("Forge processInstanceId is required for workspace lineage".into());
    }
    let gen = replan_attempts.max(0);
    Ok(format!("{pid}-e{gen}"))
}

pub fn resolve_forge_execution_run_id(
    process_instance_id: &str,
    replan_attempts: i32,
    split_child_id: Option<&str>,
) -> Result<String, String> {
    resolve_workspace_run_id(
        &forge_execution_generation_key(process_instance_id, replan_attempts)?,
        split_child_id,
    )
}

/// Live builder returns no trees. Same contract as TS `buildAgentInvokerWorkspaces`.
pub fn build_agent_invoker_workspaces() -> Option<()> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replan_advances_generation() {
        let e0 = resolve_forge_execution_run_id("abc", 0, None).unwrap();
        let e1 = resolve_forge_execution_run_id("abc", 1, None).unwrap();
        assert_eq!(e0, "abc-e0");
        assert_eq!(e1, "abc-e1");
    }
    #[test]
    fn split_child_suffix() {
        let id = resolve_forge_execution_run_id("pid", 0, Some("1")).unwrap();
        assert_eq!(id, "pid-e0-split-1");
    }
    #[test]
    fn no_trees() {
        assert!(build_agent_invoker_workspaces().is_none());
    }
}
