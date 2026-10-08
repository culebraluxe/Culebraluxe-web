//! FORGE.RELEASE — QA SHA = candidate SHA (TST-FORGE-RELEASE-002).
//!
//! Contract: the commit QA measured IS the candidate the release publishes. Those two must be one commit, not
//! two that happen to look alike — because the publish is a fast-forward of an exact SHA, and "the QA'd the
//! thing I meant" is the whole safety property of the release. A lane that measures whatever `HEAD` happens to
//! be, while release publishes the SHA the evidence named, hands production a commit nobody reviewed.
//!
//! Four production seams carry the fact, and they may not disagree:
//!
//!   1. **the lane that measures** — `run_rust_contract_qa` (`forge/src/roles/qa.rs:145-168`, ARCH-SEAM-005):
//!        when the evidence already names the candidate Smith delivered and Inspector reviewed, a workspace
//!        whose HEAD is another commit is REFUSED rather than measured, and the recorded SHA is the one it
//!        measured. This is the lane the shipped XML binds `qa_verify` to.
//!   2. **the publish gate** — `forge_lineage_error(evidence, "qa")` (`forge/src/engine/facts.rs:186-195`):
//!        publish requires a syntactically valid `candidateSha` and `qaPassed == Some(true)`. It is checked by
//!        `DbForgeReleaseExecutor::publish` (`forge/src/engine/release.rs:190`) before the release operations
//!        are asked to publish anything.
//!   3. **the anchor provenance** — `evaluate_verification` (`forge/src/engine/verification.rs:14`): a system
//!        anchor that verified a different SHA than the candidate is a blocker, and agent testimony can never
//!        satisfy an anchor requirement at all.
//!   4. **the fact the decision reads** — `project_forge_gate_facts` publishes `candidateSha`
//!        (`forge/src/engine/facts.rs:441-443`), and `publishSucceeded` is false unless the QA lineage is
//!        clean.
//!
//! The negative and fault cases are the point. Without them the test could pass on a lane that simply adopts
//! whatever `HEAD` says: so the same lane is driven with a workspace one commit ahead (refused), with a
//! `HEAD` that is not a commit (refused), with a QA that did not pass (the publish is refused before any
//! publish operation is asked for), and with a candidate SHA that is not a SHA at all (refused).
//!
//! Deterministic and isolated: no database, no network, no external provider, no environment mutation. Git is
//! the scripted `RoleHarness` boundary Forge injects and the release operations are the injected
//! `ForgeReleaseOperations` boundary.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__002__qa_sha_candidate_sha

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use forge::engine::assay::CommandResult;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::drive::ForgeRoleRunner;
use forge::engine::facts::{forge_lineage_error, project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::release::{
    DbForgeReleaseExecutor, EvidenceStore, ForgeCommandEnvelope, ForgeOperationResult,
    ForgeReleaseOperations, PublishOutcome,
};
use forge::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::service_binding::service_for_node;
use forge::engine::verification::{evaluate_verification, AnchorEvidence};
use forge::engine::writer::RecordingWriter;
use forge::roles::qa::is_measurement_node;
use workflow::{ApplicationCommandOutcome, ApplicationCommandResult, TaskStatus, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-002";
/// The candidate Smith delivered and Inspector reviewed. The release publishes exactly this.
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";
/// A commit the review never saw — the classic "QA measured something else" shape.
const UNREVIEWED: &str = "fedcba9876543210fedcba9876543210fedcba98";
/// The execution base the run recorded.
const BASE: &str = "89abcdef0123456789abcdef0123456789abcdef";
/// The structural authoring check the RUST_CONTRACT gate requires.
const STRUCTURAL: &str = "cargo check --manifest-path Cargo.toml --workspace --all-targets";

/// What the scripted workspace answers `git rev-parse HEAD` with.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Head {
    /// The reviewed candidate.
    Candidate,
    /// A different commit — the workspace moved on after the review.
    Unreviewed,
    /// Something that is not a commit at all.
    NotACommit,
    /// Git could not answer.
    Unreadable,
}

/// The workspace adapter production injects. It answers HEAD, answers the authoring check, and answers the range
/// diff with no production edit — so the rules around it, and only those rules, are under test.
struct ScriptedWorkspace {
    head: Head,
    turns: AtomicUsize,
    probes: Mutex<Vec<String>>,
}

impl ScriptedWorkspace {
    fn new(head: Head) -> Self {
        Self {
            head,
            turns: AtomicUsize::new(0),
            probes: Mutex::new(Vec::new()),
        }
    }

    fn turns(&self) -> usize {
        self.turns.load(Ordering::SeqCst)
    }

    fn probes(&self) -> Vec<String> {
        self.probes.lock().expect("not poisoned").clone()
    }
}

impl RoleHarness for ScriptedWorkspace {
    fn run_role(
        &self,
        node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        self.turns.fetch_add(1, Ordering::SeqCst);
        Ok(HarnessOutput {
            raw: format!("{node_id} described its own work\n"),
            candidate_sha: None,
            assay_commands: Vec::new(),
            acceptance_mapped: false,
            refusal: None,
            execution_base: None,
            usage: None,
        })
    }
    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn execution_base_commit(&self) -> Option<&str> {
        Some(BASE)
    }
    fn run_command(&self, command: &str) -> CommandResult {
        self.probes
            .lock()
            .expect("not poisoned")
            .push(command.to_string());
        let answered =
            |passed: bool, exit_code: i32, unmeasurable: bool, output: &str| CommandResult {
                command: command.to_string(),
                exit_code,
                passed,
                excerpt: String::new(),
                unmeasurable,
                output: output.to_string(),
            };
        match (command, self.head) {
            ("git rev-parse HEAD", Head::Candidate) => answered(true, 0, false, CANDIDATE),
            ("git rev-parse HEAD", Head::Unreviewed) => answered(true, 0, false, UNREVIEWED),
            ("git rev-parse HEAD", Head::NotACommit) => {
                answered(true, 0, false, "HEAD detached at 89abcdef\n")
            }
            ("git rev-parse HEAD", Head::Unreadable) => {
                answered(false, 128, true, "fatal: not a git repository")
            }
            _ if command.starts_with("git diff --name-only") => answered(true, 0, false, ""),
            _ => answered(true, 0, false, ""),
        }
    }
}

/// The `qa_verify` task as the engine lists it.
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

/// The evidence as it stands at the verification turn: the reviewed candidate is named, nothing is measured yet.
fn reviewed_candidate() -> ForgeGateEvidence {
    ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        ..ForgeGateEvidence::default()
    }
}

/// Drive the production QA lane once, answering with the workspace the case wants.
fn measure(
    workspace: &ScriptedWorkspace,
    entering: ForgeGateEvidence,
) -> (
    forge::engine::executor::drive::ForgeRoleOutcome,
    RecordingWriter,
) {
    let writer = RecordingWriter::default();
    let runner = ProductionRoleRunner::new(workspace, entering)
        .with_writer(&writer)
        .with_test_mode(Some("RUST_CONTRACT".into()))
        .with_contract_assay_commands(vec![STRUCTURAL.into()])
        .with_contract_acceptance_mapped(true);
    match ForgeRoleRunner::run(&runner, "qa_verify", &qa_task()) {
        Ok(outcome) => (outcome, writer),
        Err(error) => panic!("{HARNESS}: the lane under measurement must answer, got: {error}"),
    }
}

/// The release operations boundary. It records the SHA it was handed, which is the whole claim under test:
/// release must be asked to publish the CANDIDATE, never anything it resolved for itself.
#[derive(Clone)]
struct RecordingPublish {
    asked_for: Arc<Mutex<Vec<Option<String>>>>,
}

impl RecordingPublish {
    fn new() -> Self {
        Self {
            asked_for: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn asked_for(&self) -> Vec<Option<String>> {
        self.asked_for.lock().expect("not poisoned").clone()
    }
}

impl ForgeReleaseOperations for RecordingPublish {
    fn apply_migrations(&self, _t: &str, _f: &[String], _c: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: "not this story's subject".into(),
        }
    }
    fn verify_migrations(&self, _t: &str, _f: &[String]) -> ForgeOperationResult {
        self.apply_migrations("", &[], "")
    }
    fn refresh_derived(&self, _m: &[String], _c: &str) -> ForgeOperationResult {
        self.apply_migrations("", &[], "")
    }
    fn verify_derived(&self, _m: &[String], _a: &str) -> ForgeOperationResult {
        self.refresh_derived(_m, _a)
    }
    fn publish(&self, candidate_sha: Option<&str>, _proofs: &[String]) -> PublishOutcome {
        self.asked_for
            .lock()
            .expect("not poisoned")
            .push(candidate_sha.map(str::to_string));
        PublishOutcome::Published {
            published_main_hash: candidate_sha.unwrap_or_default().into(),
        }
    }
}

/// The evidence store the release executor reads and merges through.
#[derive(Clone)]
struct Stored(ForgeGateEvidence);

impl EvidenceStore for Stored {
    fn read(&self, _story_id: &str) -> ForgeGateEvidence {
        self.0.clone()
    }
    fn merge(&self, _process_instance_id: &str, _story_id: &str, _patch: ForgeGateEvidence) {}
    fn latest_refresh_command_id(&self, _process_instance_id: &str) -> Option<String> {
        None
    }
    fn frozen_proofs(&self, _story_id: &str) -> Vec<String> {
        Vec::new()
    }
}

/// Run `forge.publish_candidate` through the production release executor against the given stored evidence.
fn publish(stored: ForgeGateEvidence, operations: &RecordingPublish) -> ApplicationCommandResult {
    DbForgeReleaseExecutor {
        operations: operations.clone(),
        evidence: Stored(stored),
        pending: None,
    }
    .execute(&ForgeCommandEnvelope {
        command_type: "forge.publish_candidate".into(),
        command_id: format!("{STORY_ID}:publish"),
        process_instance_id: "instance-release-002".into(),
        story_id: STORY_ID.into(),
    })
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-002); the file and the assay use it.
fn forge_release_002__qa_sha_candidate_sha() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. PROCESS COMPOSITION. The measurement below is only the release's QA because the shipped definition
    //    binds `qa_verify` to the deterministic lane and routes its decision on the pass.
    // -----------------------------------------------------------------------------------------------------------
    let service = service_for_node("qa_verify")
        .expect("the production definition parses")
        .unwrap_or_else(|| panic!("{HARNESS}: FORGE_SDLC-v6.xml binds qa_verify to a service"));
    assert_eq!(
        service, "forge.assay",
        "{HARNESS}: the QA that gates the release is Assay's deterministic measurement"
    );
    assert!(
        is_measurement_node("qa_verify"),
        "{HARNESS}: qa_verify MEASURES, so its SHA is taken from the workspace rather than stated by a model"
    );
    let nodes = &forge_sdlc_definition().definition.nodes;
    let qa_result = nodes
        .get("qa_result")
        .expect("the production definition owns a qa_result decision");
    let qa_arms = qa_result
        .decisions
        .as_ref()
        .expect("qa_result is a decision");
    let pass_arm = qa_arms
        .iter()
        .find(|arm| arm.condition.contains("qaPassed == true"))
        .unwrap_or_else(|| panic!("{HARNESS}: qa_result admits only a QA pass, arms: {qa_arms:?}"));
    let pass_target = qa_result
        .transitions
        .as_ref()
        .expect("qa_result declares its transitions")
        .iter()
        .find(|t| t.name == pass_arm.transition)
        .unwrap_or_else(|| panic!("{HARNESS}: qa_result has a pass transition"))
        .to
        .clone();
    assert_eq!(
        pass_target, "devops_begin",
        "{HARNESS}: the release begins only on a QA pass — the publish is downstream of the measurement"
    );
    // And DEV_OPS publishes, from the same node the measured candidate came from.
    let begin_target = nodes
        .get("devops_begin")
        .and_then(|n| n.transitions.as_ref())
        .and_then(|ts| ts.iter().find(|t| t.name == "publish"))
        .map(|t| t.to.clone())
        .expect("devops_begin publishes");
    assert_eq!(
        begin_target, "publish_candidate",
        "{HARNESS}: the publish command is the next node after a QA pass"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — THE SHA QA REPORTS IS THE CANDIDATE. The workspace is at the reviewed candidate, the
    //    authoring check passes, and the lane records that exact commit. Release then publishes exactly that
    //    commit: same string, one fact, two readers.
    // -----------------------------------------------------------------------------------------------------------
    let workspace = ScriptedWorkspace::new(Head::Candidate);
    let (measured, writer) = measure(&workspace, reviewed_candidate());
    assert_eq!(
        workspace.turns(),
        0,
        "{HARNESS}: the SHA is measured, never taken from a model's description"
    );
    assert_eq!(
        measured.evidence.candidate_sha.as_deref(),
        Some(CANDIDATE),
        "{HARNESS}: the SHA the QA lane reports IS the candidate the review named"
    );
    assert_eq!(
        measured.evidence.qa_passed,
        Some(true),
        "{HARNESS}: the measurement passed, which is what admits the release"
    );
    assert!(
        workspace
            .probes()
            .iter()
            .any(|command| command == "git rev-parse HEAD"),
        "{HARNESS}: the SHA came from the workspace, not from the packet"
    );
    // The lane's own row names the commit it measured — the durable half of the same fact.
    let row = writer
        .artifacts
        .lock()
        .expect("not poisoned")
        .iter()
        .find(|a| a.kind == "qa-assay-evidence")
        .cloned()
        .expect("the measurement lane writes its own reading as a row");
    assert_eq!(
        row.sha.as_deref(),
        Some(CANDIDATE),
        "{HARNESS}: the QA row records the measured commit, got: {row:?}"
    );
    assert_eq!(
        row.verdict.as_deref(),
        Some("PASS"),
        "{HARNESS}: and the verdict that admits the release"
    );

    // THE PUBLISH GATE ACCEPTS IT, AND THE PUBLISH IS ASKED FOR THAT EXACT COMMIT.
    assert_eq!(
        forge_lineage_error(&measured.evidence, "qa"),
        None,
        "{HARNESS}: a valid candidate SHA on a QA pass satisfies the QA lineage"
    );
    let ops = RecordingPublish::new();
    let result = publish(measured.evidence.clone(), &ops);
    assert_eq!(
        result.outcome,
        ApplicationCommandOutcome::Success,
        "{HARNESS}: the publish command reports through evidence, not a transport error"
    );
    assert_eq!(
        ops.asked_for(),
        vec![Some(CANDIDATE.to_string())],
        "{HARNESS}: release publishes the CANDIDATE SHA QA measured — it resolves no other commit for itself"
    );
    assert_eq!(
        project_forge_gate_facts(&measured.evidence)
            .get("candidateSha")
            .and_then(Value::as_str),
        Some(CANDIDATE),
        "{HARNESS}: the candidate SHA is projected for the decision to read"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — A WORKSPACE THAT IS NOT THE REVIEWED CANDIDATE IS REFUSED, NOT MEASURED. This is the whole
    //    story: adopting `HEAD` here would publish a commit nobody reviewed, and the publish is a fast-forward
    //    of an exact SHA, so nothing downstream would notice.
    // -----------------------------------------------------------------------------------------------------------
    let moved = ScriptedWorkspace::new(Head::Unreviewed);
    let moved_writer = RecordingWriter::default();
    let moved_runner = ProductionRoleRunner::new(&moved, reviewed_candidate())
        .with_writer(&moved_writer)
        .with_test_mode(Some("RUST_CONTRACT".into()))
        .with_contract_assay_commands(vec![STRUCTURAL.into()])
        .with_contract_acceptance_mapped(true);
    let refusal = match ForgeRoleRunner::run(&moved_runner, "qa_verify", &qa_task()) {
        Ok(_) => {
            panic!("{HARNESS}: a workspace that is not the reviewed candidate cannot be measured")
        }
        Err(error) => error.to_string(),
    };
    assert!(
        refusal.contains("not the reviewed candidate"),
        "{HARNESS}: the refusal says the workspace is not the reviewed commit, got: {refusal}"
    );
    assert!(
        refusal.contains(CANDIDATE),
        "{HARNESS}: and it names the commit QA was asked about, got: {refusal}"
    );
    assert!(
        refusal.contains(UNREVIEWED),
        "{HARNESS}: and the commit the workspace actually reported, got: {refusal}"
    );
    assert!(
        moved_writer
            .artifacts
            .lock()
            .expect("not poisoned")
            .iter()
            .all(|a| a.kind != "qa-assay-evidence"),
        "{HARNESS}: a refused measurement records no verdict and therefore no candidate SHA"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. FAULT — A HEAD THAT IS NOT A COMMIT, AND A HEAD GIT CANNOT ANSWER, ARE BOTH REFUSED. Neither is
    //    silently treated as "no candidate" and neither is adopted as one: a workspace that cannot name itself
    //    cannot be the thing QA measured.
    // -----------------------------------------------------------------------------------------------------------
    let not_a_commit = ScriptedWorkspace::new(Head::NotACommit);
    let not_a_commit_writer = RecordingWriter::default();
    let not_a_commit_runner = ProductionRoleRunner::new(&not_a_commit, reviewed_candidate())
        .with_writer(&not_a_commit_writer)
        .with_test_mode(Some("RUST_CONTRACT".into()))
        .with_contract_assay_commands(vec![STRUCTURAL.into()])
        .with_contract_acceptance_mapped(true);
    let not_a_commit_error =
        match ForgeRoleRunner::run(&not_a_commit_runner, "qa_verify", &qa_task()) {
            Ok(_) => panic!("{HARNESS}: a HEAD that is not a commit cannot be measured"),
            Err(error) => error.to_string(),
        };
    assert!(
        not_a_commit_error.contains("not the reviewed candidate")
            || not_a_commit_error.contains("unreadable"),
        "{HARNESS}: a HEAD that is not a commit is refused, never adopted as the candidate, got: {not_a_commit_error}"
    );

    let unreadable = ScriptedWorkspace::new(Head::Unreadable);
    let unreadable_writer = RecordingWriter::default();
    let unreadable_runner = ProductionRoleRunner::new(&unreadable, reviewed_candidate())
        .with_writer(&unreadable_writer)
        .with_test_mode(Some("RUST_CONTRACT".into()))
        .with_contract_assay_commands(vec![STRUCTURAL.into()])
        .with_contract_acceptance_mapped(true);
    let unreadable_error = match ForgeRoleRunner::run(&unreadable_runner, "qa_verify", &qa_task()) {
        Ok(_) => panic!("{HARNESS}: a workspace git cannot name cannot be measured"),
        Err(error) => error.to_string(),
    };
    assert!(
        unreadable_error.contains("not the reviewed candidate")
            || unreadable_error.contains("unreadable"),
        "{HARNESS}: an unreadable HEAD is refused, never adopted as the candidate, got: {unreadable_error}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — A CANDIDATE SHA THAT IS NOT A SHA IS REFUSED BY THE PUBLISH GATE. The lane measured
    //    something; the gate still refuses a candidate it cannot compare, because a publish is a fast-forward
    //    of an exact commit id.
    // -----------------------------------------------------------------------------------------------------------
    for bogus in ["not-a-sha", "", "   "] {
        let stored = ForgeGateEvidence {
            candidate_sha: (!bogus.is_empty()).then(|| bogus.to_string()),
            qa_passed: Some(true),
            ..ForgeGateEvidence::default()
        };
        assert_eq!(
            forge_lineage_error(&stored, "qa").as_deref(),
            Some("candidateSha is missing or invalid"),
            "{HARNESS}: {bogus:?} is not a commit the release could publish"
        );
        assert!(
            !projected_bool(&project_forge_gate_facts(&stored), "publishSucceeded"),
            "{HARNESS}: and no publish can succeed on an unpublishable candidate"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — A QA THAT DID NOT PASS ADMITS NO PUBLISH, AND NONE IS ASKED FOR. The gate reads the
    //    measurement, so a failed QA never reaches the release operations at all: the executor returns before
    //    `operations.publish` is called, which is what "no publish was attempted" means here.
    // -----------------------------------------------------------------------------------------------------------
    let failed_qa = ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        qa_passed: Some(false),
        ..ForgeGateEvidence::default()
    };
    assert_eq!(
        forge_lineage_error(&failed_qa, "qa").as_deref(),
        Some("QA has not passed for this candidate"),
        "{HARNESS}: a failed QA is the QA lineage's own refusal, named as such"
    );
    let never = RecordingPublish::new();
    let refused_publish = publish(failed_qa, &never);
    assert_eq!(
        refused_publish.outcome,
        ApplicationCommandOutcome::Success,
        "{HARNESS}: the refusal is recorded as evidence rather than thrown"
    );
    assert!(
        refused_publish
            .message
            .unwrap_or_default()
            .contains("QA has not passed"),
        "{HARNESS}: and the message is the lineage's own reason"
    );
    assert!(
        never.asked_for().is_empty(),
        "{HARNESS}: no publish was even attempted for a candidate QA did not approve, asked: {:?}",
        never.asked_for()
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. THE ANCHOR PROVENANCE AGREES. A verification anchor is only allowed to vouch for the candidate it
    //    actually verified, and a model's testimony is never an anchor at all — which is the same rule stated
    //    where the evidence is read rather than where the release is gated.
    // -----------------------------------------------------------------------------------------------------------
    let (ok, blockers) = evaluate_verification(
        &["test"],
        &[AnchorEvidence {
            kind: "test".into(),
            source: "system".into(),
            exit_code: Some(0),
            verified_sha: Some(CANDIDATE.into()),
        }],
        Some(CANDIDATE),
    );
    assert!(
        ok && blockers.is_empty(),
        "{HARNESS}: a system anchor that verified the candidate satisfies the anchor, blockers: {blockers:?}"
    );

    let (wrong, blockers) = evaluate_verification(
        &["test"],
        &[AnchorEvidence {
            kind: "test".into(),
            source: "system".into(),
            exit_code: Some(0),
            verified_sha: Some(UNREVIEWED.into()),
        }],
        Some(CANDIDATE),
    );
    assert!(
        !wrong,
        "{HARNESS}: an anchor that verified a different SHA does not satisfy the candidate"
    );
    assert!(
        blockers
            .iter()
            .any(|blocker| blocker.contains("verified wrong SHA") && blocker.contains(CANDIDATE)),
        "{HARNESS}: and the blocker names both SHAs, blockers: {blockers:?}"
    );

    let (testimony, blockers) = evaluate_verification(
        &["test"],
        &[AnchorEvidence {
            kind: "test".into(),
            source: "agent".into(),
            exit_code: None,
            verified_sha: Some(CANDIDATE.into()),
        }],
        Some(CANDIDATE),
    );
    assert!(
        !testimony,
        "{HARNESS}: an agent's testimony cannot stand in for a measurement, even naming the right SHA"
    );
    assert!(
        blockers
            .iter()
            .any(|blocker| blocker.contains("agent testimony cannot satisfy")),
        "{HARNESS}: and the blocker says why, blockers: {blockers:?}"
    );

    // CONTROL: the release really is gated ON this measurement, so the refusals above are load-bearing. The
    // shipped `qa_result` admits `devops_begin` only on `qaPassed == true`.
    assert_eq!(pass_target, "devops_begin");
    assert!(
        qa_arms
            .iter()
            .all(|arm| !arm.condition.contains("qaPassed == true") || arm.transition == "pass"),
        "{HARNESS}: the only arm that admits the release is the one conditioned on a QA pass"
    );
}
