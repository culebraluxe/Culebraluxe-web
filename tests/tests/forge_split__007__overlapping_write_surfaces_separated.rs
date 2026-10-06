//! FORGE.SPLIT — overlapping write surfaces separated (TST-FORGE-SPLIT-007).
//!
//! Contract: wave planning (`forge::engine::executor::wave::plan_wave`) is the production
//! boundary that decides which READY lanes run together. This contract proves: two lanes whose write surfaces overlap are never placed in the same batch, and the planning refusal names the conflicting pair and the file.
//!
//! Level: L0 Pure — the production planner, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__007__overlapping_write_surfaces_separated

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SPLIT-007).
fn forge_split_007__overlapping_write_surfaces_separated() {
    let lanes = vec![
        lane("a", Some(&["src/shared.rs", "src/a.rs"])),
        lane("b", Some(&["src/shared.rs", "src/b.rs"])),
        lane("c", Some(&["web/c.rs"])),
    ];
    let plan = plan_wave(&lanes, 8);
    assert!(
        plan.refusals
            .iter()
            .any(|r| r.lanes == ("a".to_string(), "b".to_string()) && r.path.contains("shared.rs")),
        "the overlap on src/shared.rs must be refused: {:?}",
        plan.refusals
    );
    for batch in &plan.batches {
        let names: Vec<&str> = batch.iter().map(|l| l.lane.as_str()).collect();
        assert!(
            !(names.contains(&"a") && names.contains(&"b")),
            "overlapping lanes must not share a batch: {names:?}"
        );
    }
}
