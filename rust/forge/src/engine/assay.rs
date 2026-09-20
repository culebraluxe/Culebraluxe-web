//! Assay adjudicate + collect.

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::phase::RoleEffectPorts;

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub command: String,
    pub exit_code: i32,
    pub passed: bool,
    pub excerpt: String,
    pub unmeasurable: bool,
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

pub fn adjudicate_assay(commands: &[String], results: &[CommandResult], acceptance_mapped: bool) -> AssayReport {
    if commands.is_empty() {
        return AssayReport { verdict: AssayVerdict::Fail, blockers: vec!["NO_ASSAY_COMMANDS"] };
    }
    if results.iter().any(|r| r.unmeasurable) {
        return AssayReport { verdict: AssayVerdict::Fail, blockers: vec!["COMMAND_UNMEASURABLE"] };
    }
    if results.iter().any(|r| !r.passed) {
        return AssayReport { verdict: AssayVerdict::Fail, blockers: vec!["CMD_FAIL"] };
    }
    if !acceptance_mapped {
        return AssayReport { verdict: AssayVerdict::Unproven, blockers: vec!["ACCEPTANCE_MAP_MISSING"] };
    }
    AssayReport { verdict: AssayVerdict::Pass, blockers: vec![] }
}

pub fn collect_assay_evidence(
    mut evidence: ForgeGateEvidence,
    _ports: &RoleEffectPorts,
    run_command: Option<&dyn Fn(&str) -> CommandResult>,
    assay_commands: &[String],
    acceptance_mapped: bool,
) -> ForgeGateEvidence {
    let Some(run) = run_command else {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA FAIL: the lane was handed assay commands but no way to run them.".into());
        return evidence;
    };
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
        return evidence;
    }
    evidence.qa_passed = Some(true);
    evidence
}
