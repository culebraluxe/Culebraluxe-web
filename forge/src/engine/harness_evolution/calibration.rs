//! Council calibration (Phase 3 TD 3.15): how well do Council/owner verdicts predict outcomes?
//!
//! The durable decision rows say what was decided; a later outcome says whether it was right. This
//! module is the pure scoring half: verdict + a **stated probability** + an **observed outcome** in,
//! a Brier-style score out.
//!
//! HONESTY RULES:
//!
//! * **No invented probabilities.** A sample without a stated probability is not scored and not
//!   assumed to be 50/50; it is counted in `missing_probabilities`. A probability outside `0..=1`
//!   (or non-finite) is a typed error, not clamped.
//! * **A missing outcome is not a loss.** A sample with no observed outcome is counted in
//!   `missing_outcomes` and excluded from the score. If nothing can be scored, the score is `None`
//!   — never `0.0`, which would read as perfect calibration.
//! * **Per-verdict buckets** so "Council was right" and "owner was right" are not blended: a
//!   `Council` verdict and a `Reject` verdict answered different questions.
//!
//! Brier score definition: mean of `(p - o)^2` over scored samples, where `p` is the probability
//! the verdict was the right call and `o ∈ {0,1}` is whether it proved right. Lower is better; the
//! score is `None` when no sample has both a valid probability and an observed outcome.

use super::model::AdoptionVerdict;

/// One decision's calibration sample.
#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationSample {
    pub decision_id: String,
    pub verdict: AdoptionVerdict,
    /// The stated probability that the verdict was the right call, when one was stated.
    pub stated_probability: Option<f64>,
    /// `Some(true)` = the verdict proved right; `Some(false)` = it proved wrong; `None` = outcome
    /// not observed yet.
    pub outcome: Option<bool>,
    pub evidence_refs: Vec<String>,
}

impl CalibrationSample {
    pub fn new(decision_id: impl Into<String>, verdict: AdoptionVerdict) -> Self {
        Self {
            decision_id: decision_id.into(),
            verdict,
            stated_probability: None,
            outcome: None,
            evidence_refs: Vec::new(),
        }
    }

    pub fn with_probability(mut self, probability: f64) -> Self {
        self.stated_probability = Some(probability);
        self
    }

    pub fn with_outcome(mut self, outcome: bool) -> Self {
        self.outcome = Some(outcome);
        self
    }

    pub fn with_evidence_refs(mut self, evidence_refs: Vec<String>) -> Self {
        self.evidence_refs = evidence_refs;
        self
    }
}

/// Why a calibration report could not be produced. A malformed probability is a refusal, not a
/// clamp: silently treating `1.2` as `1.0` would invent certainty the decider never stated.
#[derive(Debug, Clone, PartialEq)]
pub enum CalibrationError {
    ProbabilityOutOfRange {
        decision_id: String,
        probability: f64,
    },
}

impl std::fmt::Display for CalibrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProbabilityOutOfRange {
                decision_id,
                probability,
            } => write!(
                f,
                "decision {decision_id} states probability {probability}, outside 0..=1"
            ),
        }
    }
}

impl std::error::Error for CalibrationError {}

/// Calibration for one verdict class.
#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationBucket {
    pub verdict: AdoptionVerdict,
    pub samples: usize,
    pub scored: usize,
    pub missing_outcomes: usize,
    pub missing_probabilities: usize,
    /// `None` when no sample in this bucket had both a probability and an outcome.
    pub brier: Option<f64>,
}

/// Whole-report calibration.
#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationReport {
    pub samples: usize,
    pub scored: usize,
    pub missing_outcomes: usize,
    pub missing_probabilities: usize,
    /// `None` when nothing was scorable.
    pub brier: Option<f64>,
    /// One bucket per verdict that has at least one sample, in `AdoptionVerdict::ALL` order.
    pub buckets: Vec<CalibrationBucket>,
}

fn validate_probability(sample: &CalibrationSample) -> Result<(), CalibrationError> {
    if let Some(probability) = sample.stated_probability {
        if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
            return Err(CalibrationError::ProbabilityOutOfRange {
                decision_id: sample.decision_id.clone(),
                probability,
            });
        }
    }
    Ok(())
}

fn brier_over(samples: &[&CalibrationSample]) -> Option<f64> {
    let mut scored = 0usize;
    let mut total = 0.0f64;
    for sample in samples {
        let (Some(probability), Some(outcome)) = (sample.stated_probability, sample.outcome) else {
            continue;
        };
        let observed = if outcome { 1.0 } else { 0.0 };
        total += (probability - observed).powi(2);
        scored += 1;
    }
    (scored > 0).then(|| total / scored as f64)
}

/// The Brier-style score over all scorable samples, or `None` when none are scorable.
pub fn brier_score(samples: &[CalibrationSample]) -> Result<Option<f64>, CalibrationError> {
    for sample in samples {
        validate_probability(sample)?;
    }
    let refs: Vec<&CalibrationSample> = samples.iter().collect();
    Ok(brier_over(&refs))
}

/// Build the full report, including per-verdict buckets and the missing-data counts.
pub fn calibration_report(
    samples: &[CalibrationSample],
) -> Result<CalibrationReport, CalibrationError> {
    for sample in samples {
        validate_probability(sample)?;
    }
    let mut buckets: Vec<CalibrationBucket> = Vec::new();
    for verdict in AdoptionVerdict::ALL {
        let in_bucket: Vec<&CalibrationSample> = samples
            .iter()
            .filter(|sample| sample.verdict == verdict)
            .collect();
        if in_bucket.is_empty() {
            continue;
        }
        buckets.push(CalibrationBucket {
            verdict,
            samples: in_bucket.len(),
            scored: in_bucket
                .iter()
                .filter(|sample| sample.stated_probability.is_some() && sample.outcome.is_some())
                .count(),
            missing_outcomes: in_bucket
                .iter()
                .filter(|sample| sample.outcome.is_none())
                .count(),
            missing_probabilities: in_bucket
                .iter()
                .filter(|sample| sample.stated_probability.is_none())
                .count(),
            brier: brier_over(&in_bucket),
        });
    }
    let refs: Vec<&CalibrationSample> = samples.iter().collect();
    Ok(CalibrationReport {
        samples: samples.len(),
        scored: samples
            .iter()
            .filter(|sample| sample.stated_probability.is_some() && sample.outcome.is_some())
            .count(),
        missing_outcomes: samples
            .iter()
            .filter(|sample| sample.outcome.is_none())
            .count(),
        missing_probabilities: samples
            .iter()
            .filter(|sample| sample.stated_probability.is_none())
            .count(),
        brier: brier_over(&refs),
        buckets,
    })
}

impl CalibrationReport {
    /// Human line for the operator surface. A missing score prints NOT SCORED, never `0.0`.
    pub fn summary_line(&self) -> String {
        let brier = self
            .brier
            .map_or("NOT SCORED".to_string(), |brier| format!("{brier:.4}"));
        format!(
            "council calibration: brier={brier} scored={} samples={} missing_outcomes={} missing_probabilities={}",
            self.scored, self.samples, self.missing_outcomes, self.missing_probabilities
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scored(decision: &str, verdict: AdoptionVerdict, probability: f64, right: bool) -> CalibrationSample {
        CalibrationSample::new(decision, verdict)
            .with_probability(probability)
            .with_outcome(right)
    }

    #[test]
    fn a_perfectly_confident_correct_verdict_scores_zero() {
        let samples = vec![scored("d1", AdoptionVerdict::Adopt, 1.0, true)];
        assert_eq!(brier_score(&samples).unwrap(), Some(0.0));
        let report = calibration_report(&samples).unwrap();
        assert_eq!(report.brier, Some(0.0));
        assert_eq!(report.scored, 1);
        assert_eq!(report.missing_outcomes, 0);
        assert_eq!(report.missing_probabilities, 0);
    }

    #[test]
    fn a_wrong_verdict_contributes_its_squared_error() {
        let samples = vec![
            scored("d1", AdoptionVerdict::Adopt, 0.9, true),   // 0.01
            scored("d2", AdoptionVerdict::Reject, 0.7, false), // 0.49
        ];
        let expected = (0.01 + 0.49) / 2.0;
        let score = brier_score(&samples).unwrap().expect("scored");
        assert!((score - expected).abs() < 1e-12, "{score} vs {expected}");
    }

    #[test]
    fn a_missing_outcome_is_excluded_and_counted_not_scored_as_loss() {
        let samples = vec![
            scored("d1", AdoptionVerdict::Council, 0.8, true),
            CalibrationSample::new("d2", AdoptionVerdict::Council).with_probability(0.2),
        ];
        let report = calibration_report(&samples).unwrap();
        assert_eq!(report.samples, 2);
        assert_eq!(report.scored, 1);
        assert_eq!(report.missing_outcomes, 1);
        assert!(
            (report.brier.unwrap() - 0.04).abs() < 1e-12,
            "only the observed outcome is scored: {:?}",
            report.brier
        );
        let bucket = &report.buckets[0];
        assert_eq!(bucket.verdict, AdoptionVerdict::Council);
        assert_eq!(bucket.missing_outcomes, 1);
    }

    #[test]
    fn a_missing_probability_is_not_imputed_as_fifty_fifty() {
        let samples = vec![
            CalibrationSample::new("d1", AdoptionVerdict::Adopt).with_outcome(true),
            scored("d2", AdoptionVerdict::Adopt, 0.5, true),
        ];
        let report = calibration_report(&samples).unwrap();
        assert_eq!(report.scored, 1);
        assert_eq!(report.missing_probabilities, 1);
        assert!(
            (report.brier.unwrap() - 0.25).abs() < 1e-12,
            "only the stated 0.5 is scored: {:?}",
            report.brier
        );
    }

    #[test]
    fn nothing_scorable_means_no_score_rather_than_zero() {
        let samples = vec![
            CalibrationSample::new("d1", AdoptionVerdict::Adopt),
            CalibrationSample::new("d2", AdoptionVerdict::Chris).with_probability(0.5),
        ];
        let report = calibration_report(&samples).unwrap();
        assert_eq!(report.brier, None);
        assert_eq!(report.scored, 0);
        assert_eq!(report.buckets.len(), 2);
        assert!(report.buckets.iter().all(|bucket| bucket.brier.is_none()));
        assert!(report.summary_line().contains("NOT SCORED"));
        assert_eq!(brier_score(&samples).unwrap(), None);
    }

    #[test]
    fn a_probability_outside_zero_to_one_is_a_typed_refusal() {
        let samples = vec![scored("d1", AdoptionVerdict::Adopt, 1.2, true)];
        let error = calibration_report(&samples).unwrap_err();
        assert!(matches!(
            error,
            CalibrationError::ProbabilityOutOfRange { .. }
        ));
        assert!(brier_score(&samples).is_err());
        let nan = vec![scored("d2", AdoptionVerdict::Adopt, f64::NAN, true)];
        assert!(calibration_report(&nan).is_err(), "NaN is not a probability");
    }

    #[test]
    fn verdicts_are_bucketed_separately_in_canonical_order() {
        let samples = vec![
            scored("d1", AdoptionVerdict::Reject, 0.6, true),
            scored("d2", AdoptionVerdict::Adopt, 0.9, false),
            scored("d3", AdoptionVerdict::Adopt, 0.8, true),
        ];
        let report = calibration_report(&samples).unwrap();
        assert_eq!(
            report.buckets.iter().map(|bucket| bucket.verdict).collect::<Vec<_>>(),
            vec![AdoptionVerdict::Adopt, AdoptionVerdict::Reject],
            "bucket order follows AdoptionVerdict::ALL"
        );
        let adopt = &report.buckets[0];
        assert_eq!(adopt.samples, 2);
        assert_eq!(adopt.scored, 2);
        // 0.9 stated, proved wrong → 0.81; 0.8 stated, proved right → 0.04.
        assert!((adopt.brier.unwrap() - (0.81 + 0.04) / 2.0).abs() < 1e-12);
        let reject = &report.buckets[1];
        assert!((reject.brier.unwrap() - 0.16).abs() < 1e-12);
        // The whole-report score blends the buckets with equal per-sample weight, not per-bucket.
        assert!((report.brier.unwrap() - (0.81 + 0.04 + 0.16) / 3.0).abs() < 1e-12);
    }

    #[test]
    fn a_verdict_with_no_samples_has_no_bucket() {
        let report = calibration_report(&[scored("d1", AdoptionVerdict::Council, 0.5, true)]).unwrap();
        assert_eq!(report.buckets.len(), 1);
        assert_eq!(report.buckets[0].verdict, AdoptionVerdict::Council);
    }
}
