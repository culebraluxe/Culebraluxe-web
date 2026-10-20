//! Phase 3 → Harness Evolution feed.
//!
//! Converts read-only outcome measurements (the scorecard's per-role/agent rows and the HOLD ledger)
//! into typed [`ImprovementTarget`]s for [`super::propose`]. This is the bridge the architecture
//! calls for: Phase 3 OBSERVES, Harness Evolution EXPERIMENTS. It writes nothing, invents no data,
//! and refuses to produce a target without an evidence reference — a weakness that cannot name the
//! row it came from is not a finding.
//!
//! The thresholds are explicit and conservative. A row is weak only when it has enough volume to
//! mean something (`min_tasks`), and the emitted target carries the observed numbers in the weakness
//! text so a human can audit the claim rather than trust a label.

use super::experiment::HarnessExperiment;
use super::model::{AdoptionDecision, AdoptionVerdict, HarnessDomain, ValidationEvidence};
use super::propose::{ImprovementSignal, ImprovementTarget};
use db::{ForgeCorrectionRepeatRow, NewForgeCorrection, NewForgeHarnessExperience};

/// One read-only scorecard bucket, as `forge scorecard` reports it. Kept deliberately small so the
/// feed does not depend on the CLI's rendering types.
#[derive(Debug, Clone, PartialEq)]
pub struct ScorecardFeedRow {
    /// The engine's `run_type` / role bucket (`lead`, `smith`, `assay`, ...).
    pub role: String,
    /// The executor/agent bucket (`opencode-harness`, `forge-assay`, ...).
    pub agent: String,
    pub tasks: u64,
    pub completed: u64,
    pub held_or_escalated: u64,
    pub failed_or_interrupted: u64,
    pub rework: u64,
    /// Mean wall minutes, when the rows carried it (`None` = NOT CAPTURED, never 0).
    pub mean_wall_minutes: Option<f64>,
    /// Durable row ids the bucket was computed from.
    pub evidence_refs: Vec<String>,
}

/// One repeated HOLD group, as the HOLD ledger reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldFeedRow {
    pub node: String,
    pub failure_class: String,
    pub holds: u32,
    pub evidence_refs: Vec<String>,
}

/// Feed thresholds. Defaults are conservative on purpose: one unlucky task is not a weakness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedOptions {
    /// Buckets smaller than this are ignored.
    pub min_tasks: u64,
    /// Completion rate below this (0.0–1.0) is a weakness.
    pub min_completion_rate: f64,
    /// Rework / tasks at or above this is a weakness.
    pub max_rework_rate: f64,
    /// Failed+interrupted / tasks at or above this is a weakness.
    pub max_failure_rate: f64,
    /// HOLD groups at or above this count are a pattern.
    pub min_holds: u32,
    /// Hard cap on emitted targets, so the proposer cannot be flooded.
    pub max_targets: usize,
}

impl Default for FeedOptions {
    fn default() -> Self {
        Self {
            min_tasks: 5,
            min_completion_rate: 0.80,
            max_rework_rate: 0.50,
            max_failure_rate: 0.25,
            min_holds: 2,
            max_targets: 8,
        }
    }
}

/// Map an engine role/agent bucket to the harness domain whose components should be experimented on.
pub fn domain_for_role(role: &str) -> HarnessDomain {
    match role {
        "smith" | "repair_smith" => HarnessDomain::RustBackend,
        "architect" | "lead" | "lead_pre" => HarnessDomain::ForgeWorkflow,
        "assay" | "qa_verify" | "inspector" => HarnessDomain::QaTesting,
        "dev_ops" | "deliver" => HarnessDomain::DevopsRelease,
        "scout" => HarnessDomain::Integrations,
        _ => HarnessDomain::ForgeWorkflow,
    }
}

/// Build improvement targets from scorecard buckets, strongest signal first. Deterministic: same
/// input, same targets (sorted by role, agent), so a proposal run is reproducible.
pub fn targets_from_scorecard(
    rows: &[ScorecardFeedRow],
    options: FeedOptions,
) -> Vec<ImprovementTarget> {
    let mut ordered = rows.to_vec();
    ordered.sort_by(|a, b| a.role.cmp(&b.role).then(a.agent.cmp(&b.agent)));
    let mut targets = Vec::new();
    for row in &ordered {
        if row.tasks < options.min_tasks || row.evidence_refs.is_empty() {
            continue;
        }
        let tasks = row.tasks as f64;
        let completion = row.completed as f64 / tasks;
        let rework_rate = row.rework as f64 / tasks;
        let failure_rate = (row.failed_or_interrupted + row.held_or_escalated) as f64 / tasks;
        let weakness = if completion < options.min_completion_rate {
            Some(format!(
                "completion {:.0}% over {} task(s)",
                completion * 100.0,
                row.tasks
            ))
        } else if failure_rate >= options.max_failure_rate {
            Some(format!(
                "failed/held {:.0}% over {} task(s)",
                failure_rate * 100.0,
                row.tasks
            ))
        } else if rework_rate >= options.max_rework_rate {
            Some(format!(
                "rework {:.0}% over {} task(s)",
                rework_rate * 100.0,
                row.tasks
            ))
        } else {
            None
        };
        if let Some(weakness) = weakness {
            targets.push(ImprovementTarget::new(
                domain_for_role(&row.role),
                ImprovementSignal::ScorecardWeakness {
                    role: row.role.clone(),
                    agent: row.agent.clone(),
                    weakness,
                },
                row.evidence_refs.clone(),
            ));
        }
    }
    // Deduplicate identical targets (same domain+signal+refs) and cap deterministically.
    let mut seen = std::collections::BTreeSet::new();
    targets.retain(|target| {
        let key = format!(
            "{:?}|{:?}|{:?}",
            target.domain, target.signal, target.evidence_refs
        );
        seen.insert(key)
    });
    targets.sort_by(|a, b| format!("{:?}", a.signal).cmp(&format!("{:?}", b.signal)));
    targets.truncate(options.max_targets);
    targets
}

/// Build targets from repeated HOLD groups. A one-off HOLD is noise; the same node+failure_class
/// recurring is a harness weakness worth proposing against.
pub fn targets_from_holds(rows: &[HoldFeedRow], options: FeedOptions) -> Vec<ImprovementTarget> {
    let mut targets: Vec<ImprovementTarget> = rows
        .iter()
        .filter(|row| row.holds >= options.min_holds && !row.evidence_refs.is_empty())
        .map(|row| {
            ImprovementTarget::new(
                domain_for_role(&row.node),
                ImprovementSignal::HoldPattern {
                    node: row.node.clone(),
                    failure_class: row.failure_class.clone(),
                    holds: row.holds,
                },
                row.evidence_refs.clone(),
            )
        })
        .collect();
    targets.sort_by(|a, b| format!("{:?}", a.signal).cmp(&format!("{:?}", b.signal)));
    targets.truncate(options.max_targets);
    targets
}

// ---------------------------------------------------------------------------------------------
// Correction ledger → RepeatedCorrection targets (Phase 3 TD 3.3/3.4/3.5)
// ---------------------------------------------------------------------------------------------

/// One correction as the `forge_correction_ledger` repeat query reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionFeedRow {
    pub lane: String,
    pub correction: String,
    pub occurrences: u32,
    /// Durable evidence ids the ledger rows carry.
    pub evidence_refs: Vec<String>,
}

/// Adapt database repeat rows into feed rows.
///
/// The database query (migration 295 table) owns the grouping and returns the union of the group's
/// `evidence_refs` as jsonb; this adapter is the one place that shape becomes feed input, so a
/// `RepeatedCorrection` target always carries the evidence its ledger rows carried. Empty/blank
/// ids are dropped; order is preserved and duplicates removed.
pub fn correction_feed_rows(repeats: &[ForgeCorrectionRepeatRow]) -> Vec<CorrectionFeedRow> {
    repeats
        .iter()
        .map(|repeat| {
            let mut evidence_refs: Vec<String> = repeat
                .evidence_refs
                .as_array()
                .map(|array| {
                    array
                        .iter()
                        .filter_map(|value| value.as_str())
                        .map(str::trim)
                        .filter(|reference| !reference.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            evidence_refs.sort();
            evidence_refs.dedup();
            CorrectionFeedRow {
                lane: repeat.lane.clone(),
                correction: repeat.correction.clone(),
                occurrences: u32::try_from(repeat.occurrences).unwrap_or(u32::MAX),
                evidence_refs,
            }
        })
        .collect()
}

/// Thresholds for correction targets. Separate from [`FeedOptions`] so the scorecard/HOLD defaults
/// stay untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorrectionFeedOptions {
    /// The same correction must be seen at least this many times to be a pattern.
    pub min_occurrences: u32,
    pub max_targets: usize,
}

impl Default for CorrectionFeedOptions {
    fn default() -> Self {
        Self {
            min_occurrences: 2,
            max_targets: 8,
        }
    }
}

/// Convert repeated corrections into `ImprovementSignal::RepeatedCorrection` targets.
///
/// One correction is a normal review event; the same instruction recurring for the same lane is a
/// harness weakness. Evidence is required, output is deduplicated and capped deterministically.
pub fn targets_from_corrections(
    rows: &[CorrectionFeedRow],
    options: CorrectionFeedOptions,
) -> Vec<ImprovementTarget> {
    let mut targets: Vec<ImprovementTarget> = rows
        .iter()
        .filter(|row| row.occurrences >= options.min_occurrences && !row.evidence_refs.is_empty())
        .map(|row| {
            ImprovementTarget::new(
                domain_for_role(&row.lane),
                ImprovementSignal::RepeatedCorrection {
                    lane: row.lane.clone(),
                    correction: row.correction.clone(),
                },
                row.evidence_refs.clone(),
            )
        })
        .collect();
    targets.sort_by(|a, b| format!("{:?}", a.signal).cmp(&format!("{:?}", b.signal)));
    targets.dedup_by(|a, b| a.signal == b.signal && a.evidence_refs == b.evidence_refs);
    targets.truncate(options.max_targets);
    targets
}

/// One learn-loop observation as the `forge_learn_finding` read reports it.
///
/// A learn finding is a **hypothesis**, not a confirmed defect: the learn loop files review
/// candidates. The feed keeps that distinction in the target type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HypothesisFeedRow {
    pub repository_key: String,
    pub finding_key: String,
    /// How many learn passes saw this pattern.
    pub occurrence: u32,
    pub last_story_id: Option<String>,
    pub evidence_refs: Vec<String>,
}

/// A hypothesis target: the "new hypothesis signal" seam. It is intentionally NOT an
/// [`ImprovementTarget`] yet — a review candidate is not a confirmed weakness, and the proposer's
/// signal enum is owned by `propose.rs`. When the proposer grows a hypothesis signal it consumes
/// this type; until then the feed can report hypotheses without pretending they are measurements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HypothesisTarget {
    pub domain: HarnessDomain,
    pub repository_key: String,
    pub finding_key: String,
    pub hypothesis: String,
    pub occurrence: u32,
    pub evidence_refs: Vec<String>,
}

impl HypothesisTarget {
    /// Stable id for logs/durable rows.
    pub fn id(&self) -> String {
        format!("hypothesis:{}:{}", self.repository_key, self.finding_key)
    }
}

/// Convert learn-loop findings into hypothesis targets. A finding with no durable evidence (no
/// last story and no ids) is skipped: an untraceable hint is not a target.
pub fn targets_from_hypotheses(
    rows: &[HypothesisFeedRow],
    max_targets: usize,
) -> Vec<HypothesisTarget> {
    let mut targets: Vec<HypothesisTarget> = rows
        .iter()
        .filter_map(|row| {
            let mut evidence = row.evidence_refs.clone();
            if let Some(story_id) = &row.last_story_id {
                let reference = format!("storyboard_story:{story_id}");
                if !evidence.contains(&reference) {
                    evidence.push(reference);
                }
            }
            evidence.retain(|reference| !reference.trim().is_empty());
            if evidence.is_empty() {
                return None;
            }
            Some(HypothesisTarget {
                domain: domain_for_role(&row.repository_key),
                repository_key: row.repository_key.clone(),
                finding_key: row.finding_key.clone(),
                hypothesis: format!(
                    "{}:{} seen {} time(s); review candidate, not a confirmed defect",
                    row.repository_key, row.finding_key, row.occurrence
                ),
                occurrence: row.occurrence,
                evidence_refs: evidence,
            })
        })
        .collect();
    targets.sort_by(|a, b| {
        b.occurrence
            .cmp(&a.occurrence)
            .then(a.repository_key.cmp(&b.repository_key))
            .then(a.finding_key.cmp(&b.finding_key))
    });
    targets.dedup_by(|a, b| a.repository_key == b.repository_key && a.finding_key == b.finding_key);
    targets.truncate(max_targets);
    targets
}

/// Combine the deterministic target sources, dedupe across them and apply one cap. This is what a
/// proposal run should consume so the scorecard/HOLD/correction sources cannot flood the proposer.
pub fn targets_from_learning_sources(
    scorecard: &[ScorecardFeedRow],
    holds: &[HoldFeedRow],
    corrections: &[CorrectionFeedRow],
    options: FeedOptions,
) -> Vec<ImprovementTarget> {
    let mut targets = targets_from_scorecard(scorecard, options);
    targets.extend(targets_from_holds(holds, options));
    targets.extend(targets_from_corrections(
        corrections,
        CorrectionFeedOptions {
            max_targets: options.max_targets,
            ..CorrectionFeedOptions::default()
        },
    ));
    let mut seen = std::collections::BTreeSet::new();
    targets.retain(|target| {
        let key = format!(
            "{:?}|{:?}|{:?}",
            target.domain, target.signal, target.evidence_refs
        );
        seen.insert(key)
    });
    targets.sort_by(|a, b| format!("{:?}", a.signal).cmp(&format!("{:?}", b.signal)));
    targets.truncate(options.max_targets);
    targets
}

// ---------------------------------------------------------------------------------------------
// Experiment outcome → Phase 3 writeback (correction ledger / experience, never a second store)
// ---------------------------------------------------------------------------------------------

/// A rejected experiment's correction, ready to become a `forge_correction_ledger` row.
///
/// The decider's rationale is NOT the correction text. `correction` is the corrective statement
/// (None when the outcome only states a rationale), and `rejection_rationale` is the rationale
/// verbatim. The applied ledger schema has one text column, so [`CorrectionWriteback::to_new`]
/// documents the one honest mapping: when no separate correction text exists, the verbatim
/// rationale is what the ledger stores. A future additive column can split them without changing
/// the rows written today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionWriteback {
    pub lane: String,
    /// The corrective statement, when the outcome stated one.
    pub correction: Option<String>,
    /// The decider's stated reason for rejecting, verbatim. None = not stated, not "unknown".
    pub rejection_rationale: Option<String>,
    pub context: Option<String>,
    pub root_cause: Option<String>,
    pub spec_location: Option<String>,
    pub permanent_rule_created: bool,
    pub evidence_refs: Vec<String>,
    pub experiment_id: Option<String>,
}

impl CorrectionWriteback {
    /// The text the ledger's single `correction` column stores: the stated correction when there is
    /// one, otherwise the verbatim rejection rationale. No generated text, no placeholder.
    pub fn ledger_text(&self) -> Option<&str> {
        self.correction
            .as_deref()
            .or(self.rejection_rationale.as_deref())
    }

    pub fn to_new(&self) -> NewForgeCorrection<'_> {
        // `ledger_text` is enforced non-empty by `writeback_from_experiment`; `to_new` is only
        // called for a writeback built by that function.
        NewForgeCorrection {
            lane: &self.lane,
            correction: self.ledger_text().unwrap_or_default(),
            context: self.context.as_deref(),
            root_cause: self.root_cause.as_deref(),
            spec_location: self.spec_location.as_deref(),
            permanent_rule_created: self.permanent_rule_created,
            evidence_refs: serde_json::json!(self.evidence_refs),
            experiment_id: self.experiment_id.as_deref(),
        }
    }
}

/// An adopted experiment's lesson, ready to become a `forge_harness_experience` row (migration 294).
/// The scope fields come from the experiment; the lesson and confidence are evidence-derived, never
/// invented by the feed.
#[derive(Debug, Clone, PartialEq)]
pub struct ExperienceWriteback {
    pub experiment_id: String,
    pub domain: HarnessDomain,
    pub executor: String,
    pub mode: String,
    pub k: u32,
    pub max_steps: u32,
    pub max_probes: u32,
    pub seed: u64,
    pub harness_revision_id: Option<String>,
    pub manifest_hash: String,
    pub lesson: String,
    pub applicability: Vec<String>,
    pub counterevidence: Vec<String>,
    /// The composition's own stated confidence (max over component decisions), or `None` when the
    /// composition stated none. Never fabricated as 0.
    pub confidence: Option<u8>,
    pub contrast_id: String,
    pub validation: ValidationEvidence,
    pub evidence_refs: Vec<String>,
}

impl ExperienceWriteback {
    /// Deterministic experience id: one adopted outcome has one lesson per contrast.
    pub fn experience_id(&self) -> String {
        format!("exp-{}-{}", self.experiment_id, self.contrast_id)
    }

    pub fn to_new(&self) -> Result<NewForgeHarnessExperience, String> {
        let validation = serde_json::to_value(&self.validation)
            .map_err(|error| format!("validation evidence is not serializable: {error}"))?;
        Ok(NewForgeHarnessExperience {
            experience_id: self.experience_id(),
            domain: self.domain.id().to_string(),
            executor: self.executor.clone(),
            mode: self.mode.clone(),
            k: i32::try_from(self.k).unwrap_or(i32::MAX),
            max_steps: i32::try_from(self.max_steps).unwrap_or(i32::MAX),
            max_probes: i32::try_from(self.max_probes).unwrap_or(i32::MAX),
            seed: i64::try_from(self.seed).unwrap_or(i64::MAX),
            harness_revision_id: self.harness_revision_id.clone(),
            manifest_hash: self.manifest_hash.clone(),
            lesson: self.lesson.clone(),
            applicability: serde_json::json!(self.applicability),
            counterevidence: serde_json::json!(self.counterevidence),
            confidence: i32::from(self.confidence.ok_or_else(|| {
                "experience writeback has no stated confidence; refusing to fabricate one"
                    .to_string()
            })?),
            contrast_id: self.contrast_id.clone(),
            validation,
            evidence_refs: serde_json::json!(self.evidence_refs),
            expires_at: None,
        })
    }
}

/// The Phase 3 writeback for one experiment outcome.
#[derive(Debug, Clone, PartialEq)]
pub enum ExperimentWriteback {
    /// A rejected experiment becomes a correction row.
    Correction(Box<CorrectionWriteback>),
    /// An adopted experiment becomes an experience row.
    Experience(Box<ExperienceWriteback>),
}

impl ExperimentWriteback {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Correction(_) => "correction",
            Self::Experience(_) => "experience",
        }
    }
}

fn writeback_evidence(decision: &AdoptionDecision, experiment: &HarnessExperiment) -> Vec<String> {
    let mut refs: Vec<String> = decision.evidence_refs.clone();
    for contrast in &experiment.contrasts {
        refs.extend(contrast.evidence_refs.iter().cloned());
    }
    if let Some(validation) = &experiment.validation {
        refs.extend(validation.evidence_refs.iter().cloned());
    }
    refs.retain(|reference| !reference.trim().is_empty());
    refs.sort();
    refs.dedup();
    // No fallback. The experiment row naming itself is not evidence for its own outcome: a
    // writeback without at least one externally-owned evidence id is refused by the caller.
    refs
}

/// Map an experiment outcome to its Phase 3 writeback.
///
/// * `Adopt` + clean validation → an experience lesson (contrast id, validation gates and the
///   evidence ids are carried; confidence is the composition's own stated confidence, never a
///   number this function invents).
/// * `Reject` → a correction row (what the lane was told, why it was rejected as context; root
///   cause/spec location stay NULL because the feed did not establish them).
/// * `MoreEvidence`/`Council`/`Chris` → no writeback: there is no outcome yet.
pub fn writeback_from_experiment(
    experiment: &HarnessExperiment,
    decision: &AdoptionDecision,
) -> Result<Option<ExperimentWriteback>, String> {
    let evidence_refs = writeback_evidence(decision, experiment);
    if evidence_refs.is_empty() {
        return Err(
            "experiment outcome carries no durable evidence ids; refusing a self-referential writeback"
                .to_string(),
        );
    }
    match decision.verdict {
        AdoptionVerdict::Adopt => {
            let contrast = experiment
                .contrasts
                .first()
                .ok_or_else(|| "adopted experiment has no contrast to cite".to_string())?;
            let composition = experiment
                .composition
                .as_ref()
                .ok_or_else(|| "adopted experiment has no composition to cite".to_string())?;
            let validation = experiment
                .validation
                .clone()
                .unwrap_or_else(ValidationEvidence::pending);
            if !validation.is_clean() {
                return Err(
                    "adopted experiment does not carry clean validation; no lesson can be written"
                        .to_string(),
                );
            }
            let lesson = if !decision.rationale.trim().is_empty() {
                decision.rationale.trim().to_string()
            } else if !composition.summary.trim().is_empty() {
                composition.summary.trim().to_string()
            } else {
                return Err(
                    "adopted experiment has neither rationale nor composition summary".into(),
                );
            };
            if composition.successor_manifest_hash.trim().is_empty() {
                return Err("adopted experiment has no successor manifest hash".into());
            }
            // ComponentDecisionRecord::new starts at 0 = "no confidence stated"; only a stated
            // (>0) confidence is carried, and the maximum is used so one well-supported component
            // does not get averaged away by unstated ones.
            let confidence = composition
                .decisions
                .iter()
                .map(|record| record.confidence)
                .filter(|confidence| *confidence > 0)
                .max();
            Ok(Some(ExperimentWriteback::Experience(Box::new(
                ExperienceWriteback {
                    experiment_id: experiment.id.clone(),
                    domain: experiment.domain,
                    executor: experiment.pins.executor.clone(),
                    mode: experiment.mode.id().to_string(),
                    k: experiment.budget.k,
                    max_steps: experiment.budget.max_steps,
                    max_probes: experiment.budget.max_probes,
                    seed: experiment.seed,
                    harness_revision_id: experiment.successor_revision.clone(),
                    manifest_hash: composition.successor_manifest_hash.clone(),
                    lesson,
                    applicability: vec![
                        experiment.domain.id().to_string(),
                        experiment.pins.executor.clone(),
                    ],
                    counterevidence: Vec::new(),
                    confidence,
                    contrast_id: contrast.id.clone(),
                    validation,
                    evidence_refs,
                },
            ))))
        }
        AdoptionVerdict::Reject => {
            let correction = decision.rationale.trim();
            if correction.is_empty() {
                return Err(
                    "rejected experiment has no rationale to record as a correction".into(),
                );
            }
            Ok(Some(ExperimentWriteback::Correction(Box::new(
                CorrectionWriteback {
                    lane: experiment.domain.id().to_string(),
                    // The reject outcome states a rationale, not a separate corrective text.
                    correction: None,
                    rejection_rationale: Some(correction.to_string()),
                    context: Some(format!(
                        "harness-experiment:{} verdict=rejected successor={:?}",
                        experiment.id, experiment.successor_revision
                    )),
                    root_cause: None,
                    spec_location: None,
                    permanent_rule_created: false,
                    evidence_refs,
                    experiment_id: Some(experiment.id.clone()),
                },
            ))))
        }
        AdoptionVerdict::MoreEvidence | AdoptionVerdict::Council | AdoptionVerdict::Chris => {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        role: &str,
        agent: &str,
        tasks: u64,
        completed: u64,
        failed: u64,
        rework: u64,
    ) -> ScorecardFeedRow {
        ScorecardFeedRow {
            role: role.into(),
            agent: agent.into(),
            tasks,
            completed,
            held_or_escalated: 0,
            failed_or_interrupted: failed,
            rework,
            mean_wall_minutes: Some(12.0),
            evidence_refs: vec![format!("storyboard_story_run:{role}/{agent}")],
        }
    }

    #[test]
    fn weak_completion_produces_a_target() {
        let targets = targets_from_scorecard(
            &[row("smith", "opencode-harness", 10, 3, 1, 2)],
            FeedOptions::default(),
        );
        assert_eq!(targets.len(), 1);
        assert!(matches!(
            targets[0].signal,
            ImprovementSignal::ScorecardWeakness { .. }
        ));
        assert_eq!(targets[0].domain, HarnessDomain::RustBackend);
        assert!(!targets[0].evidence_refs.is_empty());
    }

    #[test]
    fn healthy_bucket_produces_no_target() {
        let targets = targets_from_scorecard(
            &[row("smith", "opencode-harness", 20, 20, 0, 1)],
            FeedOptions::default(),
        );
        assert!(targets.is_empty());
    }

    #[test]
    fn small_buckets_are_ignored() {
        let targets = targets_from_scorecard(
            &[row("smith", "opencode-harness", 2, 0, 2, 2)],
            FeedOptions::default(),
        );
        assert!(targets.is_empty());
    }

    #[test]
    fn a_target_without_evidence_is_refused() {
        let mut weak = row("smith", "opencode-harness", 10, 1, 0, 0);
        weak.evidence_refs.clear();
        assert!(targets_from_scorecard(&[weak], FeedOptions::default()).is_empty());
    }

    #[test]
    fn role_maps_to_domain() {
        assert_eq!(domain_for_role("smith"), HarnessDomain::RustBackend);
        assert_eq!(domain_for_role("assay"), HarnessDomain::QaTesting);
        assert_eq!(domain_for_role("dev_ops"), HarnessDomain::DevopsRelease);
        assert_eq!(domain_for_role("unheard-of"), HarnessDomain::ForgeWorkflow);
    }

    #[test]
    fn repeated_holds_produce_a_hold_pattern() {
        let rows = vec![HoldFeedRow {
            node: "qa_verify".into(),
            failure_class: "MISSING_EVIDENCE".into(),
            holds: 3,
            evidence_refs: vec!["forge_hold_record:1".into()],
        }];
        let targets = targets_from_holds(&rows, FeedOptions::default());
        assert_eq!(targets.len(), 1);
        assert!(matches!(
            targets[0].signal,
            ImprovementSignal::HoldPattern { holds: 3, .. }
        ));
        assert_eq!(targets[0].domain, HarnessDomain::QaTesting);
    }

    #[test]
    fn a_single_hold_is_noise() {
        let rows = vec![HoldFeedRow {
            node: "qa_verify".into(),
            failure_class: "MISSING_EVIDENCE".into(),
            holds: 1,
            evidence_refs: vec!["forge_hold_record:1".into()],
        }];
        assert!(targets_from_holds(&rows, FeedOptions::default()).is_empty());
    }

    #[test]
    fn output_is_capped_and_deterministic() {
        let rows: Vec<_> = (0..20)
            .map(|index| row(&format!("role{index:02}"), "opencode-harness", 10, 1, 0, 0))
            .collect();
        let first = targets_from_scorecard(&rows, FeedOptions::default());
        let second = targets_from_scorecard(&rows, FeedOptions::default());
        assert_eq!(first, second, "same input must produce the same targets");
        assert_eq!(first.len(), 8);
    }

    #[test]
    fn repeated_corrections_become_repeated_correction_targets() {
        let rows = vec![CorrectionFeedRow {
            lane: "smith".into(),
            correction: "name the migration".into(),
            occurrences: 3,
            evidence_refs: vec!["forge_correction_ledger:1".into()],
        }];
        let targets = targets_from_corrections(&rows, CorrectionFeedOptions::default());
        assert_eq!(targets.len(), 1);
        match &targets[0].signal {
            ImprovementSignal::RepeatedCorrection { lane, correction } => {
                assert_eq!(lane, "smith");
                assert_eq!(correction, "name the migration");
            }
            other => panic!("expected a repeated correction, got {other:?}"),
        }
        assert_eq!(targets[0].domain, HarnessDomain::RustBackend);
    }

    #[test]
    fn a_one_off_correction_is_not_a_pattern_and_needs_evidence() {
        let once = vec![CorrectionFeedRow {
            lane: "smith".into(),
            correction: "name the migration".into(),
            occurrences: 1,
            evidence_refs: vec!["forge_correction_ledger:1".into()],
        }];
        assert!(targets_from_corrections(&once, CorrectionFeedOptions::default()).is_empty());
        let no_evidence = vec![CorrectionFeedRow {
            lane: "smith".into(),
            correction: "name the migration".into(),
            occurrences: 4,
            evidence_refs: Vec::new(),
        }];
        assert!(
            targets_from_corrections(&no_evidence, CorrectionFeedOptions::default()).is_empty()
        );
    }

    #[test]
    fn correction_targets_are_deduped_and_capped() {
        let rows: Vec<CorrectionFeedRow> = (0..20)
            .map(|index| CorrectionFeedRow {
                lane: "smith".into(),
                correction: format!("correction {index}"),
                occurrences: 2,
                evidence_refs: vec![format!("forge_correction_ledger:{index}")],
            })
            .collect();
        let options = CorrectionFeedOptions {
            min_occurrences: 2,
            max_targets: 4,
        };
        let targets = targets_from_corrections(&rows, options);
        assert_eq!(targets.len(), 4);
        let mut duplicate = rows.clone();
        duplicate.push(rows[0].clone());
        let again = targets_from_corrections(&duplicate, options);
        assert_eq!(
            targets, again,
            "a duplicated row must not duplicate a target"
        );
    }

    #[test]
    fn learn_hypotheses_become_hypothesis_targets_with_evidence() {
        let rows = vec![HypothesisFeedRow {
            repository_key: "forge".into(),
            finding_key: "silent-failure-pattern:src/x.rs".into(),
            occurrence: 3,
            last_story_id: Some("STORY-1".into()),
            evidence_refs: vec!["forge_learn_finding:forge/silent".into()],
        }];
        let targets = targets_from_hypotheses(&rows, 8);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].domain, HarnessDomain::ForgeWorkflow);
        assert!(targets[0].hypothesis.contains("review candidate"));
        assert!(targets[0]
            .evidence_refs
            .contains(&"storyboard_story:STORY-1".to_string()));
        assert_eq!(
            targets[0].id(),
            "hypothesis:forge:silent-failure-pattern:src/x.rs"
        );
    }

    #[test]
    fn an_untraceable_hypothesis_is_skipped_and_output_is_capped() {
        let rows = vec![
            HypothesisFeedRow {
                repository_key: "forge".into(),
                finding_key: "no-evidence".into(),
                occurrence: 9,
                last_story_id: None,
                evidence_refs: Vec::new(),
            },
            HypothesisFeedRow {
                repository_key: "forge".into(),
                finding_key: "has-evidence".into(),
                occurrence: 2,
                last_story_id: None,
                evidence_refs: vec!["forge_learn_finding:forge/has-evidence".into()],
            },
        ];
        let targets = targets_from_hypotheses(&rows, 8);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].finding_key, "has-evidence");
        let many: Vec<HypothesisFeedRow> = (0..10)
            .map(|index| HypothesisFeedRow {
                repository_key: "forge".into(),
                finding_key: format!("finding-{index}"),
                occurrence: 1,
                last_story_id: Some("STORY-1".into()),
                evidence_refs: Vec::new(),
            })
            .collect();
        assert_eq!(targets_from_hypotheses(&many, 3).len(), 3);
    }

    #[test]
    fn combined_learning_sources_are_deduped_and_capped() {
        let scorecard = vec![row("smith", "opencode-harness", 10, 1, 0, 0)];
        let holds = vec![HoldFeedRow {
            node: "qa_verify".into(),
            failure_class: "MISSING_EVIDENCE".into(),
            holds: 3,
            evidence_refs: vec!["forge_hold_record:1".into()],
        }];
        let corrections = vec![CorrectionFeedRow {
            lane: "assay".into(),
            correction: "run the focused test first".into(),
            occurrences: 2,
            evidence_refs: vec!["forge_correction_ledger:1".into()],
        }];
        let options = FeedOptions {
            max_targets: 2,
            ..FeedOptions::default()
        };
        let targets = targets_from_learning_sources(&scorecard, &holds, &corrections, options);
        assert_eq!(targets.len(), 2);
        let repeated = targets_from_learning_sources(&scorecard, &holds, &corrections, options);
        assert_eq!(targets, repeated);
    }

    #[test]
    fn ledger_repeat_rows_adapt_into_evidence_linked_repeated_correction_targets() {
        let repeats = vec![ForgeCorrectionRepeatRow {
            lane: "smith".into(),
            correction_key: "ab".repeat(32),
            correction: "name the migration".into(),
            occurrences: 3,
            latest_at: chrono::Utc::now(),
            evidence_refs: serde_json::json!([
                "forge_correction_ledger:1",
                "forge_hold_record:2",
                "forge_correction_ledger:1",
                "  ",
            ]),
        }];
        let rows = correction_feed_rows(&repeats);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].occurrences, 3);
        assert_eq!(
            rows[0].evidence_refs,
            vec![
                "forge_correction_ledger:1".to_string(),
                "forge_hold_record:2".to_string()
            ],
            "evidence union is deduplicated and blanks dropped"
        );
        let targets = targets_from_corrections(&rows, CorrectionFeedOptions::default());
        assert_eq!(targets.len(), 1, "real ledger rows produce a target");
        assert_eq!(
            targets[0].evidence_refs, rows[0].evidence_refs,
            "the target carries the ledger evidence"
        );
    }

    #[test]
    fn an_experience_without_a_stated_confidence_is_not_given_a_fabricated_zero() {
        let decision = AdoptionDecision::new(
            "decision-1",
            HarnessDomain::RustBackend,
            "successor-1",
            AdoptionVerdict::Adopt,
            "policy",
            "the candidate reduced rework",
        )
        .with_evidence_refs(vec!["evidence-1".into()]);
        let mut experiment = experiment_for_writeback(
            decision.clone(),
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        experiment.composition.as_mut().unwrap().decisions[0].confidence = 0;
        let writeback = writeback_from_experiment(&experiment, &decision)
            .expect("writeback")
            .expect("adopted outcome has a writeback");
        match writeback {
            ExperimentWriteback::Experience(experience) => {
                assert_eq!(experience.confidence, None, "0 stated means 'none stated'");
                let error = experience
                    .to_new()
                    .expect_err("db row requires a confidence");
                assert!(error.contains("no stated confidence"), "{error}");
            }
            other => panic!("expected an experience writeback, got {other:?}"),
        }
    }

    #[test]
    fn a_writeback_without_external_evidence_is_refused_not_self_cited() {
        let decision = AdoptionDecision::new(
            "decision-2",
            HarnessDomain::RustBackend,
            "successor-1",
            AdoptionVerdict::Reject,
            "policy",
            "replay gate failed",
        );
        let mut experiment =
            experiment_for_writeback(decision.clone(), ValidationEvidence::pending());
        experiment.contrasts[0].evidence_refs.clear();
        let error = writeback_from_experiment(&experiment, &decision)
            .expect_err("experiment id is not evidence for its own outcome");
        assert!(error.contains("no durable evidence"), "{error}");
    }

    fn experiment_for_writeback(
        _decision: AdoptionDecision,
        validation: ValidationEvidence,
    ) -> HarnessExperiment {
        use crate::engine::harness_evolution::model::{
            BehaviorEvidence, Component, ComponentDecision, ComponentDecisionRecord, ComponentKind,
            CompositionDecision, ContrastReport, EvolutionBudget, EvolutionMode, HarnessManifest,
            HarnessRevision, ProbeIdentity, QaVerdict, RevisionStatus,
        };
        use crate::engine::harness_evolution::probe::ProbePins;
        let base = HarnessRevision::draft(
            "base-1",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([Component::green(
                ComponentKind::SystemPrompt,
                "base",
                "feed test",
            )]),
            "owner",
        );
        let pins = ProbePins::development("task-1");
        let identity = ProbeIdentity {
            model: "deepseek-chat".into(),
            model_policy: "default".into(),
            executor: "opencode".into(),
            tools: Vec::new(),
            environment_class:
                crate::engine::harness_evolution::model::EnvironmentClass::Development,
            input_dataset: "forge-harness-evolution".into(),
        };
        let mut experiment = HarnessExperiment::new(
            "exp-1",
            HarnessDomain::RustBackend,
            &base,
            pins,
            EvolutionMode::Parallel,
            EvolutionBudget::k1(),
            7,
        )
        .expect("experiment");
        experiment.state = RevisionStatus::Adopted;
        experiment.successor_revision = Some("successor-1".into());
        let mut reference = BehaviorEvidence::new("evidence-ref", "probe-ref");
        reference.success = true;
        reference.qa = QaVerdict::Pass;
        experiment.contrasts = vec![ContrastReport {
            id: "contrast-1".into(),
            base_revision: "base-1".into(),
            candidate_revision: "successor-1".into(),
            reference_identity: identity.clone(),
            candidate_identity: identity,
            dimensions: Vec::new(),
            summary: "better".into(),
            evidence_refs: vec!["qa:1".into()],
        }];
        experiment.composition = Some(CompositionDecision {
            id: "composition-1".into(),
            base_revision: "base-1".into(),
            candidates: vec!["c1".into()],
            contrast_ids: vec!["contrast-1".into()],
            decisions: vec![ComponentDecisionRecord::new(
                ComponentKind::SystemPrompt,
                ComponentDecision::Adopt,
                "better",
            )
            .with_confidence(70)],
            successor_manifest_hash: "manifest-hash".into(),
            summary: "composed".into(),
            evidence_refs: vec!["evidence-1".into()],
        });
        experiment.validation = Some(validation);
        experiment
    }

    #[test]
    fn an_adopted_experiment_writes_back_an_experience_not_a_correction() {
        let decision = AdoptionDecision::new(
            "decision-1",
            HarnessDomain::RustBackend,
            "successor-1",
            AdoptionVerdict::Adopt,
            "policy",
            "the candidate reduced rework",
        )
        .with_evidence_refs(vec!["evidence-1".into()]);
        let experiment = experiment_for_writeback(
            decision.clone(),
            ValidationEvidence::all_passed(vec!["validation-1".into()]),
        );
        let writeback = writeback_from_experiment(&experiment, &decision)
            .expect("writeback")
            .expect("adopted outcome has a writeback");
        assert_eq!(writeback.kind(), "experience");
        match writeback {
            ExperimentWriteback::Experience(experience) => {
                assert_eq!(experience.experiment_id, "exp-1");
                assert_eq!(experience.manifest_hash, "manifest-hash");
                assert_eq!(experience.contrast_id, "contrast-1");
                assert_eq!(experience.lesson, "the candidate reduced rework");
                assert_eq!(
                    experience.confidence,
                    Some(70),
                    "confidence comes from composition"
                );
                assert_eq!(experience.executor, "opencode");
                assert_eq!(experience.mode, "parallel");
                assert_eq!(experience.k, 1);
                let new_row = experience.to_new().expect("db row");
                assert_eq!(new_row.contrast_id, "contrast-1");
                assert_eq!(new_row.domain, "rust-backend");
                assert_eq!(new_row.confidence, 70);
                assert!(new_row.evidence_refs.as_array().unwrap().len() >= 2);
            }
            other => panic!("expected an experience writeback, got {other:?}"),
        }
    }

    #[test]
    fn a_rejected_experiment_writes_back_a_correction_without_inventing_root_cause() {
        let decision = AdoptionDecision::new(
            "decision-2",
            HarnessDomain::RustBackend,
            "successor-1",
            AdoptionVerdict::Reject,
            "policy",
            "replay gate failed; the prompt change regressed the focused test",
        );
        let experiment = experiment_for_writeback(decision.clone(), ValidationEvidence::pending());
        let writeback = writeback_from_experiment(&experiment, &decision)
            .expect("writeback")
            .expect("rejected outcome has a writeback");
        assert_eq!(writeback.kind(), "correction");
        match writeback {
            ExperimentWriteback::Correction(correction) => {
                assert_eq!(correction.lane, "rust-backend");
                assert_eq!(
                    correction.correction, None,
                    "a reject outcome states a rationale, not a correction text"
                );
                assert_eq!(
                    correction.rejection_rationale.as_deref(),
                    Some("replay gate failed; the prompt change regressed the focused test"),
                    "the rationale has its own verbatim field"
                );
                assert!(correction.context.as_deref().unwrap().contains("exp-1"));
                assert_eq!(correction.root_cause, None, "not invented");
                assert_eq!(correction.spec_location, None, "not invented");
                assert!(!correction.permanent_rule_created);
                assert_eq!(correction.experiment_id.as_deref(), Some("exp-1"));
                assert!(
                    correction.evidence_refs.contains(&"qa:1".to_string()),
                    "the contrast's evidence id is carried: {:?}",
                    correction.evidence_refs
                );
                let new_row = correction.to_new();
                assert_eq!(new_row.lane, "rust-backend");
                assert_eq!(new_row.experiment_id, Some("exp-1"));
                // The ledger's single text column stores the verbatim rationale when no separate
                // corrective text exists; no generated text.
                assert_eq!(
                    new_row.correction,
                    "replay gate failed; the prompt change regressed the focused test"
                );
            }
            other => panic!("expected a correction writeback, got {other:?}"),
        }
    }

    #[test]
    fn non_outcome_verdicts_write_nothing_back() {
        for verdict in [
            AdoptionVerdict::MoreEvidence,
            AdoptionVerdict::Council,
            AdoptionVerdict::Chris,
        ] {
            let decision = AdoptionDecision::new(
                format!("decision-{verdict:?}"),
                HarnessDomain::RustBackend,
                "successor-1",
                verdict,
                "policy",
                "not an outcome yet",
            );
            let experiment =
                experiment_for_writeback(decision.clone(), ValidationEvidence::pending());
            assert_eq!(
                writeback_from_experiment(&experiment, &decision).unwrap(),
                None
            );
        }
    }

    #[test]
    fn writeback_refuses_an_adoption_without_clean_validation_or_a_contrast() {
        let decision = AdoptionDecision::new(
            "decision-3",
            HarnessDomain::RustBackend,
            "successor-1",
            AdoptionVerdict::Adopt,
            "policy",
            "rationale",
        );
        let experiment = experiment_for_writeback(decision.clone(), ValidationEvidence::pending());
        assert!(writeback_from_experiment(&experiment, &decision).is_err());
    }
}
