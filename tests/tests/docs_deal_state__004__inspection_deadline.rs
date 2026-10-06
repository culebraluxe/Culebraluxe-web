//! DOCS.DEAL.STATE — inspection deadline (TST-DOCS-DEAL-STATE-004).
//!
//! Contract: the inspection-period contingency date is `deal.inspection_deadline`, projected by
//! RE_supermodel into an OPTIONAL fork branch. From the production definition
//! (`middle/workflow/definitions/RE_supermodel-v1.xml`, parsed by
//! `forge::engine::xml::parse_re_supermodel`):
//!
//!   * The branch activates only when BOTH `inspectionApplicable == true` AND
//!     `inspectionDeadlineScheduled == true` — a canonical deadline never appears without a canonical
//!     inspection period.
//!   * `inspection_deadline_timer` reads `due-at-variable="inspectionDeadline"` and fires into the
//!     human escalation task.
//!   * Amending issues `deal.set_inspection_deadline`, whose `reschedule` edge re-arms the SAME timer.
//!   * No inspection period or no scheduled deadline routes the branch straight to the join — no timer
//!     is created, no deadline is invented.
//!
//! Level: L0 Pure — the parsed production definition graph, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_deal_state__004__inspection_deadline

use forge::engine::xml::parse_re_supermodel;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-DEAL-STATE-004).
fn docs_deal_state_004__inspection_deadline() {
    let definition = parse_re_supermodel().expect("RE_supermodel-v1.xml must parse");
    let nodes = &definition.definition.nodes;

    let applicable = nodes
        .get("inspection_deadline_applicable")
        .expect("inspection_deadline_applicable");
    let applicable_arms = applicable.decisions.as_deref().unwrap_or_default();
    assert!(
        applicable_arms
            .iter()
            .any(|a| a.condition == "inspectionApplicable == true" && a.transition == "scheduled"),
        "the monitor requires an applicable inspection track: {applicable_arms:?}"
    );

    let scheduled = nodes
        .get("inspection_deadline_scheduled")
        .expect("inspection_deadline_scheduled");
    let scheduled_arms = scheduled.decisions.as_deref().unwrap_or_default();
    assert!(
        scheduled_arms
            .iter()
            .any(|a| a.condition == "inspectionDeadlineScheduled == true"
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
    assert_eq!(
        skip.to, "join_tracks",
        "no canonical date → no timer, straight to the join"
    );

    let timer = nodes
        .get("inspection_deadline_timer")
        .expect("inspection_deadline_timer");
    assert_eq!(timer.node_type, "timer");
    let spec = timer.timer.as_ref().expect("timer spec");
    assert_eq!(spec.due_at_variable.as_deref(), Some("inspectionDeadline"));
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
    assert_eq!(fire.to, "inspection_deadline_escalation");

    let amend = nodes
        .get("set_inspection_deadline")
        .expect("set_inspection_deadline command node");
    assert_eq!(
        amend.command_type.as_deref(),
        Some("deal.set_inspection_deadline")
    );
    let reschedule = amend
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "reschedule")
        .expect("reschedule");
    assert_eq!(reschedule.to, "inspection_deadline_timer");

    // Negative control: exactly one inspection-deadline timer.
    let timers: Vec<&str> = nodes
        .values()
        .filter(|n| {
            n.node_type == "timer"
                && n.timer.as_ref().and_then(|s| s.due_at_variable.as_deref())
                    == Some("inspectionDeadline")
        })
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(timers, vec!["inspection_deadline_timer"]);
}
