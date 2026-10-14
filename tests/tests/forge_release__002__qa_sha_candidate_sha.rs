//! FORGE.RELEASE — the SHA QA measured is the candidate SHA (TST-FORGE-RELEASE-002).
//!
//! Contract: what QA measured and what release publishes are the same delivery, and the
//! candidate identity that joins them belongs to the run — not to a Git reading QA takes.
//! QA holds no git identity by rule (`docs/agent/FORGE-QA-BOUNDARY.md`, a binding boundary):
//! "QA must not touch Git, inspect Git, query Git, or communicate with Git ... bind findings,
//! plans, receipts, caches, retries, idempotency, or verdicts to a commit SHA or other Git
//! identity". The Assay lane therefore measures the plan the Story Run froze, reports its own
//! verdict over that plan, leaves the reviewed candidate the run carries untouched, and records a
//! measurement row keyed to the run and carrying NO candidate SHA at all.
//!
//! This file asserted the retired shape until now: it expected the lane to read
//! `git rev-parse HEAD` and to refuse a workspace sitting on another commit (the ARCH-SEAM-005
//! refusal `08b3342c4` added). `c2c556adc fix(forge): isolate QA from Git and release authority`
//! (trunk `8343d85bf`) removed that authority: the HEAD comparison, the `forge_assay__006`
//! candidate-vs-QA SHA test it belonged to, and the candidate SHA on the artifact row were retired
//! together, and `forge_assay__006__qa_verdict_independent_of_git_identity` is the replacement at
//! contract level. This proof now walks the boundaries the contract actually has:
//!
//!   1. **the measurement** — `run_frozen_assay` / `measure_frozen_plan`
//!      (`forge/src/roles/qa/mod.rs:127`), reached through `AssayHooks::turn_without_model` and the
//!      shared lane lifecycle (`run_lane_turn`). The lane measures the run's frozen plan and
//!      reports its own verdict; the reviewed candidate survives into the evidence untouched, so
//!      the SHA release publishes is the SHA QA verified. Every command the lane runs is one of the
//!      plan's — it reads no Git — and its measurement row (`forge_tool_artifact`, kind
//!      `qa-assay-evidence`) is keyed to the Story Run.
//!   2. **the release gate** — `forge_lineage_error(evidence, "qa")`
//!      (`forge/src/engine/facts.rs:236`), the gate the release executor's publish command runs
//!      FIRST: a publish without a valid candidate SHA, or without a QA pass, is refused. Candidate
//!      identity is checked here, on the authorized path, and never inside QA.
//!   3. **the promotion gate** — `promotion_eligibility`
//!      (`forge/src/engine/evidence_gate.rs:30`): a promotion is eligible only when the candidate is
//!      present AND the anchor carries an independent QA PASS with evidence — a `FAIL`, missing
//!      anchor evidence, or a missing candidate blocks it. The `STALE_APPROVAL_SHA` /
//!      `VERIFIED_SHA_MISMATCH` conjuncts an earlier draft of this proof asserted were RETIRED by the
//!      same isolation ruling, which is why no promotion-time sha comparison is needed either.
//!
//! Negative and fault cases are load-bearing: a measurement nobody can key to a Story Run is
//! refused and records nothing, a workspace sitting on another commit cannot substitute for the
//! reviewed candidate (the lane never looks, so its refusal cannot depend on the commit), an empty
//! or invalid candidate SHA is refused by the release gate before any publish is considered, and a
//! QA FAIL records `qaPassed = false` for the very same candidate.
//!
//! The external provider boundary is faked, not mocked over: the `RoleHarness` is the
//! model/worktree adapter production injects, scripted to answer the frozen plan's commands and to
//! record every command it is asked to run.
//!
//! Deterministic and isolated: no database, no network, no git, no PROD mutation.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__002__qa_sha_candidate_sha

use std::sync::{Arc, Mutex};

use forge::engine::assay::CommandResult;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::evidence_gate::{promotion_eligibility, qa_fail, qa_pass};
use forge::engine::facts::{forge_lineage_error, ForgeGateEvidence};
use forge::engine::qa_plan::{
    AcceptanceJudgment, ApprovedAcceptanceCondition, ApprovedAssayCommand, ApprovedAssayPlan,
    ApprovedAssertionCheck, AssayParser, AssayRunner, CheckAggregation,
};
use forge::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::writer::RecordingWriter;
use forge::roles::qa::AssayService;
use forge::roles::AbstractForgeService;
use workflow::{Result, TaskStatus, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-002";
/// The commit Smith delivered and Lead froze: the reviewed candidate.
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";
/// A DIFFERENT commit — what an unreviewed workspace might sit on.
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";
/// The durable Story Run the frozen plan is keyed to: what makes a measurement
/// evidence rather than a reading nobody can trace.
const RUN: &str = "TST-FORGE-RELEASE-002-RUN-1";
/// The plan's runtime test — the RUST_CONTRACT authoring check's own file.
const RUNTIME_TEST_COMMAND: &str =
    "cargo test --manifest-path Cargo.toml -p test-harness --test forge_release__002__qa_sha_candidate_sha";
/// The plan's structural check: the test artifact compiles.
const STRUCTURAL_COMMAND: &str = "cargo check --manifest-path Cargo.toml --workspace --all-targets";
/// The assertion the runtime test reports — the name the plan's check resolves.
const ASSERTION: &str = "forge_release__002__qa_sha_candidate_sha";

/// The model/worktree adapter production injects, faked at the boundary. It records
/// every command the lane asks it to run and answers the frozen plan's two commands
/// with the output their runners read.
///
/// `head` is the commit this workspace SITS ON, and the double knows it: it answers a
/// `git rev-parse HEAD` like a real workspace would. Whether the lane ever asks is what
/// the cases below pin — since the QA isolation (`c2c556adc`) it must not, so the field
/// exists to make the refusal independent of the commit rather than to be consulted.
struct ScriptedWorkspace {
    head: &'static str,
    commands: Mutex<Vec<String>>,
}

impl ScriptedWorkspace {
    fn new(head: &'static str) -> Self {
        Self {
            head,
            commands: Mutex::new(Vec::new()),
        }
    }

    /// Every command the lane asked this workspace to run, in the order it asked.
    fn commands(&self) -> Vec<String> {
        self.commands
            .lock()
            .expect("the workspace is not poisoned")
            .clone()
    }
}

impl RoleHarness for ScriptedWorkspace {
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }

    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        unreachable!("a RUST_CONTRACT measurement node never spends a model turn")
    }

    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }

    fn run_command(&self, command: &str) -> CommandResult {
        self.commands
            .lock()
            .expect("the workspace is not poisoned")
            .push(command.to_string());
        // The plan's runtime test reports a libtest summary for the assertion its check
        // resolves; the structural check reports an exit-zero run. A Git reading is not
        // among the plan's commands — and this double would answer one, so the assertions
        // that read `commands()` are what catch a lane that reached for it.
        let output = if command.trim() == "git rev-parse HEAD" {
            self.head.to_string()
        } else if command == RUNTIME_TEST_COMMAND {
            format!(
                "test {ASSERTION} ... ok\n\
                 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
            )
        } else {
            String::new()
        };
        CommandResult {
            command: command.to_string(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            cancelled: false,
            output,
        }
    }
}

/// The QA verify task as the engine lists it: the Assay lane's measurement node.
fn qa_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-qa-verify".into(),
        process_instance_id: "instance-release-002".into(),
        story_id: STORY_ID.into(),
        token_id: Some("token-release-002".into()),
        node_id: Some("qa_verify".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["qa_verify".into()],
        write_surface: None,
    }
}

/// The RUST_CONTRACT authoring checks: one runtime test and one structural
/// check, both named in the packet's acceptance, so the acceptance is mapped.
fn contract_commands() -> Vec<String> {
    vec![RUNTIME_TEST_COMMAND.into(), STRUCTURAL_COMMAND.into()]
}

/// The approved assay plan the Story Run froze, as `forge_story_run` freezes it:
/// the acceptance the packet named, resolved to commands, checks and conditions.
/// This is what the lane measures — and its commands are the only ones it may run.
fn frozen_plan_snapshot(writer: &RecordingWriter) {
    let plan = ApprovedAssayPlan {
        schema_version: 1,
        plan_id: "plan-release-002".into(),
        plan_version: 1,
        commands: vec![
            ApprovedAssayCommand {
                id: "cmd-runtime".into(),
                command: RUNTIME_TEST_COMMAND.into(),
                runner: AssayRunner::RustLibtest,
                parser: AssayParser::RustLibtest,
                working_directory: "lane_root".into(),
                environment_identity: "forge-sanitized-assay-v2".into(),
            },
            ApprovedAssayCommand {
                id: "cmd-structural".into(),
                command: STRUCTURAL_COMMAND.into(),
                runner: AssayRunner::CommandExit,
                parser: AssayParser::CommandExit,
                working_directory: "lane_root".into(),
                environment_identity: "forge-sanitized-assay-v2".into(),
            },
        ],
        checks: vec![
            ApprovedAssertionCheck {
                id: "check-runtime".into(),
                command_id: "cmd-runtime".into(),
                assertion: ASSERTION.into(),
            },
            ApprovedAssertionCheck {
                id: "check-structural".into(),
                command_id: "cmd-structural".into(),
                assertion: "exit_code_zero".into(),
            },
        ],
        conditions: vec![
            ApprovedAcceptanceCondition {
                id: "AC-product".into(),
                check_ids: vec!["check-structural".into()],
                aggregation: CheckAggregation::AllRequired,
                judgment: AcceptanceJudgment::Product,
            },
            ApprovedAcceptanceCondition {
                id: "AC-test-artifact".into(),
                check_ids: vec!["check-runtime".into()],
                aggregation: CheckAggregation::AllRequired,
                judgment: AcceptanceJudgment::TestArtifact,
            },
        ],
        negative_control: None,
    };
    let identity = plan.identity().expect("the frozen plan hashes");
    writer.assay_plan_snapshots.lock().unwrap().insert(
        RUN.into(),
        db::forge_assay::AssayPlanSnapshotRow {
            story_run_id: RUN.into(),
            story_id: STORY_ID.into(),
            assay_commands_snapshot: Some(contract_commands().join("\n")),
            snapshot: Some(serde_json::json!({
                "plan": plan,
                "approved_by": "test-operator",
                "approved_at": "2026-10-09T12:00:00Z",
                "approved_hash": identity.hash,
            })),
        },
    );
}

/// Run the QA measurement lane once over a workspace sitting on `head`, for the
/// Story Run `run` (or none at all), returning the outcome (or the refusal), the
/// writer's records and the workspace the lane asked for commands.
fn measure(
    head: &'static str,
    reviewed: &str,
    run: Option<&str>,
) -> (
    std::result::Result<forge::engine::executor::ForgeRoleOutcome, workflow::WorkflowError>,
    Arc<RecordingWriter>,
    Arc<ScriptedWorkspace>,
) {
    let harness = Arc::new(ScriptedWorkspace::new(head));
    let writer = Arc::new(RecordingWriter::default());
    frozen_plan_snapshot(&writer);
    let mut current = ForgeGateEvidence {
        candidate_sha: Some(reviewed.into()),
        ..ForgeGateEvidence::default()
    };
    current.extra.insert(
        "recordedBase",
        Value::from("89abcdef0123456789abcdef0123456789abcdef"),
    );
    let mut runner = ProductionRoleRunner::new(harness.clone(), current)
        .with_writer(writer.clone())
        .with_test_mode(Some("RUST_CONTRACT".into()))
        .with_contract_assay_commands(contract_commands())
        .with_contract_acceptance_mapped(true);
    if let Some(run) = run {
        runner = runner.with_story_run(Some(run.into()));
    }
    let outcome = AssayService::new(&runner).execute("qa_verify", &qa_task());
    (outcome, writer, harness)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-002); the file and the assay use it.
fn forge_release_002__qa_sha_candidate_sha() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — THE MEASUREMENT IS ABOUT THE REVIEWED CANDIDATE THE RUN CARRIES. The story run holds the
    //    candidate and the plan it froze; the lane measures that plan, reports its own verdict over it, and
    //    leaves the candidate untouched — so the SHA QA verified and the SHA the release will publish are one
    //    fact. And it reads no Git while doing it: every command it runs is one of the plan's.
    // -----------------------------------------------------------------------------------------------------------
    let (outcome, writer, harness) = measure(CANDIDATE, CANDIDATE, Some(RUN));
    let outcome = outcome.expect("the measurement of the reviewed candidate completes");
    assert_eq!(
        outcome.transition_name.as_deref(),
        Some("complete"),
        "{HARNESS}: a clean measurement of the candidate completes the QA node"
    );
    assert_eq!(
        outcome.evidence.candidate_sha.as_deref(),
        Some(CANDIDATE),
        "{HARNESS}: the QA-verified SHA is the candidate SHA, got: {:?}",
        outcome.evidence.candidate_sha
    );
    assert_eq!(
        outcome.evidence.qa_passed,
        Some(true),
        "{HARNESS}: the measurement passed on the candidate"
    );

    // NO GIT WAS READ. The frozen plan's commands are the only commands the lane ran, and the reviewed candidate
    // the run carried is untouched by any workspace reading. The retired `git rev-parse HEAD` comparison
    // (`08b3342c4` "Assay refuses a workspace HEAD that is not the reviewed candidate") was removed from QA by
    // the isolation (`c2c556adc`, trunk `8343d85bf`); candidate identity is the release path's (part 3), and a
    // lane that reaches for a commit again fails here.
    let commands = harness.commands();
    assert_eq!(
        commands,
        contract_commands(),
        "{HARNESS}: the lane runs the frozen plan's commands, and only those"
    );
    assert!(
        !commands.iter().any(|command| command.contains("git")),
        "{HARNESS}: QA holds no Git authority — no lane command may read Git, got: {commands:?}"
    );

    // The lane's own measurement row is keyed to the run it measured and names the run's verdict. It carries NO
    // candidate SHA, by design: "qa findings carry no Git identity" (`assay_tool_artifact`,
    // `docs/agent/FORGE-QA-BOUNDARY.md`). What joins the QA pass to the candidate is the release gate below.
    let artifacts = writer
        .artifacts
        .lock()
        .expect("the writer is not poisoned")
        .clone();
    let row = artifacts
        .iter()
        .find(|a| a.kind == "qa-assay-evidence")
        .expect("{HARNESS}: the QA lane's measurement is a tool-artifact row");
    assert_eq!(
        row.story_run_id.as_deref(),
        Some(RUN),
        "{HARNESS}: the measurement row names the Story Run it came out of"
    );
    assert_eq!(
        row.sha, None,
        "{HARNESS}: a QA finding is never bound to a candidate SHA, got: {:?}",
        row.sha
    );
    assert_eq!(
        row.verdict.as_deref(),
        Some("PASS"),
        "{HARNESS}: the row's verdict is the lane's own reading"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE REFUSAL — A MEASUREMENT NOBODY CAN KEY TO A STORY RUN IS NOT MEASURED, AND THE WORKSPACE HEAD CANNOT
    //    SUBSTITUTE FOR THE CANDIDATE. The lane's receipt is keyed to the run (its id, the plan hash, the task,
    //    the measurement node), so a run without a durable one is refused before anything runs — and the answer
    //    is the same whichever commit the workspace sits on, because QA never looks at the commit.
    // -----------------------------------------------------------------------------------------------------------
    let (unkeyed, unkeyed_writer, unkeyed_workspace) = measure(CANDIDATE, CANDIDATE, None);
    let error = unkeyed
        .err()
        .expect("{HARNESS}: a measurement with no durable Story Run is refused, not recorded");
    let message = error.to_string();
    // The refusal is the lane's own: it will not write a receipt nobody can key to a run.
    assert!(
        message.contains("QA measurement requires a durable Story Run"),
        "{HARNESS}: the refusal names what is missing, got: {message}"
    );
    assert!(
        unkeyed_writer.artifacts.lock().unwrap().is_empty(),
        "{HARNESS}: a refused measurement records no row"
    );
    assert!(
        unkeyed_workspace.commands().is_empty(),
        "{HARNESS}: nothing was measured, so no command ran"
    );

    // THE SAME ANSWER ON ANOTHER COMMIT. A workspace sitting on an unreviewed commit changes nothing: the lane
    // refuses for the missing run, not for the commit it found — and it asks for no commit at all, so an
    // unreviewed commit can never stand in for the reviewed candidate.
    let (other_head, _other_writer, other_workspace) = measure(OTHER, CANDIDATE, None);
    let other_error = other_head
        .err()
        .expect("{HARNESS}: a workspace on another commit is no more measurable");
    assert_eq!(
        other_error.to_string(),
        message,
        "{HARNESS}: the refusal does not depend on which commit the workspace sits on"
    );
    assert!(
        !other_workspace
            .commands()
            .iter()
            .any(|command| command.contains("git")),
        "{HARNESS}: the unreviewed commit is not even read, got: {:?}",
        other_workspace.commands()
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE RELEASE GATE — THE PUBLISH COMMAND RUNS THE QA LINEAGE FIRST.
    //    `forge_lineage_error(evidence, \"qa\")` is the gate
    //    `DbForgeReleaseExecutor::publish` runs before it will publish: a
    //    valid candidate SHA AND a QA pass, or the publish is refused.
    // -----------------------------------------------------------------------------------------------------------
    let verified = ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        qa_passed: Some(true),
        ..ForgeGateEvidence::default()
    };
    assert!(
        forge_lineage_error(&verified, "qa").is_none(),
        "{HARNESS}: a QA pass on a valid candidate clears the QA-stage lineage gate"
    );

    let unverified = ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        qa_passed: Some(false),
        ..ForgeGateEvidence::default()
    };
    assert_eq!(
        forge_lineage_error(&unverified, "qa"),
        Some("QA has not passed for this candidate".into()),
        "{HARNESS}: a QA FAIL on the candidate refuses the release at the QA gate"
    );

    let missing = ForgeGateEvidence {
        qa_passed: Some(true),
        ..ForgeGateEvidence::default()
    };
    assert_eq!(
        forge_lineage_error(&missing, "qa"),
        Some("candidateSha is missing or invalid".into()),
        "{HARNESS}: a release without a candidate SHA is refused at the QA gate"
    );

    let invalid = ForgeGateEvidence {
        candidate_sha: Some("not-a-sha".into()),
        qa_passed: Some(true),
        ..ForgeGateEvidence::default()
    };
    assert!(
        forge_lineage_error(&invalid, "qa").is_some(),
        "{HARNESS}: an invalid candidate SHA is refused at the QA gate"
    );

    // An EMPTY candidate is no candidate: the string is there but names no commit, and the same gate refuses it —
    // which is where "an empty candidate is never silently adopted" lives now that QA takes the candidate from the
    // run instead of reading the workspace for it (the case this file used to assert at the measurement).
    let empty = ForgeGateEvidence {
        candidate_sha: Some(String::new()),
        qa_passed: Some(true),
        ..ForgeGateEvidence::default()
    };
    assert_eq!(
        forge_lineage_error(&empty, "qa"),
        Some("candidateSha is missing or invalid".into()),
        "{HARNESS}: an empty candidate is refused at the QA gate, not adopted"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE PROMOTION GATE — ONLY A PASS ON A PRESENT CANDIDATE PROMOTES.
    //    `promotion_eligibility` is the anchor gate: it reads the independent
    //    QA verdict and whether the candidate is present, and refuses a FAIL,
    //    missing anchor evidence, or a missing candidate. Candidate identity is
    //    checked at the release gate (part 3) and never inside QA, so no
    //    promotion-time sha comparison is needed — that conjunct was retired by
    //    the same isolation ruling.
    // -----------------------------------------------------------------------------------------------------------
    let eligible = promotion_eligibility(true, &qa_pass());
    assert!(
        eligible.0,
        "{HARNESS}: a candidate with independent QA evidence promotes"
    );

    let failed = promotion_eligibility(true, &qa_fail());
    assert!(
        !failed.0 && failed.1.contains(&"QA_FAIL"),
        "{HARNESS}: a failed anchor never promotes: {:?}",
        failed.1
    );

    let no_evidence = promotion_eligibility(true, &forge::engine::evidence_gate::no_evidence());
    assert!(
        !no_evidence.0 && no_evidence.1.contains(&"NO_ANCHOR_EVIDENCE"),
        "{HARNESS}: an anchor with no evidence never promotes: {:?}",
        no_evidence.1
    );

    let no_candidate = promotion_eligibility(false, &qa_pass());
    assert!(
        !no_candidate.0 && no_candidate.1.contains(&"NO_CANDIDATE"),
        "{HARNESS}: a promotion without a candidate is refused: {:?}",
        no_candidate.1
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. CONTROL — THE QA NODE THE MEASUREMENT RUNS ON IS THE ASSAY LANE'S.
    //    The definition's service binding dispatches `qa_verify` to the
    //    deterministic verification service, so the seam above is the one
    //    the engine dispatches to.
    // -----------------------------------------------------------------------------------------------------------
    let definition = forge_sdlc_definition();
    let qa_verify = definition
        .definition
        .nodes
        .get("qa_verify")
        .expect("{HARNESS}: the production definition owns a qa_verify node");
    assert_eq!(
        forge::engine::service_binding::service_for_node("qa_verify"),
        Ok(Some("forge.assay")),
        "{HARNESS}: the QA measurement node is bound to the Assay service"
    );
    assert_eq!(
        qa_verify.responsibility.as_deref(),
        Some("qa"),
        "{HARNESS}: the measurement node is the QA lane's"
    );
    assert!(
        qa_verify
            .transitions
            .as_ref()
            .expect("{HARNESS}: qa_verify declares its transitions")
            .iter()
            .any(|t| t.name == "complete" && t.to == "qa_result"),
        "{HARNESS}: a completed measurement proceeds to the QA result decision"
    );
}
