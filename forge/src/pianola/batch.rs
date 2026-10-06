//! Pianola batch size enforcement and anti-scale gate.
//!
//! This is the fourth Pianola slice. It answers one question before the
//! experiment starts and one question after it ends:
//!
//! 1. Is this assignment exactly the blessed shape — 2 workers x 2 stories
//!    = 4 total, no duplicate story IDs, no two stories touching the same
//!    target file? (`validate_experiment_batch`,
//!    `validate_experiment_batch_with_targets`.)
//! 2. May the supervisor queue more stories now that work has completed?
//!    Never on its own: once the first 4 complete, continuation requires
//!    explicit HITL captain approval (`require_hitl_to_continue`).
//!
//! System-of-record invariant: Forge stays authoritative. This module
//! creates no table, no trigger, and no queue; it only validates shapes and
//! refuses to grow them. All overlap detection reuses the supervisor's
//! existing `check_overlapping_targets` over Forge `StoryPacketRow`s —
//! never a new deduplication store.

use std::collections::HashSet;

use db::StoryPacketRow;

use super::supervisor::check_overlapping_targets;

/// Total stories in the experiment. Checked at runtime by
/// `validate_experiment_batch`; the supervisor additionally caps the tick to
/// this many ids and never dispatches beyond it.
pub const PIANOLA_EXPERIMENT_CAP: usize = 4;

/// Worker lanes in the experiment. Anything else is a request to scale,
/// which requires explicit captain review after the evidence report.
pub const PIANOLA_WORKER_COUNT: usize = 2;

/// Stories per worker lane. `WORKER_COUNT * STORIES_PER_WORKER` must equal
/// `EXPERIMENT_CAP`; the constructor-time assertion below keeps the three
/// constants honest with each other.
pub const PIANOLA_STORIES_PER_WORKER: usize = 2;

/// Lane identity, as carried by the Forge work rows (`claimed_by`).
pub type WorkerId = String;

/// Story identity, as carried by `storyboard_story.id`.
pub type StoryId = String;

/// Exact text of the `DO_NOT_SCALE` guard file beside this module. The unit
/// test `do_not_scale_file_text_matches` reads the file with `include_str!`
/// so the file and this constant cannot drift apart silently.
pub const DO_NOT_SCALE_TEXT: &str =
    "This experiment is capped at 4 stories. Scaling requires explicit captain review after evidence report.";

const _: () = {
    assert!(
        PIANOLA_WORKER_COUNT * PIANOLA_STORIES_PER_WORKER == PIANOLA_EXPERIMENT_CAP,
        "pianola caps must satisfy WORKER_COUNT * STORIES_PER_WORKER == EXPERIMENT_CAP"
    );
};

/// Validate the experiment assignment shape: exactly 2 workers, exactly 2
/// stories per worker, exactly 4 stories total, no duplicate story IDs.
///
/// Target-file overlap needs the story packets, so it lives in
/// `validate_experiment_batch_with_targets`; this shape-only check runs
/// first and is what the guard script exercises per case.
pub fn validate_experiment_batch(assignments: &[(WorkerId, Vec<StoryId>)]) -> Result<(), String> {
    if assignments.len() != PIANOLA_WORKER_COUNT {
        return Err(format!(
            "pianola experiment requires exactly {} workers, got {}",
            PIANOLA_WORKER_COUNT,
            assignments.len()
        ));
    }
    for (worker, stories) in assignments {
        if stories.len() != PIANOLA_STORIES_PER_WORKER {
            return Err(format!(
                "pianola worker {worker} must hold exactly {} stories, got {}",
                PIANOLA_STORIES_PER_WORKER,
                stories.len()
            ));
        }
    }
    let total: usize = assignments.iter().map(|(_, stories)| stories.len()).sum();
    if total != PIANOLA_EXPERIMENT_CAP {
        return Err(format!(
            "pianola experiment is capped at {} stories, got {total}",
            PIANOLA_EXPERIMENT_CAP
        ));
    }
    let mut seen = HashSet::new();
    for (_, stories) in assignments {
        for story in stories {
            if !seen.insert(story.clone()) {
                return Err(format!("pianola batch rejects duplicate story id: {story}"));
            }
        }
    }
    Ok(())
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
    let assigned: HashSet<&str> = assignments
        .iter()
        .flat_map(|(_, stories)| stories.iter().map(String::as_str))
        .collect();
    if packets.len() != PIANOLA_EXPERIMENT_CAP {
        return Err(format!(
            "pianola batch needs exactly {} story packets, got {}",
            PIANOLA_EXPERIMENT_CAP,
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
/// first 4. While the experiment is still in flight (`completed < cap`)
/// there is nothing to decide, so this returns `Ok`; once the batch has
/// completed it returns `Err` naming the HITL approval the captain must
/// give before any further story is dispatched.
///
/// Callers treat `Err` as "stop and wait for the captain", never as a
/// retryable error.
pub fn require_hitl_to_continue(completed_stories: usize) -> Result<(), String> {
    if completed_stories < PIANOLA_EXPERIMENT_CAP {
        return Ok(());
    }
    Err(format!(
        "pianola experiment cap reached ({completed_stories}/{} stories); \
         the supervisor refuses to auto-queue further stories — \
         continuing requires HITL captain approval",
        PIANOLA_EXPERIMENT_CAP
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
    fn rejects_three_workers() {
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
        let err = validate_experiment_batch(&assignments).unwrap_err();
        assert!(err.contains("exactly 2 workers"), "unexpected: {err}");
    }

    #[test]
    fn rejects_single_worker() {
        let assignments = vec![(
            "worker-a".to_string(),
            vec!["TST-1".to_string(), "TST-2".to_string()],
        )];
        let err = validate_experiment_batch(&assignments).unwrap_err();
        assert!(err.contains("exactly 2 workers"), "unexpected: {err}");
    }

    #[test]
    fn rejects_three_stories_per_worker() {
        let assignments = vec![
            (
                "worker-a".to_string(),
                vec![
                    "TST-1".to_string(),
                    "TST-2".to_string(),
                    "TST-3".to_string(),
                ],
            ),
            (
                "worker-b".to_string(),
                vec![
                    "TST-4".to_string(),
                    "TST-5".to_string(),
                    "TST-6".to_string(),
                ],
            ),
        ];
        let err = validate_experiment_batch(&assignments).unwrap_err();
        assert!(err.contains("exactly 2 stories"), "unexpected: {err}");
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
    fn hitl_gate_passes_mid_experiment() {
        assert!(require_hitl_to_continue(0).is_ok());
        assert!(require_hitl_to_continue(3).is_ok());
    }

    #[test]
    fn hitl_gate_refuses_after_four_complete() {
        let err = require_hitl_to_continue(4).unwrap_err();
        assert!(err.contains("HITL"), "unexpected: {err}");
        let err = require_hitl_to_continue(7).unwrap_err();
        assert!(err.contains("refuses to auto-queue"), "unexpected: {err}");
    }

    #[test]
    fn do_not_scale_file_text_matches() {
        let on_disk = include_str!("DO_NOT_SCALE");
        assert_eq!(on_disk.trim(), DO_NOT_SCALE_TEXT);
    }
}
