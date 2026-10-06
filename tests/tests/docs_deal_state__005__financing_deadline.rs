//! DOCS.DEAL.STATE — financing deadline (TST-DOCS-DEAL-STATE-005).
//!
//! Contract: the financing-commitment date is `deal.financing_deadline`, projected by
//! RE_supermodel into an OPTIONAL fork branch. From the production definition
//! (`middle/workflow/definitions/RE_supermodel-v1.xml`, parsed by
//! `forge::engine::xml::parse_re_supermodel`):
//!
//!   * The branch activates only when BOTH `financingApplicable == true` AND
//!     `financingDeadlineScheduled == true` — a cash transaction never gets a financing clock.
//!   * `financing_deadline_timer` reads `due-at-variable="financingDeadline"` and fires into the
//!     human escalation task.
//!   * Amending issues `deal.set_financing_deadline`, whose `reschedule` edge re-arms the SAME timer.
//!   * No scheduled deadline routes the branch straight to the join — no timer is created.
//!
//! Level: L0 Pure — the parsed production definition graph, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_deal_state__005__financing_deadline

use forge::engine::xml::parse_re_supermodel;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-DEAL-STATE-005).
fn docs_deal_state_005__financing_deadline() {
    let definition = parse_re_supermodel().expect("RE_supermodel-v1.xml must parse");
    let nodes = &definition.definition.nodes;

    let applicable = nodes
        .get("financing_deadline_applicable")
        .expect("financing_deadline_applicable");
    let applicable_arms = applicable.decisions.as_deref().unwrap_or_default();
    assert!(
        applicable_arms
            .iter()
            .any(|a| a.condition == "financingApplicable == true" && a.transition == "scheduled"),
        "the monitor requires financing to apply: {applicable_arms:?}"
    );

    let scheduled = nodes
        .get("financing_deadline_scheduled")
        .expect("financing_deadline_scheduled");
    let scheduled_arms = scheduled.decisions.as_deref().unwrap_or_default();
    assert!(
        scheduled_arms
            .iter()
            .any(|a| a.condition == "financingDeadlineScheduled == true"
                && a.transition == "monitor"),
        "the monitor requires a canonical scheduled deadline: {scheduled_arms:?}"
    );
    let skip = scheduled
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "skip")
        .expect("skip");
    assert_eq!(skip.to, "join_tracks");

    let timer = nodes
        .get("financing_deadline_timer")
        .expect("financing_deadline_timer");
    assert_eq!(timer.node_type, "timer");
    let spec = timer.timer.as_ref().expect("timer spec");
    assert_eq!(spec.due_at_variable.as_deref(), Some("financingDeadline"));
    assert!(
        spec.due_at.is_none(),
        "no literal due date is invented: {spec:?}"
    );
    let fire = timer
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "escalate")
        .expect("escalate");
    assert_eq!(fire.to, "financing_deadline_escalation");

    let amend = nodes
        .get("set_financing_deadline")
        .expect("set_financing_deadline command node");
    assert_eq!(
        amend.command_type.as_deref(),
        Some("deal.set_financing_deadline")
    );
    let reschedule = amend
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "reschedule")
        .expect("reschedule");
    assert_eq!(reschedule.to, "financing_deadline_timer");

    // Negative control: exactly one financing-deadline timer.
    let timers: Vec<&str> = nodes
        .values()
        .filter(|n| {
            n.node_type == "timer"
                && n.timer.as_ref().and_then(|s| s.due_at_variable.as_deref())
                    == Some("financingDeadline")
        })
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(timers, vec!["financing_deadline_timer"]);
}
