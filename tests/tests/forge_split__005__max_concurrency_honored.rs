//! FORGE.SPLIT — max concurrency honored (TST-FORGE-SPLIT-005).
//!
//! Contract: wave planning (`forge::engine::executor::wave::plan_wave`) is the production
//! boundary that decides which READY lanes run together. This contract proves: no batch exceeds the concurrency cap, and the cap of 1 forces strict serialization.
//!
//! Level: L0 Pure — the production planner, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__005__max_concurrency_honored

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SPLIT-005).
fn forge_split_005__max_concurrency_honored() {
    let lanes = vec![
        lane("a", Some(&["app/a.rs"])),
        lane("b", Some(&["docs/b.rs"])),
        lane("c", Some(&["web/c.rs"])),
        lane("d", Some(&["db/d.rs"])),
        lane("e", Some(&["cli/e.rs"])),
    ];
    let plan = plan_wave(&lanes, 2);
    assert!(
        plan.batches.iter().all(|b| b.len() <= 2),
        "no batch exceeds the cap: {:?}",
        plan.batches.iter().map(|b| b.len()).collect::<Vec<_>>()
    );
    let total: usize = plan.batches.iter().map(|b| b.len()).sum();
    assert_eq!(total, 5, "no sibling is lost by capping");

    let single = plan_wave(&lanes, 1);
    assert!(
        single.batches.iter().all(|b| b.len() == 1),
        "cap 1 serializes: {:?}",
        single.batches.iter().map(|b| b.len()).collect::<Vec<_>>()
    );
}
