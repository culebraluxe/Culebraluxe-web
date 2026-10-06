//! DOCS.DEAL.STATE — closing dates (TST-DOCS-DEAL-STATE-003).
//!
//! Contract: the canonical target closing date is `deal.closing_date`, and the RE_supermodel only
//! projects it into timer semantics — it never invents one. From the production definition
//! (`middle/workflow/definitions/RE_supermodel-v1.xml`, parsed by
//! `forge::engine::xml::parse_re_supermodel`):
//!
//!   * The closing-date monitor activates ONLY when `closingDateScheduled == true`
//!     (a canonical closing date exists): decision `closing_deadline_applicable`.
//!   * The timer node `closing_date_timer` reads its due instant from the canonical fact —
//!     `due-at-variable="closingDate"` — and fires into `closing_date_escalation`.
//!   * Amending the date is the canonical command `deal.set_closing_date`, and its `reschedule`
//!     edge re-arms the SAME timer node — the instance continues, never restarts.
//!   * With no canonical date, the branch joins immediately and NO timer is created:
//!     `closing_deadline_applicable` routes non-scheduled to `closing_schedule_join`.
//!
//! Level: L0 Pure — the parsed production definition graph, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_deal_state__003__closing_dates

use forge::engine::xml::parse_re_supermodel;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-DEAL-STATE-003).
fn docs_deal_state_003__closing_dates() {
    let definition = parse_re_supermodel().expect("RE_supermodel-v1.xml must parse");
    let nodes = &definition.definition.nodes;

    // 1. Activation gate: monitor iff the application says a canonical date exists.
    let gate = nodes
        .get("closing_deadline_applicable")
        .expect("closing_deadline_applicable decision");
    let arms = gate.decisions.as_deref().unwrap_or_default();
    assert!(
        arms.iter()
            .any(|a| a.condition == "closingDateScheduled == true" && a.transition == "monitor"),
        "monitor requires closingDateScheduled == true: {arms:?}"
    );
    let skip = gate
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "skip")
        .expect("skip transition");
    assert_eq!(
        skip.to, "closing_schedule_join",
        "no canonical date → straight to the join"
    );
    let monitor = gate
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "monitor")
        .expect("monitor transition");
    assert_eq!(monitor.to, "closing_date_timer");

    // 2. The timer reads its due date from the canonical fact, never a literal.
    let timer = nodes
        .get("closing_date_timer")
        .expect("closing_date_timer node");
    assert_eq!(timer.node_type, "timer");
    let spec = timer.timer.as_ref().expect("timer spec");
    assert_eq!(
        spec.due_at_variable.as_deref(),
        Some("closingDate"),
        "due-at-variable pins the deadline to the canonical deal.closing_date fact"
    );
    assert!(
        spec.due_at.is_none(),
        "no literal due date is invented: {spec:?}"
    );

    // 3. Fire → human escalation; amend → canonical command → the SAME timer re-arms.
    let fire = timer
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "escalate")
        .expect("escalate transition");
    assert_eq!(fire.to, "closing_date_escalation");
    let amend = nodes
        .get("set_closing_date")
        .expect("set_closing_date command node");
    assert_eq!(amend.command_type.as_deref(), Some("deal.set_closing_date"));
    let reschedule = amend
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "reschedule")
        .expect("reschedule transition");
    assert_eq!(
        reschedule.to, "closing_date_timer",
        "amending the date continues the SAME instance at the SAME timer"
    );

    // 4. Negative control: exactly one closing-date timer exists. A second timer on any other
    //    node for closingDate would be an invented parallel SLA framework.
    let timers_on_closing_date: Vec<&str> = nodes
        .values()
        .filter(|n| {
            n.node_type == "timer"
                && n.timer.as_ref().and_then(|s| s.due_at_variable.as_deref())
                    == Some("closingDate")
        })
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(timers_on_closing_date, vec!["closing_date_timer"]);
}
