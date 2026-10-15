//! The half of these tests about what a turn is told and what it is measured against: the story packet a
//! scaffold is made of, the nodes a planning grant is resolved for, the scaffold the resolved policy states,
//! the one stop vocabulary every live stop shares, and the request the context policy bounded.
//!
//! Split out of `tests.rs` on 2026-10-14, when the boundedness gate resumed running and found that file
//! past its 800-line ceiling. Nothing moved between here and there but the text.
// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
/// The story the C1 tests hand the turn: a stated objective and stated acceptance criteria, which is what a
/// scaffold is made of. The title is deliberately NOT the objective — a scaffold that fell back to the title
/// would be authoring the job's goal from the row it was filed under.
fn story_packet_with_stated_facts() -> StoryPacket {
    StoryPacket {
        id: "STORY-V2".into(),
        title: "a story title that is not a goal".into(),
        goal: Some("carry the resolved planning policy into the turn".into()),
        acceptance_criteria: Some("- the turn states the objective it was given".into()),
        ..Default::default()
    }
}

/// A node the engine dispatches whose RESOLVED profile grants planning, discovered from the engine's own tables.
///
/// Discovered rather than named: a node written down here could drift out of the lane tables and leave this test
/// proving a policy nobody resolves. The resolver is the same one the lifecycle calls.
fn node_with_a_planning_grant() -> String {
    discovered_node(|profile| profile.planning_policy != PlanningPolicy::None)
}

/// The other half: a node whose resolved profile grants NO planning. Its turn must be told no scaffold, whatever
/// packets the run happens to carry.
fn node_without_a_planning_grant() -> String {
    discovered_node(|profile| profile.planning_policy == PlanningPolicy::None)
}

/// The first node the engine's own definition binds whose resolved profile answers the predicate.
fn discovered_node(wanted: fn(&crate::engine::harness_profile::HarnessProfile) -> bool) -> String {
    crate::engine::service_binding::forge_service_bindings()
        .expect("the SDLC definition parses")
        .keys()
        .find(|node| {
            crate::engine::harness_profile::resolve(
                &crate::engine::harness_profile::HarnessProfileInput {
                    node_id: node.as_str(),
                    backend: crate::engine::harness::HarnessBackend::OpenCode,
                    model: crate::engine::harness::ModelSelection::from_parts(None, None),
                    execution_id: None,
                    role_attempt: 0,
                    execution_generation: None,
                },
            )
            .map(|profile| wanted(&profile))
            .unwrap_or(false)
        })
        .cloned()
        .expect("the definition binds a node with such a profile")
}

/// C1, LIVE: the profiled turn is TOLD the scaffold its resolved policy states — read from the text the turn was
/// handed, not from a second render of the same inputs.
#[test]
fn a_profiled_turn_is_told_the_scaffold_its_resolved_policy_states() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-planning-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let node = node_with_a_planning_grant();
    let profile = resolved_profile(&node);
    let expected = profile.planning_policy;
    assert_ne!(
        expected,
        PlanningPolicy::None,
        "{node} was chosen for its grant"
    );
    let (harness, seen) =
        harness_capturing_task_text(&workspace, V2_STREAM, story_packet_with_stated_facts());
    let mut task = role_task();
    task.node_id = Some(node.clone());

    // The outcome is not this test's subject: a bare temp workspace has no repository, so the post-turn readings
    // may fail. What C1 asserts is what the turn was TOLD before it ran.
    let _ = harness.run_role_profiled_report(&profile, &node, &task, None);
    let prompt = seen
        .lock()
        .expect("the task-text sink is not poisoned")
        .first()
        .cloned()
        .expect("the turn was handed a task text");
    let header = format!("Planning policy ({}):", expected.label());
    let scaffold = prompt
        .split_once(&header)
        .map(|(_, rest)| rest)
        .unwrap_or_else(|| {
            panic!("the turn must carry the policy its profile resolved ({header}): {prompt}")
        });
    assert!(
        scaffold.contains("carry the resolved planning policy into the turn"),
        "the scaffold states the packet's own objective: {scaffold}"
    );
    assert!(
        scaffold.contains("- the turn states the objective it was given"),
        "and its own acceptance criteria: {scaffold}"
    );
    assert!(
        !scaffold.contains("a story title that is not a goal"),
        "the scaffold invents no objective from the title: {scaffold}"
    );
    assert!(
        !scaffold.contains("(none)"),
        "a fact the packet does not state is omitted, not reported as none: {scaffold}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

/// The fail-closed half, live: a lane with no planning grant is told no scaffold even under a profiled turn and a
/// packet full of facts. Planning is granted by the resolved policy, never by the presence of a story.
#[test]
fn a_lane_without_a_planning_grant_is_told_no_scaffold() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-noplan-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let node = node_without_a_planning_grant();
    let profile = resolved_profile(&node);
    assert_eq!(profile.planning_policy, PlanningPolicy::None);
    let (harness, seen) =
        harness_capturing_task_text(&workspace, V2_STREAM, story_packet_with_stated_facts());
    let mut task = role_task();
    task.node_id = Some(node.clone());

    let _ = harness.run_role_profiled_report(&profile, &node, &task, None);
    let prompt = seen
        .lock()
        .expect("the task-text sink is not poisoned")
        .first()
        .cloned()
        .expect("the turn was handed a task text");
    assert!(
        !prompt.contains("Planning policy"),
        "a lane that granted no planning must be told no planning text: {prompt}"
    );
    assert!(
        prompt.contains("carry the resolved planning policy into the turn"),
        "the story itself still reaches the turn: {prompt}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

/// C3: every live stop Forge applies reports through the ONE vocabulary, and a stop IS the canonical termination.
///
/// The four stops are the four ways a live attempt can end without finishing: its measured allowance ran out, its
/// stream broke the rule every turn is given, it stopped making progress Forge measures, or it was stopped from
/// outside. Three of them are built by the named functions the live path itself calls, and the fourth is folded
/// through the production normalization path — so this pins the mapping production uses rather than a copy of it
/// written beside it.
#[test]
fn every_live_stop_carries_its_reason_in_the_one_vocabulary() {
    use crate::engine::planning::Termination;

    // The allowance: the spend cap mid-turn, and the ceiling the client applies to one turn.
    let budget = budget_stop(Some(0.5), 0.01);
    assert_eq!(budget.code, BUDGET_EXHAUSTED_CODE, "{budget:?}");
    assert_eq!(
        budget.termination(),
        Termination::Stop(StopReason::BudgetExhausted),
        "an allowance running out is one decision, whichever allowance it was"
    );

    // The rule: a stream Forge cannot read.
    let unreadable = stream_parse_trip(STREAM_PARSE_FAILURE_CAP).expect("the cap trips");
    assert_eq!(
        unreadable.termination(),
        Termination::Stop(StopReason::PolicyViolation)
    );

    // Outside Forge's own decision: the interrupt handle, whose reason the caller supplied.
    let interrupted = interrupt_stop("the driver cancelled this generation");
    assert_eq!(interrupted.code, TURN_INTERRUPTED_CODE, "{interrupted:?}");
    assert_eq!(
        interrupted.termination(),
        Termination::Stop(StopReason::Interrupted)
    );

    // The guard, over the production event-normalization path: repetition with no measured progress.
    let mut reads = String::new();
    for at_ms in 1..=3 {
        reads.push_str(&tool_line(at_ms * 100, "read", "filePath", "src/lib.rs"));
        reads.push('\n');
    }
    let (stuck, _) = first_guard_stop(&reads, "feature_scout");
    let stuck = stuck.expect("three identical no-progress actions are a runaway");
    assert_eq!(
        stuck.termination(),
        Termination::Stop(StopReason::NoProgress),
        "a stall is the guard's own stop, in the same vocabulary as every other live stop"
    );
}

/// C2, at the provider seam: the context a resolved policy BOUNDED is what the provider request carries — and the
/// material that must survive survives it.
///
/// The production path is lifecycle → resolved profile → context policy → bounded directive → the harness's
/// `self_heal` argument → the provider's task text. The directive here is composed by
/// `build_self_heal_directive_contextual_report`, the function `roles::lifecycle` calls, under the SAME resolved
/// profile's `context_policy`; the assertion is read from the turn's own `opts.task`, not from a second render of
/// the same inputs.
#[test]
fn the_provider_request_carries_the_context_the_resolved_policy_bounded() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-context-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let node = node_with_a_planning_grant();
    let profile = resolved_profile(&node);
    let mut task = role_task();
    task.node_id = Some(node.clone());
    let (harness, seen) =
        harness_capturing_task_text(&workspace, V2_STREAM, story_packet_with_stated_facts());

    // An attempt trail over Forge's own re-send budget: three oversized earlier replies, then the reply to repair.
    let replies: Vec<String> = vec![
        format!("ELIDED-ONE-MARKER{}", "a".repeat(2000)),
        format!("ELIDED-TWO-MARKER{}", "b".repeat(2000)),
        format!("KEPT-THIRD-MARKER{}", "c".repeat(2000)),
        "AUTHORITATIVE-LAST-MARKER: the reply to repair".to_string(),
    ];
    let composed = build_self_heal_directive_contextual_report(
        &node,
        &["candidateSha".to_string()],
        Some("write the file and commit it"),
        &["the candidate was absent".to_string()],
        &replies,
        &profile.context_policy,
    );
    let decision = composed
        .decision
        .expect("a trail this size is a context decision, not an absence");
    assert_eq!(decision.policy_version, CONTEXT_POLICY_VERSION);
    assert!(
        decision.elided > 0,
        "the policy bounded the trail: {decision:?}"
    );
    assert!(
        decision.output_tokens < decision.input_tokens,
        "the bounded context is smaller than what it bounded: {decision:?}"
    );
    assert!(
        decision.retained > 0,
        "and it did not bound away everything: {decision:?}"
    );

    let _ = harness.run_role_profiled_report(&profile, &node, &task, Some(&composed.directive));

    let prompt = seen
        .lock()
        .expect("the task-text sink is not poisoned")
        .first()
        .cloned()
        .expect("the turn was handed a task text");
    assert!(
        prompt.contains("attempt 1 reply omitted"),
        "the elision record reaches the provider, so the trail stays traceable: {prompt}"
    );
    assert!(
        !prompt.contains("ELIDED-ONE-MARKER") && !prompt.contains("ELIDED-TWO-MARKER"),
        "what the policy elided must NOT be in the provider request: {prompt}"
    );
    assert!(
        prompt.contains("AUTHORITATIVE-LAST-MARKER"),
        "the reply to repair is the material that must survive, and it does: {prompt}"
    );
    assert!(
        prompt.contains("KEPT-THIRD-MARKER"),
        "so does the attempt the policy had no reason to remove: {prompt}"
    );
    let _ = fs::remove_dir_all(&workspace);
}
