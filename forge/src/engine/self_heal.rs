//! Port of agents/self-heal.ts and the runner's bounded attempt loop.
//!
//! The directive is the ONE piece of an execution's context Forge composes and re-sends (the provider builds every
//! other prompt), so it is also the one piece a `context_policy::ContextPolicy` can govern. The entry points say which
//! posture a caller is in:
//!
//! - [`build_self_heal_directive`] is the UNPROFILED one. A caller with no resolved profile may not apply policy it
//!   never resolved, so it re-sends a single bounded tail exactly as it always has.
//! - [`build_self_heal_directive_contextual_report`] is the one PRODUCTION calls (`roles::lifecycle`): the resolved
//!   `ContextPolicy` decides which attempts are re-sent verbatim, which are elided behind a placeholder, and which are
//!   cut to a bounded tail — because Forge, not the model, is what accumulates this text — and the SAME call returns
//!   the [`ContextDecision`] it made, so the decision can be recorded where it was taken.
//! - [`build_self_heal_directive_contextual`] is that same composition without the decision, for a caller that only
//!   wants the text.

use crate::engine::context_policy::{
    elision_placeholder, plan_with_pressure, ContextClass, ContextDecision, ContextItem,
    ContextPolicy,
};

/// The one window Forge owns: how many tokens of an execution's own re-sent context it will hand back to the vendor.
///
/// This is deliberately NOT the model's context window. Forge cannot read that, and a ratio of a capacity nobody
/// measured would be a fabricated threshold. This budget is Forge's own re-send limit, so a `ContextPolicy` ratio
/// applied to it is a ratio of something real.
pub const FORGE_RESEND_BUDGET_TOKENS: usize = 1000;

/// Rough byte-to-token estimate: four characters per token, floored at one for any non-empty text.
///
/// It is an estimate and is never presented as a measurement: it decides ordering and pressure, and every text it
/// cuts is labelled with the budget that cut it so the reading cannot be mistaken for the vendor's own count.
fn estimated_tokens(text: &str) -> usize {
    (text.chars().count() / 4).max(1)
}

/// The authoritative head of every self-heal directive. WHICH fields are missing is Forge's own order, and no policy
/// may elide it.
fn directive_head(node_id: &str, missing: &[String]) -> String {
    let fields = if missing.is_empty() {
        "(unreported)".into()
    } else {
        missing.join(", ")
    };
    format!("SELF-HEAL REPROMPT (node {node_id}): this run was HELD because it did not deliver: {fields}.\n")
}

fn why_section(rejection_reasons: &[String]) -> String {
    let mut out = String::new();
    if !rejection_reasons.is_empty() {
        out.push_str("WHY (sidecar; not part of the retry hash): ");
        out.push_str(&rejection_reasons.join("; "));
        out.push('\n');
    }
    out
}

fn required_output_section(evidence_instruction: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(inst) = evidence_instruction.filter(|s| !s.is_empty()) {
        out.push_str("The role's required structured output is: ");
        out.push_str(inst);
        out.push('\n');
    }
    out
}

const DIRECTIVE_CLOSING: &str = "Re-run this role. Fix ONLY what is missing above, and end your reply by emitting the required structured evidence so those fields are present and valid.";

/// The unprofiled directive. `prior_reply` is tailed to a fixed bound, unconditionally, as it always was.
pub fn build_self_heal_directive(
    node_id: &str,
    missing: &[String],
    evidence_instruction: Option<&str>,
    rejection_reasons: &[String],
    prior_reply: Option<&str>,
) -> String {
    let mut out = directive_head(node_id, missing);
    out.push_str(&why_section(rejection_reasons));
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
    out.push_str(&required_output_section(evidence_instruction));
    out.push_str(DIRECTIVE_CLOSING);
    out
}

/// The profiled directive: the current attempt's reply plus every earlier attempt's reply, re-sent under `policy`.
///
/// `attempt_replies` is OLDEST FIRST, and its last entry is the reply the retry must repair. The most recent reply is
/// ACTIVE — it is the artifact being repaired, not a historical observation — and the earlier ones are
/// HISTORICAL_OBSERVATION, the class a `ContextPolicy` elides first.
///
/// A reply the policy elides is replaced by [`elision_placeholder`], which names what was dropped, so the model learns
/// that an earlier attempt exists and the run record keeps a pointer to it. Nothing is dropped silently: an oversized
/// reply is cut to its tail WITH a marker naming the budget that cut it, because Forge owns no summarizer and must not
/// claim in its own prompt to have written a summary it never wrote.
/// A composed directive and the context decision it was composed under.
///
/// The decision is `None` exactly when there was no attempt trail to plan: a decision over no items would be a
/// record of nothing, and `retained: 0, elided: 0` reads as a measurement.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextualDirective {
    pub directive: String,
    pub decision: Option<ContextDecision>,
}

pub fn build_self_heal_directive_contextual(
    node_id: &str,
    missing: &[String],
    evidence_instruction: Option<&str>,
    rejection_reasons: &[String],
    attempt_replies: &[String],
    policy: &ContextPolicy,
) -> String {
    build_self_heal_directive_contextual_report(
        node_id,
        missing,
        evidence_instruction,
        rejection_reasons,
        attempt_replies,
        policy,
    )
    .directive
}

/// The same directive, WITH the context decision it was composed under — the entry point the production lifecycle
/// calls, because a decision nobody can read back is a decision nobody can check.
pub fn build_self_heal_directive_contextual_report(
    node_id: &str,
    missing: &[String],
    evidence_instruction: Option<&str>,
    rejection_reasons: &[String],
    attempt_replies: &[String],
    policy: &ContextPolicy,
) -> ContextualDirective {
    let mut out = directive_head(node_id, missing);
    out.push_str(&why_section(rejection_reasons));
    let (trail, decision) = attempt_trail_section(attempt_replies, policy);
    out.push_str(&trail);
    out.push_str(&required_output_section(evidence_instruction));
    out.push_str(DIRECTIVE_CLOSING);
    ContextualDirective {
        directive: out,
        decision,
    }
}

/// The re-sent attempt trail, as the policy plans it, WITH the decision that produced it. Empty text and no
/// decision when there is nothing to re-send.
fn attempt_trail_section(
    attempt_replies: &[String],
    policy: &ContextPolicy,
) -> (String, Option<ContextDecision>) {
    let replies: Vec<&str> = attempt_replies
        .iter()
        .map(|reply| reply.trim())
        .filter(|reply| !reply.is_empty())
        .collect();
    if replies.is_empty() {
        return (String::new(), None);
    }

    let items: Vec<ContextItem> = replies
        .iter()
        .enumerate()
        .map(|(index, reply)| ContextItem {
            id: format!("attempt-{}", index + 1),
            class: if index + 1 == replies.len() {
                ContextClass::Active
            } else {
                ContextClass::HistoricalObservation
            },
            tokens: estimated_tokens(reply),
            label: format!("attempt {} reply", index + 1),
        })
        .collect();
    let plan = plan_with_pressure(&items, FORGE_RESEND_BUDGET_TOKENS, policy);
    let action = &plan.action;

    let mut out =
        String::from("PRIOR ATTEMPTS (the last one is the reply to repair; do NOT start over):\n");
    for (index, (item, reply)) in items.iter().zip(replies.iter()).enumerate() {
        let attempt = index + 1;
        if action.elide.contains(&item.id) {
            // Elided by policy, with a placeholder that keeps the attempt visible and locatable.
            out.push_str(&elision_placeholder(item));
            out.push('\n');
            continue;
        }
        out.push_str(&format!("--- attempt {attempt} reply ---\n"));
        let hard = (FORGE_RESEND_BUDGET_TOKENS as f32 * policy.hard_limit_ratio) as usize;
        // Two honest reasons to cut a reply, and a marker for each: the policy called for a summary at hard pressure
        // (Forge has none to write, so the tail is what survives), or the reply alone exceeds Forge's whole re-send
        // budget. Under neither does anything get cut — an ordinary retry re-sends its reply whole.
        let cut = if action.summarize.contains(&item.id) {
            Some((hard, "under hard context pressure"))
        } else if item.tokens > FORGE_RESEND_BUDGET_TOKENS {
            Some((FORGE_RESEND_BUDGET_TOKENS, "over the re-send budget"))
        } else {
            None
        };
        match cut {
            Some((kept_tokens, reason)) => {
                out.push_str(&truncated_tail(reply, kept_tokens, item, reason))
            }
            None => out.push_str(reply),
        }
        out.push_str("\n---\n");
    }
    // The decision travels with the text it decided: what the policy saw, what it kept, and what it removed.
    (out, Some(plan.decision(&items)))
}

/// The tail of `reply` that fits `kept_tokens`, behind the marker that says a cut happened and why.
fn truncated_tail(reply: &str, kept_tokens: usize, item: &ContextItem, reason: &str) -> String {
    let keep_chars = kept_tokens.saturating_mul(4);
    let omitted_tokens = item.tokens.saturating_sub(kept_tokens);
    let start = reply.len().saturating_sub(keep_chars);
    // Never split a character: a byte-index slice on a multi-byte boundary is a panic.
    let start = (start..=reply.len())
        .find(|index| reply.is_char_boundary(*index))
        .unwrap_or(reply.len());
    format!(
        "[{} truncated {reason}: the last {kept_tokens} tokens are shown, {omitted_tokens} omitted; the full reply remains in the execution record (item {})]\n{}",
        item.label,
        item.id,
        &reply[start..],
    )
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
        let d =
            build_self_heal_directive("lead", &["leadDecision".into()], None, &[], Some(&prior));
        assert!(d.contains("END-CONTRACT"));
        assert!(d.contains("repair this"));
        assert!(!d.contains(&"x".repeat(20).repeat(10)[..20]) || d.contains("END-CONTRACT"));
    }
    #[test]
    fn enforce_off_is_one_attempt() {
        assert_eq!(attempt_budget(false, 3), 1);
        assert_eq!(attempt_budget(true, 2), 3);
    }

    /// The C2 control: an ordinary retry's reply is re-sent WHOLE. A policy that trimmed every retry would trade the
    /// context problem for a worse one — a role asked to repair a reply it can no longer see.
    #[test]
    fn a_reply_inside_the_re_send_budget_is_re_sent_whole() {
        let reply = format!(
            "FINDING: the parser drops empty input. {}",
            "y".repeat(2000)
        );
        let directive = build_self_heal_directive_contextual(
            "smith",
            &["smithDecision".into()],
            None,
            &[],
            &[reply.clone()],
            &ContextPolicy::default(),
        );
        assert!(directive.contains("FINDING: the parser drops empty input."));
        assert!(
            !directive.contains("truncated"),
            "nothing in this trail is over pressure: {directive}"
        );
        assert!(directive.contains("attempt 1 reply"));
    }

    /// Pressure, measured against Forge's own re-send budget: an older attempt becomes a placeholder that still names
    /// it, while the newest — the one being repaired — stays verbatim.
    #[test]
    fn an_older_attempt_is_elided_by_policy_and_the_newest_is_kept_verbatim() {
        let older = format!("OLD-ATTEMPT-MARKER {}", "o".repeat(3200));
        let newest = "NEWEST-REPAIR-ME";
        let directive = build_self_heal_directive_contextual(
            "smith",
            &["smithDecision".into()],
            None,
            &[],
            &[older, newest.to_string()],
            &ContextPolicy::default(),
        );
        assert!(
            directive.contains("attempt 1 reply omitted"),
            "an elided attempt must say so, and which one: {directive}"
        );
        assert!(!directive.contains("OLD-ATTEMPT-MARKER"));
        assert!(directive.contains(newest));
        // The authoritative part of the directive is never a candidate for elision.
        assert!(directive.contains("did not deliver: smithDecision"));
    }

    /// A reply larger than the whole budget is cut to its tail — and SAYS it was cut, in Forge's own words. The old
    /// path did this silently, which is the difference between a bound and a lost reading.
    #[test]
    fn a_reply_over_the_budget_keeps_its_tail_behind_an_explicit_marker() {
        let reply = format!("HEAD-OF-REPLY{}TAIL-OF-REPLY", "z".repeat(8000));
        let directive = build_self_heal_directive_contextual(
            "smith",
            &["smithDecision".into()],
            None,
            &[],
            &[reply],
            &ContextPolicy::default(),
        );
        assert!(directive.contains("TAIL-OF-REPLY"));
        assert!(!directive.contains("HEAD-OF-REPLY"));
        assert!(directive.contains("truncated over the re-send budget"));
        assert!(directive.contains("the full reply remains in the execution record"));
    }

    /// A trail the policy elides still carries Forge's own instruction, and says the earlier attempt exists at all.
    /// Nothing here is silent, and nothing here is a summary Forge did not write.
    #[test]
    fn the_contextual_directive_never_claims_a_summary_it_did_not_write() {
        let big = "q".repeat(20_000);
        let directive = build_self_heal_directive_contextual(
            "lead",
            &["leadDecision".into()],
            Some("leadDecision: one of GO|HOLD"),
            &["role did not deliver leadDecision".into()],
            &[big, "second".into()],
            &ContextPolicy::default(),
        );
        assert!(!directive.contains("summarized"));
        assert!(directive.contains("WHY (sidecar"));
        assert!(directive.contains("leadDecision: one of GO|HOLD"));
        assert!(directive.contains("PRIOR ATTEMPTS"));
    }

    /// C2's telemetry, read from the PRODUCTION preparation function: the directive the lifecycle composes for the
    /// next attempt carries the decision the same policy made about the same trail. The decision is the record; the
    /// directive is the text.
    #[test]
    fn the_production_directive_reports_the_decision_it_was_composed_under() {
        let trail = vec!["x".repeat(20_000), "the newest reply".to_string()];
        let composed = build_self_heal_directive_contextual_report(
            "smith",
            &["smithDecision".into()],
            None,
            &[],
            &trail,
            &ContextPolicy::default(),
        );
        let decision = composed
            .decision
            .expect("two attempt replies are a plan, so a decision is recorded");
        assert!(
            decision.input_tokens > FORGE_RESEND_BUDGET_TOKENS,
            "the trail is over Forge's own re-send budget, which is what makes this a decision: {decision:?}"
        );
        assert!(
            decision.output_tokens <= decision.input_tokens,
            "the plan never grows the context: {decision:?}"
        );
        assert_eq!(decision.pressure.label(), "soft");
        assert_eq!(decision.elided, 1, "the older attempt is the one elided");
        assert!(composed.directive.contains("PRIOR ATTEMPTS"));
    }

    /// Nothing to plan is not a decision of zero: with no attempt trail there is no plan, so no record is written
    /// rather than one claiming nothing was elided from nothing.
    #[test]
    fn a_directive_with_no_trail_records_no_decision() {
        let composed = build_self_heal_directive_contextual_report(
            "smith",
            &["smithDecision".into()],
            None,
            &[],
            &[],
            &ContextPolicy::default(),
        );
        assert!(composed.decision.is_none());
        assert!(!composed.directive.contains("PRIOR ATTEMPTS"));
        assert!(composed.directive.contains("SELF-HEAL REPROMPT"));
    }
}
