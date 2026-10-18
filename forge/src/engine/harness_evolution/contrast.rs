//! Contrast — the behavioral difference between a reference and a candidate execution.
//!
//! Harness² has no ground truth. What it has is the same task executed by two harness revisions
//! under one pinned execution identity, and the difference between what actually happened. This
//! module builds that difference over the typed [`BehaviorDimension`]s and refuses to call it a
//! contrast when the executions are not comparable:
//!
//! * **identity/environment mismatch invalidates the contrast.** If the model, model policy,
//!   executor, tool set, environment class or input dataset differs, the two executions did not
//!   differ only in the harness revision, so the report is an
//!   [`EvolutionError::ProbeIdentityMismatch`] — never a "close enough" comparison.
//! * **an uncaptured dimension is `Unknown`, not `Unchanged`.** A run with no recorded cost is not
//!   a free run; a run with no QA verdict is not a passing one.
//! * **no "looks nicer" dimension.** Every dimension is a count or a verdict.
//! * **the contrast is only a behavioral signal.** It is necessary evidence for Compose, never
//!   sufficient evidence for adoption; Forge validation (`super::validate`) is what turns behavior
//!   into a promotion decision.
//!
//! Both executions must cite durable evidence refs; a contrast with no evidence behind it cannot
//! justify a composition, so it is refused here rather than discovered later.

use super::model::{
    BehaviorDimension, BehaviorEvidence, ContrastDirection, ContrastReport, DimensionContrast,
    EvolutionError, ProbeIdentity,
};

/// Classify a contrast for the composer: did behavior improve, regress, stay flat, or is there not
/// enough captured evidence to say anything?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastVerdict {
    /// At least one dimension improved and none regressed.
    Improved,
    /// At least one dimension regressed (with or without improvements); regressions outrank.
    Regressed,
    /// Every captured dimension is unchanged.
    Neutral,
    /// No dimension was captured on both sides; the executions cannot be compared.
    Insufficient,
}

impl ContrastVerdict {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Improved => "improved",
            Self::Regressed => "regressed",
            Self::Neutral => "neutral",
            Self::Insufficient => "insufficient",
        }
    }

    /// True only when the evidence supports changing the harness.
    pub const fn supports_change(self) -> bool {
        matches!(self, Self::Improved)
    }
}

/// Build a behavioral contrast between two executions of the same task.
///
/// The identities are the pins; the evidence is what actually happened. `id` is the durable
/// contrast id, `base_revision`/`candidate_revision` are the two harness revisions under test.
///
/// Fails closed:
/// * missing ids or evidence ids → [`EvolutionError::MissingEvidence`] / [`EvolutionError::InvalidProbe`];
/// * a comparator compared with itself → [`EvolutionError::InvalidProbe`];
/// * any identity pin or environment class mismatch → [`EvolutionError::ProbeIdentityMismatch`];
/// * an execution without evidence refs → [`EvolutionError::MissingEvidence`].
pub fn contrast(
    id: impl Into<String>,
    base_revision: impl Into<String>,
    candidate_revision: impl Into<String>,
    reference: &BehaviorEvidence,
    candidate: &BehaviorEvidence,
    reference_identity: &ProbeIdentity,
    candidate_identity: &ProbeIdentity,
) -> Result<ContrastReport, EvolutionError> {
    let id = id.into();
    let base_revision = base_revision.into();
    let candidate_revision = candidate_revision.into();

    if id.trim().is_empty() {
        return Err(EvolutionError::MissingEvidence(
            "contrast has no id".to_string(),
        ));
    }
    if base_revision.trim().is_empty() || candidate_revision.trim().is_empty() {
        return Err(EvolutionError::MissingEvidence(
            "contrast must name both harness revisions".to_string(),
        ));
    }

    reference.validate()?;
    candidate.validate()?;
    if reference.evidence_id == candidate.evidence_id {
        return Err(EvolutionError::InvalidProbe(format!(
            "contrast {} compares evidence {} with itself",
            id, reference.evidence_id
        )));
    }
    if !reference.has_evidence() || !candidate.has_evidence() {
        return Err(EvolutionError::MissingEvidence(format!(
            "contrast {} needs durable evidence refs on both executions",
            id
        )));
    }

    // The environment is part of the pinned identity; name it explicitly so a production/staging
    // mix-up is an obvious refusal rather than one differing string among many.
    if reference_identity.environment_class != candidate_identity.environment_class {
        return Err(EvolutionError::ProbeIdentityMismatch {
            reference: reference_identity.summary(),
            candidate: candidate_identity.summary(),
        });
    }

    let evidence_refs = merged_evidence_refs(reference, candidate);
    let mut dimensions = Vec::with_capacity(BehaviorDimension::ALL.len());
    for dimension in BehaviorDimension::ALL {
        let note = dimension_note(dimension, reference, candidate);
        let dimension_contrast = DimensionContrast::from_evidence(
            dimension,
            reference,
            candidate,
            note,
        )
        .with_evidence_refs(evidence_refs.clone());
        dimensions.push(dimension_contrast);
    }

    let report = ContrastReport {
        id,
        base_revision,
        candidate_revision,
        reference_identity: reference_identity.clone(),
        candidate_identity: candidate_identity.clone(),
        summary: summarize(&dimensions),
        evidence_refs,
        dimensions,
    };
    // Identity comparison is already made above, but the report's own contract is the single
    // source of truth for "is this comparable"; call it instead of duplicating the rule.
    report.validate()?;
    Ok(report)
}

/// The verdict the composer uses. Regressions outrank improvements, and a report with no captured
/// dimension is `Insufficient` rather than `Neutral`.
pub fn verdict(report: &ContrastReport) -> ContrastVerdict {
    let mut improved = false;
    let mut regressed = false;
    let mut captured = false;
    for contrast in &report.dimensions {
        match contrast.direction {
            ContrastDirection::Improved => {
                improved = true;
                captured = true;
            }
            ContrastDirection::Regressed => {
                regressed = true;
                captured = true;
            }
            ContrastDirection::Unchanged => captured = true,
            ContrastDirection::Unknown => {}
        }
    }
    if regressed {
        ContrastVerdict::Regressed
    } else if improved {
        ContrastVerdict::Improved
    } else if captured {
        ContrastVerdict::Neutral
    } else {
        ContrastVerdict::Insufficient
    }
}

/// True when the contrast actually differs on at least one captured dimension.
pub fn is_decisive(report: &ContrastReport) -> bool {
    report.dimensions.iter().any(|contrast| {
        matches!(
            contrast.direction,
            ContrastDirection::Improved | ContrastDirection::Regressed
        )
    })
}

/// The dimensions that regressed, in dimension order. Convenience over
/// [`ContrastReport::regressions`] for callers that also want the evidence.
pub fn regressed_dimensions(report: &ContrastReport) -> Vec<BehaviorDimension> {
    report.regressions()
}

/// Merge the two executions' evidence refs deterministically: trimmed, deduplicated, sorted.
fn merged_evidence_refs(
    reference: &BehaviorEvidence,
    candidate: &BehaviorEvidence,
) -> Vec<String> {
    let mut refs: Vec<String> = reference
        .evidence_refs
        .iter()
        .chain(candidate.evidence_refs.iter())
        .map(|reference| reference.trim().to_string())
        .filter(|reference| !reference.is_empty())
        .collect();
    refs.sort();
    refs.dedup();
    refs
}

/// A short factual note for one dimension: the two raw values and whether either was captured.
fn dimension_note(
    dimension: BehaviorDimension,
    reference: &BehaviorEvidence,
    candidate: &BehaviorEvidence,
) -> String {
    match (dimension.value(reference), dimension.value(candidate)) {
        (Some(reference_value), Some(candidate_value)) => format!(
            "{}: reference {reference_value} -> candidate {candidate_value}",
            dimension.id()
        ),
        (Some(reference_value), None) => format!(
            "{}: reference {reference_value} -> candidate not captured",
            dimension.id()
        ),
        (None, Some(candidate_value)) => format!(
            "{}: reference not captured -> candidate {candidate_value}",
            dimension.id()
        ),
        (None, None) => format!("{}: neither execution captured this value", dimension.id()),
    }
}

fn summarize(dimensions: &[DimensionContrast]) -> String {
    let mut improved = 0;
    let mut regressed = 0;
    let mut unchanged = 0;
    let mut unknown = 0;
    for contrast in dimensions {
        match contrast.direction {
            ContrastDirection::Improved => improved += 1,
            ContrastDirection::Regressed => regressed += 1,
            ContrastDirection::Unchanged => unchanged += 1,
            ContrastDirection::Unknown => unknown += 1,
        }
    }
    format!(
        "{improved} improved, {regressed} regressed, {unchanged} unchanged, {unknown} not captured"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::{EnvironmentClass, QaVerdict};

    fn identity() -> ProbeIdentity {
        ProbeIdentity {
            model: "flash".into(),
            model_policy: "default".into(),
            executor: "opencode".into(),
            tools: vec!["read".into(), "grep".into()],
            environment_class: EnvironmentClass::Development,
            input_dataset: "task-6-fixture".into(),
        }
    }

    fn clean_reference() -> BehaviorEvidence {
        let mut evidence = BehaviorEvidence::new("evidence-ref-1", "probe-ref-1");
        evidence.success = true;
        evidence.tests_passed = 5;
        evidence.tests_failed = 0;
        evidence.qa = QaVerdict::Pass;
        evidence.steps = 10;
        evidence.tokens_in = 100;
        evidence.tokens_out = 50;
        evidence.cost_micros = Some(1_000);
        evidence.elapsed_ms = 1_000;
        evidence.evidence_refs = vec!["story-run:1".into(), "receipt:1".into()];
        evidence
    }

    fn improved_candidate() -> BehaviorEvidence {
        let mut evidence = BehaviorEvidence::new("evidence-cand-1", "probe-cand-1");
        evidence.success = true;
        evidence.tests_passed = 7;
        evidence.tests_failed = 0;
        evidence.qa = QaVerdict::Pass;
        evidence.steps = 6;
        evidence.tokens_in = 80;
        evidence.tokens_out = 40;
        evidence.cost_micros = Some(800);
        evidence.elapsed_ms = 900;
        evidence.evidence_refs = vec!["story-run:2".into(), "receipt:2".into()];
        evidence
    }

    fn report() -> ContrastReport {
        contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &improved_candidate(),
            &identity(),
            &identity(),
        )
        .unwrap()
    }

    #[test]
    fn contrast_covers_every_typed_dimension() {
        let report = report();
        assert_eq!(report.dimensions.len(), BehaviorDimension::ALL.len());
        let ids: Vec<&str> = report
            .dimensions
            .iter()
            .map(|contrast| contrast.dimension.id())
            .collect();
        for dimension in BehaviorDimension::ALL {
            assert!(ids.contains(&dimension.id()), "missing {}", dimension.id());
        }
        report.validate().unwrap();
    }

    #[test]
    fn better_candidate_improves_the_expected_dimensions() {
        let report = report();
        let improved = report.improvements();
        assert!(improved.contains(&BehaviorDimension::TestsPassed));
        assert!(improved.contains(&BehaviorDimension::Steps));
        assert!(improved.contains(&BehaviorDimension::Tokens));
        assert!(improved.contains(&BehaviorDimension::Cost));
        assert!(improved.contains(&BehaviorDimension::Elapsed));
        assert!(report.regressions().is_empty());
        assert_eq!(verdict(&report), ContrastVerdict::Improved);
        assert!(is_decisive(&report));
    }

    #[test]
    fn identity_mismatch_invalidates_the_contrast() {
        let mut other = identity();
        other.model = "different-model".into();
        let error = contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &improved_candidate(),
            &identity(),
            &other,
        )
        .unwrap_err();
        assert_eq!(error.code(), "probe_identity_mismatch");
    }

    #[test]
    fn environment_mismatch_invalidates_the_contrast() {
        let mut other = identity();
        other.environment_class = EnvironmentClass::Staging;
        let error = contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &improved_candidate(),
            &identity(),
            &other,
        )
        .unwrap_err();
        assert_eq!(error.code(), "probe_identity_mismatch");
        // The refusal names both environments so the operator can see what differed.
        assert!(error.to_string().contains("staging"));
    }

    #[test]
    fn tool_order_does_not_invalidate_the_contrast() {
        let mut other = identity();
        other.tools = vec!["grep".into(), "read".into(), "read".into()];
        let report = contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &improved_candidate(),
            &identity(),
            &other,
        );
        assert!(report.is_ok());
    }

    #[test]
    fn uncaptured_cost_is_unknown_not_unchanged() {
        let mut candidate = improved_candidate();
        candidate.cost_micros = None;
        let report = contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &candidate,
            &identity(),
            &identity(),
        )
        .unwrap();
        let cost = report
            .dimensions
            .iter()
            .find(|contrast| contrast.dimension == BehaviorDimension::Cost)
            .unwrap();
        assert_eq!(cost.direction, ContrastDirection::Unknown);
        assert_eq!(cost.candidate_value, None);
        assert!(cost.reference_value.is_some());
    }

    #[test]
    fn missing_qa_verdict_is_unknown() {
        let mut candidate = improved_candidate();
        candidate.qa = QaVerdict::NotRun;
        let report = contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &candidate,
            &identity(),
            &identity(),
        )
        .unwrap();
        let qa = report
            .dimensions
            .iter()
            .find(|contrast| contrast.dimension == BehaviorDimension::Qa)
            .unwrap();
        assert_eq!(qa.direction, ContrastDirection::Unknown);
    }

    #[test]
    fn a_regression_is_detected_and_outranks_improvements() {
        let mut candidate = improved_candidate();
        candidate.forbidden_attempts = 2;
        candidate.tool_failures = 4;
        let report = contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &candidate,
            &identity(),
            &identity(),
        )
        .unwrap();
        let regressed = regressed_dimensions(&report);
        assert!(regressed.contains(&BehaviorDimension::ForbiddenAttempts));
        assert!(regressed.contains(&BehaviorDimension::ToolFailures));
        assert_eq!(verdict(&report), ContrastVerdict::Regressed);
        assert!(!ContrastVerdict::Regressed.supports_change());
    }

    #[test]
    fn identical_behavior_is_neutral_not_improved() {
        let report = contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &clean_reference_with_other_ids(),
            &identity(),
            &identity(),
        )
        .unwrap();
        assert_eq!(verdict(&report), ContrastVerdict::Neutral);
        assert!(!verdict(&report).supports_change());
        assert!(!is_decisive(&report));
    }

    fn clean_reference_with_other_ids() -> BehaviorEvidence {
        let mut evidence = clean_reference();
        evidence.evidence_id = "evidence-ref-2".into();
        evidence.probe_id = "probe-ref-2".into();
        evidence
    }

    #[test]
    fn an_execution_compared_with_itself_is_refused() {
        let error = contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &clean_reference(),
            &identity(),
            &identity(),
        )
        .unwrap_err();
        assert_eq!(error.code(), "invalid_probe");
    }

    #[test]
    fn evidence_without_durable_refs_is_refused() {
        let mut bare = improved_candidate();
        bare.evidence_refs.clear();
        let error = contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &bare,
            &identity(),
            &identity(),
        )
        .unwrap_err();
        assert_eq!(error.code(), "missing_evidence");
    }

    #[test]
    fn report_evidence_refs_are_merged_sorted_and_deduplicated() {
        let mut reference = clean_reference();
        reference.evidence_refs = vec!["receipt:1".into(), "story-run:1".into(), " ".into()];
        let mut candidate = improved_candidate();
        candidate.evidence_refs = vec!["story-run:1".into(), "receipt:2".into()];
        let report = contrast(
            "contrast-1",
            "base-1",
            "candidate-1",
            &reference,
            &candidate,
            &identity(),
            &identity(),
        )
        .unwrap();
        assert_eq!(
            report.evidence_refs,
            vec![
                "receipt:1".to_string(),
                "receipt:2".to_string(),
                "story-run:1".to_string()
            ]
        );
        for dimension in &report.dimensions {
            assert_eq!(dimension.evidence_refs, report.evidence_refs);
        }
    }

    #[test]
    fn a_report_with_no_captured_dimension_is_insufficient() {
        let report = ContrastReport {
            id: "contrast-1".into(),
            base_revision: "base-1".into(),
            candidate_revision: "candidate-1".into(),
            reference_identity: identity(),
            candidate_identity: identity(),
            dimensions: vec![DimensionContrast {
                dimension: BehaviorDimension::Qa,
                direction: ContrastDirection::Unknown,
                reference_value: None,
                candidate_value: None,
                note: "not captured".into(),
                evidence_refs: vec!["receipt:1".into()],
            }],
            summary: String::new(),
            evidence_refs: vec!["receipt:1".into()],
        };
        assert_eq!(verdict(&report), ContrastVerdict::Insufficient);
        assert!(!is_decisive(&report));
    }

    #[test]
    fn blank_ids_are_refused() {
        let error = contrast(
            " ",
            "base-1",
            "candidate-1",
            &clean_reference(),
            &improved_candidate(),
            &identity(),
            &identity(),
        )
        .unwrap_err();
        assert_eq!(error.code(), "missing_evidence");
    }
}
