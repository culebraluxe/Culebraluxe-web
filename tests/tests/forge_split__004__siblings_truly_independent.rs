//! FORGE.SPLIT — siblings truly independent (TST-FORGE-SPLIT-004).
//!
//! Contract: wave planning (`forge::engine::executor::wave::plan_wave`) is the production
//! boundary that decides which READY lanes run together. This contract proves: two siblings whose write surfaces are disjoint share one batch — they are independent and can run together, and each keeps its own identity through the plan.
//!
//! Level: L0 Pure — the production planner, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__004__siblings_truly_independent

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SPLIT-004).
fn forge_split_004__siblings_truly_independent() {
    let lanes = vec![
        lane("a", Some(&["app/a.rs"])),
        lane("b", Some(&["docs/b.rs"])),
        lane("c", Some(&["api/readme.md"])),
    ];
    let plan = plan_wave(&lanes, 8);
    assert!(
        plan.refusals.is_empty(),
        "disjoint surfaces produce no refusals: {:?}",
        plan.refusals
    );
    assert_eq!(
        plan.batches.len(),
        1,
        "all independent siblings share one batch"
    );
    let mut tasks: Vec<_> = plan.batches[0].iter().map(|l| l.task).collect();
    tasks.sort();
    assert_eq!(
        tasks,
        vec!["a", "b", "c"],
        "every sibling keeps its identity"
    );
}
