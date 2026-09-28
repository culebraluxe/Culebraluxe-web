//! The QA run/verdict agreement check — one fact, one writer, in Rust.
//!
//! Rust home of the pure half of `workflow_app/forge/forge-qa-consistency.ts` (deleted with the TypeScript
//! application in `4cf98110`), used by `forge doctor`.
//!
//! A QA run's own `result_status` on `storyboard_story_run` and the durable
//! `forge_workflow_evidence.qa_passed` verdict are two readings of the same fact, so when they disagree the
//! disagreement IS the bug — the class found on 2026-09-16, where a QA lane that held still had a verdict
//! recorded against it.
//!
//! PURE, and READ-ONLY: this is a report, not a repair. Nothing here writes, no clock is read, and a run
//! with no verdict reports `unknown` — never `agree`.

/// A durable verdict, as it comes off the evidence row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QaVerdict {
    Pass,
    Fail,
}

impl QaVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
        }
    }
}

/// The agreement reading for one QA run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QaConsistency {
    Agree {
        run_status: String,
        verdict: QaVerdict,
    },
    Disagree {
        run_status: Option<String>,
        verdict: QaVerdict,
        detail: String,
    },
    Unknown {
        reason: String,
    },
}

impl QaConsistency {
    pub fn state(&self) -> &'static str {
        match self {
            Self::Agree { .. } => "agree",
            Self::Disagree { .. } => "disagree",
            Self::Unknown { .. } => "unknown",
        }
    }
}

/// A verdict that cannot be read is NOT a FAIL — it is no verdict at all. `true` and `false` are the durable
/// boolean this reads; the `PASS`/`FAIL` token spelling goes through [`normalize_qa_verdict_token`].
pub fn normalize_qa_verdict(value: Option<bool>) -> Option<QaVerdict> {
    match value {
        Some(true) => Some(QaVerdict::Pass),
        Some(false) => Some(QaVerdict::Fail),
        None => None,
    }
}

/// The token spelling of a verdict, case-insensitive. Anything else reads as no verdict.
pub fn normalize_qa_verdict_token(value: Option<&str>) -> Option<QaVerdict> {
    match value.map(|token| token.trim().to_uppercase()) {
        Some(token) if token == "PASS" => Some(QaVerdict::Pass),
        Some(token) if token == "FAIL" => Some(QaVerdict::Fail),
        _ => None,
    }
}

/// The verdict a run's own status implies. `Complete` ⇒ PASS and `Failed` ⇒ FAIL. Any other status (Hold,
/// Cancelled, blank, absent) makes no clean pass/fail claim, so it implies nothing — a run that did not
/// complete cleanly cannot certify a pass.
pub fn expected_verdict_for_run_status(run_status: Option<&str>) -> Option<QaVerdict> {
    match run_status.map(|status| status.trim().to_lowercase()) {
        Some(status) if status == "complete" => Some(QaVerdict::Pass),
        Some(status) if status == "failed" => Some(QaVerdict::Fail),
        _ => None,
    }
}

/// The engine's own QA-lane rule: a run whose `run_type` is the qa/assay family. Kept here so the doctor
/// selects QA runs by one definition instead of a second list.
pub fn is_qa_run_type(run_type: Option<&str>) -> bool {
    match run_type.map(|value| value.trim().to_lowercase()) {
        Some(token) if !token.is_empty() => token.contains("qa") || token.contains("assay"),
        _ => false,
    }
}

/// The check. A run status and a verdict either agree, or the disagreement names BOTH values. A run with no
/// readable verdict is `unknown` — never `agree`.
pub fn check_qa_run_verdict_consistency(
    run_status: Option<&str>,
    verdict: Option<QaVerdict>,
) -> QaConsistency {
    let Some(verdict) = verdict else {
        return QaConsistency::Unknown {
            reason: "no durable QA verdict recorded for this run".to_string(),
        };
    };

    match expected_verdict_for_run_status(run_status) {
        None => QaConsistency::Disagree {
            run_status: run_status.map(str::to_string),
            verdict,
            detail: format!(
                "run status '{}' makes no clean pass/fail claim but the durable verdict is {}",
                describe_run_status(run_status),
                verdict.as_str()
            ),
        },
        Some(expected) if expected == verdict => QaConsistency::Agree {
            run_status: describe_run_status(run_status),
            verdict,
        },
        Some(expected) => QaConsistency::Disagree {
            run_status: run_status.map(str::to_string),
            verdict,
            detail: format!(
                "run status '{}' expects {} but the durable verdict is {}",
                describe_run_status(run_status),
                expected.as_str(),
                verdict.as_str()
            ),
        },
    }
}

/// One line for the doctor. A disagreement always names both values.
pub fn render_qa_consistency_line(result: &QaConsistency) -> String {
    match result {
        QaConsistency::Agree {
            run_status,
            verdict,
        } => format!("agree (run {run_status} / verdict {})", verdict.as_str()),
        QaConsistency::Disagree { detail, .. } => format!("DISAGREE — {detail}"),
        QaConsistency::Unknown { reason } => format!("unknown ({reason})"),
    }
}

fn describe_run_status(run_status: Option<&str>) -> String {
    match run_status.map(str::trim) {
        Some(token) if !token.is_empty() => token.to_string(),
        _ => "none".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_with_no_readable_verdict_is_unknown_and_never_agree() {
        let reading = check_qa_run_verdict_consistency(Some("Complete"), None);
        assert!(matches!(reading, QaConsistency::Unknown { .. }));
        assert!(render_qa_consistency_line(&reading).starts_with("unknown ("));
    }

    #[test]
    fn a_completed_run_with_a_pass_agrees_and_a_failed_run_with_a_pass_does_not() {
        assert!(matches!(
            check_qa_run_verdict_consistency(Some("Complete"), Some(QaVerdict::Pass)),
            QaConsistency::Agree { .. }
        ));
        let disagreement = check_qa_run_verdict_consistency(Some("Failed"), Some(QaVerdict::Pass));
        match &disagreement {
            QaConsistency::Disagree { detail, .. } => {
                // The line names BOTH readings, so a reader can weigh them without a second query.
                assert!(detail.contains("expects FAIL"));
                assert!(detail.contains("durable verdict is PASS"));
            }
            other => panic!("expected a disagreement, got {other:?}"),
        }
        assert!(render_qa_consistency_line(&disagreement).starts_with("DISAGREE — "));
    }

    #[test]
    fn a_hold_makes_no_clean_pass_claim_and_disagrees_with_any_verdict() {
        let reading = check_qa_run_verdict_consistency(Some("Hold"), Some(QaVerdict::Pass));
        match reading {
            QaConsistency::Disagree { detail, .. } => {
                assert!(detail.contains("makes no clean pass/fail claim"))
            }
            other => panic!("expected a disagreement, got {other:?}"),
        }
    }

    #[test]
    fn a_verdict_spelled_as_a_token_or_a_boolean_is_the_same_verdict() {
        assert_eq!(normalize_qa_verdict(Some(true)), Some(QaVerdict::Pass));
        assert_eq!(normalize_qa_verdict(Some(false)), Some(QaVerdict::Fail));
        assert_eq!(
            normalize_qa_verdict_token(Some(" pass ")),
            Some(QaVerdict::Pass)
        );
        assert_eq!(
            normalize_qa_verdict_token(Some("fail")),
            Some(QaVerdict::Fail)
        );
        assert_eq!(normalize_qa_verdict_token(Some("maybe")), None);
        assert_eq!(normalize_qa_verdict(None), None);
    }

    #[test]
    fn the_qa_lane_is_selected_by_one_definition_not_a_second_list() {
        assert!(is_qa_run_type(Some("QA")));
        assert!(is_qa_run_type(Some("qa-lane")));
        assert!(is_qa_run_type(Some("Assay")));
        assert!(!is_qa_run_type(Some("Smith")));
        assert!(!is_qa_run_type(None));
        assert!(!is_qa_run_type(Some("   ")));
    }
}
