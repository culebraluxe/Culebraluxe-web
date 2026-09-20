//! Port of qa-classify-line.ts.

pub const ENGINE_FAILURE_CLASSES: &[&str] = &[
    "CODE_DEFECT", "TEST_DEFECT", "ARCHITECTURE_GAP", "REQUIREMENTS_GAP",
    "UNKNOWN_CAUSE", "ENVIRONMENT", "MIGRATION", "PUBLISH_CONFLICT",
    "DEPLOYMENT", "PRODUCTION_SMOKE", "HOLD",
];

pub fn parse_failure_class(notes: Option<&str>) -> Option<String> {
    let notes = notes?;
    let line = notes.lines().map(str::trim).find(|s| s.starts_with("FAILURE_CLASS:"))?;
    let value = line["FAILURE_CLASS:".len()..].trim().to_ascii_uppercase();
    ENGINE_FAILURE_CLASSES.iter().copied().find(|c| *c == value).map(|s| s.to_string())
}

pub fn build_classify_directive(evaluated_sha: &str, blockers: &[String], excerpts: &[String]) -> String {
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
