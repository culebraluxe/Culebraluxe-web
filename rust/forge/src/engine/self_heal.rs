//! Port of agents/self-heal.ts and the runner's bounded attempt loop.

pub fn build_self_heal_directive(
    node_id: &str,
    missing: &[String],
    evidence_instruction: Option<&str>,
    rejection_reasons: &[String],
    prior_reply: Option<&str>,
) -> String {
    let fields = if missing.is_empty() {
        "(unreported)".into()
    } else {
        missing.join(", ")
    };
    let mut out = format!(
        "SELF-HEAL REPROMPT (node {node_id}): this run was HELD because it did not deliver: {fields}.\n"
    );
    if !rejection_reasons.is_empty() {
        out.push_str("WHY (sidecar; not part of the retry hash): ");
        out.push_str(&rejection_reasons.join("; "));
        out.push('\n');
    }
    if let Some(prior) = prior_reply.map(str::trim).filter(|s| !s.is_empty()) {
        let tail = if prior.len() > 4000 {
            &prior[prior.len() - 4000..]
        } else {
            prior
        };
        out.push_str("YOUR PREVIOUS REPLY (repair this; do NOT start over):\n---\n");
        out.push_str(tail);
        out.push_str("\n---\n");
    }
    if let Some(inst) = evidence_instruction.filter(|s| !s.is_empty()) {
        out.push_str("The role's required structured output is: ");
        out.push_str(inst);
        out.push('\n');
    }
    out.push_str("Re-run this role. Fix ONLY what is missing above, and end your reply by emitting the required structured evidence so those fields are present and valid.");
    out
}

pub fn attempt_budget(enforce: bool, extra_retries: u32) -> u32 {
    if enforce {
        extra_retries.saturating_add(1).max(1)
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prior_reply_is_tailed_not_headed() {
        let prior = format!("{}END-CONTRACT", "x".repeat(4010));
        let d = build_self_heal_directive("lead", &["leadDecision".into()], None, &[], Some(&prior));
        assert!(d.contains("END-CONTRACT"));
        assert!(d.contains("repair this"));
        assert!(!d.contains(&"x".repeat(20).repeat(10)[..20]) || d.contains("END-CONTRACT"));
    }
    #[test]
    fn enforce_off_is_one_attempt() {
        assert_eq!(attempt_budget(false, 3), 1);
        assert_eq!(attempt_budget(true, 2), 3);
    }
}
