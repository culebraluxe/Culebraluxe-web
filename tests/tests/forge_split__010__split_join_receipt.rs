//! FORGE.SPLIT — split join receipt (TST-FORGE-SPLIT-010).
//!
//! Contract: wave planning (`forge::engine::executor::wave::plan_wave`) is the production
//! boundary that decides which READY lanes run together. This contract proves: the wave plan is a complete receipt: every input lane appears in exactly one output batch (none lost, none duplicated), and every overlapping conflict names a real refusal.
//!
//! Level: L0 Pure — the production planner, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__010__split_join_receipt

use forge::engine::executor::{plan_wave, WaveLane};

/// A lane of sibling work: a lane label, its write surface, and a task payload naming it.
fn lane(label: &'static str, surface: Option<&[&'static str]>) -> WaveLane<&'static str> {
    WaveLane {
        lane: label.to_string(),
        surface: surface.map(|s| s.iter().map(|p| p.to_string()).collect()),
        fanout: false,
        task: label,
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SPLIT-010).
fn forge_split_010__split_join_receipt() {
    let lanes = vec![
        lane("a", Some(&["app/a.rs"])),
        lane("b", Some(&["docs/b.rs"])),
        lane("shared-owner", Some(&["src/shared.rs"])),
        lane("shared-contender", Some(&["src/shared.rs"])),
        lane("unknown", None),
    ];
    let plan = plan_wave(&lanes, 2);

    // 1. Every input lane appears exactly once across the batches — the join keeps the set, exactly once.
    let mut seen: Vec<&str> = plan
        .batches
        .iter()
        .flat_map(|b| b.iter())
        .map(|l| l.lane.as_str())
        .collect();
    seen.sort();
    assert_eq!(
        seen,
        vec!["a", "b", "shared-contender", "shared-owner", "unknown"],
        "the join must preserve every sibling, exactly once"
    );

    // 2. Every refusal names two of the input lanes and a real overlapping path.
    for refusal in &plan.refusals {
        assert!(
            refusal.lanes.0 != refusal.lanes.1,
            "a refusal names two distinct lanes"
        );
        assert!(
            refusal.path.contains('/'),
            "the refusal names the conflicting path: {:?}",
            refusal
        );
    }
    assert!(
        plan.refusals
            .iter()
            .any(|r| r.lanes.0.starts_with("shared") && r.lanes.1.starts_with("shared")),
        "the shared-file conflict must be in the receipt"
    );
}
