//! Compose — turn behavioral contrasts into a successor harness.
//!
//! Harness² composes a refined harness from the *evidence* of the probed candidates, not from the
//! candidates' own claims. For every component kind the proposers touched this module makes one
//! decision:
//!
//! * **Keep** the base component — no captured behavioral improvement justifies changing it;
//! * **Adopt** a candidate change — its contrast improved and none regressed;
//! * **Merge** several candidates — every candidate on that kind improved and their guidance can
//!   be combined without conflict;
//! * **Reject** the candidate change — a regression (which outranks any improvement) or a
//!   conflicting edit means the base stays.
//!
//! Every changing decision is evidence-linked: `evidence_for` cites the contrast evidence that
//! justified it, `evidence_against` cites the regressions that blocked it. Confidence is derived
//! from that evidence and is capped below 100 — a behavioral contrast can justify a composition
//! but never certifies a promotion; Forge validation (`super::validate`) and the adoption policy
//! own that decision.
//!
//! The successor is a **new immutable revision** built from the base manifest. The base revision
//! and its parent chain are never overwritten, and choosing the unchanged base (all `Keep`) is a
//! legal composition. The returned [`ComposedHarness::changes`] is the exact change list that
//! turns the base manifest into the successor manifest, so the policy can re-prove
//! `successor = base + changes` before any probe runs.

use super::contrast::{self, ContrastVerdict};
use super::model::{
    Candidate, ChangeAction, Component, ComponentChange, ComponentDecision,
    ComponentDecisionRecord, ComponentKind, CompositionDecision, ContrastReport, EvolutionError,
    HarnessRevision, RevisionStatus,
};

/// Confidence ceiling for a composition decision. A behavioral contrast is evidence, not proof, so
/// no composition decision is ever reported as certain.
pub const COMPOSITION_CONFIDENCE_CEILING: u8 = 95;

/// One candidate together with the contrast that measured it. The pairing is explicit so a
/// composition can never cite a contrast that does not belong to the candidate it adopts.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateEvidence<'a> {
    pub candidate: &'a Candidate,
    pub contrast: &'a ContrastReport,
}

/// One composition request.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposeRequest<'a> {
    pub id: String,
    pub successor_id: String,
    pub base: &'a HarnessRevision,
    pub evidence: Vec<CandidateEvidence<'a>>,
    pub created_by: String,
}

/// A composed successor: the decision record, the new immutable revision, and the exact changes
/// that reconstruct the successor from the base manifest.
///
/// `changes` is the canonical list this composer produced, rationales included. Callers that need
/// a change list for validation/audit must carry THIS list (the experiment seam adapts it through
/// `experiment::ComposedSuccessor::from`), never re-derive one from the manifest pair — a
/// re-derived diff synthesizes rationales and loses the evidence-linked ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedHarness {
    pub decision: CompositionDecision,
    pub successor: HarnessRevision,
    pub changes: Vec<ComponentChange>,
}

/// Compose the evidence for one step into a successor revision.
pub fn compose(request: &ComposeRequest<'_>) -> Result<ComposedHarness, EvolutionError> {
    if request.id.trim().is_empty() || request.successor_id.trim().is_empty() {
        return Err(EvolutionError::InvalidComposition(
            "composition must name itself and its successor".into(),
        ));
    }
    request.base.verify_integrity()?;
    if request.evidence.is_empty() {
        return Err(EvolutionError::InvalidComposition(
            "composition has no candidate evidence".into(),
        ));
    }

    let mut candidate_ids: Vec<String> = Vec::new();
    let mut contrast_ids: Vec<String> = Vec::new();
    let mut all_links: Vec<String> = Vec::new();
    for entry in &request.evidence {
        entry.candidate.validate()?;
        if entry.candidate.base_revision != request.base.id {
            return Err(EvolutionError::InvalidComposition(format!(
                "candidate {} targets {} instead of the composition base {}",
                entry.candidate.id, entry.candidate.base_revision, request.base.id
            )));
        }
        if candidate_ids.contains(&entry.candidate.id) {
            return Err(EvolutionError::InvalidComposition(format!(
                "candidate {} appears more than once",
                entry.candidate.id
            )));
        }
        candidate_ids.push(entry.candidate.id.clone());

        entry.contrast.validate()?;
        if entry.contrast.base_revision != request.base.id {
            return Err(EvolutionError::InvalidComposition(format!(
                "contrast {} measured {} instead of the composition base {}",
                entry.contrast.id, entry.contrast.base_revision, request.base.id
            )));
        }
        if entry.contrast.candidate_revision == request.base.id {
            return Err(EvolutionError::InvalidComposition(format!(
                "contrast {} compares the base revision with itself",
                entry.contrast.id
            )));
        }
        if !contrast_ids.contains(&entry.contrast.id) {
            contrast_ids.push(entry.contrast.id.clone());
            all_links.extend(contrast_links(entry.contrast));
        }

        for change in &entry.candidate.changes {
            if change.kind.minimum_authority().is_red() {
                return Err(EvolutionError::InvalidComposition(format!(
                    "{} is a protected RED surface and cannot be composed by the optimizer",
                    change.kind
                )));
            }
        }
    }

    let mut manifest = request.base.manifest.clone();
    let mut changes: Vec<ComponentChange> = Vec::new();
    let mut decisions: Vec<ComponentDecisionRecord> = Vec::new();
    let mut adopted = 0usize;
    let mut merged = 0usize;
    let mut kept = 0usize;
    let mut rejected = 0usize;

    for kind in ComponentKind::ALL {
        // Collect the proposals for this kind in request order (deterministic).
        let mut proposals: Vec<KindProposal<'_>> = Vec::new();
        let mut duplicate = false;
        for entry in &request.evidence {
            for change in &entry.candidate.changes {
                if change.kind == kind {
                    if proposals
                        .iter()
                        .any(|proposal| std::ptr::eq(proposal.candidate, entry.candidate))
                    {
                        duplicate = true;
                    }
                    proposals.push(KindProposal {
                        candidate: entry.candidate,
                        change,
                        contrast: entry.contrast,
                    });
                }
            }
        }
        if duplicate {
            return Err(EvolutionError::InvalidComposition(format!(
                "a candidate changes {kind} more than once"
            )));
        }
        if proposals.is_empty() {
            continue;
        }

        let outcome = decide(request.base, kind, &proposals)?;
        let confidence = confidence_for(outcome.decision, &proposals);
        let record =
            ComponentDecisionRecord::new(kind, outcome.decision, outcome.rationale.clone())
                .with_evidence_for(outcome.evidence_for.clone())
                .with_evidence_against(outcome.evidence_against.clone())
                .with_confidence(confidence);

        match outcome.plan {
            KindPlan::None => {
                if outcome.decision == ComponentDecision::Adopt
                    || outcome.decision == ComponentDecision::Merge
                {
                    return Err(EvolutionError::InvalidComposition(format!(
                        "{} decision for {kind} carries no component change",
                        outcome.decision.id()
                    )));
                }
            }
            KindPlan::Set(component) => {
                manifest = manifest.with_component(component.clone());
                changes.push(ComponentChange::set(component, outcome.rationale.clone()));
            }
            KindPlan::Remove => {
                manifest = manifest.without_component(kind);
                changes.push(ComponentChange::remove(kind, outcome.rationale.clone()));
            }
        }

        match outcome.decision {
            ComponentDecision::Adopt => adopted += 1,
            ComponentDecision::Merge => merged += 1,
            ComponentDecision::Keep => kept += 1,
            ComponentDecision::Reject => rejected += 1,
        }
        decisions.push(record);
    }

    let successor = HarnessRevision::draft(
        &request.successor_id,
        request.base.domain,
        manifest,
        &request.created_by,
    )
    .with_parent(Some(request.base.id.clone()))
    .with_status(RevisionStatus::Composed);
    successor.verify_integrity()?;

    // The policy re-proves this equality before a probe runs; if composition could not reproduce
    // the successor from the base + change list, that check would be meaningless.
    let reconstructed = request.base.manifest.apply_changes(&changes)?;
    if reconstructed != successor.manifest {
        return Err(EvolutionError::InvalidComposition(
            "successor manifest does not equal the base manifest plus the composed changes".into(),
        ));
    }

    all_links.sort();
    all_links.dedup();

    let decision = CompositionDecision {
        id: request.id.clone(),
        base_revision: request.base.id.clone(),
        candidates: candidate_ids,
        contrast_ids,
        decisions,
        successor_manifest_hash: successor.manifest_hash.clone(),
        summary: format!(
            "composed {} component(s): {adopted} adopted, {merged} merged, {kept} kept, {rejected} rejected",
            adopted + merged + kept + rejected
        ),
        evidence_refs: all_links,
    };
    decision.validate()?;

    Ok(ComposedHarness {
        decision,
        successor,
        changes,
    })
}

struct KindProposal<'a> {
    candidate: &'a Candidate,
    change: &'a ComponentChange,
    contrast: &'a ContrastReport,
}

enum KindPlan {
    None,
    Set(Component),
    Remove,
}

struct KindOutcome {
    decision: ComponentDecision,
    plan: KindPlan,
    evidence_for: Vec<String>,
    evidence_against: Vec<String>,
    rationale: String,
}

/// Decide one component kind from its candidate contrasts.
///
/// A regression outranks an improvement — a candidate that made anything worse is not adopted
/// even if it also made something better. That is the conservative direction Harness² needs:
/// without ground truth, "mostly better" is not a licence to ship a regression.
fn decide(
    base: &HarnessRevision,
    kind: ComponentKind,
    proposals: &[KindProposal<'_>],
) -> Result<KindOutcome, EvolutionError> {
    let verdicts: Vec<ContrastVerdict> = proposals
        .iter()
        .map(|proposal| contrast::verdict(proposal.contrast))
        .collect();
    let regressed: Vec<usize> = verdicts
        .iter()
        .enumerate()
        .filter(|(_, verdict)| **verdict == ContrastVerdict::Regressed)
        .map(|(index, _)| index)
        .collect();
    let improved: Vec<usize> = verdicts
        .iter()
        .enumerate()
        .filter(|(_, verdict)| **verdict == ContrastVerdict::Improved)
        .map(|(index, _)| index)
        .collect();

    let improved_links: Vec<String> = improved
        .iter()
        .flat_map(|index| contrast_links(proposals[*index].contrast))
        .collect();
    let regressed_links: Vec<String> = regressed
        .iter()
        .flat_map(|index| contrast_links(proposals[*index].contrast))
        .collect();

    if !regressed.is_empty() {
        return Ok(KindOutcome {
            decision: ComponentDecision::Reject,
            plan: KindPlan::None,
            evidence_for: improved_links,
            evidence_against: regressed_links,
            rationale: format!(
                "reject the {kind} change: {} of {} candidate contrast(s) regressed, so the base component stays",
                regressed.len(),
                proposals.len()
            ),
        });
    }

    if improved.is_empty() {
        let links: Vec<String> = proposals
            .iter()
            .flat_map(|proposal| contrast_links(proposal.contrast))
            .collect();
        return Ok(KindOutcome {
            decision: ComponentDecision::Keep,
            plan: KindPlan::None,
            evidence_for: links,
            evidence_against: Vec::new(),
            rationale: format!(
                "keep the base {kind}: no captured behavioral improvement justified a change"
            ),
        });
    }

    if improved.len() == 1 {
        let proposal = &proposals[improved[0]];
        let plan = match proposal.change.action {
            ChangeAction::Set => {
                let component = proposal.change.component.clone().ok_or_else(|| {
                    EvolutionError::InvalidComposition(format!(
                        "SET {} carries no component",
                        proposal.change.kind
                    ))
                })?;
                KindPlan::Set(component)
            }
            ChangeAction::Remove => KindPlan::Remove,
        };
        return Ok(KindOutcome {
            decision: ComponentDecision::Adopt,
            plan,
            evidence_for: improved_links,
            evidence_against: Vec::new(),
            rationale: format!(
                "adopt the {kind} change from candidate {}: its contrast improved with no regression",
                proposal.candidate.id
            ),
        });
    }

    // More than one improving candidate on the same kind. Removals are idempotent; a set/remove
    // conflict is refused rather than guessed.
    let all_remove = improved
        .iter()
        .all(|index| proposals[*index].change.action == ChangeAction::Remove);
    if all_remove {
        return Ok(KindOutcome {
            decision: ComponentDecision::Adopt,
            plan: KindPlan::Remove,
            evidence_for: improved_links,
            evidence_against: Vec::new(),
            rationale: format!(
                "adopt removal of {kind}: {} improving candidates agree and removing is idempotent",
                improved.len()
            ),
        });
    }
    let all_set = improved
        .iter()
        .all(|index| proposals[*index].change.action == ChangeAction::Set);
    if !all_set {
        return Ok(KindOutcome {
            decision: ComponentDecision::Reject,
            plan: KindPlan::None,
            evidence_for: improved_links,
            evidence_against: Vec::new(),
            rationale: format!(
                "reject the {kind} change: the improving candidates disagree on whether to set or remove it"
            ),
        });
    }

    match merge_components(base, kind, proposals, &improved)? {
        Some(component) => Ok(KindOutcome {
            decision: ComponentDecision::Merge,
            plan: KindPlan::Set(component),
            evidence_for: improved_links,
            evidence_against: Vec::new(),
            rationale: format!(
                "merge {} improving candidates on {kind}: every contrast improved and the guidance combines without conflict",
                improved.len()
            ),
        }),
        None => Ok(KindOutcome {
            decision: ComponentDecision::Keep,
            plan: KindPlan::None,
            evidence_for: improved_links,
            evidence_against: Vec::new(),
            rationale: format!(
                "keep the base {kind}: the improving candidates proposed no content change to combine"
            ),
        }),
    }
}

/// Combine the improving candidates' guidance onto the base component.
///
/// Candidate components are full replacements derived from the base, so the merge appends only the
/// added suffix when the candidate content extends the base. The authority is never expanded: an
/// existing component keeps the base class, a new one gets the most protective proposed class (the
/// policy still rejects any expansion).
fn merge_components(
    base: &HarnessRevision,
    kind: ComponentKind,
    proposals: &[KindProposal<'_>],
    chosen: &[usize],
) -> Result<Option<Component>, EvolutionError> {
    let components: Vec<Component> = chosen
        .iter()
        .map(|index| {
            proposals[*index].change.component.clone().ok_or_else(|| {
                EvolutionError::InvalidComposition(format!(
                    "SET {} carries no component",
                    proposals[*index].change.kind
                ))
            })
        })
        .collect::<Result<_, _>>()?;

    let base_component = base.manifest.get(kind);
    let (content, authority, description) = match base_component {
        Some(base_component) => {
            let base_content = base_component.content.as_str();
            let mut additions: Vec<String> = Vec::new();
            for component in &components {
                if component.content == base_content {
                    continue;
                }
                let addition = component
                    .content
                    .strip_prefix(base_content)
                    .map(str::trim)
                    .filter(|rest| !rest.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| component.content.trim().to_string());
                if !addition.is_empty() && !additions.contains(&addition) {
                    additions.push(addition);
                }
            }
            let mut content = base_component.content.clone();
            if !additions.is_empty() {
                if !content.ends_with('\n') {
                    content.push('\n');
                }
                content.push('\n');
                content.push_str(&additions.join("\n\n"));
            }
            (
                content,
                base_component.authority,
                base_component.description.clone(),
            )
        }
        None => {
            let mut additions: Vec<String> = Vec::new();
            for component in &components {
                let content = component.content.trim();
                if !content.is_empty() && !additions.iter().any(|addition| addition == content) {
                    additions.push(content.to_string());
                }
            }
            let authority = components
                .iter()
                .map(|component| component.authority)
                .max()
                .unwrap_or_else(|| kind.minimum_authority());
            let description = components
                .first()
                .map(|component| component.description.clone())
                .unwrap_or_else(|| format!("{kind} merged guidance"));
            (additions.join("\n\n"), authority, description)
        }
    };

    if base_component.is_some_and(|base_component| base_component.content == content) {
        return Ok(None);
    }
    if content.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(Component::new(kind, authority, content, description)))
}

/// Confidence for a component decision, in percent. The only inputs are captured behavioral
/// evidence: improved contrasts raise it, regressions lower it, and no decision is ever 100.
fn confidence_for(decision: ComponentDecision, proposals: &[KindProposal<'_>]) -> u8 {
    let improved = proposals
        .iter()
        .filter(|proposal| contrast::verdict(proposal.contrast) == ContrastVerdict::Improved)
        .count();
    let regressed = proposals
        .iter()
        .filter(|proposal| contrast::verdict(proposal.contrast) == ContrastVerdict::Regressed)
        .count();
    if decision == ComponentDecision::Keep {
        return 0;
    }
    let improved = u8::try_from(improved).unwrap_or(u8::MAX);
    let regressed = u8::try_from(regressed).unwrap_or(u8::MAX);
    let raw = 50i16 + 15i16 * i16::from(improved) - 20i16 * i16::from(regressed);
    let clamped = raw.clamp(1, i16::from(COMPOSITION_CONFIDENCE_CEILING));
    clamped as u8
}

/// Durable evidence links for a contrast. A contrast with no direct refs still identifies itself,
/// so a decision can never be evidence-free.
fn contrast_links(contrast: &ContrastReport) -> Vec<String> {
    if contrast.evidence_refs.is_empty() {
        vec![format!("contrast:{}", contrast.id)]
    } else {
        contrast.evidence_refs.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::contrast::contrast;
    use crate::engine::harness_evolution::model::{
        BehaviorEvidence, EnvironmentClass, HarnessManifest, ProbeIdentity, QaVerdict,
    };

    fn base_revision() -> HarnessRevision {
        let manifest = HarnessManifest::empty()
            .with_component(Component::green(
                ComponentKind::SystemPrompt,
                "baseline prompt",
                "base prompt",
            ))
            .with_component(Component::green(
                ComponentKind::Memory,
                "baseline memory",
                "base memory",
            ));
        HarnessRevision::draft(
            "base-1",
            crate::engine::harness_evolution::model::HarnessDomain::RustBackend,
            manifest,
            "owner",
        )
    }

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

    fn evidence(id: &str, success: bool, steps: u64) -> BehaviorEvidence {
        let mut evidence = BehaviorEvidence::new(format!("evidence-{id}"), format!("probe-{id}"));
        evidence.success = success;
        evidence.tests_passed = 5;
        evidence.tests_failed = 0;
        evidence.qa = QaVerdict::Pass;
        evidence.steps = steps;
        evidence.evidence_refs = vec![format!("receipt:{id}")];
        evidence
    }

    fn improved_contrast(id: &str) -> ContrastReport {
        contrast(
            id,
            "base-1",
            "candidate-rev-1",
            &evidence("ref", true, 10),
            &evidence("cand", true, 5),
            &identity(),
            &identity(),
        )
        .unwrap()
    }

    fn regressed_contrast(id: &str) -> ContrastReport {
        let mut candidate = evidence("bad", true, 20);
        candidate.tool_failures = 3;
        contrast(
            id,
            "base-1",
            "candidate-rev-2",
            &evidence("ref2", true, 10),
            &candidate,
            &identity(),
            &identity(),
        )
        .unwrap()
    }

    fn neutral_contrast(id: &str) -> ContrastReport {
        let mut reference = evidence("ref3", true, 10);
        reference.evidence_id = "evidence-ref3".into();
        reference.probe_id = "probe-ref3".into();
        let mut candidate = evidence("cand3", true, 10);
        candidate.evidence_id = "evidence-cand3".into();
        candidate.probe_id = "probe-cand3".into();
        contrast(
            id,
            "base-1",
            "candidate-rev-3",
            &reference,
            &candidate,
            &identity(),
            &identity(),
        )
        .unwrap()
    }

    fn system_prompt_candidate(id: &str, addition: &str) -> Candidate {
        Candidate::new(
            id,
            "base-1",
            crate::engine::harness_evolution::model::ProposalKind::Invariant,
            vec![ComponentChange::set(
                Component::green(
                    ComponentKind::SystemPrompt,
                    format!("baseline prompt\n\n{addition}"),
                    "base prompt",
                ),
                "clearer prompt",
            )],
        )
    }

    fn memory_candidate(id: &str, addition: &str) -> Candidate {
        Candidate::new(
            id,
            "base-1",
            crate::engine::harness_evolution::model::ProposalKind::Domain,
            vec![ComponentChange::set(
                Component::green(
                    ComponentKind::Memory,
                    format!("baseline memory\n\n{addition}"),
                    "base memory",
                ),
                "domain knowledge",
            )],
        )
    }

    #[test]
    fn adopted_candidate_produces_a_new_child_successor() {
        let base = base_revision();
        let candidate = system_prompt_candidate("cand-1", "ADDED GUIDANCE");
        let report = improved_contrast("contrast-1");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![CandidateEvidence {
                candidate: &candidate,
                contrast: &report,
            }],
            created_by: "smith".into(),
        };
        let composed = compose(&request).unwrap();
        assert_eq!(composed.decision.decisions.len(), 1);
        assert_eq!(
            composed.decision.decisions[0].decision,
            ComponentDecision::Adopt
        );
        assert_eq!(composed.successor.parent_revision_id, Some("base-1".into()));
        assert_eq!(composed.successor.status, RevisionStatus::Composed);
        assert_ne!(composed.successor.manifest_hash, base.manifest_hash);
        assert!(composed
            .successor
            .manifest
            .get(ComponentKind::SystemPrompt)
            .unwrap()
            .content
            .contains("ADDED GUIDANCE"));
        assert_eq!(composed.changes.len(), 1);
        assert_eq!(composed.changes[0].kind, ComponentKind::SystemPrompt);
        composed.decision.validate().unwrap();
    }

    #[test]
    fn composed_changes_reconstruct_the_successor_from_the_base() {
        let base = base_revision();
        let invariant = system_prompt_candidate("cand-1", "ADDED GUIDANCE");
        let domain = memory_candidate("cand-2", "DOMAIN FACT");
        let first = improved_contrast("contrast-1");
        let second = improved_contrast("contrast-2");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![
                CandidateEvidence {
                    candidate: &invariant,
                    contrast: &first,
                },
                CandidateEvidence {
                    candidate: &domain,
                    contrast: &second,
                },
            ],
            created_by: "smith".into(),
        };
        let composed = compose(&request).unwrap();
        let reconstructed = base.manifest.apply_changes(&composed.changes).unwrap();
        assert_eq!(reconstructed, composed.successor.manifest);
        assert_eq!(
            reconstructed.manifest_hash(),
            composed.decision.successor_manifest_hash
        );
    }

    #[test]
    fn a_regressed_contrast_rejects_the_change_and_keeps_base() {
        let base = base_revision();
        let candidate = system_prompt_candidate("cand-1", "RISKY GUIDANCE");
        let report = regressed_contrast("contrast-1");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![CandidateEvidence {
                candidate: &candidate,
                contrast: &report,
            }],
            created_by: "smith".into(),
        };
        let composed = compose(&request).unwrap();
        assert_eq!(
            composed.decision.decisions[0].decision,
            ComponentDecision::Reject
        );
        assert!(!composed.decision.decisions[0].evidence_against.is_empty());
        assert!(composed.changes.is_empty());
        assert_eq!(composed.successor.manifest_hash, base.manifest_hash);
    }

    #[test]
    fn choosing_the_unchanged_base_is_a_legal_composition() {
        let base = base_revision();
        let candidate = system_prompt_candidate("cand-1", "NO-OP GUIDANCE");
        let report = neutral_contrast("contrast-1");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![CandidateEvidence {
                candidate: &candidate,
                contrast: &report,
            }],
            created_by: "smith".into(),
        };
        let composed = compose(&request).unwrap();
        assert_eq!(
            composed.decision.decisions[0].decision,
            ComponentDecision::Keep
        );
        assert!(composed.changes.is_empty());
        assert_eq!(composed.successor.manifest_hash, base.manifest_hash);
        composed.decision.validate().unwrap();
    }

    #[test]
    fn two_improving_candidates_on_one_kind_merge() {
        let base = base_revision();
        let first = system_prompt_candidate("cand-1", "FIRST GUIDANCE");
        let second = system_prompt_candidate("cand-2", "SECOND GUIDANCE");
        let first_report = improved_contrast("contrast-1");
        let second_report = improved_contrast("contrast-2");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![
                CandidateEvidence {
                    candidate: &first,
                    contrast: &first_report,
                },
                CandidateEvidence {
                    candidate: &second,
                    contrast: &second_report,
                },
            ],
            created_by: "smith".into(),
        };
        let composed = compose(&request).unwrap();
        assert_eq!(composed.decision.decisions.len(), 1);
        assert_eq!(
            composed.decision.decisions[0].decision,
            ComponentDecision::Merge
        );
        let content = &composed
            .successor
            .manifest
            .get(ComponentKind::SystemPrompt)
            .unwrap()
            .content;
        assert!(content.contains("FIRST GUIDANCE"));
        assert!(content.contains("SECOND GUIDANCE"));
        // The base text appears once; the merge appended suffixes rather than duplicating it.
        assert_eq!(content.matches("baseline prompt").count(), 1);
    }

    #[test]
    fn one_regressed_candidate_rejects_only_its_kind() {
        let base = base_revision();
        let prompt = system_prompt_candidate("cand-1", "GOOD GUIDANCE");
        let memory = memory_candidate("cand-2", "BAD MEMORY");
        let good = improved_contrast("contrast-1");
        let bad = regressed_contrast("contrast-2");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![
                CandidateEvidence {
                    candidate: &prompt,
                    contrast: &good,
                },
                CandidateEvidence {
                    candidate: &memory,
                    contrast: &bad,
                },
            ],
            created_by: "smith".into(),
        };
        let composed = compose(&request).unwrap();
        let by_kind: Vec<(ComponentKind, ComponentDecision)> = composed
            .decision
            .decisions
            .iter()
            .map(|record| (record.kind, record.decision))
            .collect();
        assert!(by_kind.contains(&(ComponentKind::SystemPrompt, ComponentDecision::Adopt)));
        assert!(by_kind.contains(&(ComponentKind::Memory, ComponentDecision::Reject)));
        assert!(!composed
            .successor
            .manifest
            .get(ComponentKind::Memory)
            .unwrap()
            .content
            .contains("BAD MEMORY"));
    }

    #[test]
    fn candidate_targeting_another_base_is_refused() {
        let base = base_revision();
        let mut candidate = system_prompt_candidate("cand-1", "X");
        candidate.base_revision = "other-base".into();
        let report = improved_contrast("contrast-1");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![CandidateEvidence {
                candidate: &candidate,
                contrast: &report,
            }],
            created_by: "smith".into(),
        };
        assert_eq!(compose(&request).unwrap_err().code(), "invalid_composition");
    }

    #[test]
    fn contrast_measured_against_another_base_is_refused() {
        let base = base_revision();
        let candidate = system_prompt_candidate("cand-1", "X");
        let report = contrast(
            "contrast-1",
            "other-base",
            "candidate-rev-1",
            &evidence("ref", true, 10),
            &evidence("cand", true, 5),
            &identity(),
            &identity(),
        )
        .unwrap();
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![CandidateEvidence {
                candidate: &candidate,
                contrast: &report,
            }],
            created_by: "smith".into(),
        };
        assert_eq!(compose(&request).unwrap_err().code(), "invalid_composition");
    }

    #[test]
    fn a_red_surface_candidate_is_refused() {
        let base = base_revision();
        let candidate = Candidate::new(
            "cand-1",
            "base-1",
            crate::engine::harness_evolution::model::ProposalKind::Invariant,
            vec![ComponentChange::set(
                Component::red(ComponentKind::Plugins, "new plugin", "plugin"),
                "add a plugin",
            )],
        );
        let report = improved_contrast("contrast-1");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![CandidateEvidence {
                candidate: &candidate,
                contrast: &report,
            }],
            created_by: "smith".into(),
        };
        assert_eq!(compose(&request).unwrap_err().code(), "invalid_composition");
    }

    #[test]
    fn parent_revision_is_never_overwritten() {
        let base = base_revision();
        let original_hash = base.manifest_hash.clone();
        let original_prompt = base
            .manifest
            .get(ComponentKind::SystemPrompt)
            .unwrap()
            .content
            .clone();
        let candidate = system_prompt_candidate("cand-1", "ADDED");
        let report = improved_contrast("contrast-1");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![CandidateEvidence {
                candidate: &candidate,
                contrast: &report,
            }],
            created_by: "smith".into(),
        };
        let composed = compose(&request).unwrap();
        assert_eq!(base.manifest_hash, original_hash);
        assert_eq!(
            base.manifest
                .get(ComponentKind::SystemPrompt)
                .unwrap()
                .content,
            original_prompt
        );
        assert_eq!(composed.successor.parent_revision_id, Some(base.id.clone()));
        composed.successor.verify_integrity().unwrap();
    }

    #[test]
    fn every_changing_decision_is_evidence_linked() {
        let base = base_revision();
        let candidate = system_prompt_candidate("cand-1", "ADDED");
        let report = improved_contrast("contrast-1");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![CandidateEvidence {
                candidate: &candidate,
                contrast: &report,
            }],
            created_by: "smith".into(),
        };
        let composed = compose(&request).unwrap();
        for record in &composed.decision.decisions {
            if record.decision.requires_evidence() {
                assert!(
                    !record.evidence_for.is_empty() || !record.evidence_against.is_empty(),
                    "{} decision is not evidence-linked",
                    record.kind
                );
            }
            assert!(record.confidence <= COMPOSITION_CONFIDENCE_CEILING);
        }
        assert!(!composed.decision.evidence_refs.is_empty());
        composed.decision.validate().unwrap();
    }

    #[test]
    fn empty_evidence_and_duplicate_candidates_are_refused() {
        let base = base_revision();
        let empty = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![],
            created_by: "smith".into(),
        };
        assert_eq!(compose(&empty).unwrap_err().code(), "invalid_composition");

        let candidate = system_prompt_candidate("cand-1", "ADDED");
        let report = improved_contrast("contrast-1");
        let duplicate = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![
                CandidateEvidence {
                    candidate: &candidate,
                    contrast: &report,
                },
                CandidateEvidence {
                    candidate: &candidate,
                    contrast: &report,
                },
            ],
            created_by: "smith".into(),
        };
        assert_eq!(
            compose(&duplicate).unwrap_err().code(),
            "invalid_composition"
        );
    }

    #[test]
    fn conflicting_set_and_remove_candidates_are_rejected() {
        let base = base_revision();
        let set = system_prompt_candidate("cand-1", "SET GUIDANCE");
        let remove = Candidate::new(
            "cand-2",
            "base-1",
            crate::engine::harness_evolution::model::ProposalKind::Invariant,
            vec![ComponentChange::remove(
                ComponentKind::SystemPrompt,
                "remove prompt",
            )],
        );
        let first = improved_contrast("contrast-1");
        let second = improved_contrast("contrast-2");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![
                CandidateEvidence {
                    candidate: &set,
                    contrast: &first,
                },
                CandidateEvidence {
                    candidate: &remove,
                    contrast: &second,
                },
            ],
            created_by: "smith".into(),
        };
        let composed = compose(&request).unwrap();
        assert_eq!(
            composed.decision.decisions[0].decision,
            ComponentDecision::Reject
        );
        assert_eq!(composed.successor.manifest_hash, base.manifest_hash);
    }

    #[test]
    fn merge_never_expands_authority() {
        let base = base_revision();
        let first = Candidate::new(
            "cand-1",
            "base-1",
            crate::engine::harness_evolution::model::ProposalKind::Domain,
            vec![ComponentChange::set(
                Component::yellow(ComponentKind::Skills, "skill A", "skills"),
                "add skill A",
            )],
        );
        let second = Candidate::new(
            "cand-2",
            "base-1",
            crate::engine::harness_evolution::model::ProposalKind::Domain,
            vec![ComponentChange::set(
                Component::yellow(ComponentKind::Skills, "skill B", "skills"),
                "add skill B",
            )],
        );
        let first_report = improved_contrast("contrast-1");
        let second_report = improved_contrast("contrast-2");
        let request = ComposeRequest {
            id: "composition-1".into(),
            successor_id: "successor-1".into(),
            base: &base,
            evidence: vec![
                CandidateEvidence {
                    candidate: &first,
                    contrast: &first_report,
                },
                CandidateEvidence {
                    candidate: &second,
                    contrast: &second_report,
                },
            ],
            created_by: "smith".into(),
        };
        let composed = compose(&request).unwrap();
        let merged = composed
            .successor
            .manifest
            .get(ComponentKind::Skills)
            .unwrap();
        assert_eq!(merged.authority, ComponentKind::Skills.minimum_authority());
        assert!(merged.content.contains("skill A"));
        assert!(merged.content.contains("skill B"));
    }
}
