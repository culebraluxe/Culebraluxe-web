//! Remainder of workflow_app/forge/agents/qa/run.ts — acceptance + negative control.

use crate::engine::assay::{AssayVerdict, CommandResult};
use crate::engine::qa_assert::{assertion_resolution, AssertionResolution};

#[derive(Debug, Clone, Default)]
pub struct AcceptanceCondition {
    pub id: String,
    pub assertions: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NegativeControl {
    pub command: String,
    pub assertions: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AssayPlan {
    pub commands: Vec<String>,
    pub conditions: Vec<AcceptanceCondition>,
    pub negative_control: Option<NegativeControl>,
}

#[derive(Debug, Clone, Default)]
pub struct StaticSlice {
    pub arch_ran: bool,
    pub arch_ok: bool,
    pub arch_errors: Vec<String>,
    pub migration_ok: Option<bool>,
    pub migration_ran: Option<bool>,
    pub migration_findings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct QaReport {
    pub verdict: AssayVerdict,
    pub blockers: Vec<String>,
    pub unproven: Vec<String>,
    pub failed_conditions: Vec<String>,
}

pub fn acceptance_map_changed(frozen: &[AcceptanceCondition], current: &[AcceptanceCondition]) -> bool {
    if frozen.len() != current.len() {
        return true;
    }
    frozen.iter().zip(current).any(|(a, b)| a.id != b.id || a.assertions != b.assertions)
}

pub fn adjudicate_negative_control(
    control: &NegativeControl,
    result: Option<&CommandResult>,
    conditions: &[AcceptanceCondition],
) -> (bool, bool, bool, Vec<String>) {
    // missing, unmeasurable, survived, killing
    let refs: Vec<String> = if control.assertions.is_empty() {
        conditions.iter().flat_map(|c| c.assertions.clone()).collect()
    } else {
        control.assertions.clone()
    };
    let Some(result) = result else {
        return (true, false, false, vec![]);
    };
    if result.unmeasurable {
        return (false, true, false, vec![]);
    }
    let killing: Vec<String> = refs
        .into_iter()
        .filter(|r| matches!(assertion_resolution(&result.output, r), AssertionResolution::Failed))
        .collect();
    (false, false, killing.is_empty(), killing)
}

pub fn adjudicate_qa(
    plan: &AssayPlan,
    commands: &[CommandResult],
    static_gate: Option<&StaticSlice>,
    frozen_map: Option<&[AcceptanceCondition]>,
    negative_result: Option<&CommandResult>,
) -> QaReport {
    let mut blockers = Vec::new();
    if plan.commands.is_empty() {
        blockers.push("NO_ASSAY_COMMANDS".into());
    }
    if commands.len() != plan.commands.len() {
        blockers.push("ASSAY_COMMAND_DRIFT".into());
    } else {
        for (i, planned) in plan.commands.iter().enumerate() {
            if commands[i].command != *planned {
                blockers.push(format!("ASSAY_COMMAND_SUBSTITUTED {}", commands[i].command));
            }
        }
    }
    for c in commands {
        if !c.passed {
            blockers.push(if c.unmeasurable {
                format!("CMD_UNMEASURABLE {}", c.command)
            } else {
                format!("CMD_FAIL {}", c.command)
            });
        }
    }
    let mut migration_failure = false;
    if let Some(arch) = static_gate {
        if arch.arch_ran && !arch.arch_ok {
            for e in arch.arch_errors.iter().take(8) {
                blockers.push(format!("ARCH {e}"));
            }
        }
        if arch.migration_ok == Some(false) {
            migration_failure = true;
            if arch.migration_ran != Some(true) {
                blockers.push(format!(
                    "MIGRATION_UNMEASURABLE {}",
                    arch.migration_findings.first().cloned().unwrap_or_else(|| "migration lint did not run".into())
                ));
            } else if arch.migration_findings.is_empty() {
                blockers.push("MIGRATION migration lint reported a failure".into());
            } else {
                for f in arch.migration_findings.iter().take(8) {
                    blockers.push(format!("MIGRATION {f}"));
                }
            }
        }
    }

    let proof: String = commands
        .iter()
        .filter(|c| !c.unmeasurable)
        .map(|c| if c.output.is_empty() { c.excerpt.as_str() } else { c.output.as_str() })
        .collect::<Vec<_>>()
        .join("\n");

    let mut unproven = Vec::new();
    let mut failed_conditions = Vec::new();
    for condition in &plan.conditions {
        if condition.assertions.is_empty() {
            unproven.push(condition.id.clone());
            blockers.push(format!("UNPROVEN {}", condition.id));
            continue;
        }
        let outcomes: Vec<_> = condition
            .assertions
            .iter()
            .map(|r| (r.clone(), assertion_resolution(&proof, r)))
            .collect();
        let failed: Vec<_> = outcomes.iter().filter(|(_, r)| matches!(r, AssertionResolution::Failed)).collect();
        if !failed.is_empty() {
            failed_conditions.push(condition.id.clone());
            for (r#ref, _) in failed {
                blockers.push(format!("ASSERTION_FAILED {} {ref}", condition.id));
            }
            continue;
        }
        if outcomes.iter().any(|(_, r)| matches!(r, AssertionResolution::Passed)) {
            continue;
        }
        unproven.push(condition.id.clone());
        blockers.push(format!("UNPROVEN {}", condition.id));
        for (r#ref, res) in outcomes {
            match res {
                AssertionResolution::Skipped => {
                    blockers.push(format!("ASSERTION_SKIPPED {} {ref}", condition.id));
                }
                AssertionResolution::Absent { reason, detail } if !matches!(reason, crate::engine::qa_assert::AbsentReason::NotRun) => {
                    blockers.push(format!("ASSERTION_ORIGIN_UNMET {} {ref} — {detail}", condition.id));
                }
                _ => blockers.push(format!("ASSERTION_NOT_RUN {} {ref}", condition.id)),
            }
        }
    }

    if let Some(frozen) = frozen_map {
        if acceptance_map_changed(frozen, &plan.conditions) {
            blockers.push("ACCEPTANCE_MAP_CHANGED".into());
        }
    }

    let mut negative_missing = false;
    let mut negative_unmeasurable = false;
    let mut negative_survived = false;
    if let Some(control) = &plan.negative_control {
        let (missing, unmeas, survived, _) = adjudicate_negative_control(control, negative_result, &plan.conditions);
        negative_missing = missing;
        negative_unmeasurable = unmeas;
        negative_survived = survived;
        if missing {
            blockers.push(format!("NEGATIVE_CONTROL_MISSING {}", control.command));
        } else if unmeas {
            blockers.push(format!("NEGATIVE_CONTROL_UNMEASURABLE {}", control.command));
        } else if survived {
            blockers.push(format!("NEGATIVE_CONTROL_SURVIVED {}", control.command));
        }
    }

    let command_failure = blockers.iter().any(|b| {
        b.starts_with("CMD_")
            || b.starts_with("ASSAY_COMMAND_SUBSTITUTED")
            || b.starts_with("ARCH ")
            || b == "NO_ASSAY_COMMANDS"
            || b == "ASSAY_COMMAND_DRIFT"
    });
    let negative_failure = negative_missing || negative_unmeasurable;
    let verdict = if command_failure || migration_failure || !failed_conditions.is_empty() || negative_failure {
        AssayVerdict::Fail
    } else if !unproven.is_empty() || blockers.iter().any(|b| b == "ACCEPTANCE_MAP_CHANGED") || negative_survived {
        AssayVerdict::Unproven
    } else if !blockers.is_empty() {
        AssayVerdict::Fail
    } else {
        AssayVerdict::Pass
    };

    QaReport { verdict, blockers, unproven, failed_conditions }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::assay::CommandResult;

    fn cmd(name: &str, output: &str, passed: bool) -> CommandResult {
        CommandResult {
            command: name.into(),
            exit_code: if passed { 0 } else { 1 },
            passed,
            excerpt: output.into(),
            unmeasurable: false,
            output: output.into(),
        }
    }

    #[test]
    fn unmapped_clause_is_unproven() {
        let plan = AssayPlan {
            commands: vec!["npm test".into()],
            conditions: vec![AcceptanceCondition { id: "C1".into(), assertions: vec![] }],
            negative_control: None,
        };
        let report = adjudicate_qa(&plan, &[cmd("npm test", "ok 1 - x\n", true)], None, None, None);
        assert_eq!(report.verdict, AssayVerdict::Unproven);
        assert!(report.blockers.iter().any(|b| b == "UNPROVEN C1"));
    }
}
