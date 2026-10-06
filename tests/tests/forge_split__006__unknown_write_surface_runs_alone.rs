//! FORGE.SPLIT — unknown write surface runs alone (TST-FORGE-SPLIT-006).
//!
//! Contract: wave planning (`forge::engine::executor::wave::plan_wave`) is the production
//! boundary that decides which READY lanes run together. This contract proves: a lane whose write surface is unknown (None) never shares a batch with another lane — the planner cannot prove independence, so it serializes it alone.
//!
//! Level: L0 Pure — the production planner, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__006__unknown_write_surface_runs_alone

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SPLIT-006).
fn forge_split_006__unknown_write_surface_runs_alone() {
    let lanes = vec![
        lane("a", Some(&["app/a.rs"])),
        lane("unknown", None),
        lane("b", Some(&["docs/b.rs"])),
    ];
    let plan = plan_wave(&lanes, 8);
    for batch in &plan.batches {
        let has_unknown = batch.iter().any(|l| l.surface.is_none());
        if has_unknown {
            assert_eq!(
                batch.len(),
                1,
                "an unknown-surface lane must run alone, never batched: {:?}",
                batch.iter().map(|l| l.lane.clone()).collect::<Vec<_>>()
            );
        }
    }
    // And the known lanes still batch together — the unknown lane does not poison the rest.
    let known: Vec<usize> = plan
        .batches
        .iter()
        .filter(|b| b.iter().all(|l| l.surface.is_some()))
        .map(|b| b.len())
        .collect();
    assert_eq!(known, vec![2], "the two known lanes share one batch");
}
