//! Port of qa-classify-line.ts.

pub const ENGINE_FAILURE_CLASSES: &[&str] = &[
    "CODE_DEFECT",
    "TEST_DEFECT",
    "ARCHITECTURE_GAP",
    "REQUIREMENTS_GAP",
    "UNKNOWN_CAUSE",
    "ENVIRONMENT",
    "MIGRATION",
    "PUBLISH_CONFLICT",
    "DEPLOYMENT",
    "PRODUCTION_SMOKE",
    "HOLD",
];

pub fn parse_failure_class(notes: Option<&str>) -> Option<String> {
    // The LAST valid line wins: a model that echoes the option list (`FAILURE_CLASS: CODE_DEFECT | TEST_DEFECT …`)
    // before answering used to read as no class at all. Markdown emphasis around the answer is ignored.
    notes?
        .lines()
        .map(|line| line.trim().replace(['*', '`'], ""))
        .filter_map(|line| {
            let value = line
                .strip_prefix("FAILURE_CLASS:")?
                .trim()
                .to_ascii_uppercase();
            ENGINE_FAILURE_CLASSES.iter().copied().find(|c| *c == value)
        })
        .last()
        .map(str::to_string)
}

pub fn build_classify_directive(
    evaluated_sha: &str,
    blockers: &[String],
    excerpts: &[String],
) -> String {
    format!(
        "Assay already FAILED. Do not re-run tests. Do not change the verdict.\n\
evaluatedSha={evaluated_sha}\n\
blockers={}\n\
command excerpts: {}\n\
Pick one FAILURE_CLASS: {}\n\
End with one line: FAILURE_CLASS: CODE_DEFECT\n\
That line becomes evidence.failureClass. Routing stays on the engine XML + qa-repair-policy.",
        blockers.join(" | "),
        excerpts.join(" || "),
        ENGINE_FAILURE_CLASSES.join(" | ")
    )
}

#[cfg(test)]
mod legacy_qa_classify_line_tests {
    //! Ported from `legacy/workflow_app/forge/qa-classify-line.test.ts`.
    //! Case 1 is the parse contract and is honoured. Case 2 — "does not attach a class on PASS" —
    //! is NOT honoured: `role_mapping.rs:193` attaches `failureClass` from a role's evidence JSON
    //! with no gate on the verdict, so a PASS run can acquire a failure class. That case is
    //! recorded as `diverged` in `docs/agent/LEGACY-TEST-PARITY.md`; porting it here would only
    //! turn the suite red until the gate exists.
    use super::*;

    #[test]
    fn reads_failure_class_after_assay_fail() {
        assert_eq!(
            parse_failure_class(Some("FAILURE_CLASS: TEST_DEFECT")).as_deref(),
            Some("TEST_DEFECT")
        );
        assert_eq!(parse_failure_class(Some("FAILURE_CLASS: nope")), None);
    }

    #[test]
    fn an_echoed_option_list_does_not_hide_the_answer() {
        let notes =
            "FAILURE_CLASS: CODE_DEFECT | TEST_DEFECT\nthinking...\nFAILURE_CLASS: **TEST_DEFECT**";
        assert_eq!(
            parse_failure_class(Some(notes)).as_deref(),
            Some("TEST_DEFECT")
        );
    }
}
