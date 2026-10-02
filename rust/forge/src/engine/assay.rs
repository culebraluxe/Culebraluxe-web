//! Port of agents/qa/run.ts adjudicate + assay-collect.ts (no SHA conjunct).

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::phase::RoleEffectPorts;

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub command: String,
    pub exit_code: i32,
    pub passed: bool,
    pub excerpt: String,
    pub unmeasurable: bool,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssayVerdict {
    Pass,
    Fail,
    Unproven,
}

#[derive(Debug, Clone)]
pub struct AssayReport {
    pub verdict: AssayVerdict,
    pub blockers: Vec<&'static str>,
}

pub fn adjudicate_assay(
    commands: &[String],
    results: &[CommandResult],
    acceptance_mapped: bool,
) -> AssayReport {
    if commands.is_empty() {
        return AssayReport {
            verdict: AssayVerdict::Fail,
            blockers: vec!["NO_ASSAY_COMMANDS"],
        };
    }
    if results.iter().any(|r| r.unmeasurable) {
        return AssayReport {
            verdict: AssayVerdict::Fail,
            blockers: vec!["COMMAND_UNMEASURABLE"],
        };
    }
    if results.iter().any(|r| !r.passed) {
        return AssayReport {
            verdict: AssayVerdict::Fail,
            blockers: vec!["CMD_FAIL"],
        };
    }
    if !acceptance_mapped {
        return AssayReport {
            verdict: AssayVerdict::Unproven,
            blockers: vec!["ACCEPTANCE_MAP_MISSING"],
        };
    }
    AssayReport {
        verdict: AssayVerdict::Pass,
        blockers: vec![],
    }
}

fn is_rust_contract_runtime_test(command: &str) -> bool {
    let command = command.trim_start();
    command == "cargo test"
        || command.starts_with("cargo test ")
        || command == "cargo nextest"
        || command.starts_with("cargo nextest ")
}

/// The roots that hold PRODUCTION code. A test-authoring story may not change anything under these: the
/// deliverable of a RUST_CONTRACT story is a TEST ARTIFACT, never a product fix.
///
/// A test that fails against this code is a FINDING to be scheduled as its own product work. Moving, relaxing
/// or silencing production code so a test turns green destroys the very evidence the story exists to add — the
/// point of the test is to FIND the bug, not to hide it.
const PRODUCTION_ROOTS: [&str; 6] = [
    "rust/core/",
    "rust/forge/",
    "rust/server/",
    "rust/integrations/",
    "rust/cli/",
    "rust/ui/",
];

pub fn is_rust_contract_production_path(path: &str) -> bool {
    let path = path.trim();
    PRODUCTION_ROOTS.iter().any(|root| path.starts_with(root))
}

/// The production files changed by the exact Smith execution range. QA deliberately uses the SAME
/// base..candidate range Smith validates, so a repair commit cannot hide a production edit made by an earlier
/// Smith attempt in the same story workspace.
pub fn rust_contract_production_edits(
    base_sha: Option<&str>,
    candidate_sha: Option<&str>,
    run: &dyn Fn(&str) -> CommandResult,
) -> Result<Vec<String>, String> {
    let base = base_sha
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "QA FAIL: RUST_CONTRACT execution base is missing.".to_string())?;
    let candidate = candidate_sha
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "QA FAIL: RUST_CONTRACT exact candidate SHA is missing.".to_string())?;

    // `--no-renames`: with rename detection a file moved out of a production root lists only its new path, and
    // the move would read as a test-only change.
    let command = format!("git diff --name-only --no-renames {base}..{candidate}");
    let listed = run(&command);
    if listed.unmeasurable || !listed.passed {
        return Err(format!(
            "QA FAIL: RUST_CONTRACT could not measure exact candidate range {base}..{candidate}."
        ));
    }

    let mut found: Vec<String> = listed
        .output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && is_rust_contract_production_path(line))
        .map(str::to_string)
        .collect();
    found.sort();
    found.dedup();
    Ok(found)
}

/// Test-authoring stories measure two different facts:
///
/// 1. Is the authored test artifact structurally valid and compilable? This is the QA gate.
/// 2. Does the current application satisfy the assertion the new test expresses? This is an observation.
///
/// A newly-authored regression/contract test is allowed to expose a real product defect. Rewriting that test until
/// it turns green destroys the evidence the story was created to add. Therefore runtime test failures are recorded
/// in `last_failure` but do not fail the authoring gate when the non-runtime assay checks (normally
/// `cargo check --workspace --all-targets`) are clean.
pub fn collect_rust_contract_assay_evidence(
    mut evidence: ForgeGateEvidence,
    run_command: Option<&dyn Fn(&str) -> CommandResult>,
    assay_commands: &[String],
    acceptance_mapped: bool,
) -> AssayEvidence {
    let Some(run) = run_command else {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA FAIL: the RUST_CONTRACT lane had no command runner.".into());
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    };

    if assay_commands.is_empty() {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA FAIL: RUST_CONTRACT has no assay commands.".into());
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    }

    let results: Vec<CommandResult> = assay_commands.iter().map(|command| run(command)).collect();

    if results.iter().any(|result| result.unmeasurable) {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA FAIL: RUST_CONTRACT assay command was unmeasurable.".into());
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    }

    if !acceptance_mapped {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA UNPROVEN: RUST_CONTRACT acceptance mapping is missing.".into());
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Unproven,
        };
    }

    let structural: Vec<&CommandResult> = results
        .iter()
        .filter(|result| !is_rust_contract_runtime_test(&result.command))
        .collect();
    if structural.is_empty() {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection = Some(
            "QA FAIL: RUST_CONTRACT needs a non-runtime authoring check (for example cargo check --all-targets)."
                .into(),
        );
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    }

    let structural_failures: Vec<String> = structural
        .iter()
        .filter(|result| !result.passed)
        .map(|result| result.command.clone())
        .collect();
    if !structural_failures.is_empty() {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection = Some(format!(
            "QA FAIL: RUST_CONTRACT authoring checks failed=[{}]",
            structural_failures.join(" | ")
        ));
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    }

    // THE TEST-ONLY RULE. A test-authoring story's deliverable is a test; production code is not its to move.
    // This is an AUTHORING defect, so it lives in the structural gate that is allowed to fail — never in the
    // runtime bucket below, where a failing test is legitimate evidence about the application.
    let base_sha = evidence
        .extra
        .get("recordedBase")
        .and_then(|value| value.as_str());
    let production_edits =
        match rust_contract_production_edits(base_sha, evidence.candidate_sha.as_deref(), run) {
            Ok(edits) => edits,
            Err(reason) => {
                evidence.qa_passed = Some(false);
                evidence.deliverable_rejection = Some(reason);
                return AssayEvidence {
                    evidence,
                    verdict: AssayVerdict::Fail,
                };
            }
        };
    if !production_edits.is_empty() {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection = Some(format!(
            "QA FAIL: RUST_CONTRACT artifact modified production code; the deliverable is a test, not a product \
             fix. Commit the failing test as the finding and schedule the product fix as its own story. touched=[{}]",
            production_edits.join(" | ")
        ));
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    }

    let runtime_failures: Vec<String> = results
        .iter()
        .filter(|result| is_rust_contract_runtime_test(&result.command) && !result.passed)
        .map(|result| result.command.clone())
        .collect();

    evidence.qa_passed = Some(true);
    evidence.deliverable_rejection = None;
    if !runtime_failures.is_empty() {
        evidence.last_failure = Some(format!(
            "RUST_CONTRACT observation: authored test currently fails against existing application code; \
             test artifact accepted and product debugging is separate work. failed=[{}]",
            runtime_failures.join(" | ")
        ));
    }

    AssayEvidence {
        evidence,
        verdict: AssayVerdict::Pass,
    }
}

/// The QA lane's reading, with the durable evidence it produced.
///
/// The two are kept apart on purpose: `verdict` is the assay's **own three-way reading** (`PASS`/`FAIL`/`UNPROVEN`,
/// the token the `forge_tool_artifact` row carries), while `evidence.qa_passed` is the gate's boolean. Collapsing
/// "not proven" into "failed" inside the evidence is the gate's business; it must not erase what the lane measured.
pub struct AssayEvidence {
    pub evidence: ForgeGateEvidence,
    pub verdict: AssayVerdict,
}

pub fn collect_assay_evidence(
    mut evidence: ForgeGateEvidence,
    ports: &RoleEffectPorts,
    run_command: Option<&dyn Fn(&str) -> CommandResult>,
    assay_commands: &[String],
    acceptance_mapped: bool,
) -> AssayEvidence {
    if run_command.is_none() {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA FAIL: the lane was handed assay commands but no way to run them.".into());
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    }
    let run = run_command.unwrap();
    let results: Vec<CommandResult> = assay_commands.iter().map(|c| run(c)).collect();
    let report = adjudicate_assay(assay_commands, &results, acceptance_mapped);
    if report.verdict != AssayVerdict::Pass {
        let failed: Vec<String> = results
            .iter()
            .filter(|r| !r.passed && !r.unmeasurable)
            .map(|r| r.command.clone())
            .collect();
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection = Some(format!(
            "QA {:?}: blockers=[{}] failed=[{}]",
            report.verdict,
            report.blockers.join(", "),
            failed.join(" | ")
        ));
        return AssayEvidence {
            evidence,
            verdict: report.verdict,
        };
    }
    evidence.qa_passed = Some(true);
    AssayEvidence {
        evidence,
        verdict: AssayVerdict::Pass,
    }
}

#[cfg(test)]
mod rust_contract_tests {
    use super::*;

    fn result(command: &str, passed: bool) -> CommandResult {
        CommandResult {
            command: command.into(),
            exit_code: if passed { 0 } else { 101 },
            passed,
            excerpt: String::new(),
            unmeasurable: false,
            output: String::new(),
        }
    }

    fn contract_gate() -> ForgeGateEvidence {
        let mut gate = ForgeGateEvidence {
            candidate_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
            ..ForgeGateEvidence::default()
        };
        gate.extra.insert(
            "recordedBase",
            workflow::Value::from("89abcdef0123456789abcdef0123456789abcdef"),
        );
        gate
    }

    #[test]
    fn runtime_failure_is_product_evidence_not_test_authoring_failure() {
        let commands = vec![
            "cargo test --manifest-path rust/Cargo.toml -p test-harness --test contract"
                .to_string(),
            "cargo check --manifest-path rust/Cargo.toml --workspace --all-targets".to_string(),
        ];
        let evidence = collect_rust_contract_assay_evidence(
            contract_gate(),
            Some(&|command| {
                if command.starts_with("cargo test") {
                    result(command, false)
                } else {
                    result(command, true)
                }
            }),
            &commands,
            true,
        );
        assert_eq!(evidence.verdict, AssayVerdict::Pass);
        assert_eq!(evidence.evidence.qa_passed, Some(true));
        assert!(evidence.evidence.deliverable_rejection.is_none());
        assert!(evidence
            .evidence
            .last_failure
            .as_deref()
            .unwrap_or_default()
            .contains("product debugging is separate work"));
    }

    #[test]
    fn test_authoring_story_may_not_move_production_code() {
        let commands = vec![
            "cargo check --manifest-path rust/Cargo.toml --workspace --all-targets".to_string(),
        ];
        let gate = contract_gate();
        let evidence = collect_rust_contract_assay_evidence(
            gate,
            Some(&|command| {
                if command.starts_with("git diff --name-only") {
                    // An earlier commit in the Smith execution range touched production code; QA must still see it.
                    CommandResult {
                        command: command.into(),
                        exit_code: 0,
                        passed: true,
                        excerpt: String::new(),
                        unmeasurable: false,
                        output: "rust/test-harness/tests/wf_human_task__001__candidate_claim.rs\n\
                                 rust/core/workflow/src/engine/engine_options.rs\n"
                            .into(),
                    }
                } else {
                    result(command, true)
                }
            }),
            &commands,
            true,
        );
        assert_eq!(evidence.verdict, AssayVerdict::Fail);
        assert_eq!(evidence.evidence.qa_passed, Some(false));
        let rejection = evidence.evidence.deliverable_rejection.unwrap_or_default();
        assert!(rejection.contains("modified production code"));
        assert!(rejection.contains("engine_options.rs"));
    }

    #[test]
    fn production_edit_check_uses_the_whole_execution_range() {
        let gate = contract_gate();
        let observed = std::sync::Mutex::new(String::new());
        let commands = vec![
            "cargo check --manifest-path rust/Cargo.toml --workspace --all-targets".to_string(),
        ];
        let evidence = collect_rust_contract_assay_evidence(
            gate,
            Some(&|command| {
                if command.starts_with("git diff --name-only") {
                    *observed.lock().expect("range observation lock") = command.to_string();
                }
                result(command, true)
            }),
            &commands,
            true,
        );
        assert_eq!(evidence.verdict, AssayVerdict::Pass);
        assert_eq!(
            observed.lock().expect("range observation lock").as_str(),
            "git diff --name-only --no-renames 89abcdef0123456789abcdef0123456789abcdef..0123456789abcdef0123456789abcdef01234567"
        );
    }

    #[test]
    fn authoring_check_failure_still_blocks_the_story() {
        let commands = vec![
            "cargo test --manifest-path rust/Cargo.toml -p test-harness --test contract"
                .to_string(),
            "cargo check --manifest-path rust/Cargo.toml --workspace --all-targets".to_string(),
        ];
        let evidence = collect_rust_contract_assay_evidence(
            ForgeGateEvidence::default(),
            Some(&|command| result(command, command.starts_with("cargo test"))),
            &commands,
            true,
        );
        assert_eq!(evidence.verdict, AssayVerdict::Fail);
        assert_eq!(evidence.evidence.qa_passed, Some(false));
        assert!(evidence
            .evidence
            .deliverable_rejection
            .as_deref()
            .unwrap_or_default()
            .contains("authoring checks failed"));
    }
}
