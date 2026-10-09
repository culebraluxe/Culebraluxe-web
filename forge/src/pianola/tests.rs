//! Pianola unit tests: stall boundary, batch caps, overlap, green path.
//!
//! These run without a database: row structs are built literally and the
//! clock is injected via `detect_stall_at`.

use super::load_batch_for_experiment;
use super::supervisor::{
    can_continue_automatically, can_continue_for_packet, check_overlapping_targets,
    detect_stall_at, is_stalled, overlapping_pairs,
};
use super::PianolaConfig;
use crate::engine::assay::CommandResult;
use db::{ForgeQueueWorkRow, StoryPacketRow};

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

fn packet_with_brief(id: &str, goal: &str, brief: &str) -> StoryPacketRow {
    StoryPacketRow {
        id: id.to_string(),
        title: format!("story {id}"),
        goal: Some(goal.to_string()),
        architect_brief: Some(brief.to_string()),
        acceptance_criteria: None,
        test_mode: None,
        assay_commands: Some(goal.to_string()),
    }
}

fn work_item(id: &str, story_id: &str, updated_at: Option<&str>) -> ForgeQueueWorkRow {
    ForgeQueueWorkRow {
        id: id.to_string(),
        story_id: story_id.to_string(),
        state: "Claimed".to_string(),
        lease_owner: Some("worker-a".to_string()),
        claim_generation: 1,
        error_text: None,
        queued_at: None,
        updated_at: updated_at.map(str::to_string),
        attempts: Some(1),
        max_attempts: Some(3),
        heartbeat_at: None,
        lease_expires_at: None,
    }
}

fn stamp_ms(now_ms: i64, age_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(now_ms - age_ms)
        .expect("test timestamp in range")
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn assay_result(command: &str, passed: bool) -> CommandResult {
    CommandResult {
        command: command.to_string(),
        exit_code: if passed { 0 } else { 101 },
        passed,
        excerpt: String::new(),
        unmeasurable: false,
        output: String::new(),
    }
}

fn assay_output(command: &str, output: &str) -> CommandResult {
    CommandResult {
        command: command.to_string(),
        exit_code: 0,
        passed: true,
        excerpt: String::new(),
        unmeasurable: false,
        output: output.to_string(),
    }
}

// --- batch guards (fleet shape; the bound lives in `batch`) ---

#[test]
fn rejects_empty_batch() {
    let ids: Vec<String> = vec![];
    let packets: Vec<StoryPacketRow> = vec![];
    let err = load_batch_for_experiment(&ids, &packets).unwrap_err();
    assert!(err.contains("at least one story"), "unexpected: {err}");
}

#[test]
fn accepts_single_story_batch() {
    let ids = vec!["TST-1".to_string()];
    let packets = vec![packet("TST-1", "forge/src/a.rs")];
    assert!(load_batch_for_experiment(&ids, &packets).is_ok());
}

#[test]
fn rejects_duplicate_story_ids() {
    let ids = vec![
        "TST-1".to_string(),
        "TST-1".to_string(),
        "TST-2".to_string(),
        "TST-3".to_string(),
    ];
    let packets = vec![
        packet("TST-1", "forge/src/a.rs"),
        packet("TST-1", "forge/src/a.rs"),
        packet("TST-2", "forge/src/b.rs"),
        packet("TST-3", "forge/src/c.rs"),
    ];
    let err = load_batch_for_experiment(&ids, &packets).unwrap_err();
    assert!(err.contains("duplicate"), "unexpected: {err}");
}

#[test]
fn accepts_four_disjoint_stories() {
    let ids = vec![
        "TST-1".to_string(),
        "TST-2".to_string(),
        "TST-3".to_string(),
        "TST-4".to_string(),
    ];
    let packets = vec![
        packet("TST-1", "cargo test -p forge --lib a"),
        packet("TST-2", "cargo test -p forge --lib b"),
        packet("TST-3", "cargo test -p forge --lib c"),
        packet("TST-4", "cargo test -p forge --lib d"),
    ];
    assert!(load_batch_for_experiment(&ids, &packets).is_ok());
}

#[test]
fn experiment_caps_are_two_by_two_equals_four() {
    let config = PianolaConfig::default();
    assert_eq!(config.max_workers, 2);
    assert_eq!(config.stories_per_worker, 2);
    assert_eq!(config.total_cap, 4);
    assert_eq!(
        config.max_workers * config.stories_per_worker,
        config.total_cap
    );
}

#[test]
fn fleet_config_scales_to_n_workers() {
    // FIX-010: N=9 needs a config, not a code change.
    let fleet = PianolaConfig::for_fleet(9, 2).expect("fleet config");
    assert_eq!(fleet.max_workers, 9);
    assert_eq!(fleet.stories_per_worker, 2);
    assert_eq!(fleet.total_cap, 18);
    assert_eq!(
        fleet.max_workers * fleet.stories_per_worker,
        fleet.total_cap
    );
    assert!(PianolaConfig::for_fleet(0, 2).is_err());
    assert!(PianolaConfig::for_fleet(9, 0).is_err());
}

#[test]
fn accepts_five_story_batch_without_truncation() {
    // FIX-010: the loader guards identity and disjointness, never the
    // count — five disjoint stories load as given.
    let ids = vec![
        "TST-1".to_string(),
        "TST-2".to_string(),
        "TST-3".to_string(),
        "TST-4".to_string(),
        "TST-5".to_string(),
    ];
    let packets = vec![
        packet("TST-1", "cargo test -p forge --lib a"),
        packet("TST-2", "cargo test -p forge --lib b"),
        packet("TST-3", "cargo test -p forge --lib c"),
        packet("TST-4", "cargo test -p forge --lib d"),
        packet("TST-5", "cargo test -p forge --lib e"),
    ];
    assert_eq!(
        load_batch_for_experiment(&ids, &packets).expect("fleet batch"),
        ids
    );
}

// --- stall detection at the boundary ---

#[test]
fn stall_boundary_is_strictly_older_than_threshold() {
    let threshold = 300_000_u64;
    let now_ms = 1_800_000_000_000_i64;
    // Exactly at the threshold: at the boundary, not past it.
    assert!(!is_stalled(
        Some(&stamp_ms(now_ms, threshold as i64)),
        now_ms,
        threshold
    ));
    // One second past the threshold: stalled.
    assert!(is_stalled(
        Some(&stamp_ms(now_ms, threshold as i64 + 1_000)),
        now_ms,
        threshold
    ));
    // Fresh item: not stalled.
    assert!(!is_stalled(
        Some(&stamp_ms(now_ms, 1_000)),
        now_ms,
        threshold
    ));
}

#[test]
fn stall_unknown_or_future_age_is_never_a_stall() {
    let threshold = 300_000_u64;
    let now_ms = 1_800_000_000_000_i64;
    assert!(!is_stalled(None, now_ms, threshold));
    assert!(!is_stalled(Some(""), now_ms, threshold));
    assert!(!is_stalled(Some("not-a-timestamp"), now_ms, threshold));
    // Future-dated row: negative age, not a stall.
    assert!(!is_stalled(
        Some(&stamp_ms(now_ms, -60_000)),
        now_ms,
        threshold
    ));
}

#[test]
fn detect_stall_at_returns_only_stale_items() {
    let threshold = 300_000_u64;
    let now_ms = 1_800_000_000_000_i64;
    let items = vec![
        work_item("fresh", "TST-1", Some(&stamp_ms(now_ms, 1_000))),
        work_item(
            "stale",
            "TST-2",
            Some(&stamp_ms(now_ms, threshold as i64 + 5_000)),
        ),
        work_item("unknown", "TST-3", None),
    ];
    let stalled = detect_stall_at(&items, threshold, now_ms);
    assert_eq!(stalled.len(), 1);
    assert_eq!(stalled[0].id, "stale");
    assert_eq!(stalled[0].story_id, "TST-2");
}

// --- overlapping targets ---

#[test]
fn overlapping_targets_rejected_with_path_and_pair() {
    let packets = vec![
        packet("TST-1", "cover forge/src/pianola/supervisor.rs with tests"),
        packet(
            "TST-2",
            "refactor forge/src/pianola/supervisor.rs for clarity",
        ),
        packet("TST-3", "cargo test -p forge --lib b"),
        packet("TST-4", "cargo test -p forge --lib c"),
    ];
    let err = check_overlapping_targets(&packets).unwrap_err();
    assert!(
        err.contains("forge/src/pianola/supervisor.rs"),
        "unexpected: {err}"
    );
    assert!(
        err.contains("TST-1") && err.contains("TST-2"),
        "unexpected: {err}"
    );
    let pairs = overlapping_pairs(&packets);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].1, "TST-1");
    assert_eq!(pairs[0].2, "TST-2");
}

#[test]
fn scope_and_operating_surface_text_counts_as_target() {
    // `story_packet` folds scope/operating_surface into architect_brief, so a
    // shared path there must trip the same guard as one in assay_commands.
    let packets = vec![
        packet_with_brief(
            "TST-1",
            "add contract tests",
            "SCOPE:\nforge/src/pianola/status.rs\nOPERATING SURFACE:\nread-only",
        ),
        packet_with_brief(
            "TST-2",
            "summarize receipts",
            "SCOPE:\nforge/src/pianola/status.rs",
        ),
    ];
    assert!(check_overlapping_targets(&packets).is_err());
}

#[test]
fn disjoint_targets_pass_overlap_check() {
    let packets = vec![
        packet("TST-1", "cover forge/src/pianola/supervisor.rs"),
        packet("TST-2", "cover forge/src/pianola/batch.rs"),
    ];
    assert!(check_overlapping_targets(&packets).is_ok());
    assert!(overlapping_pairs(&packets).is_empty());
}

// --- green-path continuation ---

#[test]
fn green_path_continues_only_on_clean_pass() {
    let passing = vec![
        assay_result("cargo check -p forge --all-targets", true),
        assay_result("cargo test -p forge --lib pianola", true),
    ];
    assert!(can_continue_automatically("TST-1", &passing));
    assert!(!can_continue_automatically("TST-1", &[]));
    let failing = vec![
        assay_result("cargo check -p forge --all-targets", true),
        assay_result("cargo test -p forge --lib pianola", false),
    ];
    assert!(!can_continue_automatically("TST-1", &failing));
    let unmeasurable = vec![CommandResult {
        unmeasurable: true,
        ..assay_result("cargo test -p forge --lib pianola", true)
    }];
    assert!(!can_continue_automatically("TST-1", &unmeasurable));
}

#[test]
fn green_path_refuses_security_and_prod_touch() {
    let clean = vec![assay_result("cargo test -p forge --lib pianola", true)];
    assert!(can_continue_automatically("TST-1", &clean));
    let security = vec![assay_output(
        "cargo test -p forge --lib entitlements",
        "touches casbin authz policy evaluation",
    )];
    assert!(!can_continue_automatically("TST-1", &security));
    let prod_touch = vec![assay_output(
        "cargo test -p forge --lib pianola",
        "modified production file middle/workflow/src/engine/engine_options.rs",
    )];
    assert!(!can_continue_automatically("TST-1", &prod_touch));
    let brief_conflict = vec![assay_output(
        "cargo test -p forge --lib pianola",
        "architect_brief conflict: goal says read-only but brief asks for writes",
    )];
    assert!(!can_continue_automatically("TST-1", &brief_conflict));
}

#[test]
fn packet_continue_refuses_brief_conflict_and_overlap() {
    let passing = vec![assay_result("cargo test -p forge --lib pianola", true)];
    let clean_batch = vec![
        packet("TST-1", "cover forge/src/pianola/supervisor.rs"),
        packet("TST-2", "cover forge/src/pianola/batch.rs"),
    ];
    assert!(can_continue_for_packet(
        &clean_batch[0],
        &passing,
        &clean_batch
    ));
    let conflicted = packet_with_brief(
        "TST-1",
        "add tests",
        "this brief is in conflict with the goal above",
    );
    assert!(!can_continue_for_packet(
        &conflicted,
        &passing,
        &clean_batch
    ));
    let overlapping_batch = vec![
        packet("TST-1", "cover forge/src/pianola/supervisor.rs"),
        packet("TST-2", "refactor forge/src/pianola/supervisor.rs"),
    ];
    assert!(!can_continue_for_packet(
        &overlapping_batch[0],
        &passing,
        &overlapping_batch
    ));
}
