//! Forge Validate — the stage that makes a behavioral contrast insufficient on its own.
//!
//! Harness² can only see behavior: a candidate produced a passing run with fewer steps. Forge can
//! see more — the policy's protected surfaces, the regression rails, replay validation, and the
//! Phase 3 evidence that motivated the experiment in the first place. This module runs those four
//! gates on the **composed successor** and maps the outcome into [`ValidationEvidence`], the shape
//! the adoption policy consumes.
//!
//! The rule the whole subsystem is built around lives here: **a behavioral contrast alone is not
//! sufficient**. A perfect contrast with no regression rails, no replay and no Phase 3 evidence
//! produces a `MissingEvidence`-shaped outcome — every gate `NotRun` — and
//! [`ValidationOutcome::sufficient_for_adoption`] stays false. Only four passed gates with durable
//! evidence refs make an automatic adoption legal, and even then the adoption policy still checks
//! the authority class of every changed surface.
//!
//! The static policy re-check is deliberately re-run here rather than trusted from Propose: the
//! predecessor ran before any probe, and the candidate has since been rebuilt into a successor by
//! Compose. The gate re-proves `successor = base + changes` and re-refuses protected surfaces.

use super::experiment::{ValidationContext, Validator};
use super::model::{
    ComponentChange, ContrastReport, EvolutionError, GateStatus, HarnessRevision,
    ValidationEvidence,
};
use super::policy;

/// The four Forge validation gates, in the order [`ValidationEvidence::gates`] reports them.
pub const VALIDATION_GATE_NAMES: [&str; 4] = [
    "static_policy",
    "regression_rails",
    "replay",
    "phase3_evidence",
];

/// The regression rails input: did the successor still pass the rails the base passed?
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RegressionRails {
    pub status: GateStatus,
    pub evidence_refs: Vec<String>,
}

impl RegressionRails {
    pub fn passed(evidence_refs: Vec<String>) -> Self {
        Self {
            status: GateStatus::Passed,
            evidence_refs,
        }
    }

    pub fn failed(evidence_refs: Vec<String>) -> Self {
        Self {
            status: GateStatus::Failed,
            evidence_refs,
        }
    }

    pub fn not_run() -> Self {
        Self::default()
    }
}

/// Replay validation input: a recorded replay of the probed execution, linked by id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReplayValidation {
    pub replay_id: String,
    pub status: GateStatus,
    pub evidence_refs: Vec<String>,
}

impl ReplayValidation {
    pub fn not_run() -> Self {
        Self::default()
    }

    pub fn passed(replay_id: impl Into<String>, evidence_refs: Vec<String>) -> Self {
        Self {
            replay_id: replay_id.into(),
            status: GateStatus::Passed,
            evidence_refs,
        }
    }

    pub fn failed(replay_id: impl Into<String>, evidence_refs: Vec<String>) -> Self {
        Self {
            replay_id: replay_id.into(),
            status: GateStatus::Failed,
            evidence_refs,
        }
    }
}

/// Phase 3 evidence review: did the signal that motivated the candidate actually change?
///
/// The note is required for a passing review — an evidence review that cannot say what it found is
/// not a review.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Phase3EvidenceReview {
    pub status: GateStatus,
    pub note: String,
    pub evidence_refs: Vec<String>,
}

impl Phase3EvidenceReview {
    pub fn not_run() -> Self {
        Self::default()
    }

    pub fn passed(note: impl Into<String>, evidence_refs: Vec<String>) -> Self {
        Self {
            status: GateStatus::Passed,
            note: note.into(),
            evidence_refs,
        }
    }

    pub fn failed(note: impl Into<String>, evidence_refs: Vec<String>) -> Self {
        Self {
            status: GateStatus::Failed,
            note: note.into(),
            evidence_refs,
        }
    }
}

/// Everything the validation stage is given: the base revision, the composed successor, the exact
/// change list, the behavioral contrasts that justified it, and the three Forge-side inputs.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationRequest<'a> {
    pub base: &'a HarnessRevision,
    pub successor: &'a HarnessRevision,
    pub changes: &'a [ComponentChange],
    pub contrasts: Vec<&'a ContrastReport>,
    pub regression_rails: RegressionRails,
    pub replay: ReplayValidation,
    pub phase3: Phase3EvidenceReview,
}

/// What Forge validation concluded. Gate failures are an outcome, not an error: a failed gate
/// means "reject", a `NotRun` gate means "more evidence". Only a contract violation (a contrast
/// measured against another base, a successor that is not a child of the base) is an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationOutcome {
    pub validation: ValidationEvidence,
    pub candidate_revision: String,
    pub contrast_ids: Vec<String>,
    pub reasons: Vec<String>,
}

impl ValidationOutcome {
    /// Read-only view of the evidence the adoption policy consumes.
    pub fn validation_evidence(&self) -> &ValidationEvidence {
        &self.validation
    }

    /// Every gate passed and the evidence is linked. The only state in which automatic adoption is
    /// even a candidate; the authority class check still happens in the policy.
    pub fn sufficient_for_adoption(&self) -> bool {
        self.validation.is_clean()
    }

    pub fn is_clean(&self) -> bool {
        self.validation.is_clean()
    }

    pub fn failed_gates(&self) -> Vec<&'static str> {
        self.validation.failed_gates()
    }

    pub fn missing_gates(&self) -> Vec<&'static str> {
        self.validation.missing_gates()
    }

    /// True when the only Forge-side evidence is the behavioral contrast: the rails, replay and
    /// Phase 3 review have not run. This is the state the module exists to refuse to promote.
    pub fn behavioral_contrast_alone(&self) -> bool {
        self.validation.regression_rails == GateStatus::NotRun
            && self.validation.replay == GateStatus::NotRun
            && self.validation.phase3_evidence == GateStatus::NotRun
    }
}

/// Run the four Forge validation gates on a composed successor.
pub fn validate(request: &ValidationRequest<'_>) -> Result<ValidationOutcome, EvolutionError> {
    if request.contrasts.is_empty() {
        return Err(EvolutionError::MissingEvidence(
            "Forge validation needs at least one behavioral contrast; there is nothing to validate"
                .into(),
        ));
    }
    request.base.verify_integrity()?;
    request.successor.verify_integrity()?;
    if request.successor.parent_revision_id.as_deref() != Some(request.base.id.as_str()) {
        return Err(EvolutionError::MissingEvidence(format!(
            "successor {} is not a child of the validated base {}",
            request.successor.id, request.base.id
        )));
    }
    if request.successor.domain != request.base.domain {
        return Err(EvolutionError::MissingEvidence(format!(
            "successor domain {} does not match base domain {}",
            request.successor.domain, request.base.domain
        )));
    }

    let mut reasons: Vec<String> = Vec::new();
    let mut evidence_refs: Vec<String> = Vec::new();
    let mut contrast_ids: Vec<String> = Vec::new();
    for contrast in &request.contrasts {
        contrast.validate()?;
        if contrast.base_revision != request.base.id {
            return Err(EvolutionError::MissingEvidence(format!(
                "contrast {} was measured against {} rather than the validated base {}",
                contrast.id, contrast.base_revision, request.base.id
            )));
        }
        if contrast.candidate_revision == request.base.id {
            return Err(EvolutionError::MissingEvidence(format!(
                "contrast {} compares the base revision with itself",
                contrast.id
            )));
        }
        if !contrast_ids.contains(&contrast.id) {
            contrast_ids.push(contrast.id.clone());
        }
        evidence_refs.extend(contrast_evidence_refs(contrast));
    }

    // Gate 1: static policy, re-run against the composed successor.
    let static_policy =
        match policy::validate_candidate(request.base, request.successor, request.changes) {
            Ok(()) => GateStatus::Passed,
            Err(violations) => {
                for violation in &violations {
                    reasons.push(format!(
                        "static policy {}: {}",
                        violation.code.id(),
                        violation.message
                    ));
                }
                GateStatus::Failed
            }
        };

    // Gate 2: regression rails.
    let regression_rails = match request.regression_rails.status {
        GateStatus::Passed if request.regression_rails.evidence_refs.is_empty() => {
            reasons.push(
                "regression rails were reported passed without any durable evidence refs".into(),
            );
            GateStatus::Failed
        }
        GateStatus::Passed => GateStatus::Passed,
        GateStatus::Failed => {
            reasons.push("regression rails failed on the composed successor".into());
            GateStatus::Failed
        }
        GateStatus::NotRun => {
            reasons.push("regression rails were not run".into());
            GateStatus::NotRun
        }
    };

    // Gate 3: replay validation.
    let replay = match request.replay.status {
        GateStatus::Passed
            if request.replay.replay_id.trim().is_empty()
                || request.replay.evidence_refs.is_empty() =>
        {
            reasons.push(
                "replay validation was reported passed without a replay id or durable evidence"
                    .into(),
            );
            GateStatus::Failed
        }
        GateStatus::Passed => GateStatus::Passed,
        GateStatus::Failed => {
            reasons.push("replay validation failed on the composed successor".into());
            GateStatus::Failed
        }
        GateStatus::NotRun => {
            reasons.push("replay validation was not run".into());
            GateStatus::NotRun
        }
    };

    // Gate 4: Phase 3 evidence review.
    let phase3_evidence = match request.phase3.status {
        GateStatus::Passed
            if request.phase3.note.trim().is_empty() || request.phase3.evidence_refs.is_empty() =>
        {
            reasons.push(
                "Phase 3 evidence review was reported passed without a note or durable evidence"
                    .into(),
            );
            GateStatus::Failed
        }
        GateStatus::Passed => GateStatus::Passed,
        GateStatus::Failed => {
            reasons.push("Phase 3 evidence review failed".into());
            GateStatus::Failed
        }
        GateStatus::NotRun => {
            reasons.push("Phase 3 evidence review was not run".into());
            GateStatus::NotRun
        }
    };

    evidence_refs.extend(request.regression_rails.evidence_refs.iter().cloned());
    evidence_refs.extend(request.replay.evidence_refs.iter().cloned());
    evidence_refs.extend(request.phase3.evidence_refs.iter().cloned());
    let evidence_refs = canonical_refs(evidence_refs);
    if evidence_refs.is_empty() {
        reasons.push("no durable evidence refs were supplied by any gate".into());
        // Keep the list empty: a fabricated ref would be worse than none.
    }

    let validation = ValidationEvidence {
        static_policy,
        regression_rails,
        replay,
        phase3_evidence,
        evidence_refs,
    };

    Ok(ValidationOutcome {
        validation,
        candidate_revision: request.successor.id.clone(),
        contrast_ids,
        reasons,
    })
}

/// The bridge from an experiment [`ValidationContext`] to Forge's real validation stage.
///
/// The experiment carries the composed change list verbatim, so this adapter builds the
/// [`ValidationRequest`] from the context's base, successor, **exact changes** and contrasts — no
/// diff is synthesized and no gate evidence is invented. The three Forge-side gate inputs
/// (regression rails, replay, Phase 3 review) are supplied when the stage is constructed because
/// they are operator/CI evidence, not harness content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeValidationStage {
    pub regression_rails: RegressionRails,
    pub replay: ReplayValidation,
    pub phase3: Phase3EvidenceReview,
}

impl ForgeValidationStage {
    pub fn new(
        regression_rails: RegressionRails,
        replay: ReplayValidation,
        phase3: Phase3EvidenceReview,
    ) -> Self {
        Self {
            regression_rails,
            replay,
            phase3,
        }
    }

    /// Build the exact request the seam carries. A missing successor is a contract error, never a
    /// quietly empty validation: "no successor" would otherwise read as "not run".
    pub fn request<'a>(
        &self,
        context: &ValidationContext<'a>,
    ) -> Result<ValidationRequest<'a>, EvolutionError> {
        let successor = context.successor.ok_or_else(|| {
            EvolutionError::MissingEvidence(
                "Forge validation needs the composed successor revision".into(),
            )
        })?;
        Ok(ValidationRequest {
            base: context.base,
            successor,
            changes: context.changes,
            contrasts: context.contrasts.iter().collect(),
            regression_rails: self.regression_rails.clone(),
            replay: self.replay.clone(),
            phase3: self.phase3.clone(),
        })
    }

    /// Run the four gates and keep the reasons/contrast ids for the operator surface.
    pub fn validate_outcome(
        &self,
        context: &ValidationContext<'_>,
    ) -> Result<ValidationOutcome, EvolutionError> {
        let request = self.request(context)?;
        validate(&request)
    }
}

impl Validator for ForgeValidationStage {
    fn validate(
        &self,
        context: &ValidationContext<'_>,
    ) -> Result<ValidationEvidence, EvolutionError> {
        self.validate_outcome(context)
            .map(|outcome| outcome.validation)
    }
}

/// Evidence links for a contrast. A contrast with no direct refs still identifies itself, so a
/// validation never silently reports "no evidence" for a contrast that exists.
fn contrast_evidence_refs(contrast: &ContrastReport) -> Vec<String> {
    if contrast.evidence_refs.is_empty() {
        vec![format!("contrast:{}", contrast.id)]
    } else {
        contrast.evidence_refs.clone()
    }
}

fn canonical_refs(refs: Vec<String>) -> Vec<String> {
    let mut refs: Vec<String> = refs
        .into_iter()
        .map(|reference| reference.trim().to_string())
        .filter(|reference| !reference.is_empty())
        .collect();
    refs.sort();
    refs.dedup();
    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::contrast::contrast;
    use crate::engine::harness_evolution::model::{
        BehaviorEvidence, Component, ComponentChange, ComponentKind, ContrastDirection,
        DimensionContrast, EnvironmentClass, HarnessDomain, HarnessManifest, ProbeIdentity,
        QaVerdict,
    };

    fn identity() -> ProbeIdentity {
        ProbeIdentity {
            model: "flash".into(),
            model_policy: "default".into(),
            executor: "opencode".into(),
            tools: vec!["read".into()],
            environment_class: EnvironmentClass::Development,
            input_dataset: "task-6-fixture".into(),
        }
    }

    fn evidence(id: &str, steps: u64) -> BehaviorEvidence {
        let mut evidence = BehaviorEvidence::new(format!("evidence-{id}"), format!("probe-{id}"));
        evidence.success = true;
        evidence.tests_passed = 5;
        evidence.qa = QaVerdict::Pass;
        evidence.steps = steps;
        evidence.evidence_refs = vec![format!("receipt:{id}")];
        evidence
    }

    fn contrast_report() -> ContrastReport {
        contrast(
            "contrast-1",
            "base-1",
            "candidate-rev-1",
            &evidence("ref", 10),
            &evidence("cand", 5),
            &identity(),
            &identity(),
        )
        .unwrap()
    }

    fn base_revision() -> HarnessRevision {
        let manifest = HarnessManifest::empty().with_component(Component::green(
            ComponentKind::SystemPrompt,
            "baseline prompt",
            "base prompt",
        ));
        HarnessRevision::draft("base-1", HarnessDomain::RustBackend, manifest, "owner")
    }

    fn composed_successor(
        base: &HarnessRevision,
        addition: &str,
    ) -> (HarnessRevision, Vec<ComponentChange>) {
        let changes = vec![ComponentChange::set(
            Component::green(
                ComponentKind::SystemPrompt,
                format!("baseline prompt\n\n{addition}"),
                "base prompt",
            ),
            "validated change",
        )];
        let revision = HarnessRevision::from_changes(base, "successor-1", &changes, "smith")
            .unwrap()
            .with_status(crate::engine::harness_evolution::model::RevisionStatus::Composed);
        (revision, changes)
    }

    fn clean_request<'a>(
        base: &'a HarnessRevision,
        successor: &'a HarnessRevision,
        changes: &'a [ComponentChange],
        report: &'a ContrastReport,
    ) -> ValidationRequest<'a> {
        ValidationRequest {
            base,
            successor,
            changes,
            contrasts: vec![report],
            regression_rails: RegressionRails::passed(vec!["rails:330".into()]),
            replay: ReplayValidation::passed("replay:42", vec!["replay:42".into()]),
            phase3: Phase3EvidenceReview::passed(
                "the scorecard weakness disappeared in the probe",
                vec!["scorecard:run-1".into()],
            ),
        }
    }

    #[test]
    fn four_passing_gates_are_clean_and_sufficient() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let report = contrast_report();
        let outcome = validate(&clean_request(&base, &successor, &changes, &report)).unwrap();
        assert!(outcome.is_clean());
        assert!(outcome.sufficient_for_adoption());
        assert!(outcome.failed_gates().is_empty());
        assert!(outcome.missing_gates().is_empty());
        assert!(!outcome.behavioral_contrast_alone());
        assert_eq!(outcome.contrast_ids, vec!["contrast-1".to_string()]);
        assert!(!outcome.validation.evidence_refs.is_empty());
    }

    #[test]
    fn a_behavioral_contrast_alone_is_never_sufficient() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let report = contrast_report();
        let request = ValidationRequest {
            base: &base,
            successor: &successor,
            changes: &changes,
            contrasts: vec![&report],
            regression_rails: RegressionRails::not_run(),
            replay: ReplayValidation::not_run(),
            phase3: Phase3EvidenceReview::not_run(),
        };
        let outcome = validate(&request).unwrap();
        assert!(outcome.behavioral_contrast_alone());
        assert!(!outcome.sufficient_for_adoption());
        assert!(!outcome.is_clean());
        assert_eq!(
            outcome.missing_gates(),
            vec!["regression_rails", "replay", "phase3_evidence"]
        );
        // The static policy gate ran and passed; it alone does not promote a harness.
        assert_eq!(outcome.validation.static_policy, GateStatus::Passed);
    }

    #[test]
    fn a_policy_violation_fails_the_static_gate() {
        let base = base_revision();
        // A successor whose manifest is NOT base + changes: the policy re-check catches it.
        let bogus = HarnessManifest::empty().with_component(Component::green(
            ComponentKind::SystemPrompt,
            "unrelated",
            "base prompt",
        ));
        let successor =
            HarnessRevision::draft("successor-1", HarnessDomain::RustBackend, bogus, "smith")
                .with_parent(Some(base.id.clone()));
        let changes = vec![ComponentChange::set(
            Component::green(
                ComponentKind::SystemPrompt,
                "baseline prompt\n\nADDED",
                "base prompt",
            ),
            "claimed change",
        )];
        let report = contrast_report();
        let outcome = validate(&clean_request(&base, &successor, &changes, &report)).unwrap();
        assert_eq!(outcome.validation.static_policy, GateStatus::Failed);
        assert!(outcome.failed_gates().contains(&"static_policy"));
        assert!(!outcome.sufficient_for_adoption());
    }

    #[test]
    fn failed_regression_rails_fail_the_gate() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let report = contrast_report();
        let mut request = clean_request(&base, &successor, &changes, &report);
        request.regression_rails = RegressionRails::failed(vec!["rails:330".into()]);
        let outcome = validate(&request).unwrap();
        assert_eq!(outcome.validation.regression_rails, GateStatus::Failed);
        assert!(outcome.failed_gates().contains(&"regression_rails"));
    }

    #[test]
    fn passed_rails_without_evidence_are_failed() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let report = contrast_report();
        let mut request = clean_request(&base, &successor, &changes, &report);
        request.regression_rails = RegressionRails::passed(vec![]);
        let outcome = validate(&request).unwrap();
        assert_eq!(outcome.validation.regression_rails, GateStatus::Failed);
    }

    #[test]
    fn passed_replay_without_a_replay_id_is_failed() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let report = contrast_report();
        let mut request = clean_request(&base, &successor, &changes, &report);
        request.replay = ReplayValidation::passed("", vec!["replay:42".into()]);
        let outcome = validate(&request).unwrap();
        assert_eq!(outcome.validation.replay, GateStatus::Failed);
    }

    #[test]
    fn failed_replay_is_failed() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let report = contrast_report();
        let mut request = clean_request(&base, &successor, &changes, &report);
        request.replay = ReplayValidation::failed("replay:42", vec!["replay:42".into()]);
        let outcome = validate(&request).unwrap();
        assert_eq!(outcome.validation.replay, GateStatus::Failed);
    }

    #[test]
    fn passed_phase3_review_without_a_note_is_failed() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let report = contrast_report();
        let mut request = clean_request(&base, &successor, &changes, &report);
        request.phase3 = Phase3EvidenceReview::passed("", vec!["scorecard:run-1".into()]);
        let outcome = validate(&request).unwrap();
        assert_eq!(outcome.validation.phase3_evidence, GateStatus::Failed);
    }

    #[test]
    fn contrasts_measured_against_another_base_are_refused() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let foreign = contrast(
            "contrast-1",
            "other-base",
            "candidate-rev-1",
            &evidence("ref", 10),
            &evidence("cand", 5),
            &identity(),
            &identity(),
        )
        .unwrap();
        let request = clean_request(&base, &successor, &changes, &foreign);
        assert_eq!(validate(&request).unwrap_err().code(), "missing_evidence");
    }

    #[test]
    fn a_successor_that_is_not_a_child_of_the_base_is_refused() {
        let base = base_revision();
        let (mut successor, changes) = composed_successor(&base, "ADDED");
        successor.parent_revision_id = Some("someone-else".into());
        let report = contrast_report();
        let request = clean_request(&base, &successor, &changes, &report);
        assert_eq!(validate(&request).unwrap_err().code(), "missing_evidence");
    }

    #[test]
    fn no_contrasts_means_nothing_to_validate() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let report = contrast_report();
        let mut request = clean_request(&base, &successor, &changes, &report);
        request.contrasts.clear();
        assert_eq!(validate(&request).unwrap_err().code(), "missing_evidence");
    }

    #[test]
    fn an_invalid_contrast_is_a_typed_error() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let report = contrast_report();
        // A hand-built report whose identities differ is refused by ContrastReport::validate, and
        // validate propagates the typed error instead of validating a non-comparable contrast.
        let hand_built = ContrastReport {
            id: "contrast-1".into(),
            base_revision: "base-1".into(),
            candidate_revision: "candidate-rev-1".into(),
            reference_identity: identity(),
            candidate_identity: ProbeIdentity {
                model: "other-model".into(),
                ..identity()
            },
            dimensions: vec![DimensionContrast {
                dimension: crate::engine::harness_evolution::model::BehaviorDimension::Success,
                direction: ContrastDirection::Improved,
                reference_value: Some(0.0),
                candidate_value: Some(1.0),
                note: String::new(),
                evidence_refs: vec!["receipt:ref".into()],
            }],
            summary: String::new(),
            evidence_refs: vec!["receipt:ref".into()],
        };
        let mut request = clean_request(&base, &successor, &changes, &report);
        request.contrasts = vec![&hand_built];
        assert_eq!(
            validate(&request).unwrap_err().code(),
            "probe_identity_mismatch"
        );
    }

    #[test]
    fn gate_names_are_stable() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base, "ADDED");
        let report = contrast_report();
        let outcome = validate(&clean_request(&base, &successor, &changes, &report)).unwrap();
        let names: Vec<&str> = outcome
            .validation
            .gates()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, VALIDATION_GATE_NAMES.to_vec());
    }
}

#[cfg(test)]
mod seam_tests {
    //! D3 seam tests: the validator receives the experiment's exact composed changes, and a
    //! "passed" Forge gate with no durable evidence is downgraded to Failed at the seam.

    use super::*;
    use crate::engine::harness_evolution::experiment::ValidationContext;
    use crate::engine::harness_evolution::model::{
        BehaviorDimension, BehaviorEvidence, Component, ComponentDecision, ComponentDecisionRecord,
        ComponentKind, CompositionDecision, DimensionContrast, EnvironmentClass, HarnessDomain,
        HarnessManifest, ProbeIdentity, QaVerdict,
    };

    fn identity() -> ProbeIdentity {
        ProbeIdentity {
            model: "flash".into(),
            model_policy: "default".into(),
            executor: "opencode".into(),
            tools: vec!["read".into()],
            environment_class: EnvironmentClass::Development,
            input_dataset: "seam-fixture".into(),
        }
    }

    fn evidence(id: &str, steps: u64) -> BehaviorEvidence {
        let mut evidence = BehaviorEvidence::new(format!("evidence-{id}"), format!("probe-{id}"));
        evidence.success = true;
        evidence.qa = QaVerdict::Pass;
        evidence.steps = steps;
        evidence.evidence_refs = vec![format!("receipt:{id}")];
        evidence
    }

    fn base_revision() -> HarnessRevision {
        HarnessRevision::draft(
            "base-1",
            HarnessDomain::RustBackend,
            HarnessManifest::empty().with_component(Component::green(
                ComponentKind::SystemPrompt,
                "baseline prompt",
                "base prompt",
            )),
            "owner",
        )
    }

    fn composed_successor(base: &HarnessRevision) -> (HarnessRevision, Vec<ComponentChange>) {
        let changes = vec![ComponentChange::set(
            Component::green(
                ComponentKind::SystemPrompt,
                "baseline prompt\n\nplan before editing",
                "base prompt",
            ),
            "validated change",
        )];
        let successor = HarnessRevision::from_changes(base, "successor-1", &changes, "smith")
            .expect("successor is base + changes");
        (successor, changes)
    }

    fn composition(successor: &HarnessRevision) -> CompositionDecision {
        CompositionDecision {
            id: "composition-1".into(),
            base_revision: "base-1".into(),
            candidates: vec!["candidate-1".into()],
            contrast_ids: vec!["contrast-1".into()],
            decisions: vec![ComponentDecisionRecord::new(
                ComponentKind::SystemPrompt,
                ComponentDecision::Adopt,
                "the candidate was better",
            )
            .with_evidence_for(vec!["evidence:1".into()])],
            successor_manifest_hash: successor.manifest_hash.clone(),
            summary: "seam composition".into(),
            evidence_refs: vec!["evidence:1".into()],
        }
    }

    fn contrast_report() -> ContrastReport {
        let identity = identity();
        let reference = evidence("ref", 10);
        let candidate = evidence("cand", 5);
        ContrastReport {
            id: "contrast-1".into(),
            base_revision: "base-1".into(),
            candidate_revision: "successor-1".into(),
            reference_identity: identity.clone(),
            candidate_identity: identity,
            dimensions: vec![DimensionContrast::from_evidence(
                BehaviorDimension::Steps,
                &reference,
                &candidate,
                "fewer steps",
            )],
            summary: "seam contrast".into(),
            evidence_refs: vec!["evidence:1".into()],
        }
    }

    fn clean_stage() -> ForgeValidationStage {
        ForgeValidationStage::new(
            RegressionRails::passed(vec!["rails:seam".into()]),
            ReplayValidation::passed("replay:seam", vec!["replay:seam".into()]),
            Phase3EvidenceReview::passed(
                "the scorecard weakness disappeared",
                vec!["scorecard:run-seam".into()],
            ),
        )
    }

    fn context<'a>(
        base: &'a HarnessRevision,
        successor: &'a HarnessRevision,
        changes: &'a [ComponentChange],
        composition: &'a CompositionDecision,
        contrasts: &'a [ContrastReport],
        changed_kinds: &'a [ComponentKind],
    ) -> ValidationContext<'a> {
        ValidationContext {
            experiment_id: "experiment-seam",
            domain: HarnessDomain::RustBackend,
            base,
            successor: Some(successor),
            composition,
            contrasts,
            changed_kinds,
            changes,
        }
    }

    #[test]
    fn validator_receives_the_exact_composed_changes() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base);
        let composition = composition(&successor);
        let contrast = contrast_report();
        let kinds = [ComponentKind::SystemPrompt];
        let stage = clean_stage();
        let ctx = context(
            &base,
            &successor,
            &changes,
            &composition,
            std::slice::from_ref(&contrast),
            &kinds,
        );

        let evidence = stage.validate(&ctx).expect("exact changes validate");
        assert_eq!(evidence.static_policy, GateStatus::Passed);
        assert!(evidence.is_clean());
        assert_eq!(evidence.regression_rails, GateStatus::Passed);

        // The same successor with an EMPTY change list is not base + changes: the static policy
        // gate must fail, proving the validator consumed the composed changes rather than a diff
        // synthesized from the two manifests.
        let no_changes: [ComponentChange; 0] = [];
        let wrong = context(
            &base,
            &successor,
            &no_changes,
            &composition,
            std::slice::from_ref(&contrast),
            &kinds,
        );
        let evidence = stage
            .validate(&wrong)
            .expect("a failed gate is an outcome, not an error");
        assert_eq!(
            evidence.static_policy,
            GateStatus::Failed,
            "the validator must see the exact changes it was handed"
        );
        assert!(!evidence.is_clean());
    }

    #[test]
    fn unlinked_passed_gate_is_downgraded_to_failed_at_the_seam() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base);
        let composition = composition(&successor);
        let contrast = contrast_report();
        let kinds = [ComponentKind::SystemPrompt];
        let stage = ForgeValidationStage::new(
            // "passed" with no durable evidence refs must not count as passed.
            RegressionRails::passed(Vec::new()),
            ReplayValidation::passed("replay:seam", vec!["replay:seam".into()]),
            Phase3EvidenceReview::passed(
                "the scorecard weakness disappeared",
                vec!["scorecard:run-seam".into()],
            ),
        );
        let ctx = context(
            &base,
            &successor,
            &changes,
            &composition,
            std::slice::from_ref(&contrast),
            &kinds,
        );

        let outcome = stage.validate_outcome(&ctx).expect("outcome");
        assert_eq!(outcome.validation.regression_rails, GateStatus::Failed);
        assert!(!outcome.sufficient_for_adoption());
        assert!(outcome
            .reasons
            .iter()
            .any(|reason| reason.contains("without any durable evidence refs")));
    }

    #[test]
    fn missing_successor_and_missing_contrast_are_refused() {
        let base = base_revision();
        let (successor, changes) = composed_successor(&base);
        let composition = composition(&successor);
        let kinds = [ComponentKind::SystemPrompt];
        let stage = clean_stage();
        let contrasts: [ContrastReport; 0] = [];

        let no_successor = ValidationContext {
            experiment_id: "experiment-seam",
            domain: HarnessDomain::RustBackend,
            base: &base,
            successor: None,
            composition: &composition,
            contrasts: &contrasts,
            changed_kinds: &kinds,
            changes: &changes,
        };
        let error = stage
            .validate(&no_successor)
            .expect_err("missing successor is a contract error");
        assert!(error.to_string().contains("successor"));

        let no_contrast = context(
            &base,
            &successor,
            &changes,
            &composition,
            &contrasts,
            &kinds,
        );
        let error = stage
            .validate(&no_contrast)
            .expect_err("no contrast means nothing to validate");
        assert!(error.to_string().contains("contrast"));
    }
}
