//! FORGE-FIX-010 — dynamic worker count: 9-way discovery over a fixture queue.
//!
//! The fleet supervisor used to hard-cap the run at 2 workers x 2 stories =
//! 4 total (exact-count checks, a compile-time assert on the number 4, and a
//! `supervisor_tick` that truncated the batch to the cap). This assay pins
//! the fleet behavior against fixture packets:
//!
//! 1. N=9 discovery → 9-way split, full coverage, no truncation, no overlap —
//!    via config, with no code change.
//! 2. Overlap/target guards still reject conflicting batches at N=9.
//! 3. The cap survives as a safety bound: the 18-story fleet batch exceeds
//!    the default experiment cap (4) and fails closed naming the HITL
//!    captain approval; a fleet cap of 18 admits it.
//!
//! No database is involved: discovery and validation are pure functions over
//! fixture story/packet rows.

use db::StoryPacketRow;
use forge::pianola::batch::{
    discover_worker_batches, require_hitl_to_continue, validate_fleet_batch,
    validate_fleet_batch_with_targets, PIANOLA_EXPERIMENT_CAP, PIANOLA_WORKER_COUNT,
};
use forge::pianola::PianolaConfig;

fn worker_ids(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("worker-{i}")).collect()
}

fn story_ids(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("TST-{i}")).collect()
}

/// One disjoint target per story: `forge/src/fleet/probe_<i>.rs` names a
/// distinct normalized path, so the overlap guard must pass.
fn packet(id: &str, index: usize) -> StoryPacketRow {
    let text = format!("cover forge/src/fleet/probe_{index}.rs with tests");
    StoryPacketRow {
        id: id.to_string(),
        title: format!("fleet story {id}"),
        goal: Some(text.clone()),
        architect_brief: None,
        acceptance_criteria: None,
        test_mode: None,
        assay_commands: Some(text),
    }
}

fn disjoint_packets(ids: &[String]) -> Vec<StoryPacketRow> {
    ids.iter()
        .enumerate()
        .map(|(index, id)| packet(id, index))
        .collect()
}

#[test]
fn nine_way_discovery_covers_queue_without_truncation() {
    // Criterion 1: N=9 discovery over an 18-story queue → 9 lanes of 2,
    // full coverage, nothing truncated, nothing assigned twice.
    let fleet = PianolaConfig::for_fleet(9, 2).expect("fleet config needs no code change");
    assert_eq!(fleet.max_workers, 9);
    assert_eq!(fleet.total_cap, 18);

    let workers = worker_ids(fleet.max_workers);
    let queue = story_ids(18);
    let lanes = discover_worker_batches(&workers, &queue).expect("9-way discovery");
    assert_eq!(lanes.len(), 9, "discovery owes one lane per worker");
    for (worker, lane) in &lanes {
        assert_eq!(lane.len(), 2, "lane {worker} owes its share of the queue");
    }
    let mut covered: Vec<&str> = lanes
        .iter()
        .flat_map(|(_, lane)| lane.iter().map(String::as_str))
        .collect();
    covered.sort_unstable();
    let mut expected: Vec<&str> = queue.iter().map(String::as_str).collect();
    expected.sort_unstable();
    assert_eq!(covered, expected, "every queued story runs exactly once");

    let packets = disjoint_packets(&queue);
    assert!(
        validate_fleet_batch_with_targets(&lanes, &packets, fleet.total_cap).is_ok(),
        "disjoint 9-way split validates at its fleet cap"
    );
}

#[test]
fn overlap_and_target_guards_hold_at_nine_workers() {
    // Criterion 2: the valuable enforcement — disjoint targets — still
    // rejects conflicting batches at any N.
    let workers = worker_ids(9);
    let queue = story_ids(18);
    let lanes = discover_worker_batches(&workers, &queue).expect("9-way discovery");

    // Two stories claiming one file fail the batch with the overlap named.
    let mut conflicting = disjoint_packets(&queue);
    conflicting[7] = packet(&queue[7], 3);
    let err = validate_fleet_batch_with_targets(&lanes, &conflicting, 18).unwrap_err();
    assert!(
        err.contains("overlapping target"),
        "overlap must be named, got: {err}"
    );
    assert!(
        err.contains("forge/src/fleet/probe_3.rs"),
        "overlap path must be named, got: {err}"
    );

    // A packet for a story on no lane fails the batch.
    let mut stray = disjoint_packets(&queue);
    stray[17] = packet("TST-99", 99);
    let err = validate_fleet_batch_with_targets(&lanes, &stray, 18).unwrap_err();
    assert!(
        err.contains("not assigned"),
        "stray packet must fail, got: {err}"
    );

    // A story on two lanes fails the batch.
    let mut duplicated = lanes.clone();
    duplicated[8].1.push(queue[0].clone());
    let err = validate_fleet_batch(&duplicated, 18).unwrap_err();
    assert!(
        err.contains("duplicate story id"),
        "double assignment must fail, got: {err}"
    );
}

#[test]
fn default_cap_is_a_safety_bound_not_the_shape() {
    // Criterion 3: the 18-story fleet batch exceeds the default experiment
    // cap (4) and fails closed naming the captain approval — while the
    // default 2x2 experiment shape still validates unchanged.
    let workers = worker_ids(9);
    let queue = story_ids(18);
    let lanes = discover_worker_batches(&workers, &queue).expect("9-way discovery");

    let err = validate_fleet_batch(&lanes, PIANOLA_EXPERIMENT_CAP).unwrap_err();
    assert!(
        err.contains("safety cap"),
        "bound must be named, got: {err}"
    );
    assert!(
        err.contains("captain approval"),
        "HITL approval must be named, got: {err}"
    );
    assert!(validate_fleet_batch(&lanes, 18).is_ok());

    // The HITL gate holds the line at whatever cap it is given.
    assert!(require_hitl_to_continue(17, 18).is_ok());
    let err = require_hitl_to_continue(18, 18).unwrap_err();
    assert!(
        err.contains("HITL"),
        "cap reached must name HITL, got: {err}"
    );

    // The default experiment is untouched: 2 workers, cap 4, still the seed.
    assert_eq!(PIANOLA_WORKER_COUNT, 2);
    let default = PianolaConfig::default();
    assert_eq!(
        (
            default.max_workers,
            default.stories_per_worker,
            default.total_cap
        ),
        (2, 2, 4)
    );
}
