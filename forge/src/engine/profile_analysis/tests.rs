// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;

fn record(
    profile: &str,
    kind: TurnKind,
    cost: Option<f64>,
    accepted: bool,
    edit_ms: Option<i64>,
) -> TurnRecord {
    TurnRecord {
        profile_id: profile.into(),
        // No revision unless a test states one: a row that recorded none is an `unknown` population, and the
        // helpers below must not quietly hand every fixture a revision it never had.
        profile_version: None,
        kind,
        provider: None,
        model: None,
        role: None,
        work_type: None,
        execution_phase: None,
        cost_usd: cost,
        accepted,
        trajectory: TrajectoryMetrics {
            first_edit_at_ms: edit_ms,
            last_edit_at_ms: edit_ms,
            verification_calls_after_last_edit: 2,
            ..TrajectoryMetrics::default()
        },
        time_to_first_edit_ms: edit_ms,
        termination_findings: Vec::new(),
    }
}

/// A turn that needs no attempt index to explain it: the role's first turn.
fn first(profile: &str, cost: Option<f64>, accepted: bool, edit_ms: Option<i64>) -> TurnRecord {
    record(profile, TurnKind::First, cost, accepted, edit_ms)
}

/// A repair turn after a rejection.
fn repair(profile: &str, cost: Option<f64>, accepted: bool) -> TurnRecord {
    record(profile, TurnKind::Repair, cost, accepted, None)
}

/// A store row as the turn-keyed select returns it: nothing optional is known unless a test says so.
fn row(attempt_key: &str) -> TurnTrajectoryRow {
    TurnTrajectoryRow {
        attempt_key: attempt_key.to_string(),
        generation_id: None,
        task_id: Some("task-1".to_string()),
        role_attempt: Some(0),
        workflow_node: Some("implement".to_string()),
        role: Some("forge.implement".to_string()),
        harness_profile_id: "smith-structured-v3".to_string(),
        profile_version: Some(3),
        model: None,
        provider: None,
        turn_started_at: None,
        turn_ended_at: None,
        outcome: Some("completed".to_string()),
        first_edit_latency_ms: Some(1200),
        candidate_sha: None,
        tool_call_count: 4,
        edit_call_count: 1,
        shell_call_count: 2,
        structured_call_count: 1,
        first_edit_at_ms: Some(1200),
        last_edit_at_ms: Some(2000),
        verification_calls_after_last_edit: 1,
        tokens_input: None,
        tokens_output: None,
        cost_usd: None,
        usage_known: None,
        story_id: "story-1".to_string(),
        run_phase: Some("build".to_string()),
        result_status: Some("completed".to_string()),
        work_type: Some("feature".to_string()),
        last_failure: None,
        qa_passed: None,
        qa_verified_sha: None,
    }
}

/// A node the DEFINITION itself binds to an edit-capable lane, discovered from the binding table rather than
/// typed here: a hard-coded node name could pass while the definition no longer has it.
fn node_that_may_edit() -> String {
    crate::engine::service_binding::forge_service_bindings()
        .expect("the SDLC definition parses")
        .keys()
        .find(|node| {
            policy_for_node(node)
                .map(|policy| policy.surface().edit)
                .unwrap_or(false)
        })
        .cloned()
        .expect("the definition binds at least one node to an edit-capable lane")
}

/// The whole C3 chain over a store row: the node's action policy decides whether an edit was owed, the row's own
/// counters decide what happened, and the finding is recorded on the graded turn.
#[test]
fn a_turn_that_edited_nothing_where_its_node_owed_an_edit_reports_the_termination() {
    let mut editing_row = row("turn-1");
    editing_row.workflow_node = Some(node_that_may_edit());
    editing_row.edit_call_count = 0;
    assert_eq!(
        turn_termination_findings(&editing_row),
        vec![TerminationFinding::NoEditTermination]
    );
    let comparison = compare_turns(&[editing_row]);
    assert_eq!(comparison[0].no_edit_terminations, 1);
}

#[test]
fn a_node_the_policy_layer_cannot_place_claims_no_missing_edit() {
    let mut orphan_row = row("turn-1");
    orphan_row.workflow_node = Some("no-such-node".to_string());
    orphan_row.edit_call_count = 0;
    assert!(turn_termination_findings(&orphan_row).is_empty());
}

#[test]
fn a_rows_verification_count_is_never_on_its_own_a_waste_claim() {
    let mut busy_row = row("turn-1");
    busy_row.workflow_node = Some(node_that_may_edit());
    busy_row.edit_call_count = 2;
    busy_row.verification_calls_after_last_edit = 9;
    assert!(turn_termination_findings(&busy_row).is_empty());
}

#[test]
fn cost_per_accepted_candidate_is_spend_over_accepts_not_over_turns() {
    // Profile A: four turns, one accepted, $4 of known spend. Cost per accepted is $4, not $1.
    let records = [
        first("a", Some(1.0), false, None),
        first("a", Some(1.0), false, None),
        first("a", Some(1.0), true, Some(10)),
        first("a", Some(1.0), false, None),
    ];
    let row = &compare(&records)[0];
    assert_eq!(row.turns, 4);
    assert_eq!(row.accepted, 1);
    assert_eq!(row.acceptance_rate, 0.25);
    assert_eq!(row.cost_per_accepted_usd, Some(4.0));
}

#[test]
fn a_cheap_profile_that_is_always_rejected_has_no_cost_per_accept() {
    let records = [
        first("cheap", Some(0.1), false, None),
        first("cheap", Some(0.1), false, None),
    ];
    let row = &compare(&records)[0];
    assert_eq!(row.cost_per_accepted_usd, None);
    assert_eq!(row.total_cost_usd, 0.2);
}

#[test]
fn rows_come_back_sorted_by_profile_id_so_the_table_is_reproducible() {
    let records = [
        first("zeta", Some(1.0), true, Some(1)),
        first("alpha", Some(1.0), true, Some(1)),
    ];
    let ids: Vec<String> = compare(&records)
        .into_iter()
        .map(|r| r.profile_id)
        .collect();
    assert_eq!(ids, vec!["alpha".to_string(), "zeta".to_string()]);
}

#[test]
fn the_median_time_to_first_edit_ignores_turns_that_never_edited() {
    let records = [
        first("a", Some(1.0), true, Some(40)),
        first("a", Some(1.0), false, Some(80)),
        first("a", Some(1.0), false, None),
    ];
    assert_eq!(compare(&records)[0].median_time_to_first_edit_ms, Some(60));
}

#[test]
fn an_unmeasured_turn_is_counted_and_never_imputed_as_zero() {
    // Two readings, and one turn nobody measured. The total is the two readings, and the third is admitted as
    // unpriced rather than landing in the sum as a fabricated $0.00.
    let records = [
        first("a", Some(1.0), true, None),
        first("a", Some(1.0), false, None),
        first("a", None, false, None),
    ];
    let row = &compare(&records)[0];
    assert_eq!(row.turns, 3);
    assert_eq!(row.accepted, 1);
    assert_eq!(row.unpriced_turns, 1);
    assert_eq!(row.total_cost_usd, 2.0);
    assert_eq!(row.cost_per_accepted_usd, Some(2.0));
}

#[test]
fn a_first_turn_and_a_repair_turn_are_reported_as_different_populations() {
    // The collapse this replaces: a row keyed on run+profile let the repair turn overwrite the first turn, so the
    // comparison could not tell a profile that lands first time from one that needs two turns.
    let records = [
        first("a", Some(1.0), false, Some(40)),
        first("a", None, false, None),
        repair("a", Some(1.0), true),
        repair("a", Some(1.0), true),
    ];
    let row = &compare(&records)[0];
    assert_eq!(row.turns, 4);
    assert_eq!(row.accepted, 2);
    assert_eq!(row.first_turns.turns, 2);
    assert_eq!(row.first_turns.accepted, 0);
    assert_eq!(row.first_turns.acceptance_rate, 0.0);
    assert_eq!(row.first_turns.median_time_to_first_edit_ms, Some(40));
    assert_eq!(row.repair_turns.turns, 2);
    assert_eq!(row.repair_turns.accepted, 2);
    assert_eq!(row.repair_turns.acceptance_rate, 1.0);
    assert_eq!(row.repair_turns.median_time_to_first_edit_ms, None);
}

#[test]
fn a_profile_with_no_edits_reports_no_edit_timing() {
    let records = [first("a", Some(1.0), false, None)];
    let row = &compare(&records)[0];
    assert_eq!(row.median_time_to_first_edit_ms, None);
    assert_eq!(row.mean_verification_after_last_edit, None);
}

#[test]
fn a_verdict_on_another_candidate_does_not_accept_this_turn() {
    // QA verified the previous candidate; this turn produced a different one, so the old verdict grades nothing here.
    let mut graded = row("task-1:role-attempt:1@gen-1");
    graded.role_attempt = Some(1);
    graded.candidate_sha = Some("new".to_string());
    graded.qa_passed = Some(true);
    graded.qa_verified_sha = Some("old".to_string());
    assert!(!turn_accepted(&graded));

    // The verdict on this turn's own candidate is this turn's verdict.
    graded.qa_verified_sha = Some("new".to_string());
    assert!(turn_accepted(&graded));

    // A pass with no SHA it verified is unplaceable, and an unplaceable pass is not an acceptance.
    graded.qa_verified_sha = None;
    assert!(!turn_accepted(&graded));

    // And a turn that produced no candidate cannot be accepted by anyone's verdict.
    graded.qa_verified_sha = Some("new".to_string());
    graded.candidate_sha = None;
    assert!(!turn_accepted(&graded));
}

#[test]
fn the_rows_own_reading_is_the_one_home_for_a_turns_spend() {
    let mut graded = row("task-1:role-attempt:0@gen-1");
    graded.usage_known = Some(true);
    graded.cost_usd = Some(0.5);
    assert_eq!(turn_cost_usd(&graded), Some(0.5));
    assert!(!turn_spend_is_unknown(&graded));

    // The flag, not the number, admits a reading: a cost with no `known = true` beside it is not priced, so a
    // half-written row can never be counted as spend.
    graded.usage_known = None;
    assert_eq!(turn_cost_usd(&graded), None);
    graded.usage_known = Some(false);
    assert_eq!(turn_cost_usd(&graded), None);
    assert!(turn_spend_is_unknown(&graded));
}

#[test]
fn an_unknown_reading_is_not_zero_and_is_unlike_never_being_measured() {
    // Taken and unknown: a reading was asked for and could not be taken.
    let mut unknown = row("task-1:role-attempt:0@gen-1");
    unknown.usage_known = Some(false);
    unknown.cost_usd = None;
    assert_eq!(turn_cost_usd(&unknown), None);
    assert!(turn_spend_is_unknown(&unknown));

    // Never measured at all: no reading either way. Also not 0, and not the same fact as an unknown reading.
    let unmeasured = row("task-1:role-attempt:0@gen-1");
    assert_eq!(unmeasured.usage_known, None);
    assert_eq!(turn_cost_usd(&unmeasured), None);
    assert!(!turn_spend_is_unknown(&unmeasured));

    // Neither fact is a free turn: both are counted as unpriced, and neither becomes 0.00 of spend.
    let comparison = compare(&[turn_record(&unknown), turn_record(&unmeasured)]);
    assert_eq!(comparison[0].unpriced_turns, 2);
    assert_eq!(comparison[0].total_cost_usd, 0.0);
    assert_eq!(comparison[0].cost_per_accepted_usd, None);
}

#[test]
fn a_measured_turn_is_priced_and_an_unmeasured_one_is_not_a_free_turn() {
    let mut measured = row("task-1:role-attempt:0");
    measured.usage_known = Some(true);
    measured.cost_usd = Some(0.25);
    assert_eq!(turn_cost_usd(&measured), Some(0.25));
    assert!(!turn_spend_is_unknown(&measured));

    // The harness reported no reading: unknown rather than a saving, and the comparison says so.
    measured.usage_known = Some(false);
    measured.cost_usd = None;
    assert_eq!(turn_cost_usd(&measured), None);
    assert!(turn_spend_is_unknown(&measured));
    assert_eq!(compare_turns(&[measured])[0].unpriced_turns, 1);
}

#[test]
fn the_turn_kind_comes_from_the_role_attempt_index() {
    assert_eq!(TurnKind::of_role_attempt(Some(0)), TurnKind::First);
    assert_eq!(TurnKind::of_role_attempt(Some(1)), TurnKind::Repair);
    assert_eq!(TurnKind::of_role_attempt(Some(7)), TurnKind::Repair);
    // A row that does not claim to be a repair is not reported as one.
    assert_eq!(TurnKind::of_role_attempt(None), TurnKind::First);
}

#[test]
fn a_record_reads_its_identity_from_the_row() {
    let mut graded = row("task-1:role-attempt:2@gen-1");
    graded.role_attempt = Some(2);
    graded.provider = Some("anthropic".to_string());
    graded.model = Some("claude-sonnet".to_string());
    graded.candidate_sha = Some("sha".to_string());
    graded.qa_passed = Some(true);
    graded.qa_verified_sha = Some("sha".to_string());
    graded.usage_known = Some(true);
    graded.cost_usd = Some(0.75);
    let record = turn_record(&graded);
    assert_eq!(record.profile_id, "smith-structured-v3");
    assert_eq!(
        record.profile_version,
        Some(3),
        "the revision comes off the row, beside the id rather than folded into it"
    );
    assert_eq!(record.kind, TurnKind::Repair);
    assert_eq!(record.provider.as_deref(), Some("anthropic"));
    assert_eq!(record.model.as_deref(), Some("claude-sonnet"));
    assert_eq!(record.role.as_deref(), Some("forge.implement"));
    assert_eq!(record.work_type.as_deref(), Some("feature"));
    assert_eq!(record.execution_phase.as_deref(), Some("build"));
    assert_eq!(record.cost_usd, Some(0.75));
    assert!(record.accepted);
    assert_eq!(record.time_to_first_edit_ms, Some(1200));
    assert_eq!(record.trajectory.tool_call_count, 4);
    assert_eq!(record.trajectory.verification_calls_after_last_edit, 1);

    // And the store's own read compares without the caller grading anything by hand.
    assert_eq!(compare_turns(&[graded])[0].turns, 1);
}

#[test]
fn reporting_segments_keep_provider_model_role_work_type_and_phase_distinct() {
    let mut first = turn_record(&row("task-1:role-attempt:0"));
    first.provider = Some("anthropic".into());
    first.model = Some("claude-sonnet".into());
    let mut second = first.clone();
    second.provider = Some("openai".into());
    second.model = Some("gpt".into());
    second.role = Some("forge.review".into());
    second.work_type = Some("bug".into());
    second.execution_phase = Some("repair".into());

    let segments = segment_turns(&[second, first]);

    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].key.provider.as_deref(), Some("anthropic"));
    assert_eq!(segments[0].key.model.as_deref(), Some("claude-sonnet"));
    assert_eq!(segments[0].key.role.as_deref(), Some("forge.implement"));
    assert_eq!(segments[0].key.work_type.as_deref(), Some("feature"));
    assert_eq!(segments[0].key.execution_phase.as_deref(), Some("build"));
    assert_eq!(segments[1].key.provider.as_deref(), Some("openai"));
    assert_eq!(segments[1].key.execution_phase.as_deref(), Some("repair"));
}

#[test]
fn reporting_segments_keep_two_revisions_of_one_profile_apart() {
    // One profile, two revisions. The id names the profile and the revision says which one ran: a population keyed
    // on the name alone would state a cost neither revision charged, because the v1 turn's spend is presented as
    // the v2 profile's. A row that recorded no revision is a third population, not a member of either.
    let mut first_revision = first("smith-structured", Some(1.0), true, Some(100));
    first_revision.profile_version = Some(1);
    let mut second_revision = first_revision.clone();
    second_revision.profile_version = Some(2);
    let mut unversioned = first_revision.clone();
    unversioned.profile_version = None;

    let segments = segment_turns(&[second_revision, unversioned, first_revision]);

    assert_eq!(segments.len(), 3, "each revision is its own population");
    assert_eq!(
        segments
            .iter()
            .map(|segment| segment.key.profile_version)
            .collect::<Vec<_>>(),
        vec![None, Some(1), Some(2)],
        "the absent revision sorts first and stays distinct from every recorded one"
    );
    assert!(segments.iter().all(|segment| segment.turns == 1));
    assert!(segments
        .iter()
        .all(|segment| segment.key.profile_id == "smith-structured"));
}

#[test]
fn comparing_keeps_two_revisions_of_one_profile_apart() {
    // The same rule, one table over. The comparison's population is one profile AT ONE REVISION, so a v1 turn's
    // spend is never presented as what the v2 profile costs, and a row that recorded no revision is a third
    // population that belongs to neither. Distinct costs, so a merge would be visible as a sum.
    let mut first_revision = first("smith-structured", Some(1.0), true, Some(100));
    first_revision.profile_version = Some(1);
    let mut second_revision = first_revision.clone();
    second_revision.profile_version = Some(2);
    second_revision.cost_usd = Some(2.0);
    let mut unversioned = first_revision.clone();
    unversioned.profile_version = None;
    unversioned.cost_usd = Some(3.0);

    let rows = compare(&[second_revision, unversioned, first_revision]);

    assert_eq!(rows.len(), 3, "each revision is its own population");
    assert_eq!(
        rows.iter()
            .map(|row| row.profile_version)
            .collect::<Vec<_>>(),
        vec![None, Some(1), Some(2)],
        "the absent revision sorts first and stays distinct from every recorded one"
    );
    assert!(rows.iter().all(|row| row.turns == 1));
    assert!(rows.iter().all(|row| row.profile_id == "smith-structured"));
    assert_eq!(
        rows.iter()
            .map(|row| row.total_cost_usd)
            .collect::<Vec<_>>(),
        vec![3.0, 1.0, 2.0],
        "each population states its own turn's spend and never a sum across revisions"
    );
}
