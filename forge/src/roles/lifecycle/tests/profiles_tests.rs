//! The half of these tests about which profile a turn runs under, and what the row records when it does: the
//! architect handoff delivered on its first turn, the measurement turn paid once and measured rather than
//! claimed, the lead's decision holding when it is missing, and the backend and model a recorded turn names.
//!
//! Split out of `tests.rs` on 2026-10-14, when the boundedness gate resumed running and found that file
//! past its 800-line ceiling. Nothing moved between here and there but the text.
// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
/// A harness that accepts ONLY the profiled entry points, and records the profile it was handed.
///
/// The unprofiled methods are the ones the lifecycle used before a profile existed, so a seam that resolved a
/// profile and then ran the turn beside it would leave the action surface, the context budget, the planning grant
/// and the stuck thresholds as decoration. This double makes that mistake impossible to miss: it panics.
#[derive(Default)]
struct ProfiledOnlyHarness {
    seen: std::sync::Mutex<Vec<harness_profile::ProfileIdentity>>,
    /// The measurement this double reports for each turn, when a test needs a MEASURED turn.
    ///
    /// The turn's row is written from the report's own trajectory, so `None` here means "no measurement" and no row
    /// — that is the default, because most tests in this file are about the lifecycle's shape rather than its row.
    metrics: Option<crate::engine::trajectory::TrajectoryMetrics>,
}

impl ProfiledOnlyHarness {
    fn seen(&self) -> Vec<harness_profile::ProfileIdentity> {
        self.seen.lock().unwrap().clone()
    }

    fn answer(
        &self,
        profile: &harness_profile::HarnessProfile,
        node_id: &str,
    ) -> crate::engine::runner::HarnessTurnReport {
        self.seen.lock().unwrap().push(profile.identity.clone());
        crate::engine::runner::HarnessTurnReport {
            // The row is written from the report's trajectory, and the IDENTITY on that row comes from the profile
            // this double was handed — which is the point of the double.
            trajectory: self.metrics.clone(),
            outcome: Ok(HarnessOutput {
                trajectory: None,
                raw: format!("ran {node_id}\n"),
                candidate_sha: None,
                assay_commands: vec![],
                acceptance_mapped: false,
                refusal: None,
                execution_base: None,
                usage: None,
            }),
            measured_usage: None,
        }
    }
}

impl RoleHarness for ProfiledOnlyHarness {
    fn run_role(&self, _: &str, _: &ActiveForgeRoleTask, _: Option<&str>) -> Result<HarnessOutput> {
        panic!(
            "a turn must run under the profile the lifecycle resolved, not this unprofiled method"
        )
    }
    fn run_role_report(
        &self,
        _: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        panic!(
            "a turn must run under the profile the lifecycle resolved, not this unprofiled method"
        )
    }
    fn run_role_scoped_report(
        &self,
        _: &str,
        _: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        panic!(
            "a turn must run under the profile the lifecycle resolved, not this unprofiled method"
        )
    }
    fn run_role_profiled_report(
        &self,
        profile: &harness_profile::HarnessProfile,
        node_id: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        self.answer(profile, node_id)
    }
    fn run_role_scoped_profiled_report(
        &self,
        profile: &harness_profile::HarnessProfile,
        _: &str,
        node_id: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        self.answer(profile, node_id)
    }
    fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn run_command(&self, command: &str) -> CommandResult {
        CommandResult {
            cancelled: false,
            command: command.into(),
            exit_code: 1,
            passed: false,
            excerpt: "not run by this harness".into(),
            unmeasurable: true,
            output: String::new(),
        }
    }
}

/// BOTH rails must hand the adapter the profile the lifecycle resolved.
///
/// The scoped and unscoped paths are separate harness methods, and a profile wired at only one of them is the
/// half-wired rail this file warns about elsewhere. What the adapter is handed must also be the LAUNCH's own
/// decisions rather than defaults the seam invented for itself — otherwise a recorded turn would name a backend
/// and a model intent that never ran. A turn the lane does not accept is re-asked, and every re-ask is a later
/// ATTEMPT of the same authorized node, which its profile must say rather than looking like a new unit of work.
#[test]
fn a_turn_runs_under_the_profile_the_lifecycle_resolved() {
    let harness = ProfiledOnlyHarness::default();
    let writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("smith", "TST-PROFILED-TURN");
    let control = CountingAttemptControl::default();
    let mut context = context(&harness, &current, Some(&writer));
    context.harness_backend = crate::engine::harness::HarnessBackend::Maestro;
    context.model_intent = crate::engine::harness::ModelSelection::Judgment;
    context.model_attempt_control = Some(&control);

    run_forge_role_turn(&context, "smith", &task, &NoRoleHooks).expect("turn runs");

    let unscoped = harness.seen();
    assert!(
        !unscoped.is_empty(),
        "the default hooks still ask the harness for a turn"
    );
    assert_eq!(unscoped[0].profile_id, harness_profile::HARNESS_PROFILE_ID);
    assert_eq!(
        unscoped[0].profile_version,
        harness_profile::HARNESS_PROFILE_VERSION
    );
    assert_eq!(
        unscoped[0].execution_id, None,
        "a lane with no durable execution runs the unscoped rail"
    );
    assert_eq!(
        unscoped[0].execution_generation.as_deref(),
        Some("test-generation")
    );
    for (index, profile) in unscoped.iter().enumerate() {
        // The node the WORKFLOW authorized, recorded as identity rather than produced here.
        assert_eq!(profile.node_id, "smith");
        // The launch's own decisions, not defaults: these two are what makes a turn reconstructable.
        assert_eq!(
            profile.backend,
            crate::engine::harness::HarnessBackend::Maestro
        );
        assert_eq!(
            profile.model,
            crate::engine::harness::ModelSelection::Judgment
        );
        // A repair is a later attempt of the SAME authorized node, and its profile says so.
        assert_eq!(profile.role_attempt, index as u32);
    }

    // The scoped rail is a second call site, and it resolves its own profile.
    let scoped = ForgeRoleContext {
        execution_id: Some("exec-1"),
        ..context
    };
    run_forge_role_turn(&scoped, "smith", &task, &NoRoleHooks).expect("scoped turn runs");

    let all = harness.seen();
    assert!(
        all.len() > unscoped.len(),
        "the scoped rail hands over a profile too"
    );
    for (index, profile) in all[unscoped.len()..].iter().enumerate() {
        assert_eq!(profile.execution_id.as_deref(), Some("exec-1"));
        assert_eq!(profile.node_id, "smith");
        assert_eq!(
            profile.role_attempt, index as u32,
            "the scoped rail's own attempts are numbered from zero"
        );
    }
}

/// C6.1: a recorded turn says WHICH configuration ran it.
///
/// The reporting segment key groups turns by provider and model, and before a resolved profile existed a live turn
/// could only ever record `unknown` for both — the columns existed and stayed empty, because the lane had no reading
/// of its own to write. The resolved profile IS that reading: the backend that ran the turn, and the model selection
/// Forge handed that backend. This proves the row is written from the profile the turn actually ran under, not from a
/// default the accounting seam invented for itself.
#[test]
fn a_recorded_turn_names_the_backend_and_model_its_profile_resolved() {
    /// Run one measured turn under the given launch decisions and hand back every row it wrote.
    ///
    /// More than one row is normal here: a reply the lane does not accept is re-asked, and each re-ask is a later
    /// attempt of the SAME authorized node — so the identity has to ride EVERY row rather than only the first.
    fn rows_for(
        backend: crate::engine::harness::HarnessBackend,
        model: crate::engine::harness::ModelSelection,
    ) -> Vec<crate::engine::trajectory::TurnTrajectory> {
        let mut harness = ProfiledOnlyHarness::default();
        harness.metrics = Some(crate::engine::trajectory::TrajectoryMetrics {
            tool_call_count: 3,
            edit_call_count: 1,
            shell_call_count: 1,
            structured_call_count: 1,
            first_edit_at_ms: Some(120),
            last_edit_at_ms: Some(140),
            verification_calls_after_last_edit: 1,
        });
        let writer = RecordingWriter::default();
        let current = ForgeGateEvidence::default();
        let task = role_task("smith", "TST-TURN-IDENTITY");
        let control = CountingAttemptControl::default();
        let mut context = context(&harness, &current, Some(&writer));
        context.story_run_id = Some("run-1");
        context.model_attempt_control = Some(&control);
        context.harness_backend = backend;
        context.model_intent = model;

        run_forge_role_turn(&context, "smith", &task, &NoRoleHooks).expect("turn runs");

        let rows = writer.trajectory.lock().unwrap();
        assert!(!rows.is_empty(), "a measured turn writes a row");
        rows.clone()
    }

    let tiered = rows_for(
        crate::engine::harness::HarnessBackend::Maestro,
        crate::engine::harness::ModelSelection::Judgment,
    );
    for (index, row) in tiered.iter().enumerate() {
        assert_eq!(
            row.provider.as_deref(),
            Some("maestro"),
            "the backend that ran THIS turn, not a default"
        );
        assert_eq!(
            row.model.as_deref(),
            Some("judgment"),
            "the selection Forge handed that backend"
        );
        assert_eq!(row.workflow_node.as_deref(), Some("smith"));
        assert_eq!(
            row.role_attempt,
            Some(index as i32),
            "every row is its own attempt of the same node, and each says so"
        );
    }
    let first = &tiered[0];
    assert!(
        first.turn_started_at.is_some() && first.turn_ended_at.is_some(),
        "the clock read around the harness call still rides the same row"
    );

    // An operator's explicit override names the vendor's own selection, so it is carried verbatim rather than
    // flattened into one of Forge's two tiers: a reader can tell an override from a default.
    let overridden = rows_for(
        crate::engine::harness::HarnessBackend::OpenCode,
        crate::engine::harness::ModelSelection::Explicit("vendor-model-x".into()),
    );
    for row in &overridden {
        assert_eq!(row.provider.as_deref(), Some("opencode"));
        assert_eq!(
            row.model.as_deref(),
            Some("vendor-model-x"),
            "an explicit selection is recorded as the operator named it"
        );
    }
}
