// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;

fn batch(id: &str, story_count: i64) -> ForgeBatchRow {
    ForgeBatchRow {
        id: id.to_string(),
        label: None,
        status: "Staged".to_string(),
        scheduled_for: None,
        fired_at: None,
        created_at: Some("2026-09-28T00:00:00.000Z".to_string()),
        created_by: None,
        note: None,
        model_policy: None,
        story_count,
        queued_count: 0,
        skipped_count: 0,
    }
}

#[test]
fn the_board_shows_each_batch_once_even_though_the_view_repeats_its_counts_per_member() {
    let rows = vec![
        batch("b-1", 3),
        batch("b-1", 3),
        batch("b-1", 3),
        batch("b-2", 1),
    ];
    let batches = dedupe_batches(rows);
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].id, "b-1");
    assert_eq!(batches[0].story_count, 3);
    assert_eq!(batches[1].id, "b-2");
}

#[test]
fn a_blank_hold_reason_reads_as_unknown_never_as_an_empty_string() {
    assert_eq!(normalize_reason(None), "unknown");
    assert_eq!(normalize_reason(Some("")), "unknown");
    assert_eq!(normalize_reason(Some("   ")), "unknown");
    assert_eq!(
        normalize_reason(Some(" prod parity drift ")),
        "prod parity drift"
    );
}

#[test]
fn a_malformed_evidence_blob_is_an_empty_list_not_a_failed_read() {
    assert!(parse_string_array(None).is_empty());
    assert!(parse_string_array(Some("not json")).is_empty());
    assert!(parse_string_array(Some(r#"{"not":"an array"}"#)).is_empty());
    assert_eq!(
        parse_string_array(Some(r#"["web/src/api/engine.rs"]"#)),
        vec!["web/src/api/engine.rs".to_string()]
    );
}

#[test]
fn engine_work_status_distinguishes_retry_from_a_fresh_queue_item() {
    assert_eq!(engine_work_status_bucket("Done", None), Some("Done"));
    assert_eq!(engine_work_status_bucket("Error", None), Some("Error"));
    assert_eq!(
        engine_work_status_bucket("Ready", Some("run-1")),
        Some("Retry")
    );
    assert_eq!(engine_work_status_bucket("Ready", None), None);
    assert_eq!(engine_work_status_bucket("Claimed", Some("run-1")), None);
}
#[test]
fn every_timestamp_in_the_batch_read_is_rendered_in_utc_iso_independent_of_the_session_timezone() {
    let sql = batch_select();
    let rendered = sql.matches("at time zone 'UTC'").count();
    assert_eq!(
        rendered, 3,
        "scheduled_for, fired_at and created_at must all be rendered in UTC"
    );
    assert!(
        sql.contains(ISO_UTC),
        "the one ISO shape is used for all three"
    );
    // The read is a constant: a bound parameter here would mean a caller-supplied fragment.
    assert!(!sql.contains('$'), "batch_select takes no parameters");
}
