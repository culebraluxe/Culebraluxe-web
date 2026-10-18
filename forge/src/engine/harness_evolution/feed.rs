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

use super::model::HarnessDomain;
use super::propose::{ImprovementSignal, ImprovementTarget};

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
        let key = format!("{:?}|{:?}|{:?}", target.domain, target.signal, target.evidence_refs);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(role: &str, agent: &str, tasks: u64, completed: u64, failed: u64, rework: u64) -> ScorecardFeedRow {
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
        let targets = targets_from_scorecard(&[row("smith", "opencode-harness", 10, 3, 1, 2)], FeedOptions::default());
        assert_eq!(targets.len(), 1);
        assert!(matches!(targets[0].signal, ImprovementSignal::ScorecardWeakness { .. }));
        assert_eq!(targets[0].domain, HarnessDomain::RustBackend);
        assert!(!targets[0].evidence_refs.is_empty());
    }

    #[test]
    fn healthy_bucket_produces_no_target() {
        let targets = targets_from_scorecard(&[row("smith", "opencode-harness", 20, 20, 0, 1)], FeedOptions::default());
        assert!(targets.is_empty());
    }

    #[test]
    fn small_buckets_are_ignored() {
        let targets = targets_from_scorecard(&[row("smith", "opencode-harness", 2, 0, 2, 2)], FeedOptions::default());
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
        assert!(matches!(targets[0].signal, ImprovementSignal::HoldPattern { holds: 3, .. }));
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
}
