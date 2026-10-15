// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use crate::engine::assay::CommandResult;
use crate::engine::qa_plan::{
    AcceptanceJudgment, ApprovedAcceptanceCondition, ApprovedAssayCommand, ApprovedAssayPlan,
    ApprovedAssertionCheck, AssayParser, AssayRunner, CheckAggregation,
};

fn cmd(name: &str, output: &str, passed: bool) -> CommandResult {
    CommandResult {
        cancelled: false,
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
        conditions: vec![AcceptanceCondition {
            id: "C1".into(),
            assertions: vec![],
        }],
        negative_control: None,
    };
    let report = adjudicate_qa(
        &plan,
        &[cmd("npm test", "ok 1 - x\n", true)],
        None,
        None,
        None,
    );
    assert_eq!(report.verdict, AssayVerdict::Unproven);
    assert!(report.blockers.iter().any(|b| b == "UNPROVEN C1"));
}

#[test]
fn one_pass_and_one_absent_required_assertion_stays_unproven() {
    let plan = AssayPlan {
        commands: vec!["cargo test".into()],
        conditions: vec![AcceptanceCondition {
            id: "C1".into(),
            assertions: vec!["present assertion".into(), "missing assertion".into()],
        }],
        negative_control: None,
    };
    let report = adjudicate_qa(
        &plan,
        &[cmd("cargo test", "ok 1 - present assertion\n", true)],
        None,
        None,
        None,
    );
    assert_eq!(report.verdict, AssayVerdict::Unproven);
    assert!(report.unproven.iter().any(|condition| condition == "C1"));
}

fn frozen_plan(aggregation: CheckAggregation) -> ApprovedAssayPlan {
    ApprovedAssayPlan {
        schema_version: 1,
        plan_id: "plan".into(),
        plan_version: 1,
        commands: vec![ApprovedAssayCommand {
            id: "cmd".into(),
            command: "cargo test -p sample".into(),
            runner: AssayRunner::RustLibtest,
            parser: AssayParser::RustLibtest,
            working_directory: "lane_root".into(),
            environment_identity: "forge-sanitized-assay-v2".into(),
        }],
        checks: vec![
            ApprovedAssertionCheck {
                id: "check-a".into(),
                command_id: "cmd".into(),
                assertion: "sample::first".into(),
            },
            ApprovedAssertionCheck {
                id: "check-b".into(),
                command_id: "cmd".into(),
                assertion: "sample::second".into(),
            },
        ],
        conditions: vec![ApprovedAcceptanceCondition {
            id: "AC-1".into(),
            check_ids: vec!["check-a".into(), "check-b".into()],
            aggregation,
            judgment: AcceptanceJudgment::Product,
        }],
        negative_control: None,
    }
}

fn measured_rust(output: &str) -> CommandResult {
    cmd(
        "cargo test -p sample",
        output,
        !output.contains(" ... FAILED"),
    )
}

#[test]
fn frozen_all_required_needs_every_assertion_and_any_of_is_explicit() {
    let output = "test sample::first ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out\n";
    let all = adjudicate_frozen_qa(
        &frozen_plan(CheckAggregation::AllRequired),
        &[measured_rust(output)],
    );
    assert_eq!(all.verdict, AssayVerdict::Unproven);

    let any = adjudicate_frozen_qa(
        &frozen_plan(CheckAggregation::AnyOf),
        &[measured_rust(output)],
    );
    assert_eq!(any.verdict, AssayVerdict::Pass);
}

#[test]
fn frozen_judgments_evaluate_and_record_the_approved_negative_control() {
    let mut plan = frozen_plan(CheckAggregation::AllRequired);
    plan.checks.push(ApprovedAssertionCheck {
        id: "control-check".into(),
        command_id: "control-command".into(),
        assertion: "sample::must_fail".into(),
    });
    plan.commands.push(ApprovedAssayCommand {
        id: "control-command".into(),
        command: "cargo test -p sample --negative-control".into(),
        runner: AssayRunner::RustLibtest,
        parser: AssayParser::RustLibtest,
        working_directory: "lane_root".into(),
        environment_identity: "forge-sanitized-assay-v2".into(),
    });
    plan.negative_control = Some(crate::engine::qa_plan::ApprovedNegativeControl {
        command_id: "control-command".into(),
        expected_failed_check_ids: vec!["control-check".into()],
    });
    let results = vec![
            measured_rust(
                "test sample::first ... ok\ntest sample::second ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
            ),
            cmd(
                "cargo test -p sample --negative-control",
                "test sample::must_fail ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n",
                false,
            ),
        ];

    let judgments = adjudicate_frozen_judgments(&plan, &results);
    assert_eq!(judgments.product.verdict, AssayVerdict::Pass);
    assert!(judgments.product.negative_control_confirmed);
    assert!(!judgments
        .product
        .blockers
        .iter()
        .any(|blocker| blocker == "NEGATIVE_CONTROL_CONFIRMED"));

    let mut surviving = results;
    surviving[1] = cmd(
            "cargo test -p sample --negative-control",
            "test sample::must_fail ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
            true,
        );
    let judgments = adjudicate_frozen_judgments(&plan, &surviving);
    assert_eq!(judgments.product.verdict, AssayVerdict::Unproven);
    assert!(!judgments.product.negative_control_confirmed);
}

#[test]
fn frozen_assertions_are_bound_to_their_command_and_structured_runner_output() {
    let echo = measured_rust("the output says sample::first passed\n");
    let report = adjudicate_frozen_qa(&frozen_plan(CheckAggregation::AllRequired), &[echo]);
    assert_eq!(report.verdict, AssayVerdict::Unproven);

    let substituted = cmd("cargo test -p another", "test sample::first ... ok\n", true);
    let report = adjudicate_frozen_qa(&frozen_plan(CheckAggregation::AllRequired), &[substituted]);
    assert_eq!(report.verdict, AssayVerdict::Unproven);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker.contains("ASSAY_COMMAND_SUBSTITUTED")));
}

#[test]
fn zero_test_success_and_skipped_required_check_are_unproven() {
    let zero = measured_rust(
        "test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out\n",
    );
    let report = adjudicate_frozen_qa(&frozen_plan(CheckAggregation::AnyOf), &[zero]);
    assert_eq!(report.verdict, AssayVerdict::Unproven);

    assert!(!rust_libtest_ran_zero_tests(
            "running 1 test\ntest sample::one ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n\n   Doc-tests forge\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
        ));
    assert!(rust_libtest_ran_zero_tests(
            "test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
        ));

    let skipped = measured_rust("test sample::first ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out\n");
    let report = adjudicate_frozen_qa(&frozen_plan(CheckAggregation::AllRequired), &[skipped]);
    assert_eq!(report.verdict, AssayVerdict::Unproven);
}

#[test]
fn cancelled_execution_is_recorded_separately_and_stays_unproven() {
    let mut result = measured_rust("partial output before cancellation");
    result.exit_code = crate::engine::assay::CMD_CANCELLED_EXIT;
    result.cancelled = true;
    result.passed = false;
    result.unmeasurable = true;
    result.excerpt = "CMD_CANCELLED: assay command received signal 15".into();
    let plan = frozen_plan(CheckAggregation::AllRequired);
    let report = adjudicate_frozen_qa(&plan, &[result.clone()]);
    assert_eq!(report.verdict, AssayVerdict::Unproven);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "CMD_CANCELLED"));
    assert_eq!(
        resolve_planned_check(&plan.commands[0], &plan.checks[0], Some(&result)),
        CheckObservation::Cancelled
    );
}

#[test]
fn valid_test_artifact_can_pass_while_product_assertion_failure_remains_visible() {
    use crate::engine::qa_plan::AcceptanceJudgment;
    let plan = ApprovedAssayPlan {
        schema_version: 1,
        plan_id: "contract-plan".into(),
        plan_version: 1,
        commands: vec![
            ApprovedAssayCommand {
                id: "compile".into(),
                command: "cargo check --all-targets".into(),
                runner: AssayRunner::CommandExit,
                parser: AssayParser::CommandExit,
                working_directory: "lane_root".into(),
                environment_identity: "forge-sanitized-assay-v2".into(),
            },
            ApprovedAssayCommand {
                id: "runtime".into(),
                command: "cargo test -p app".into(),
                runner: AssayRunner::RustLibtest,
                parser: AssayParser::RustLibtest,
                working_directory: "lane_root".into(),
                environment_identity: "forge-sanitized-assay-v2".into(),
            },
        ],
        checks: vec![
            ApprovedAssertionCheck {
                id: "artifact-check".into(),
                command_id: "compile".into(),
                assertion: "exit_code_zero".into(),
            },
            ApprovedAssertionCheck {
                id: "product-check".into(),
                command_id: "runtime".into(),
                assertion: "sample::behavior".into(),
            },
        ],
        conditions: vec![
            ApprovedAcceptanceCondition {
                id: "artifact".into(),
                check_ids: vec!["artifact-check".into()],
                aggregation: CheckAggregation::AllRequired,
                judgment: AcceptanceJudgment::TestArtifact,
            },
            ApprovedAcceptanceCondition {
                id: "product".into(),
                check_ids: vec!["product-check".into()],
                aggregation: CheckAggregation::AllRequired,
                judgment: AcceptanceJudgment::Product,
            },
        ],
        negative_control: None,
    };
    let results = vec![
            cmd("cargo check --all-targets", "", true),
            cmd(
                "cargo test -p app",
                "test sample::behavior ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored\n",
                false,
            ),
        ];
    let judgments = adjudicate_frozen_judgments(&plan, &results);
    assert_eq!(judgments.test_artifact.verdict, AssayVerdict::Pass);
    assert_eq!(judgments.product.verdict, AssayVerdict::Fail);
}
