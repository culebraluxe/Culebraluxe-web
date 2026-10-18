//! Adoption: the only module allowed to move the active harness pointer.
//!
//! Policy, not this file, decides whether a validated experiment may be adopted. [`apply`]
//! recomputes the recommendation from the catalog's successor revision and the experiment's
//! validation evidence, then refuses any decision that disagrees with it. The resulting rules:
//!
//! * GREEN surfaces + a clean `ValidationEvidence` → the only automatic adoption path. The
//!   decision must have been made by policy (`AdoptionDecision::is_automatic`) and the experiment
//!   must be `Adoptable`.
//! * YELLOW changed surface → `Council` escalation; no pointer move.
//! * RED changed surface, failed validation, incomplete validation, empty change → the policy
//!   verdict is `Chris` / `Reject` / `MoreEvidence`; no automatic adoption is possible.
//! * A rejected revision can never activate: the catalog refuses it and this module checks the
//!   decision/recommendation agreement first.
//!
//! Rollback is a catalog operation wrapped here so it returns the exact restored revision — the
//! same id and manifest hash that was active before — rather than rebuilding anything.

use std::fmt;

use super::catalog::{Activation, CatalogError, HarnessCatalog, RevisionStore};
use super::experiment::{ExperimentError, HarnessExperiment};
use super::model::{
    AdoptionDecision, AdoptionVerdict, ComponentKind, HarnessDomain, HarnessRevision,
    RevisionStatus, ValidationEvidence,
};
use super::policy;

/// How an adoption request was handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdoptionRoute {
    /// Applied through the catalog: the pointer moved.
    AutomaticGreen,
    /// Escalated to Council; the pointer did not move.
    Council,
    /// Escalated to the owner (Chris); the pointer did not move.
    Owner,
    /// Rejected; the pointer did not move.
    Rejected,
    /// More evidence is required; the pointer did not move.
    MoreEvidence,
}

impl AdoptionRoute {
    pub const fn id(self) -> &'static str {
        match self {
            Self::AutomaticGreen => "automatic_green",
            Self::Council => "council",
            Self::Owner => "owner",
            Self::Rejected => "rejected",
            Self::MoreEvidence => "more_evidence",
        }
    }
}

/// The result of an adoption decision. `activated` is the only field that can mean the pointer
/// moved; every escalation route leaves it `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptionOutcome {
    pub decision: AdoptionDecision,
    pub route: AdoptionRoute,
    pub recommendation: AdoptionVerdict,
    pub changed_kinds: Vec<ComponentKind>,
    pub reasons: Vec<String>,
    pub activated: Option<Activation>,
}

/// The result of a rollback: the pointer move and the exact revision it restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackOutcome {
    pub activation: Activation,
    pub restored: HarnessRevision,
}

/// Why an adoption decision was refused. A refusal never moves the pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdoptionError {
    Catalog(CatalogError),
    Experiment(ExperimentError),
    WrongDomain { decision: HarnessDomain, experiment: HarnessDomain },
    NoSuccessor { experiment_id: String },
    RevisionMismatch { expected: String, actual: String },
    UnknownRevision { revision_id: String },
    RejectedRevisionCannotActivate { revision_id: String },
    ExperimentNotAdoptable { state: RevisionStatus },
    /// The policy's recommendation and the decision disagree — the policy is the authority.
    DecisionContradictsPolicy {
        recommended: AdoptionVerdict,
        decided: AdoptionVerdict,
    },
    /// The composed successor does not survive the pre-adoption policy gate
    /// (`policy::validate_candidate`): a protected surface, an authority expansion, a malformed
    /// change or a manifest that is not exactly base+changes. Refused before any pointer move.
    CandidatePolicyViolations { violations: Vec<String> },
    /// The policy would allow adoption, but the decision was not made by the policy path, so it
    /// is not an automatic GREEN adoption.
    AutomaticAdoptionRefused { reasons: Vec<String> },
}

impl fmt::Display for AdoptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(error) => write!(f, "{error}"),
            Self::Experiment(error) => write!(f, "{error}"),
            Self::WrongDomain {
                decision,
                experiment,
            } => write!(
                f,
                "adoption decision is for domain {decision}, but the experiment is {experiment}"
            ),
            Self::NoSuccessor { experiment_id } => {
                write!(f, "experiment {experiment_id} has no composed successor")
            }
            Self::RevisionMismatch { expected, actual } => write!(
                f,
                "adoption decision names revision {actual}, but the experiment composed {expected}"
            ),
            Self::UnknownRevision { revision_id } => {
                write!(f, "adoption names unknown revision {revision_id}")
            }
            Self::RejectedRevisionCannotActivate { revision_id } => write!(
                f,
                "revision {revision_id} is rejected and can never become the active harness"
            ),
            Self::ExperimentNotAdoptable { state } => write!(
                f,
                "experiment is {state}; only an adoptable experiment may be adopted"
            ),
            Self::DecisionContradictsPolicy {
                recommended,
                decided,
            } => write!(
                f,
                "policy recommends {}, but the decision says {}",
                recommended.id(),
                decided.id()
            ),
            Self::CandidatePolicyViolations { violations } => write!(
                f,
                "policy refused the composed successor: {}",
                violations.join("; ")
            ),
            Self::AutomaticAdoptionRefused { reasons } => write!(
                f,
                "automatic adoption refused: {}",
                reasons.join("; ")
            ),
        }
    }
}

impl std::error::Error for AdoptionError {}

impl From<CatalogError> for AdoptionError {
    fn from(error: CatalogError) -> Self {
        Self::Catalog(error)
    }
}

impl From<ExperimentError> for AdoptionError {
    fn from(error: ExperimentError) -> Self {
        Self::Experiment(error)
    }
}

/// Read the policy recommendation for an experiment without applying anything. Used by the
/// operator surface and by [`apply`] as the single source of truth.
pub fn recommend<S: RevisionStore>(
    experiment: &HarnessExperiment,
    catalog: &HarnessCatalog<S>,
) -> Result<(AdoptionVerdict, Vec<ComponentKind>, Vec<String>), AdoptionError> {
    let successor_id = experiment
        .successor_revision
        .clone()
        .ok_or_else(|| AdoptionError::NoSuccessor {
            experiment_id: experiment.id.clone(),
        })?;
    let base = catalog
        .revision(&experiment.base_revision)
        .ok_or_else(|| AdoptionError::UnknownRevision {
            revision_id: experiment.base_revision.clone(),
        })?;
    let successor = catalog
        .revision(&successor_id)
        .ok_or(AdoptionError::UnknownRevision {
            revision_id: successor_id,
        })?;
    let validation = experiment
        .validation
        .clone()
        .unwrap_or_else(ValidationEvidence::pending);
    let recommendation = policy_gate(base, successor, &validation)?;
    Ok((
        recommendation.verdict,
        recommendation.changed_kinds,
        recommendation.reasons,
    ))
}

/// The single pre-adoption policy gate. It re-runs `policy::validate_candidate` against the
/// **canonical base->successor change list**, not an empty placeholder: a hand-built experiment
/// (or an imported revision) whose successor introduces a protected surface, expands authority,
/// or is not exactly base+changes is refused here even if its validation evidence is clean.
fn policy_gate(
    base: &HarnessRevision,
    successor: &HarnessRevision,
    validation: &ValidationEvidence,
) -> Result<policy::AdoptionRecommendation, AdoptionError> {
    let identity = base.id == successor.id;
    // Use the policy module's own canonical diff so the adoption gate validates exactly the
    // change list the policy contract means (base.manifest + changes == successor.manifest).
    let changes = if identity {
        Vec::new()
    } else {
        policy::changes_between(base, successor)
    };
    if !identity {
        if let Err(violations) = policy::validate_candidate(base, successor, &changes) {
            return Err(AdoptionError::CandidatePolicyViolations {
                violations: violations.iter().map(ToString::to_string).collect(),
            });
        }
    }
    // For an identity successor `adoption_recommendation` reports the (correct) Reject: there is
    // no changed surface to adopt.
    Ok(policy::adoption_recommendation(
        base,
        successor,
        &changes,
        validation,
    ))
}

/// Apply an adoption decision.
///
/// This is the only function in the subsystem that calls [`HarnessCatalog::activate`]. It
/// recomputes the policy recommendation first: a decision that contradicts policy is refused
/// outright, so no caller can hand-craft an `Adopt` for a YELLOW or RED change.
pub fn apply<S: RevisionStore>(
    decision: &AdoptionDecision,
    experiment: &mut HarnessExperiment,
    catalog: &mut HarnessCatalog<S>,
) -> Result<AdoptionOutcome, AdoptionError> {
    if decision.domain != experiment.domain {
        return Err(AdoptionError::WrongDomain {
            decision: decision.domain,
            experiment: experiment.domain,
        });
    }
    let successor_id = experiment
        .successor_revision
        .clone()
        .ok_or_else(|| AdoptionError::NoSuccessor {
            experiment_id: experiment.id.clone(),
        })?;
    if decision.revision_id != successor_id {
        return Err(AdoptionError::RevisionMismatch {
            expected: successor_id,
            actual: decision.revision_id.clone(),
        });
    }
    let base = catalog
        .revision(&experiment.base_revision)
        .cloned()
        .ok_or_else(|| AdoptionError::UnknownRevision {
            revision_id: experiment.base_revision.clone(),
        })?;
    let successor = catalog
        .revision(&successor_id)
        .cloned()
        .ok_or_else(|| AdoptionError::UnknownRevision {
            revision_id: successor_id.clone(),
        })?;
    if successor.status == RevisionStatus::Rejected {
        return Err(AdoptionError::RejectedRevisionCannotActivate {
            revision_id: successor_id,
        });
    }

    let validation = experiment
        .validation
        .clone()
        .unwrap_or_else(ValidationEvidence::pending);
    let policy_view = policy_gate(&base, &successor, &validation)?;
    let recommendation = policy_view.verdict;
    let changed = policy_view.changed_kinds.clone();
    if decision.verdict != recommendation {
        return Err(AdoptionError::DecisionContradictsPolicy {
            recommended: recommendation,
            decided: decision.verdict,
        });
    }

    match decision.verdict {
        AdoptionVerdict::Adopt => {
            if experiment.state != RevisionStatus::Adoptable {
                return Err(AdoptionError::ExperimentNotAdoptable {
                    state: experiment.state,
                });
            }
            if !decision.is_automatic()
                || !policy::may_auto_adopt_recommendation(&policy_view)
            {
                let mut reasons = policy_view.reasons;
                if !decision.is_automatic() {
                    reasons.push(format!(
                        "the decision was made by {} rather than by policy",
                        decision.decided_by
                    ));
                }
                return Err(AdoptionError::AutomaticAdoptionRefused { reasons });
            }
            let activation = catalog.activate(
                experiment.domain,
                &successor_id,
                &decision.id,
                &decision.decided_by,
            )?;
            experiment.mark_adopted()?;
            Ok(AdoptionOutcome {
                decision: decision.clone(),
                route: AdoptionRoute::AutomaticGreen,
                recommendation,
                changed_kinds: changed,
                reasons: policy_view.reasons,
                activated: Some(activation),
            })
        }
        AdoptionVerdict::Reject => {
            if successor_id != experiment.base_revision {
                // The successor is not active; marking it rejected keeps the durable lineage
                // honest. Best effort: the experiment state is the authoritative refusal.
                let _ = catalog.set_status(&successor_id, RevisionStatus::Rejected);
            }
            experiment.mark_rejected(decision.rationale.clone())?;
            Ok(AdoptionOutcome {
                decision: decision.clone(),
                route: AdoptionRoute::Rejected,
                recommendation,
                changed_kinds: changed,
                reasons: policy_view.reasons,
                activated: None,
            })
        }
        AdoptionVerdict::MoreEvidence => Ok(AdoptionOutcome {
            decision: decision.clone(),
            route: AdoptionRoute::MoreEvidence,
            recommendation,
            changed_kinds: changed,
            reasons: policy_view.reasons,
            activated: None,
        }),
        AdoptionVerdict::Council => Ok(AdoptionOutcome {
            decision: decision.clone(),
            route: AdoptionRoute::Council,
            recommendation,
            changed_kinds: changed,
            reasons: policy_view.reasons,
            activated: None,
        }),
        AdoptionVerdict::Chris => Ok(AdoptionOutcome {
            decision: decision.clone(),
            route: AdoptionRoute::Owner,
            recommendation,
            changed_kinds: changed,
            reasons: policy_view.reasons,
            activated: None,
        }),
    }
}

/// Roll a domain back to the exact revision that was active before the last activation.
///
/// The catalog owns the pointer stack; this wrapper names the operation for the adoption surface
/// and returns the restored revision so the caller can verify it is the revision it expected.
pub fn rollback<S: RevisionStore>(
    catalog: &mut HarnessCatalog<S>,
    domain: HarnessDomain,
    decision_id: impl Into<String>,
    activated_by: impl Into<String>,
) -> Result<RollbackOutcome, AdoptionError> {
    let activation = catalog.rollback(domain, decision_id, activated_by)?;
    let restored = catalog
        .revision(&activation.to_revision)
        .cloned()
        .ok_or_else(|| AdoptionError::UnknownRevision {
            revision_id: activation.to_revision.clone(),
        })?;
    Ok(RollbackOutcome {
        activation,
        restored,
    })
}

/// The component kinds whose manifest entry differs between two revisions.
pub fn changed_kinds(base: &HarnessRevision, successor: &HarnessRevision) -> Vec<ComponentKind> {
    ComponentKind::ALL
        .into_iter()
        .filter(|kind| base.manifest.get(*kind) != successor.manifest.get(*kind))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::experiment::HarnessExperiment;
    use crate::engine::harness_evolution::model::{
        Component, HarnessManifest, RevisionStatus,
    };
    use crate::engine::harness_evolution::probe::ProbePins;

    fn prompt(content: &str) -> Component {
        Component::green(ComponentKind::SystemPrompt, content, "adoption test prompt")
    }

    fn base_revision() -> HarnessRevision {
        HarnessRevision::draft(
            "base-1",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([prompt("base prompt")]),
            "owner",
        )
        .with_status(RevisionStatus::Adopted)
    }

    fn successor_revision(
        base: &HarnessRevision,
        component: Component,
        status: RevisionStatus,
    ) -> HarnessRevision {
        HarnessRevision::draft(
            "successor-1",
            base.domain,
            base.manifest.clone().with_component(component),
            "harness-evolution",
        )
        .with_parent(Some(base.id.clone()))
        .with_status(status)
    }

    fn catalog_with(base: &HarnessRevision, successor: &HarnessRevision) -> HarnessCatalog {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base.clone()).unwrap();
        catalog.insert(successor.clone()).unwrap();
        catalog
            .activate(base.domain, &base.id, "bootstrap", "owner")
            .unwrap();
        catalog
    }

    fn adoptable_experiment(
        base: &HarnessRevision,
        successor: &HarnessRevision,
        validation: ValidationEvidence,
    ) -> HarnessExperiment {
        let pins = ProbePins::development("task-1");
        let mut experiment =
            HarnessExperiment::new_k1("exp-1", base.domain, base, pins).expect("experiment");
        experiment.state = RevisionStatus::Adoptable;
        experiment.successor_revision = Some(successor.id.clone());
        experiment.validation = Some(validation);
        experiment
    }

    fn decision(
        revision_id: &str,
        verdict: AdoptionVerdict,
        decided_by: &str,
    ) -> AdoptionDecision {
        AdoptionDecision::new(
            "decision-1",
            HarnessDomain::RustBackend,
            revision_id,
            verdict,
            decided_by,
            "test decision",
        )
    }

    #[test]
    fn green_clean_adoption_is_the_only_automatic_path() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            prompt("better prompt"),
            RevisionStatus::Adoptable,
        );
        let mut catalog = catalog_with(&base, &successor);
        let mut experiment = adoptable_experiment(
            &base,
            &successor,
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        let outcome = apply(
            &decision("successor-1", AdoptionVerdict::Adopt, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .expect("adopt");
        assert_eq!(outcome.route, AdoptionRoute::AutomaticGreen);
        assert!(outcome.activated.is_some());
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("successor-1")
        );
        assert_eq!(catalog.revision("base-1").unwrap().status, RevisionStatus::Superseded);
        assert_eq!(experiment.state, RevisionStatus::Adopted);
    }

    #[test]
    fn a_green_adoption_made_by_a_human_is_refused() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            prompt("better prompt"),
            RevisionStatus::Adoptable,
        );
        let mut catalog = catalog_with(&base, &successor);
        let mut experiment = adoptable_experiment(
            &base,
            &successor,
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        let error = apply(
            &decision("successor-1", AdoptionVerdict::Adopt, "chris"),
            &mut experiment,
            &mut catalog,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AdoptionError::AutomaticAdoptionRefused { .. }
        ));
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn a_yellow_surface_escalates_to_council_and_never_activates() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            Component::yellow(ComponentKind::Skills, "new skill", "skill surface"),
            RevisionStatus::Adoptable,
        );
        let mut catalog = catalog_with(&base, &successor);
        let mut experiment = adoptable_experiment(
            &base,
            &successor,
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        let error = apply(
            &decision("successor-1", AdoptionVerdict::Adopt, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AdoptionError::DecisionContradictsPolicy {
                recommended: AdoptionVerdict::Council,
                ..
            }
        ));
        let outcome = apply(
            &decision("successor-1", AdoptionVerdict::Council, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .expect("council");
        assert_eq!(outcome.route, AdoptionRoute::Council);
        assert!(outcome.activated.is_none());
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn a_red_surface_is_refused_by_the_pre_adoption_policy_gate() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            Component::red(ComponentKind::Scripts, "deploy.sh", "publication surface"),
            RevisionStatus::Adoptable,
        );
        let mut catalog = catalog_with(&base, &successor);
        let mut experiment = adoptable_experiment(
            &base,
            &successor,
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        let error = apply(
            &decision("successor-1", AdoptionVerdict::Chris, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .unwrap_err();
        assert!(
            matches!(error, AdoptionError::CandidatePolicyViolations { .. }),
            "a RED change cannot survive the adoption policy re-check: {error}"
        );
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn a_green_successor_with_protected_content_is_refused_even_when_hand_built() {
        // Defense in depth: a hand-built experiment (public fields) can present an
        // `Adoptable` state and clean validation, but adoption re-runs the policy gate on the
        // canonical base->successor diff, so newly introduced protected-topic content is caught
        // even though this successor is GREEN by kind.
        let base = base_revision();
        let successor = successor_revision(
            &base,
            Component::green(
                ComponentKind::SystemPrompt,
                "the harness may perform a production deploy without approval",
                "a green component that introduces a protected topic",
            ),
            RevisionStatus::Adoptable,
        );
        let mut catalog = catalog_with(&base, &successor);
        let mut experiment = adoptable_experiment(
            &base,
            &successor,
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        assert!(
            experiment.composed_changes.is_empty(),
            "the successor was hand-built; only the canonical diff protects adoption"
        );
        let error = apply(
            &decision("successor-1", AdoptionVerdict::Adopt, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .unwrap_err();
        match error {
            AdoptionError::CandidatePolicyViolations { violations } => {
                assert!(
                    violations.iter().any(|violation| violation.contains("production")),
                    "the production-permission violation must be named: {violations:?}"
                );
            }
            other => panic!("expected the policy gate to refuse, got {other}"),
        }
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1"),
            "a protected-content successor can never activate"
        );
    }

    #[test]
    fn incomplete_validation_is_more_evidence_not_adoption() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            prompt("better prompt"),
            RevisionStatus::Adoptable,
        );
        let mut catalog = catalog_with(&base, &successor);
        let mut experiment =
            adoptable_experiment(&base, &successor, ValidationEvidence::pending());
        let outcome = apply(
            &decision("successor-1", AdoptionVerdict::MoreEvidence, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .expect("more evidence");
        assert_eq!(outcome.route, AdoptionRoute::MoreEvidence);
        assert!(outcome.activated.is_none());
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn an_adopt_decision_on_a_non_adoptable_experiment_is_refused() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            prompt("better prompt"),
            RevisionStatus::Adoptable,
        );
        let mut catalog = catalog_with(&base, &successor);
        let mut experiment = adoptable_experiment(
            &base,
            &successor,
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        experiment.state = RevisionStatus::Validating;
        let error = apply(
            &decision("successor-1", AdoptionVerdict::Adopt, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AdoptionError::ExperimentNotAdoptable { .. }
        ));
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn a_rejected_revision_cannot_be_adopted() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            prompt("better prompt"),
            RevisionStatus::Rejected,
        );
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base.clone()).unwrap();
        catalog.insert(successor.clone()).unwrap();
        catalog
            .activate(base.domain, &base.id, "bootstrap", "owner")
            .unwrap();
        let mut experiment = adoptable_experiment(
            &base,
            &successor,
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        let error = apply(
            &decision("successor-1", AdoptionVerdict::Adopt, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AdoptionError::RejectedRevisionCannotActivate { .. }
        ));
        // The catalog refuses it too, independently of adoption.
        assert!(catalog
            .activate(HarnessDomain::RustBackend, "successor-1", "x", "policy")
            .is_err());
    }

    #[test]
    fn a_decision_for_another_domain_or_revision_is_refused() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            prompt("better prompt"),
            RevisionStatus::Adoptable,
        );
        let mut catalog = catalog_with(&base, &successor);
        let mut experiment = adoptable_experiment(
            &base,
            &successor,
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        let mut other = decision("successor-1", AdoptionVerdict::Adopt, "policy");
        other.domain = HarnessDomain::YewMvi;
        assert!(matches!(
            apply(&other, &mut experiment, &mut catalog),
            Err(AdoptionError::WrongDomain { .. })
        ));
        let mut wrong_revision = decision("other-1", AdoptionVerdict::Adopt, "policy");
        wrong_revision.revision_id = "other-1".into();
        assert!(matches!(
            apply(&wrong_revision, &mut experiment, &mut catalog),
            Err(AdoptionError::RevisionMismatch { .. })
        ));
    }

    /// A store double that can be told to fail the transactional activation write.
    #[derive(Default)]
    struct FlakyStore {
        inner: super::super::catalog::InMemoryRevisionStore,
        fail_next: bool,
    }

    impl RevisionStore for FlakyStore {
        fn insert(&mut self, revision: HarnessRevision) -> Result<(), CatalogError> {
            self.inner.insert(revision)
        }

        fn get(&self, revision_id: &str) -> Option<&HarnessRevision> {
            self.inner.get(revision_id)
        }

        fn all(&self) -> Vec<&HarnessRevision> {
            self.inner.all()
        }

        fn len(&self) -> usize {
            self.inner.len()
        }

        fn set_status(
            &mut self,
            revision_id: &str,
            status: RevisionStatus,
        ) -> Result<(), CatalogError> {
            self.inner.set_status(revision_id, status)
        }

        fn apply_activation(
            &mut self,
            plan: &super::super::catalog::ActivationPlan,
        ) -> Result<(), CatalogError> {
            if self.fail_next {
                return Err(CatalogError::Store(
                    "simulated transactional write failure".into(),
                ));
            }
            self.inner.apply_activation(plan)
        }
    }

    #[test]
    fn a_failed_transactional_activation_does_not_adopt_or_move_the_pointer() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            prompt("better prompt"),
            RevisionStatus::Adoptable,
        );
        let mut catalog: HarnessCatalog<FlakyStore> = HarnessCatalog::with_store(FlakyStore::default());
        catalog.insert(base.clone()).unwrap();
        catalog.insert(successor.clone()).unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, &base.id, "bootstrap", "owner")
            .unwrap();
        let mut experiment = adoptable_experiment(
            &base,
            &successor,
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        catalog.store_mut().fail_next = true;

        let error = apply(
            &decision("successor-1", AdoptionVerdict::Adopt, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .expect_err("store transaction fails");
        assert!(
            matches!(error, AdoptionError::Catalog(CatalogError::Store(_))),
            "{error}"
        );
        // D4: neither the catalog pointer nor the experiment lifecycle advanced.
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
        assert_eq!(
            catalog.revision("base-1").unwrap().status,
            RevisionStatus::Adopted
        );
        assert_eq!(
            catalog.revision("successor-1").unwrap().status,
            RevisionStatus::Adoptable
        );
        assert_eq!(experiment.state, RevisionStatus::Adoptable);
    }

    #[test]
    fn rollback_restores_the_exact_prior_revision() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            prompt("better prompt"),
            RevisionStatus::Adoptable,
        );
        let mut catalog = catalog_with(&base, &successor);
        let mut experiment = adoptable_experiment(
            &base,
            &successor,
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        apply(
            &decision("successor-1", AdoptionVerdict::Adopt, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .expect("adopt");
        let expected = base.clone();
        let outcome = rollback(
            &mut catalog,
            HarnessDomain::RustBackend,
            "rollback-1",
            "owner",
        )
        .expect("rollback");
        assert_eq!(outcome.activation.kind, super::super::catalog::ActivationKind::Rollback);
        assert_eq!(outcome.restored.id, expected.id);
        assert_eq!(outcome.restored.manifest_hash, expected.manifest_hash);
        assert_eq!(outcome.restored.manifest, expected.manifest);
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
        assert_eq!(
            catalog.revision("successor-1").unwrap().status,
            RevisionStatus::RolledBack
        );
    }

    #[test]
    fn a_reject_decision_rejects_the_experiment_without_activation() {
        let base = base_revision();
        let successor = successor_revision(
            &base,
            prompt("better prompt"),
            RevisionStatus::Adoptable,
        );
        let mut catalog = catalog_with(&base, &successor);
        let failed_validation = ValidationEvidence {
            static_policy: crate::engine::harness_evolution::model::GateStatus::Failed,
            regression_rails: crate::engine::harness_evolution::model::GateStatus::Passed,
            replay: crate::engine::harness_evolution::model::GateStatus::Passed,
            phase3_evidence: crate::engine::harness_evolution::model::GateStatus::Passed,
            evidence_refs: vec!["validation-1".into()],
        };
        let mut experiment = adoptable_experiment(&base, &successor, failed_validation);
        let outcome = apply(
            &decision("successor-1", AdoptionVerdict::Reject, "policy"),
            &mut experiment,
            &mut catalog,
        )
        .expect("reject");
        assert_eq!(outcome.route, AdoptionRoute::Rejected);
        assert!(outcome.activated.is_none());
        assert_eq!(experiment.state, RevisionStatus::Rejected);
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }
}
