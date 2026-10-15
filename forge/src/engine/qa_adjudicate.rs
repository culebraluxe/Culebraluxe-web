//! Remainder of workflow_app/forge/agents/qa/run.ts — acceptance + negative control.

use crate::engine::assay::CMD_BUILD_FAIL;
use crate::engine::assay::{is_build_failure_output, AssayVerdict, CommandResult};
use crate::engine::qa_assert::{assertion_resolution, AssertionResolution};
use crate::engine::qa_plan::{
    ApprovedAssayCommand, ApprovedAssayPlan, ApprovedAssertionCheck, CheckAggregation,
};

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
    pub negative_control_confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckObservation {
    Passed,
    Failed,
    Skipped,
    Absent(String),
    BuildFailed,
    TimedOut,
    Cancelled,
    Unmeasurable,
}

/// Resolve a check only against the output of its planned command and runner. This prevents a
/// matching test name in another command's log from satisfying the check.
pub fn resolve_planned_check(
    command: &ApprovedAssayCommand,
    check: &ApprovedAssertionCheck,
    result: Option<&CommandResult>,
) -> CheckObservation {
    let Some(result) = result else {
        return CheckObservation::Absent("command_not_run".into());
    };
    if crate::engine::assay::is_cmd_cancelled(result) {
        return CheckObservation::Cancelled;
    }
    if crate::engine::assay::is_cmd_timeout(result) {
        return CheckObservation::TimedOut;
    }
    if is_build_failure_output(&result.output) || is_build_failure_output(&result.excerpt) {
        return CheckObservation::BuildFailed;
    }
    if result.unmeasurable {
        return CheckObservation::Unmeasurable;
    }
    if command.runner == crate::engine::qa_plan::AssayRunner::RustLibtest
        && rust_libtest_ran_zero_tests(&result.output)
    {
        return CheckObservation::Absent("zero_tests_ran".into());
    }
    match command.runner {
        crate::engine::qa_plan::AssayRunner::CommandExit => {
            if check.assertion != "exit_code_zero" {
                CheckObservation::Absent("unsupported_command_exit_contract".into())
            } else if result.passed {
                CheckObservation::Passed
            } else {
                CheckObservation::Failed
            }
        }
        crate::engine::qa_plan::AssayRunner::RustLibtest => {
            rust_libtest_check(&result.output, &check.assertion)
        }
        crate::engine::qa_plan::AssayRunner::Tap => {
            if !result
                .output
                .lines()
                .any(|line| line.trim() == "TAP version 13")
                || !result
                    .output
                    .lines()
                    .any(|line| line.trim().starts_with("1.."))
            {
                return CheckObservation::Absent("tap_plan_missing".into());
            }
            map_assertion_resolution(assertion_resolution(&result.output, &check.assertion))
        }
        crate::engine::qa_plan::AssayRunner::Junit => {
            let trimmed = result.output.trim_start();
            if !(trimmed.starts_with("<?xml")
                || trimmed.starts_with("<testsuite")
                || trimmed.starts_with("<testsuites"))
            {
                return CheckObservation::Absent("junit_document_missing".into());
            }
            map_assertion_resolution(assertion_resolution(&result.output, &check.assertion))
        }
    }
}

fn rust_libtest_ran_zero_tests(output: &str) -> bool {
    fn count(line: &str, word: &str) -> Option<u64> {
        let at = line.find(word)?;
        let digits: String = line[..at]
            .trim_end()
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        digits.parse().ok()
    }
    let summaries = output
        .lines()
        .filter(|line| line.trim_start().starts_with("test result:"))
        .collect::<Vec<_>>();
    !summaries.is_empty()
        && summaries.iter().all(|summary| {
            count(summary, "passed").unwrap_or(0) == 0 && count(summary, "failed").unwrap_or(0) == 0
        })
}

fn rust_libtest_check(output: &str, expected: &str) -> CheckObservation {
    let (wanted_file, wanted_name) = crate::engine::qa_assert::parse_assertion_ref(expected);
    let mut matched = Vec::new();
    for line in output.lines() {
        let Some(rest) = line.trim().strip_prefix("test ") else {
            continue;
        };
        let (name, verdict) = if let Some(name) = rest.strip_suffix(" ... ok") {
            (name, CheckObservation::Passed)
        } else if let Some(name) = rest.strip_suffix(" ... FAILED") {
            (name, CheckObservation::Failed)
        } else if let Some(name) = rest.strip_suffix(" ... ignored") {
            (name, CheckObservation::Skipped)
        } else {
            continue;
        };
        let name_matches = name == wanted_name || name.ends_with(&format!("::{wanted_name}"));
        let file_matches = wanted_file.as_deref().is_none_or(|file| {
            name.starts_with(file)
                || file
                    .rsplit('/')
                    .next()
                    .is_some_and(|base| name.contains(base))
        });
        if name_matches && file_matches {
            matched.push(verdict);
        }
    }
    match matched.as_slice() {
        [one] => one.clone(),
        [] => CheckObservation::Absent("assertion_not_observed".into()),
        _ => CheckObservation::Absent("assertion_ambiguous".into()),
    }
}

fn map_assertion_resolution(resolution: AssertionResolution) -> CheckObservation {
    match resolution {
        AssertionResolution::Passed => CheckObservation::Passed,
        AssertionResolution::Failed => CheckObservation::Failed,
        AssertionResolution::Skipped => CheckObservation::Skipped,
        AssertionResolution::Absent { reason, detail } => {
            CheckObservation::Absent(format!("{reason:?}:{detail}"))
        }
    }
}

/// Deterministic production adjudication for a frozen plan. Commands and assertions retain their
/// plan identity throughout; there is no command-text acceptance fallback.
pub fn adjudicate_frozen_qa(plan: &ApprovedAssayPlan, results: &[CommandResult]) -> QaReport {
    let mut blockers = Vec::new();
    if plan.commands.is_empty() {
        blockers.push("NO_ASSAY_COMMANDS".into());
    }
    if results.len() != plan.commands.len() {
        blockers.push("ASSAY_COMMAND_DRIFT result_cardinality".into());
    }
    let by_command: std::collections::HashMap<&str, &CommandResult> = results
        .iter()
        .map(|result| (result.command.as_str(), result))
        .collect();
    for planned in &plan.commands {
        if !by_command.contains_key(planned.command.as_str()) {
            blockers.push(format!("ASSAY_COMMAND_NOT_OBSERVED {}", planned.id));
        }
    }
    for (planned, result) in plan.commands.iter().zip(results) {
        if planned.command != result.command {
            blockers.push(format!(
                "ASSAY_COMMAND_SUBSTITUTED {} observed={}",
                planned.id, result.command
            ));
        }
    }
    for result in results {
        if !plan
            .commands
            .iter()
            .any(|planned| planned.command == result.command)
        {
            blockers.push(format!("ASSAY_COMMAND_UNPLANNED {}", result.command));
        }
    }

    let checks: std::collections::HashMap<&str, CheckObservation> = plan
        .checks
        .iter()
        .map(|check| {
            let command = plan
                .commands
                .iter()
                .find(|command| command.id == check.command_id);
            let result =
                command.and_then(|command| by_command.get(command.command.as_str()).copied());
            let observation = command
                .map(|command| resolve_planned_check(command, check, result))
                .unwrap_or_else(|| CheckObservation::Absent("planned_command_missing".into()));
            (check.id.as_str(), observation)
        })
        .collect();

    let mut failed_conditions = Vec::new();
    let mut unproven = Vec::new();
    for condition in &plan.conditions {
        let outcomes: Vec<_> = condition
            .check_ids
            .iter()
            .map(|id| checks.get(id.as_str()))
            .collect();
        let passed = outcomes
            .iter()
            .filter(|o| matches!(o, Some(CheckObservation::Passed)))
            .count();
        let failed = outcomes
            .iter()
            .filter(|o| matches!(o, Some(CheckObservation::Failed)))
            .count();
        let unresolved = outcomes.len().saturating_sub(passed + failed);
        let pass = match condition.aggregation {
            CheckAggregation::AllRequired => passed == outcomes.len() && !outcomes.is_empty(),
            CheckAggregation::AnyOf => passed > 0,
        };
        if pass {
            continue;
        }
        if failed > 0 && (condition.aggregation == CheckAggregation::AllRequired || unresolved == 0)
        {
            failed_conditions.push(condition.id.clone());
            blockers.push(format!("ASSERTION_FAILED {}", condition.id));
        } else {
            unproven.push(condition.id.clone());
            blockers.push(format!("UNPROVEN {}", condition.id));
            for (check_id, observation) in condition.check_ids.iter().zip(outcomes) {
                if !matches!(
                    observation,
                    Some(CheckObservation::Passed | CheckObservation::Failed)
                ) {
                    blockers.push(format!(
                        "ASSERTION_NOT_PROVEN {} {check_id} {:?}",
                        condition.id, observation
                    ));
                }
            }
        }
    }
    let mut negative_control_passed = false;
    let mut negative_control_command = None;
    if let Some(control) = &plan.negative_control {
        let planned_command = plan
            .commands
            .iter()
            .find(|command| command.id == control.command_id);
        let control_result =
            planned_command.and_then(|command| by_command.get(command.command.as_str()).copied());
        negative_control_command = planned_command.map(|command| command.command.as_str());
        let expected_failed = !control.expected_failed_check_ids.is_empty()
            && control.expected_failed_check_ids.iter().all(|check_id| {
                matches!(
                    checks.get(check_id.as_str()),
                    Some(CheckObservation::Failed)
                )
            });
        let command_measurable = control_result.is_some_and(|result| {
            !result.unmeasurable
                && !crate::engine::assay::is_cmd_timeout(result)
                && !crate::engine::assay::is_cmd_cancelled(result)
                && !is_build_failure_output(&result.output)
                && !is_build_failure_output(&result.excerpt)
        });
        if expected_failed && command_measurable {
            negative_control_passed = true;
        } else {
            blockers.push(format!("NEGATIVE_CONTROL_UNPROVEN {}", control.command_id));
            unproven.push(format!("negative-control:{}", control.command_id));
        }
    }
    let command_failure = results.iter().any(|result| {
        !result.passed
            && !result.unmeasurable
            && !is_build_failure_output(&result.output)
            && !is_build_failure_output(&result.excerpt)
            && !(negative_control_passed
                && negative_control_command == Some(result.command.as_str()))
    });
    let build_failure = results.iter().any(|result| {
        is_build_failure_output(&result.output) || is_build_failure_output(&result.excerpt)
    });
    let timed_out = results.iter().any(crate::engine::assay::is_cmd_timeout);
    let cancelled = results.iter().any(crate::engine::assay::is_cmd_cancelled);
    let other_unmeasurable = results.iter().any(|result| {
        result.unmeasurable
            && !crate::engine::assay::is_cmd_timeout(result)
            && !crate::engine::assay::is_cmd_cancelled(result)
    });
    if command_failure {
        blockers.push("CMD_FAIL".into());
    }
    if build_failure {
        blockers.push(crate::engine::assay::CMD_BUILD_FAIL.into());
    }
    if timed_out {
        blockers.push("CMD_TIMEOUT".into());
    }
    if cancelled {
        blockers.push("CMD_CANCELLED".into());
    }
    if other_unmeasurable {
        blockers.push("COMMAND_UNMEASURABLE".into());
    }

    let verdict = if command_failure || !failed_conditions.is_empty() {
        AssayVerdict::Fail
    } else if build_failure
        || timed_out
        || cancelled
        || other_unmeasurable
        || !unproven.is_empty()
        || !blockers.is_empty()
    {
        AssayVerdict::Unproven
    } else {
        AssayVerdict::Pass
    };
    QaReport {
        verdict,
        blockers,
        unproven,
        failed_conditions,
        negative_control_confirmed: negative_control_passed,
    }
}

#[derive(Debug, Clone)]
pub struct FrozenQaJudgments {
    pub product: QaReport,
    pub test_artifact: QaReport,
}

pub fn adjudicate_frozen_judgments(
    plan: &ApprovedAssayPlan,
    results: &[CommandResult],
) -> FrozenQaJudgments {
    let product = adjudicate_judgment_subset(
        plan,
        results,
        crate::engine::qa_plan::AcceptanceJudgment::Product,
        "PRODUCT_ACCEPTANCE_PLAN_MISSING",
    );
    let test_artifact = adjudicate_judgment_subset(
        plan,
        results,
        crate::engine::qa_plan::AcceptanceJudgment::TestArtifact,
        "TEST_ARTIFACT_PLAN_MISSING",
    );
    FrozenQaJudgments {
        product,
        test_artifact,
    }
}

fn adjudicate_judgment_subset(
    plan: &ApprovedAssayPlan,
    results: &[CommandResult],
    judgment: crate::engine::qa_plan::AcceptanceJudgment,
    missing_blocker: &str,
) -> QaReport {
    let mut subset = plan.clone();
    subset
        .conditions
        .retain(|condition| condition.judgment == judgment);
    if subset.conditions.is_empty() {
        return QaReport {
            verdict: AssayVerdict::Unproven,
            blockers: vec![missing_blocker.into()],
            unproven: vec![],
            failed_conditions: vec![],
            negative_control_confirmed: false,
        };
    }
    let check_ids: std::collections::HashSet<&str> = subset
        .conditions
        .iter()
        .flat_map(|condition| condition.check_ids.iter().map(String::as_str))
        .collect();
    let check_ids: std::collections::HashSet<&str> = subset
        .negative_control
        .as_ref()
        .into_iter()
        .flat_map(|control| control.expected_failed_check_ids.iter().map(String::as_str))
        .chain(check_ids)
        .collect();
    subset
        .checks
        .retain(|check| check_ids.contains(check.id.as_str()));
    let command_ids: std::collections::HashSet<&str> = subset
        .checks
        .iter()
        .map(|check| check.command_id.as_str())
        .collect();
    let command_ids: std::collections::HashSet<&str> = subset
        .negative_control
        .as_ref()
        .map(|control| control.command_id.as_str())
        .into_iter()
        .chain(command_ids)
        .collect();
    subset
        .commands
        .retain(|command| command_ids.contains(command.id.as_str()));
    let command_texts: std::collections::HashSet<&str> = subset
        .commands
        .iter()
        .map(|command| command.command.as_str())
        .collect();
    let scoped_results: Vec<CommandResult> = results
        .iter()
        .filter(|result| command_texts.contains(result.command.as_str()))
        .cloned()
        .collect();
    adjudicate_frozen_qa(&subset, &scoped_results)
}

pub fn acceptance_map_changed(
    frozen: &[AcceptanceCondition],
    current: &[AcceptanceCondition],
) -> bool {
    if frozen.len() != current.len() {
        return true;
    }
    frozen
        .iter()
        .zip(current)
        .any(|(a, b)| a.id != b.id || a.assertions != b.assertions)
}

pub fn adjudicate_negative_control(
    control: &NegativeControl,
    result: Option<&CommandResult>,
    conditions: &[AcceptanceCondition],
) -> (bool, bool, bool, Vec<String>) {
    // missing, unmeasurable, survived, killing
    let refs: Vec<String> = if control.assertions.is_empty() {
        conditions
            .iter()
            .flat_map(|c| c.assertions.clone())
            .collect()
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
        .filter(|r| {
            matches!(
                assertion_resolution(&result.output, r),
                AssertionResolution::Failed
            )
        })
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
            blockers.push(if crate::engine::assay::is_cmd_timeout(c) {
                // A ceiling kill reads as a kill, not as a generic unmeasurable command: the claim was
                // held until the ceiling fired, and the story requeues on this blocker (FORGE-FIX-005).
                format!("CMD_TIMEOUT {}", c.command)
            } else if crate::engine::assay::is_cmd_cancelled(c) {
                format!("CMD_CANCELLED {}", c.command)
            } else if c.unmeasurable {
                format!("CMD_UNMEASURABLE {}", c.command)
            } else if is_build_failure_output(&c.output) || is_build_failure_output(&c.excerpt) {
                // A build failure is measured, but it is NOT a test failure: the toolchain never
                // ran the tests. Distinct blocker so it never records as one (FORGE-FIX-003).
                format!("{CMD_BUILD_FAIL} {}", c.command)
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
                    arch.migration_findings
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "migration lint did not run".into())
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
        .map(|c| {
            if c.output.is_empty() {
                c.excerpt.as_str()
            } else {
                c.output.as_str()
            }
        })
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
        let failed: Vec<_> = outcomes
            .iter()
            .filter(|(_, r)| matches!(r, AssertionResolution::Failed))
            .collect();
        if !failed.is_empty() {
            failed_conditions.push(condition.id.clone());
            for (r#ref, _) in failed {
                blockers.push(format!("ASSERTION_FAILED {} {ref}", condition.id));
            }
            continue;
        }
        // Conditions are all-required by default. A vector of assertion references does not
        // imply any-of: every required assertion must have an observed pass.
        if outcomes
            .iter()
            .all(|(_, r)| matches!(r, AssertionResolution::Passed))
        {
            continue;
        }
        unproven.push(condition.id.clone());
        blockers.push(format!("UNPROVEN {}", condition.id));
        for (r#ref, res) in outcomes {
            match res {
                AssertionResolution::Skipped => {
                    blockers.push(format!("ASSERTION_SKIPPED {} {ref}", condition.id));
                }
                AssertionResolution::Absent { reason, detail }
                    if !matches!(reason, crate::engine::qa_assert::AbsentReason::NotRun) =>
                {
                    blockers.push(format!(
                        "ASSERTION_ORIGIN_UNMET {} {ref} — {detail}",
                        condition.id
                    ));
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
        let (missing, unmeas, survived, _) =
            adjudicate_negative_control(control, negative_result, &plan.conditions);
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
        (b.starts_with("CMD_") && !b.starts_with(CMD_BUILD_FAIL) && !b.starts_with("CMD_CANCELLED"))
            || b.starts_with("ASSAY_COMMAND_SUBSTITUTED")
            || b.starts_with("ARCH ")
            || b == "NO_ASSAY_COMMANDS"
            || b == "ASSAY_COMMAND_DRIFT"
    });
    let negative_failure = negative_missing || negative_unmeasurable;
    // A build failure is not a test failure: it escalates (UNPROVEN) instead of failing the
    // story or burning the repair budget on a toolchain fault (FORGE-FIX-003).
    let build_failure = blockers.iter().any(|b| b.starts_with(CMD_BUILD_FAIL));
    let verdict = if command_failure
        || migration_failure
        || !failed_conditions.is_empty()
        || negative_failure
    {
        AssayVerdict::Fail
    } else if !unproven.is_empty()
        || build_failure
        || commands.iter().any(crate::engine::assay::is_cmd_cancelled)
        || blockers.iter().any(|b| b == "ACCEPTANCE_MAP_CHANGED")
        || negative_survived
    {
        AssayVerdict::Unproven
    } else if !blockers.is_empty() {
        AssayVerdict::Fail
    } else {
        AssayVerdict::Pass
    };

    QaReport {
        verdict,
        blockers,
        unproven,
        failed_conditions,
        negative_control_confirmed: false,
    }
}

#[cfg(test)]
mod tests;
