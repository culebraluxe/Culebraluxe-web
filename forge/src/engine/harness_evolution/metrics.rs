//! Read-only routing and model/role metrics for Phase 3.
//!
//! The scorecard answers "how is a role/agent doing?". This module answers the narrower routing
//! and model/role-fit question the Phase 3 review deferred (TECH-DEBT 3.6 and 3.14): per
//! `(role, model)` pair, how many attempts ran, how they ended, how much rework they took, whether
//! the route matched when a route was actually observed, and how much of the cost is priced.
//!
//! HONESTY RULES, the same ones the scorecard and `forge roi` use:
//!
//! * **null is not zero.** An unpriced attempt does not contribute `0` cost; when no attempt in a
//!   bucket was priced, `total_cost_micros`/`mean_cost_micros` are `None` (NOT CAPTURED). An
//!   uncaptured outcome does not count as a failure: `completion_rate` is `None` until at least one
//!   outcome was captured. An unobserved route is not a mismatch: `routing_accuracy` is `None`
//!   until at least one route was observed.
//! * **no invented probabilities.** Every rate is `Option<f64>` and appears only when its
//!   denominator is known and non-zero.
//! * **evidence-linked.** Each bucket lists the durable row ids it was computed from.
//!
//! Pure, deterministic (sorted by role, then model), and unit tested: same rows → same metrics.

use std::collections::BTreeMap;

/// How one attempt ended. `None` on an observation means the outcome was not captured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RunOutcome {
    Completed,
    Held,
    Failed,
    Interrupted,
}

impl RunOutcome {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Held => "held",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }
}

/// One durable attempt, as the read path can report it. A field that was not captured is `None`,
/// never `0` and never a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleModelObservation {
    pub role: String,
    pub model: String,
    /// `None` = the attempt's outcome was not captured.
    pub outcome: Option<RunOutcome>,
    /// 1-based attempt index within its role (2+ is rework).
    pub attempt: u32,
    /// `Some(true/false)` only when a route was observed AND compared.
    pub route_matched: Option<bool>,
    /// Vendor cost in micro-units, or `None` when the attempt is unpriced.
    pub cost_micros: Option<u64>,
    /// Durable row id this observation came from.
    pub evidence_ref: String,
}

/// Per `(role, model)` metrics. Every rate/cost field is `Option`; `None` means NOT CAPTURED.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleModelMetric {
    pub role: String,
    pub model: String,
    pub attempts: u64,
    pub completed: u64,
    pub held: u64,
    pub failed: u64,
    pub interrupted: u64,
    /// Attempts whose outcome was not captured (they are not counted in the rates below).
    pub uncaptured_outcomes: u64,
    /// Attempt index 2+ within its role.
    pub rework_attempts: u64,
    /// `completed / captured_outcomes`, or `None` when no outcome was captured.
    pub completion_rate: Option<f64>,
    /// `rework_attempts / attempts` (attempts is always known and non-zero for a bucket).
    pub rework_rate: Option<f64>,
    /// Attempts with an observed route comparison.
    pub route_observed: u64,
    pub route_mismatches: u64,
    /// `matched / observed`, or `None` when no route was observed.
    pub routing_accuracy: Option<f64>,
    pub priced_attempts: u64,
    pub unpriced_attempts: u64,
    /// Sum of priced micro-units, or `None` when nothing in the bucket was priced.
    pub total_cost_micros: Option<u64>,
    /// Mean priced micro-units, or `None` when nothing in the bucket was priced.
    pub mean_cost_micros: Option<f64>,
    /// `priced_attempts / attempts` — coverage of the cost measurement itself.
    pub cost_coverage: Option<f64>,
    /// Durable ids, deduplicated, sorted.
    pub evidence_refs: Vec<String>,
}

/// Compute per `(role, model)` metrics. Deterministic: sorted by role then model.
pub fn role_model_metrics(rows: &[RoleModelObservation]) -> Vec<RoleModelMetric> {
    let mut buckets: BTreeMap<(String, String), Vec<&RoleModelObservation>> = BTreeMap::new();
    for row in rows {
        buckets
            .entry((row.role.clone(), row.model.clone()))
            .or_default()
            .push(row);
    }
    buckets
        .into_iter()
        .map(|((role, model), observations)| metric_for(role, model, &observations))
        .collect()
}

fn metric_for(
    role: String,
    model: String,
    observations: &[&RoleModelObservation],
) -> RoleModelMetric {
    let attempts = observations.len() as u64;
    let mut completed = 0u64;
    let mut held = 0u64;
    let mut failed = 0u64;
    let mut interrupted = 0u64;
    let mut uncaptured_outcomes = 0u64;
    let mut rework_attempts = 0u64;
    let mut route_observed = 0u64;
    let mut route_mismatches = 0u64;
    let mut priced_attempts = 0u64;
    let mut total_cost_micros = 0u64;
    let mut evidence_refs: Vec<String> = Vec::new();
    for observation in observations {
        match observation.outcome {
            Some(RunOutcome::Completed) => completed += 1,
            Some(RunOutcome::Held) => held += 1,
            Some(RunOutcome::Failed) => failed += 1,
            Some(RunOutcome::Interrupted) => interrupted += 1,
            None => uncaptured_outcomes += 1,
        }
        if observation.attempt > 1 {
            rework_attempts += 1;
        }
        if let Some(matched) = observation.route_matched {
            route_observed += 1;
            if !matched {
                route_mismatches += 1;
            }
        }
        if let Some(cost) = observation.cost_micros {
            priced_attempts += 1;
            total_cost_micros = total_cost_micros.saturating_add(cost);
        }
        if !observation.evidence_ref.trim().is_empty()
            && !evidence_refs.contains(&observation.evidence_ref)
        {
            evidence_refs.push(observation.evidence_ref.clone());
        }
    }
    evidence_refs.sort();
    let captured_outcomes = attempts.saturating_sub(uncaptured_outcomes);
    let completion_rate =
        (captured_outcomes > 0).then(|| completed as f64 / captured_outcomes as f64);
    let rework_rate = (attempts > 0).then(|| rework_attempts as f64 / attempts as f64);
    let routing_accuracy = (route_observed > 0)
        .then(|| (route_observed - route_mismatches) as f64 / route_observed as f64);
    let (total_cost_micros, mean_cost_micros) = if priced_attempts > 0 {
        (
            Some(total_cost_micros),
            Some(total_cost_micros as f64 / priced_attempts as f64),
        )
    } else {
        (None, None)
    };
    let cost_coverage = (attempts > 0).then(|| priced_attempts as f64 / attempts as f64);
    RoleModelMetric {
        role,
        model,
        attempts,
        completed,
        held,
        failed,
        interrupted,
        uncaptured_outcomes,
        rework_attempts,
        completion_rate,
        rework_rate,
        route_observed,
        route_mismatches,
        routing_accuracy,
        priced_attempts,
        unpriced_attempts: attempts.saturating_sub(priced_attempts),
        total_cost_micros,
        mean_cost_micros,
        cost_coverage,
        evidence_refs,
    }
}

fn rate_text(rate: Option<f64>) -> String {
    match rate {
        Some(rate) => format!("{:.1}%", rate * 100.0),
        None => "NOT CAPTURED".to_string(),
    }
}

/// One line per bucket, for a CLI/report. `None` prints NOT CAPTURED, never `0%`.
pub fn render_role_model_metrics(metrics: &[RoleModelMetric]) -> String {
    if metrics.is_empty() {
        return "role/model metrics: NOT CAPTURED (no attempt rows)\n".to_string();
    }
    let mut out = String::new();
    for metric in metrics {
        out.push_str(&format!(
            "{}/{}: attempts={} completed={} held={} failed={} interrupted={} uncaptured={} rework={} completion={} rework_rate={} route_accuracy={} (observed {}) cost={} coverage={} unpriced={}\n",
            metric.role,
            metric.model,
            metric.attempts,
            metric.completed,
            metric.held,
            metric.failed,
            metric.interrupted,
            metric.uncaptured_outcomes,
            metric.rework_attempts,
            rate_text(metric.completion_rate),
            rate_text(metric.rework_rate),
            rate_text(metric.routing_accuracy),
            metric.route_observed,
            metric.total_cost_micros.map_or("NOT CAPTURED".to_string(), |micros| {
                format!("${:.6}", micros as f64 / 1_000_000.0)
            }),
            rate_text(metric.cost_coverage),
            metric.unpriced_attempts,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        role: &str,
        model: &str,
        outcome: Option<RunOutcome>,
        attempt: u32,
        route_matched: Option<bool>,
        cost_micros: Option<u64>,
    ) -> RoleModelObservation {
        RoleModelObservation {
            role: role.into(),
            model: model.into(),
            outcome,
            attempt,
            route_matched,
            cost_micros,
            evidence_ref: format!("storyboard_story_run:{role}:{model}:{attempt}"),
        }
    }

    #[test]
    fn empty_input_is_not_zero_metrics() {
        assert!(role_model_metrics(&[]).is_empty());
        assert!(render_role_model_metrics(&[]).contains("NOT CAPTURED"));
    }

    #[test]
    fn an_unpriced_bucket_reports_no_cost_rather_than_zero() {
        let metrics = role_model_metrics(&[observation(
            "smith",
            "deepseek-chat",
            Some(RunOutcome::Completed),
            1,
            None,
            None,
        )]);
        let metric = &metrics[0];
        assert_eq!(metric.priced_attempts, 0);
        assert_eq!(metric.unpriced_attempts, 1);
        assert_eq!(metric.total_cost_micros, None, "unpriced is not free");
        assert_eq!(metric.mean_cost_micros, None);
        assert_eq!(
            metric.cost_coverage,
            Some(0.0),
            "coverage IS known: 0 priced"
        );
        assert!(render_role_model_metrics(&metrics).contains("NOT CAPTURED"));
    }

    #[test]
    fn an_uncaptured_outcome_is_not_a_failure() {
        let metrics = role_model_metrics(&[
            observation("lead", "deepseek-chat", None, 1, None, Some(10)),
            observation(
                "lead",
                "deepseek-chat",
                Some(RunOutcome::Completed),
                2,
                None,
                Some(20),
            ),
        ]);
        let metric = &metrics[0];
        assert_eq!(metric.attempts, 2);
        assert_eq!(metric.uncaptured_outcomes, 1);
        assert_eq!(metric.failed, 0, "a missing outcome is not a failure");
        assert_eq!(
            metric.completion_rate,
            Some(1.0),
            "1 of 1 captured completed"
        );
        assert_eq!(
            metric.failed
                + metric.completed
                + metric.held
                + metric.interrupted
                + metric.uncaptured_outcomes,
            2
        );
    }

    #[test]
    fn a_bucket_with_no_captured_outcome_has_no_completion_rate() {
        let metrics =
            role_model_metrics(&[observation("qa", "deepseek-chat", None, 1, None, None)]);
        assert_eq!(metrics[0].completion_rate, None);
        assert_eq!(metrics[0].routing_accuracy, None);
    }

    #[test]
    fn routing_accuracy_needs_an_observed_route() {
        let unobserved = role_model_metrics(&[observation(
            "smith",
            "a",
            Some(RunOutcome::Completed),
            1,
            None,
            None,
        )]);
        assert_eq!(unobserved[0].route_observed, 0);
        assert_eq!(
            unobserved[0].routing_accuracy, None,
            "no route observed is not 100%"
        );

        let observed = role_model_metrics(&[
            observation(
                "smith",
                "a",
                Some(RunOutcome::Completed),
                1,
                Some(true),
                None,
            ),
            observation("smith", "a", Some(RunOutcome::Failed), 2, Some(false), None),
            observation(
                "smith",
                "a",
                Some(RunOutcome::Completed),
                3,
                Some(true),
                None,
            ),
        ]);
        assert_eq!(observed[0].route_observed, 3);
        assert_eq!(observed[0].route_mismatches, 1);
        assert_eq!(observed[0].routing_accuracy, Some(2.0 / 3.0));
    }

    #[test]
    fn rework_counts_attempts_after_the_first() {
        let metrics = role_model_metrics(&[
            observation("smith", "a", Some(RunOutcome::Failed), 1, None, None),
            observation("smith", "a", Some(RunOutcome::Completed), 2, None, None),
            observation("smith", "a", Some(RunOutcome::Completed), 3, None, None),
        ]);
        assert_eq!(metrics[0].rework_attempts, 2);
        assert_eq!(metrics[0].rework_rate, Some(2.0 / 3.0));
    }

    #[test]
    fn grouped_by_role_and_model_deterministically_and_cost_is_summed() {
        let metrics = role_model_metrics(&[
            observation(
                "smith",
                "b",
                Some(RunOutcome::Completed),
                1,
                None,
                Some(1_500_000),
            ),
            observation("lead", "a", Some(RunOutcome::Held), 1, None, Some(500_000)),
            observation("smith", "b", Some(RunOutcome::Completed), 2, None, None),
        ]);
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].role, "lead", "sorted by role");
        assert_eq!(metrics[0].model, "a");
        assert_eq!(metrics[1].role, "smith");
        assert_eq!(metrics[1].model, "b");
        assert_eq!(metrics[1].attempts, 2);
        assert_eq!(metrics[1].priced_attempts, 1);
        assert_eq!(metrics[1].total_cost_micros, Some(1_500_000));
        assert_eq!(metrics[1].cost_coverage, Some(0.5));
        assert_eq!(metrics[1].evidence_refs.len(), 2);
        let again = role_model_metrics(&[
            observation("lead", "a", Some(RunOutcome::Held), 1, None, Some(500_000)),
            observation(
                "smith",
                "b",
                Some(RunOutcome::Completed),
                1,
                None,
                Some(1_500_000),
            ),
            observation("smith", "b", Some(RunOutcome::Completed), 2, None, None),
        ]);
        assert_eq!(metrics, again, "same rows in another order, same report");
    }

    #[test]
    fn render_prints_not_captured_for_missing_fields() {
        let metrics = role_model_metrics(&[observation("smith", "a", None, 1, None, None)]);
        let rendered = render_role_model_metrics(&metrics);
        assert!(rendered.contains("completion=NOT CAPTURED"), "{rendered}");
        assert!(
            rendered.contains("route_accuracy=NOT CAPTURED"),
            "{rendered}"
        );
        assert!(rendered.contains("cost=NOT CAPTURED"), "{rendered}");
    }
}
