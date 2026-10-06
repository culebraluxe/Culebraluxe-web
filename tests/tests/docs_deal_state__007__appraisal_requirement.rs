//! DOCS.DEAL.STATE — appraisal requirement (TST-DOCS-DEAL-STATE-007).
//!
//! Contract: the appraisal track keys off the canonical `deal.appraisal_required` fact, projected as
//! `appraisalApplicable`. From the parsed production definition (`parse_re_supermodel`):
//!
//!   * `appraisalApplicable == true` runs the appraisal track; the false/skip path joins immediately.
//!   * NULL is NEVER silently skipped — `appraisalApplicable == null` routes to the explicit
//!     `appraisal_applicability_unresolved` task, whose `resolved` re-evaluates the same decision and
//!     whose `escalate` terminates the transaction.
//!   * Appraisal is independent of financing — both may appraise, neither forces appraisal; the only
//!     coupling allowed is through the shared join, never through a financing condition.
//!   * There is no appraisal timer (no canonical date source) — the fix guards against an invented SLA.
//!
//! Level: L0 Pure — the parsed production definition graph, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_deal_state__007__appraisal_requirement

use forge::engine::xml::parse_re_supermodel;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-DEAL-STATE-007).
fn docs_deal_state_007__appraisal_requirement() {
    let definition = parse_re_supermodel().expect("RE_supermodel-v1.xml must parse");
    let nodes = &definition.definition.nodes;

    let decision = nodes
        .get("appraisal_applicable")
        .expect("appraisal_applicable");
    let arms = decision.decisions.as_deref().unwrap_or_default();
    assert!(
        arms.iter()
            .any(|a| a.condition == "appraisalApplicable == true" && a.transition == "run"),
        "true runs the appraisal track: {arms:?}"
    );
    assert!(
        arms.iter()
            .any(|a| a.condition == "appraisalApplicable == null" && a.transition == "unresolved"),
        "null is explicit, never skipped: {arms:?}"
    );
    let skip = decision
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "skip")
        .expect("skip");
    assert_eq!(skip.to, "join_tracks");
    let unresolved = decision
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "unresolved")
        .expect("unresolved");
    assert_eq!(unresolved.to, "appraisal_applicability_unresolved");

    // The unresolved task re-enters the SAME decision and otherwise terminates.
    let task = nodes
        .get("appraisal_applicability_unresolved")
        .expect("appraisal_applicability_unresolved");
    let resolved = task
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "resolved")
        .expect("resolved");
    assert_eq!(resolved.to, "appraisal_applicable");
    let escalate = task
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "escalate")
        .expect("escalate");
    assert_eq!(escalate.to, "transaction_failed");

    // Negative controls.
    let decision_text = arms
        .iter()
        .map(|a| a.condition.as_str())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        !decision_text.contains("financing"),
        "appraisal applicability must be independent of financing: {decision_text}"
    );
    let appraisal_timers: Vec<&str> = nodes
        .values()
        .filter(|n| n.node_type == "timer")
        .filter(|n| {
            n.timer
                .as_ref()
                .and_then(|s| s.due_at_variable.as_deref())
                .map(|v| v.to_lowercase().contains("appraisal"))
                .unwrap_or(false)
        })
        .map(|n| n.id.as_str())
        .collect();
    assert!(
        appraisal_timers.is_empty(),
        "no invented appraisal SLA: {appraisal_timers:?}"
    );
}
