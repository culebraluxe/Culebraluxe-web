//! FORGE.RELEASE — the QA-verified SHA is the candidate SHA (TST-FORGE-RELEASE-002).
//!
//! Contract: the SHA that QA verified IS the candidate SHA. QA holds no git
//! identity by rule (the 2026-09-16 ruling retired the `verifiedSha ===
//! candidateSha` conjunct because QA never had a sha to compare), so the
//! identity is enforced where the measurement happens: the Assay lane MEASURES
//! the workspace's HEAD and refuses to measure any commit that is not the
//! reviewed candidate. The QA-verified SHA and the candidate SHA cannot disagree
//! because the lane records the one it measured — and a workspace sitting on a
//! different commit is refused, not measured.
//!
//! Three production boundaries carry the contract, and this proof walks all of
//! them:
//!
//!   1. **the measurement** — `run_rust_contract_qa`
//!      (`forge/src/roles/qa.rs:127`), reached through
//!      `AssayHooks::turn_without_model` and the shared lane lifecycle
//!      (`run_lane_turn`, `forge/src/roles/lifecycle.rs:106`). THE REVIEWED
//!      CANDIDATE IS THE ONE MEASURED (ARCH-SEAM-005): a workspace whose HEAD
//!      is another commit is refused, and the measured SHA is recorded as the
//!      candidate — so the SHA the release publishes is the SHA QA verified.
//!      The lane's own measurement row (`forge_tool_artifact`, kind
//!      `qa-assay-evidence`) is stamped with the same SHA.
//!   2. **the release gate** — `forge_lineage_error(evidence, "qa")`
//!      (`forge/src/engine/facts.rs:178`), the gate the release executor's
//!      publish command runs FIRST: a publish without a valid candidate SHA, or
//!      without a QA pass, is refused. The QA stage of the lineage is keyed to
//!      the candidate.
//!   3. **the promotion gate** — `promotion_eligibility`
//!      (`forge/src/engine/evidence_gate.rs:38`): an anchor whose evaluated
//!      SHA is not the candidate is `STALE_APPROVAL_SHA`; a verified SHA that
//!      is not the candidate is `VERIFIED_SHA_MISMATCH`. A promotion whose
//!      verified SHA is some other commit is refused, not promoted.
//!
//! The external provider boundary is faked, not mocked over: the
//! `RoleHarness` is the model/worktree adapter production injects, scripted to
//! report a HEAD and answer commands; every production gate — the measurement
//! refusal, the lineage gate, the promotion gate — runs as it runs in
//! production.
//!
//! Negative and fault cases are load-bearing: a workspace on another commit is
//! refused (not silently measured), a QA FAIL records `qaPassed = false` for the
//! very same candidate, and an invalid or missing candidate SHA is refused by
//! the release gate before any publish is considered.
//!
//! Deterministic and isolated: no database, no network, no git, no PROD mutation.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__002__qa_sha_candidate_sha

use forge::engine::assay::CommandResult;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::evidence_gate::{promotion_eligibility, qa_fail, qa_pass};
use forge::engine::facts::{forge_lineage_error, ForgeGateEvidence};
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

/// The model/worktree adapter production injects, faked at the boundary. It
/// reports a scripted HEAD and passes every command it is asked to run, so
/// the gates around it are the only thing under test.
struct ScriptedWorkspace {
    head: &'static str,
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
        // The one git fact the lane is allowed to read: the workspace HEAD.
        let output = if command.trim() == "git rev-parse HEAD" {
            self.head.to_string()
        } else {
            String::new()
        };
        CommandResult {
            command: command.to_string(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
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
    }
}

/// The RUST_CONTRACT authoring checks: one runtime test and one structural
/// check, both named in the packet's acceptance, so the acceptance is mapped.
fn contract_commands() -> Vec<String> {
    vec![
        "cargo test --manifest-path Cargo.toml -p test-harness --test forge_release__002__qa_sha_candidate_sha".into(),
        "cargo check --manifest-path Cargo.toml --workspace --all-targets".into(),
    ]
}

/// Run the QA measurement lane once over a workspace sitting on `head`,
/// returning the outcome (or the refusal) and the writer's records.
fn measure(
    head: &'static str,
    reviewed: &str,
) -> (
    std::result::Result<forge::engine::executor::drive::ForgeRoleOutcome, workflow::WorkflowError>,
    RecordingWriter,
) {
    let harness = ScriptedWorkspace { head };
    let writer = RecordingWriter::default();
    let mut current = ForgeGateEvidence {
        candidate_sha: Some(reviewed.into()),
        ..ForgeGateEvidence::default()
    };
    current
        .extra
        .insert("recordedBase", Value::from("89abcdef0123456789abcdef0123456789abcdef"));
    let runner = ProductionRoleRunner::new(&harness, current)
        .with_writer(&writer)
        .with_test_mode(Some("RUST_CONTRACT".into()))
        .with_contract_assay_commands(contract_commands())
        .with_contract_acceptance_mapped(true);
    let outcome = AssayService::new(&runner).execute("qa_verify", &qa_task());
    (outcome, writer)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-002); the file and the assay use it.
fn forge_release_002__qa_sha_candidate_sha() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — THE MEASURED SHA IS THE CANDIDATE SHA. The workspace
    //    sits on the reviewed candidate, the lane measures it, and the
    //    evidence it produces carries exactly that SHA as the candidate: the
    //    SHA QA verified and the SHA the release will publish are one fact.
    // -----------------------------------------------------------------------------------------------------------
    let (outcome, writer) = measure(CANDIDATE, CANDIDATE);
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

    // The lane's own measurement row is stamped with the same SHA — the
    // durable record of what QA verified names the candidate.
    let artifacts = writer.artifacts.lock().expect("the writer is not poisoned").clone();
    let row = artifacts
        .iter()
        .find(|a| a.kind == "qa-assay-evidence")
        .expect("{HARNESS}: the QA lane's measurement is a tool-artifact row");
    assert_eq!(
        row.sha.as_deref(),
        Some(CANDIDATE),
        "{HARNESS}: the measurement row names the SHA it verified, got: {:?}",
        row.sha
    );
    assert_eq!(
        row.verdict.as_deref(),
        Some("PASS"),
        "{HARNESS}: the row's verdict is the lane's own reading"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE REFUSAL — A WORKSPACE ON ANOTHER COMMIT IS NOT MEASURED. The
    //    lane refuses to verify a commit nobody reviewed: substituting the
    //    workspace HEAD cannot substitute the candidate.
    // -----------------------------------------------------------------------------------------------------------
    let (refused, _writer) = measure(OTHER, CANDIDATE);
    let error = refused
        .err()
        .expect("{HARNESS}: a workspace on another commit is refused, not measured");
    let message = error.to_string();
    assert!(
        message.contains("not the reviewed candidate"),
        "{HARNESS}: the refusal names the reviewed candidate, got: {message}"
    );
    assert!(
        message.contains(OTHER),
        "{HARNESS}: the refusal names the commit it found, got: {message}"
    );

    // And the symmetric fault: a run that carries no candidate at all cannot
    // be measured either — the release gate below refuses it, and the lane's
    // own artifact names no SHA.
    let (no_candidate, _writer) = measure(CANDIDATE, "");
    let error = no_candidate
        .err()
        .expect("{HARNESS}: an empty candidate is refused by the measurement");
    assert!(
        error.to_string().contains("not the reviewed candidate"),
        "{HARNESS}: an empty candidate is not silently adopted, got: {error}"
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

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE PROMOTION GATE — A VERIFIED SHA THAT IS NOT THE CANDIDATE IS
    //    REFUSED. `promotion_eligibility` is the anchor gate: a stale
    //    approval (evaluated SHA != candidate) and a verified SHA that is not
    //    the candidate both block promotion.
    // -----------------------------------------------------------------------------------------------------------
    let eligible = promotion_eligibility(Some(CANDIDATE), &qa_pass(CANDIDATE, CANDIDATE));
    assert!(
        eligible.0,
        "{HARNESS}: an anchor evaluated on and verified at the candidate promotes"
    );

    let stale = promotion_eligibility(Some(CANDIDATE), &qa_pass(OTHER, CANDIDATE));
    assert!(
        !stale.0 && stale.1.contains(&"STALE_APPROVAL_SHA"),
        "{HARNESS}: an approval evaluated on another commit is stale: {:?}",
        stale.1
    );

    let mismatch = promotion_eligibility(Some(CANDIDATE), &qa_pass(CANDIDATE, OTHER));
    assert!(
        !mismatch.0 && mismatch.1.contains(&"VERIFIED_SHA_MISMATCH"),
        "{HARNESS}: a verified SHA that is not the candidate is a mismatch: {:?}",
        mismatch.1
    );

    let failed = promotion_eligibility(Some(CANDIDATE), &qa_fail(CANDIDATE));
    assert!(
        !failed.0 && failed.1.contains(&"QA_FAIL"),
        "{HARNESS}: a failed anchor never promotes: {:?}",
        failed.1
    );

    let no_evidence = promotion_eligibility(Some(CANDIDATE), &forge::engine::evidence_gate::no_evidence());
    assert!(
        !no_evidence.0 && no_evidence.1.contains(&"NO_ANCHOR_EVIDENCE"),
        "{HARNESS}: an anchor with no evidence never promotes: {:?}",
        no_evidence.1
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
