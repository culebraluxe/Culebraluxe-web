//! Pianola batch validation and fleet discovery.
//!
//! This module answers one question before a run starts and one question
//! after it ends:
//!
//! 1. Is this assignment a well-formed fleet batch — at least one worker
//!    lane, every story assigned exactly once, no two stories touching the
//!    same target file? (`validate_experiment_batch`,
//!    `validate_experiment_batch_with_targets`, and the cap-bounded
//!    `validate_fleet_batch` / `validate_fleet_batch_with_targets`.)
//!    `discover_worker_batches` builds such an assignment by dividing a
//!    queue across N worker lanes (round-robin: full coverage, no
//!    truncation, no overlap by construction).
//! 2. May the supervisor queue more stories now that work has completed?
//!    Never on its own: once the batch reaches its cap, continuation
//!    requires explicit HITL captain approval
//!    (`require_hitl_to_continue`).
//!
//! The cap is a safety bound, not the batch shape: any N workers may run,
//! and a batch larger than the bound fails closed naming the captain
//! approval it needs. Raising a cap is itself a cap change and requires
//! explicit captain review after the evidence report (see `DO_NOT_SCALE`).
//!
//! System-of-record invariant: Forge stays authoritative. This module
//! creates no table, no trigger, and no queue; it only validates shapes and
//! refuses to grow them past their bound. All overlap detection reuses the
//! supervisor's existing `check_overlapping_targets` over Forge
//! `StoryPacketRow`s — never a new deduplication store.

use std::collections::HashSet;

use db::StoryPacketRow;

use super::supervisor::check_overlapping_targets;

/// Default experiment size. Checked at runtime by `validate_fleet_batch`
/// against the caller's cap; the supervisor additionally warns (never
/// truncates) when a tick runs past its configured bound.
/// The fleet path accepts any N workers — these constants only seed
/// `PianolaConfig::default()`.
pub const PIANOLA_EXPERIMENT_CAP: usize = 4;

/// Default worker lanes in the experiment. Fleet runs pass their own N;
/// growing the fleet is a cap change, which requires explicit captain
/// review after the evidence report.
pub const PIANOLA_WORKER_COUNT: usize = 2;

/// Default stories per worker lane. Fleet runs pass their own shape; the
/// runtime check in `PianolaConfig::for_fleet` keeps workers, stories per
/// worker, and the derived cap honest with each other.
pub const PIANOLA_STORIES_PER_WORKER: usize = 2;

/// Lane identity, as carried by the Forge work rows (`claimed_by`).
pub type WorkerId = String;

/// Story identity, as carried by `storyboard_story.id`.
pub type StoryId = String;

/// Exact text of the `DO_NOT_SCALE` guard file beside this module. The unit
/// test `do_not_scale_file_text_matches` reads the file with `include_str!`
/// so the file and this constant cannot drift apart silently. The semantics
/// are approval-gating, not a fixed number: caps are safety bounds, and
/// raising one needs the captain.
pub const DO_NOT_SCALE_TEXT: &str =
    "Pianola batch caps are safety bounds, not fixed shapes. Raising a cap requires explicit captain approval after the evidence report.";

/// Validate the fleet assignment shape: at least one worker lane, every
/// story assigned to exactly one lane (no duplicates), at least one story
/// total. Any N workers and any per-lane split are accepted — the size bound
/// is the caller's cap (`validate_fleet_batch`), not this shape check.
///
/// Target-file overlap needs the story packets, so it lives in
/// `validate_experiment_batch_with_targets`; this shape-only check runs
/// first and is what the guard script exercises per case.
pub fn validate_experiment_batch(assignments: &[(WorkerId, Vec<StoryId>)]) -> Result<(), String> {
    if assignments.is_empty() {
        return Err("pianola batch needs at least one worker lane".to_string());
    }
    let mut workers = HashSet::new();
    for (worker, _) in assignments {
        if !workers.insert(worker.clone()) {
            return Err(format!(
                "pianola batch rejects duplicate worker id: {worker}"
            ));
        }
    }
    let mut seen = HashSet::new();
    for (_, stories) in assignments {
        for story in stories {
            if !seen.insert(story.clone()) {
                return Err(format!("pianola batch rejects duplicate story id: {story}"));
            }
        }
    }
    let total: usize = assignments.iter().map(|(_, stories)| stories.len()).sum();
    if total == 0 {
        return Err("pianola batch needs at least one story".to_string());
    }
    Ok(())
}

/// Fleet batch validation: the shape check above, plus the safety bound.
/// A batch larger than `cap` fails closed naming the HITL captain approval
/// that raising (or exceeding) the cap requires. The cap bounds the batch;
/// it never reshapes it — discovery output is validated as built, never
/// truncated to fit.
pub fn validate_fleet_batch(
    assignments: &[(WorkerId, Vec<StoryId>)],
    cap: usize,
) -> Result<(), String> {
    validate_experiment_batch(assignments)?;
    let total: usize = assignments.iter().map(|(_, stories)| stories.len()).sum();
    if total > cap {
        return Err(format!(
            "pianola batch holds {total} stories past the safety cap of {cap}; \
             growing the batch requires HITL captain approval"
        ));
    }
    Ok(())
}

/// Divide a queue across N worker lanes (round-robin in queue order).
///
/// Full coverage by construction: every story lands on exactly one lane, so
/// the result never truncates the queue and never assigns a story twice.
/// Lanes past the end of the queue idle with an empty vec — spare fleet
/// capacity is legitimate, and the shape validator accepts it. The result
/// still owes the target check (`validate_fleet_batch_with_targets`): two
/// stories on different lanes may still claim the same file.
pub fn discover_worker_batches(
    worker_ids: &[WorkerId],
    story_ids: &[StoryId],
) -> Result<Vec<(WorkerId, Vec<StoryId>)>, String> {
    if worker_ids.is_empty() {
        return Err("pianola discovery needs at least one worker lane".to_string());
    }
    if story_ids.is_empty() {
        return Err("pianola discovery needs at least one story to divide".to_string());
    }
    let mut seen_workers = HashSet::new();
    for worker in worker_ids {
        if !seen_workers.insert(worker.clone()) {
            return Err(format!(
                "pianola discovery rejects duplicate worker id: {worker}"
            ));
        }
    }
    let mut seen_stories = HashSet::new();
    for story in story_ids {
        if !seen_stories.insert(story.clone()) {
            return Err(format!(
                "pianola discovery rejects duplicate story id: {story}"
            ));
        }
    }
    let mut lanes: Vec<(WorkerId, Vec<StoryId>)> = worker_ids
        .iter()
        .map(|worker| (worker.clone(), Vec::new()))
        .collect();
    let lane_count = lanes.len();
    for (index, story) in story_ids.iter().enumerate() {
        lanes[index % lane_count].1.push(story.clone());
    }
    Ok(lanes)
}

/// Full batch validation: the shape check above, plus the disjoint-targets
/// rule over the assigned story packets. `packets` must hold exactly the
/// stories named in `assignments`; any pair claiming the same normalized
/// file path fails the batch with the overlap named.
pub fn validate_experiment_batch_with_targets(
    assignments: &[(WorkerId, Vec<StoryId>)],
    packets: &[StoryPacketRow],
) -> Result<(), String> {
    validate_experiment_batch(assignments)?;
    check_batch_packets(assignments, packets)
}

/// Fleet batch validation with targets: the cap-bounded shape check plus
/// the disjoint-targets rule. This is what a discovery result owes before
/// it runs: `discover_worker_batches` output validated at the fleet cap.
pub fn validate_fleet_batch_with_targets(
    assignments: &[(WorkerId, Vec<StoryId>)],
    packets: &[StoryPacketRow],
    cap: usize,
) -> Result<(), String> {
    validate_fleet_batch(assignments, cap)?;
    check_batch_packets(assignments, packets)
}

/// Shared packet leg: one packet per assigned story, each packet assigned,
/// and no two packets claiming the same normalized file path.
fn check_batch_packets(
    assignments: &[(WorkerId, Vec<StoryId>)],
    packets: &[StoryPacketRow],
) -> Result<(), String> {
    let assigned: HashSet<&str> = assignments
        .iter()
        .flat_map(|(_, stories)| stories.iter().map(String::as_str))
        .collect();
    if packets.len() != assigned.len() {
        return Err(format!(
            "pianola batch needs one story packet per assigned story ({} assigned, {} packets)",
            assigned.len(),
            packets.len()
        ));
    }
    for packet in packets {
        if !assigned.contains(packet.id.as_str()) {
            return Err(format!(
                "pianola batch packet {} is not assigned to any worker lane",
                packet.id
            ));
        }
    }
    check_overlapping_targets(packets).map_err(|overlap| format!("pianola batch rejects {overlap}"))
}

/// Anti-scale gate: the supervisor may never auto-queue stories past the
/// batch's cap. While the run is still under it (`completed < cap`) there
/// is nothing to decide, so this returns `Ok`; once the batch has completed
/// it returns `Err` naming the HITL approval the captain must give before
/// any further story is dispatched — and raising `cap` itself is a cap
/// change the captain must approve.
///
/// Callers treat `Err` as "stop and wait for the captain", never as a
/// retryable error.
pub fn require_hitl_to_continue(completed_stories: usize, cap: usize) -> Result<(), String> {
    if completed_stories < cap {
        return Ok(());
    }
    Err(format!(
        "pianola batch cap reached ({completed_stories}/{cap} stories); \
         the supervisor refuses to auto-queue further stories — \
         continuing requires HITL captain approval"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(id: &str, text: &str) -> StoryPacketRow {
        StoryPacketRow {
            id: id.to_string(),
            title: format!("story {id}"),
            goal: Some(text.to_string()),
            architect_brief: None,
            acceptance_criteria: None,
            test_mode: None,
            assay_commands: Some(text.to_string()),
        }
    }

    fn good_assignment() -> Vec<(WorkerId, Vec<StoryId>)> {
        vec![
            (
                "worker-a".to_string(),
                vec!["TST-1".to_string(), "TST-2".to_string()],
            ),
            (
                "worker-b".to_string(),
                vec!["TST-3".to_string(), "TST-4".to_string()],
            ),
        ]
    }

    fn good_packets() -> Vec<StoryPacketRow> {
        vec![
            packet("TST-1", "cargo test -p forge --lib alpha"),
            packet("TST-2", "cargo test -p forge --lib beta"),
            packet("TST-3", "cargo test -p forge --lib gamma"),
            packet("TST-4", "cargo test -p forge --lib delta"),
        ]
    }

    #[test]
    fn accepts_good_two_by_two_assignment() {
        assert!(validate_experiment_batch(&good_assignment()).is_ok());
        assert!(
            validate_experiment_batch_with_targets(&good_assignment(), &good_packets()).is_ok()
        );
    }

    #[test]
    fn accepts_any_worker_count() {
        // FIX-010: the 2x2 shape is gone; three lanes of two stories each is
        // a well-formed batch. The bound moved to `validate_fleet_batch`.
        let assignments = vec![
            (
                "worker-a".to_string(),
                vec!["TST-1".to_string(), "TST-2".to_string()],
            ),
            (
                "worker-b".to_string(),
                vec!["TST-3".to_string(), "TST-4".to_string()],
            ),
            (
                "worker-c".to_string(),
                vec!["TST-5".to_string(), "TST-6".to_string()],
            ),
        ];
        assert!(validate_experiment_batch(&assignments).is_ok());
        assert!(validate_fleet_batch(&assignments, 6).is_ok());
    }

    #[test]
    fn accepts_single_worker() {
        let assignments = vec![(
            "worker-a".to_string(),
            vec!["TST-1".to_string(), "TST-2".to_string()],
        )];
        assert!(validate_experiment_batch(&assignments).is_ok());
    }

    #[test]
    fn accepts_uneven_splits() {
        let assignments = vec![
            (
                "worker-a".to_string(),
                vec![
                    "TST-1".to_string(),
                    "TST-2".to_string(),
                    "TST-3".to_string(),
                ],
            ),
            ("worker-b".to_string(), vec!["TST-4".to_string()]),
        ];
        assert!(validate_experiment_batch(&assignments).is_ok());
    }

    #[test]
    fn rejects_empty_batch_and_empty_lanes() {
        let err = validate_experiment_batch(&[]).unwrap_err();
        assert!(
            err.contains("at least one worker lane"),
            "unexpected: {err}"
        );
        let idle: Vec<(WorkerId, Vec<StoryId>)> = vec![("worker-a".to_string(), vec![])];
        let err = validate_experiment_batch(&idle).unwrap_err();
        assert!(err.contains("at least one story"), "unexpected: {err}");
    }

    #[test]
    fn rejects_duplicate_worker_ids() {
        let assignments = vec![
            ("worker-a".to_string(), vec!["TST-1".to_string()]),
            ("worker-a".to_string(), vec!["TST-2".to_string()]),
        ];
        let err = validate_experiment_batch(&assignments).unwrap_err();
        assert!(err.contains("duplicate worker id"), "unexpected: {err}");
    }

    #[test]
    fn fleet_cap_is_a_safety_bound() {
        // The 6-story batch is well-formed but past a cap of 4: it fails
        // closed naming the captain approval, and passes at its own cap.
        let assignments = vec![
            (
                "worker-a".to_string(),
                vec!["TST-1".to_string(), "TST-2".to_string()],
            ),
            (
                "worker-b".to_string(),
                vec!["TST-3".to_string(), "TST-4".to_string()],
            ),
            (
                "worker-c".to_string(),
                vec!["TST-5".to_string(), "TST-6".to_string()],
            ),
        ];
        let err = validate_fleet_batch(&assignments, PIANOLA_EXPERIMENT_CAP).unwrap_err();
        assert!(err.contains("safety cap"), "unexpected: {err}");
        assert!(err.contains("captain approval"), "unexpected: {err}");
        assert!(validate_fleet_batch(&assignments, 6).is_ok());
    }

    #[test]
    fn rejects_duplicate_story_ids() {
        let assignments = vec![
            (
                "worker-a".to_string(),
                vec!["TST-1".to_string(), "TST-2".to_string()],
            ),
            (
                "worker-b".to_string(),
                vec!["TST-2".to_string(), "TST-3".to_string()],
            ),
        ];
        let err = validate_experiment_batch(&assignments).unwrap_err();
        assert!(err.contains("duplicate story id"), "unexpected: {err}");
    }

    #[test]
    fn rejects_overlapping_target_files() {
        let packets = vec![
            packet("TST-1", "edit forge/src/shared.rs for alpha"),
            packet("TST-2", "cargo test -p forge --lib beta"),
            packet("TST-3", "edit forge/src/shared.rs for gamma"),
            packet("TST-4", "cargo test -p forge --lib delta"),
        ];
        let err = validate_experiment_batch_with_targets(&good_assignment(), &packets).unwrap_err();
        assert!(err.contains("overlapping target"), "unexpected: {err}");
    }

    #[test]
    fn rejects_packet_not_in_assignment() {
        let mut packets = good_packets();
        packets[3] = packet("TST-9", "cargo test -p forge --lib delta");
        let err = validate_experiment_batch_with_targets(&good_assignment(), &packets).unwrap_err();
        assert!(err.contains("not assigned"), "unexpected: {err}");
    }

    #[test]
    fn hitl_gate_passes_below_cap() {
        assert!(require_hitl_to_continue(0, PIANOLA_EXPERIMENT_CAP).is_ok());
        assert!(require_hitl_to_continue(3, PIANOLA_EXPERIMENT_CAP).is_ok());
        assert!(require_hitl_to_continue(17, 18).is_ok());
    }

    #[test]
    fn hitl_gate_refuses_at_cap() {
        let err = require_hitl_to_continue(4, PIANOLA_EXPERIMENT_CAP).unwrap_err();
        assert!(err.contains("HITL"), "unexpected: {err}");
        let err = require_hitl_to_continue(7, PIANOLA_EXPERIMENT_CAP).unwrap_err();
        assert!(err.contains("refuses to auto-queue"), "unexpected: {err}");
        let err = require_hitl_to_continue(18, 18).unwrap_err();
        assert!(err.contains("18/18"), "unexpected: {err}");
    }

    #[test]
    fn discovery_divides_queue_across_n_lanes() {
        let workers: Vec<WorkerId> = (0..9).map(|i| format!("worker-{i}")).collect();
        let stories: Vec<StoryId> = (1..=18).map(|i| format!("TST-{i}")).collect();
        let lanes = discover_worker_batches(&workers, &stories).expect("9-way discovery");
        assert_eq!(lanes.len(), 9);
        for (_, lane) in &lanes {
            assert_eq!(lane.len(), 2);
        }
        // Full coverage, no truncation, no overlap: the union is the queue.
        let mut covered: Vec<&str> = lanes
            .iter()
            .flat_map(|(_, lane)| lane.iter().map(String::as_str))
            .collect();
        covered.sort_unstable();
        let mut expected: Vec<&str> = stories.iter().map(String::as_str).collect();
        expected.sort_unstable();
        assert_eq!(covered, expected);
        assert!(validate_fleet_batch(&lanes, 18).is_ok());
    }

    #[test]
    fn discovery_tolerates_spare_capacity_and_rejects_empty_input() {
        let workers: Vec<WorkerId> = (0..3).map(|i| format!("worker-{i}")).collect();
        let stories: Vec<StoryId> = vec!["TST-1".to_string(), "TST-2".to_string()];
        let lanes = discover_worker_batches(&workers, &stories).expect("spare capacity");
        assert_eq!(lanes.len(), 3);
        assert!(lanes[2].1.is_empty());
        assert!(validate_experiment_batch(&lanes).is_ok());

        let err = discover_worker_batches(&[], &stories).unwrap_err();
        assert!(
            err.contains("at least one worker lane"),
            "unexpected: {err}"
        );
        let err = discover_worker_batches(&workers, &[]).unwrap_err();
        assert!(err.contains("at least one story"), "unexpected: {err}");
        let dup_workers = vec!["worker-a".to_string(), "worker-a".to_string()];
        let err = discover_worker_batches(&dup_workers, &stories).unwrap_err();
        assert!(err.contains("duplicate worker id"), "unexpected: {err}");
    }

    #[test]
    fn do_not_scale_file_text_matches() {
        let on_disk = include_str!("DO_NOT_SCALE");
        assert_eq!(on_disk.trim(), DO_NOT_SCALE_TEXT);
    }
}
