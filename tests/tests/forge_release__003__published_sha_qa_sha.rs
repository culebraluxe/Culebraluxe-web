//! FORGE.RELEASE — published SHA = QA SHA (TST-FORGE-RELEASE-003).
//!
//! Contract: what lands on `main` is the commit QA approved — or, when `main` moved while QA was running, an
//! integration commit the story's OWN QA commands proved. Never a third thing, and never an unproven merge.
//! `git_publish.rs` is explicit that this path is the ONLY door a candidate can leave by (House Rule 1 refuses
//! any other push), so a publish that is wrong here is a publish that never happened.
//!
//! Three production seams carry the fact, and they may not disagree:
//!
//!   1. **the publish itself** — `publish_candidate` (`forge/src/engine/git_publish.rs:47`). A fast-forward
//!        publishes the exact commit and answers `PublishOutcome::Published { published_main_hash: candidate }`.
//!        When `origin/main` moved, the candidate is behind, so production builds a merge commit with the
//!        latest main and the exact QA-approved candidate as its two parents, and pushes it only after every
//!        proof passes (`prove_integration`, `git_publish.rs:169`). With no proofs, or a proof that fails, the
//!        outcome is `IntegrationUnverified` and main does not move.
//!   2. **the executor** — `DbForgeReleaseExecutor::publish` (`forge/src/engine/release.rs:190`) hands the
//!        release operations the evidence's `candidateSha` — it resolves no other commit — and records the
//!        published hash the operation returned, never one it made up.
//!   3. **the decision that reads it** — `publish_result` in the REAL `FORGE_SDLC-v6.xml`
//!        (`forge_sdlc_definition`) admits the release tail only on `publishSucceeded == true`, and
//!        `project_forge_gate_facts` computes that fact from `publish_succeeded` AND a clean publish lineage.
//!
//! This test runs the REAL publisher against a REAL local git repository with a local bare `origin` — no
//! network, no remote, nothing outside a temporary directory that is removed when it finishes. A scripted
//! publisher could not answer "is the SHA on main the QA SHA"; git can, and it is the only honest oracle.
//!
//! The negative and fault cases are the point. Without them the test could pass on a publisher that pushes
//! whatever it is handed: so the same publisher is driven with a candidate that is not a commit at all, with
//! an empty candidate, and — the case that matters most — with `main` moved and NO QA command to prove the
//! merge, which must refuse to publish rather than ship an untested merge.
//!
//! Deterministic and isolated: a temp directory under the OS temp dir, a local bare origin, no network, no
//! database, no PROD mutation, no environment mutation.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__003__published_sha_qa_sha

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::git_publish::{publish_candidate, publish_switch_off};
use forge::engine::release::{
    DbForgeReleaseExecutor, EvidenceStore, ForgeCommandEnvelope, ForgeOperationResult,
    ForgeReleaseOperations, PublishOutcome,
};
use workflow::{ApplicationCommandOutcome, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-003";
/// The proof command a story supplies for its integration commit. It is deliberately trivial and deliberately
/// REAL: it runs in a checkout of the merge commit, so a proof that does not run there is a proof that fails.
const PROOF: &str = "test -f candidate.txt";

/// A throwaway git world: a bare `origin` and a working clone, both under one temporary directory removed when
/// the test finishes. Nothing here can reach the network, and `origin` is a path, not a host.
struct Sandbox {
    root: PathBuf,
    work: PathBuf,
}

impl Sandbox {
    /// Build the world and leave `main` at its first commit, pushed.
    fn build(label: &str) -> Self {
        let unique = format!(
            "forge-release-003-{}-{}-{:?}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        );
        let root = std::env::temp_dir().join(unique);
        let origin = root.join("origin.git");
        let work = root.join("work");
        std::fs::create_dir_all(&root).expect("the sandbox root is creatable");
        run_at(
            &root,
            &[
                "init",
                "--bare",
                "--initial-branch=main",
                origin.to_str().expect("utf-8 path"),
            ],
        );
        run_at(
            &root,
            &[
                "clone",
                origin.to_str().expect("utf-8 path"),
                work.to_str().expect("utf-8 path"),
            ],
        );
        for args in [
            vec!["config", "user.email", "forge-release@test.invalid"],
            vec!["config", "user.name", "Forge Release 003"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            run_at(&work, &args);
        }
        let sandbox = Sandbox { root, work };
        sandbox.commit_on_main("candidate.txt", "the first commit\n");
        sandbox.push_main();
        sandbox
    }

    /// A commit on `main`, in the working clone.
    fn commit_on_main(&self, path: &str, contents: &str) -> String {
        std::fs::write(self.work.join(path), contents).expect("the sandbox work tree is writable");
        run_at(&self.work, &["add", path]);
        run_at(&self.work, &["commit", "-m", &format!("add {path}")]);
        self.head()
    }

    /// `origin/main` as the repository resolves it — the oracle for "what is actually on main".
    fn origin_main(&self) -> String {
        // `origin` is a path, so fetch always answers; `run_at` panics with the command if it does not.
        run_at(&self.work, &["fetch", "origin", "main"]);
        run_at(
            &self.work,
            &["rev-parse", "--verify", "origin/main^{commit}"],
        )
    }

    /// The commit the working clone is at.
    fn head(&self) -> String {
        run_at(&self.work, &["rev-parse", "HEAD"])
    }

    /// A commit on a branch cut from `base`, left un-pushed — the shape of a candidate QA approved.
    fn candidate_off(&self, branch: &str, base: &str, path: &str, contents: &str) -> String {
        run_at(&self.work, &["checkout", "-B", branch, base]);
        std::fs::write(self.work.join(path), contents).expect("the sandbox work tree is writable");
        run_at(&self.work, &["add", path]);
        run_at(&self.work, &["commit", "-m", &format!("{branch}: {path}")]);
        self.head()
    }

    fn push_main(&self) {
        run_at(&self.work, &["push", "origin", "HEAD:refs/heads/main"]);
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // The worktree/proof checkouts `publish_candidate` makes live under the OS temp dir under their own
        // names and are removed by production; this only clears the world this test built.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Run a git command in `dir` and return its trimmed stdout, panicking with the command on failure — a sandbox
/// that cannot be built is a test that cannot mean anything.
fn run_at(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|error| panic!("{HARNESS}: git {args:?} could not start: {error}"));
    assert!(
        output.status.success(),
        "{HARNESS}: git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// The release operations boundary, scripted: it records what it was handed and answers with whatever the
/// caller wants, so the executor's own half of the contract can be read without a second git world.
#[derive(Clone)]
struct ScriptedPublish {
    outcome: PublishOutcome,
    asked_for: Arc<Mutex<Vec<Option<String>>>>,
}

impl ScriptedPublish {
    fn answering(outcome: PublishOutcome) -> Self {
        Self {
            outcome,
            asked_for: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn asked_for(&self) -> Vec<Option<String>> {
        self.asked_for.lock().expect("not poisoned").clone()
    }
}

impl ForgeReleaseOperations for ScriptedPublish {
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
        self.outcome.clone()
    }
}

/// The evidence store the release executor reads and merges through.
#[derive(Clone)]
struct Stored {
    stored: ForgeGateEvidence,
    patches: Arc<Mutex<Vec<ForgeGateEvidence>>>,
}

impl Stored {
    fn new(stored: ForgeGateEvidence) -> Self {
        Self {
            stored,
            patches: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn last_patch(&self) -> ForgeGateEvidence {
        self.patches
            .lock()
            .expect("not poisoned")
            .last()
            .cloned()
            .expect("the release executor writes the outcome of the stage it ran")
    }
}

impl EvidenceStore for Stored {
    fn read(&self, _story_id: &str) -> ForgeGateEvidence {
        self.stored.clone()
    }
    fn merge(&self, _process_instance_id: &str, _story_id: &str, patch: ForgeGateEvidence) {
        self.patches.lock().expect("not poisoned").push(patch);
    }
    fn latest_refresh_command_id(&self, _process_instance_id: &str) -> Option<String> {
        None
    }
    fn frozen_proofs(&self, _story_id: &str) -> Vec<String> {
        Vec::new()
    }
}

/// The evidence as it stands at the publish: QA passed on this exact candidate.
fn at_publish(candidate: &str) -> ForgeGateEvidence {
    ForgeGateEvidence {
        qa_passed: Some(true),
        candidate_sha: Some(candidate.into()),
        ..ForgeGateEvidence::default()
    }
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-003); the file and the assay use it.
fn forge_release_003__published_sha_qa_sha() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. PROCESS COMPOSITION. The publish command node the release reaches is the one this story is about, and
    //    the decision after it admits the release tail on the publish fact alone. Read from the shipped XML.
    // -----------------------------------------------------------------------------------------------------------
    let nodes = &forge_sdlc_definition().definition.nodes;
    let publish_node = nodes
        .get("publish_candidate")
        .expect("the production definition owns a publish_candidate command node");
    assert_eq!(
        publish_node.node_type, "command",
        "{HARNESS}: publishing is a command node the engine runs, not a model's description"
    );
    assert_eq!(
        publish_node.command_type.as_deref(),
        Some("forge.publish_candidate"),
        "{HARNESS}: and it is the production publish command the executor implements"
    );
    assert_eq!(
        publish_node.responsibility.as_deref(),
        Some("dev_ops"),
        "{HARNESS}: DEV_OPS owns publication — no other lane may publish"
    );
    let publish_result = nodes
        .get("publish_result")
        .expect("the production definition owns a publish_result decision");
    let arms = publish_result
        .decisions
        .as_ref()
        .expect("publish_result is a decision");
    assert_eq!(
        arms.len(),
        2,
        "{HARNESS}: publish_result is two-way: it admits the release tail or sends the story to repair"
    );
    assert!(
        arms.iter()
            .any(|arm| arm.condition.contains("publishSucceeded == true")
                && arm.transition == "continue"),
        "{HARNESS}: only a successful publish continues, arms: {arms:?}"
    );
    assert!(
        arms.iter()
            .any(|arm| arm.condition.contains("publishSucceeded == false")
                && arm.transition == "fail"),
        "{HARNESS}: and an unsuccessful publish is a failure, arms: {arms:?}"
    );

    // The kill switch is read from the process environment, which this test must not move. If an operator has
    // it held open the real publisher refuses, and that is an environment condition rather than a contract
    // failure — so it is stated as a precondition rather than silently worked around.
    let switch = std::env::var("FORGE_ALLOW_PUBLISH").ok();
    assert!(
        !publish_switch_off(switch.as_deref()),
        "{HARNESS}: this proof needs the publish switch held open; it is currently {switch:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — A FAST-FORWARD PUBLISHES EXACTLY THE QA SHA. Real git, a local bare origin, and the
    //    production publisher. `origin/main` afterwards IS the candidate: the same 40 characters, read from
    //    the repository rather than from the function's return value.
    // -----------------------------------------------------------------------------------------------------------
    let sandbox = Sandbox::build("fast-forward");
    let base = sandbox.origin_main();
    let qa_sha = sandbox.candidate_off(
        "candidate",
        &base,
        "candidate.txt",
        "the QA-approved work\n",
    );

    let published = publish_candidate(&sandbox.work, &qa_sha, &[PROOF.to_string()]);
    let published_hash = match &published {
        PublishOutcome::Published {
            published_main_hash,
        } => published_main_hash.clone(),
        other => panic!("{HARNESS}: a candidate that fast-forwards must publish, got: {other:?}"),
    };
    assert_eq!(
        published_hash, qa_sha,
        "{HARNESS}: the publish reports the QA-approved commit as the one it published"
    );
    assert_eq!(
        sandbox.origin_main(),
        qa_sha,
        "{HARNESS}: and origin/main IS the QA SHA — git is the oracle here, not the publisher's own answer"
    );
    // The QA SHA really is a descendant of main, which is why this was a fast-forward and not a merge.
    assert!(
        Command::new("git")
            .args(["merge-base", "--is-ancestor", &base, &qa_sha])
            .current_dir(&sandbox.work)
            .status()
            .map(|status| status.success())
            .unwrap_or(false),
        "{HARNESS}: the QA SHA descends from main, so no integration commit was needed"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. MAIN MOVED — AN UNPROVEN MERGE IS REFUSED AND MAIN DOES NOT MOVE. Two candidates can each pass QA and
    //    still break main together, so a merge commit is proven before it is pushed. With no QA command there
    //    is nothing to prove it with, and the honest outcome is a refusal.
    // -----------------------------------------------------------------------------------------------------------
    let racing = sandbox.commit_on_main("racer.txt", "a peer publisher landed this\n");
    sandbox.push_main();
    assert_eq!(
        sandbox.origin_main(),
        racing,
        "{HARNESS}: the peer moved main, which is what makes the next publish an integration"
    );
    // A candidate that does NOT contain the new main, so the publish cannot fast-forward.
    let behind = sandbox.candidate_off(
        "behind",
        &qa_sha,
        "behind.txt",
        "cut before the peer landed\n",
    );

    let refused = publish_candidate(&sandbox.work, &behind, &[]);
    let reason = match &refused {
        PublishOutcome::IntegrationUnverified {
            integrated_commit,
            reason,
        } => {
            assert_ne!(
                integrated_commit, &behind,
                "{HARNESS}: the integration commit is a new commit, never the candidate relabelled"
            );
            reason.clone()
        }
        other => panic!("{HARNESS}: an unproven merge must be refused, got: {other:?}"),
    };
    assert!(
        reason.contains("no QA command to prove it with"),
        "{HARNESS}: the refusal says what was missing, got: {reason}"
    );
    assert!(
        reason.contains("refusing to publish an untested merge"),
        "{HARNESS}: and states the rule it is applying, got: {reason}"
    );
    assert_eq!(
        sandbox.origin_main(),
        racing,
        "{HARNESS}: a refused integration does NOT move main"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. MAIN MOVED — A PROOF THAT FAILS IS ALSO A REFUSAL. Same rule, the other shape: the story's QA command
    //    runs in a checkout of the merge commit, and a command that fails there fails the publish.
    // -----------------------------------------------------------------------------------------------------------
    let failing_proof = publish_candidate(
        &sandbox.work,
        &behind,
        &["test -f never-written.txt".to_string()],
    );
    let failed_reason = match &failing_proof {
        PublishOutcome::IntegrationUnverified { reason, .. } => reason.clone(),
        other => panic!("{HARNESS}: a failing proof must be refused, got: {other:?}"),
    };
    assert!(
        failed_reason.contains("failed on integration commit"),
        "{HARNESS}: the refusal names the proof that failed, got: {failed_reason}"
    );
    assert_eq!(
        sandbox.origin_main(),
        racing,
        "{HARNESS}: a failed proof does NOT move main either"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. MAIN MOVED — A PROOF THAT PASSES PUBLISHES THE PROVEN INTEGRATION COMMIT. This is the second shape
    //    production allows, and it is deliberately NOT the QA SHA: main advanced, so the commit on main is a
    //    merge whose two parents are the latest main and the exact QA-approved candidate. The contract is that
    //    it is one of those two things — never a third, and never unproven.
    // -----------------------------------------------------------------------------------------------------------
    let proven = publish_candidate(&sandbox.work, &behind, &[PROOF.to_string()]);
    let integrated_hash = match &proven {
        PublishOutcome::IntegratedAndPublished {
            published_main_hash,
        } => published_main_hash.clone(),
        other => panic!("{HARNESS}: a proven integration must publish, got: {other:?}"),
    };
    assert_eq!(
        sandbox.origin_main(),
        integrated_hash,
        "{HARNESS}: the commit on main is exactly the one the publisher reported"
    );
    assert_ne!(
        integrated_hash, behind,
        "{HARNESS}: an integration commit is never the candidate relabelled — main had moved"
    );
    let parents = run_at(
        &sandbox.work,
        &["rev-list", "--parents", "-n", "1", &integrated_hash],
    );
    let parents: Vec<&str> = parents.split_whitespace().collect();
    assert_eq!(
        parents.len(),
        3,
        "{HARNESS}: an integration commit has exactly two parents, got: {parents:?}"
    );
    assert!(
        parents.contains(&behind.as_str()),
        "{HARNESS}: one parent is the exact QA-approved candidate, parents: {parents:?}"
    );
    assert!(
        parents.contains(&racing.as_str()),
        "{HARNESS}: the other is the latest main, parents: {parents:?}"
    );
    // And the candidate's work is in it, which is what the proof asserted.
    assert!(
        sandbox.work.join("behind.txt").exists(),
        "{HARNESS}: the sandbox work tree still holds the candidate's file"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — A CANDIDATE THAT IS NOT A COMMIT, AND AN EMPTY CANDIDATE, PUBLISH NOTHING. These are the two
    //    shapes a bug in the caller produces, and both must be refused before any push is attempted.
    // -----------------------------------------------------------------------------------------------------------
    let not_a_commit =
        publish_candidate(&sandbox.work, "not-a-commit-at-all", &[PROOF.to_string()]);
    let not_a_commit_reason = match &not_a_commit {
        PublishOutcome::NoCandidate { reason } => reason.clone(),
        other => {
            panic!("{HARNESS}: a candidate that is not a commit must be refused, got: {other:?}")
        }
    };
    assert!(
        not_a_commit_reason.contains("is not a commit"),
        "{HARNESS}: the refusal says the candidate is not a commit, got: {not_a_commit_reason}"
    );

    for empty in ["", "   "] {
        let outcome = publish_candidate(&sandbox.work, empty, &[]);
        match outcome {
            PublishOutcome::NoCandidate { reason } => assert!(
                reason.contains("no candidate commit recorded"),
                "{HARNESS}: an empty candidate is refused as such, got: {reason}"
            ),
            other => panic!("{HARNESS}: an empty candidate must be refused, got: {other:?}"),
        }
    }
    assert_eq!(
        sandbox.origin_main(),
        integrated_hash,
        "{HARNESS}: none of the refusals moved main"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. THE EXECUTOR'S HALF. It hands the release operations the evidence's candidate — it resolves no other
    //    commit — and records the hash the operation returned, never one it made up. That is the seam the real
    //    publisher above plugs into, so the two halves are proved together.
    // -----------------------------------------------------------------------------------------------------------
    let scripted = ScriptedPublish::answering(PublishOutcome::Published {
        published_main_hash: qa_sha.clone(),
    });
    let store = Stored::new(at_publish(&qa_sha));
    let result = DbForgeReleaseExecutor {
        operations: scripted.clone(),
        evidence: store.clone(),
        pending: None,
    }
    .execute(&ForgeCommandEnvelope {
        command_type: "forge.publish_candidate".into(),
        command_id: format!("{STORY_ID}:publish"),
        process_instance_id: "instance-release-003".into(),
        story_id: STORY_ID.into(),
    });
    assert_eq!(
        result.outcome,
        ApplicationCommandOutcome::Success,
        "{HARNESS}: the publish command reports through evidence, not a transport error"
    );
    assert_eq!(
        scripted.asked_for(),
        vec![Some(qa_sha.clone())],
        "{HARNESS}: the executor asks to publish the candidate the QA approved, and nothing else"
    );
    let patch = store.last_patch();
    assert_eq!(
        patch.publish_succeeded,
        Some(true),
        "{HARNESS}: a landed publish is recorded as succeeded"
    );
    assert_eq!(
        patch.published_sha.as_deref(),
        Some(qa_sha.as_str()),
        "{HARNESS}: and the published SHA recorded is the one the publish reported"
    );
    assert_eq!(
        patch.failure_class, None,
        "{HARNESS}: a landed publish records no failure class"
    );
    // The fact is computed over the RUN's evidence — the publish patch merged over what the run already had —
    // because `publishSucceeded` is not merely "the publish returned Ok": it is that AND a clean publish
    // lineage, which is a property of the whole chain and not of the patch alone.
    let landed = patch.merge_over(&at_publish(&qa_sha));
    assert!(
        projected_bool(&project_forge_gate_facts(&landed), "publishSucceeded"),
        "{HARNESS}: the decision after the publish reads the fact that admits the release tail, facts: {:?}",
        project_forge_gate_facts(&landed)
    );

    // And the same fact is FALSE when the publish did not succeed — so the decision above is load-bearing.
    let unlanded = ScriptedPublish::answering(PublishOutcome::PublishConflict {
        reason: "remote main advanced".into(),
    });
    let unlanded_store = Stored::new(at_publish(&qa_sha));
    DbForgeReleaseExecutor {
        operations: unlanded,
        evidence: unlanded_store.clone(),
        pending: None,
    }
    .execute(&ForgeCommandEnvelope {
        command_type: "forge.publish_candidate".into(),
        command_id: format!("{STORY_ID}:publish-conflict"),
        process_instance_id: "instance-release-003".into(),
        story_id: STORY_ID.into(),
    });
    let conflict = unlanded_store.last_patch();
    assert_eq!(
        conflict.publish_succeeded,
        Some(false),
        "{HARNESS}: a publish that did not land is recorded as not succeeded"
    );
    let conflicted = conflict.merge_over(&at_publish(&qa_sha));
    assert!(
        !projected_bool(&project_forge_gate_facts(&conflicted), "publishSucceeded"),
        "{HARNESS}: so the release tail is NOT admitted on it, facts: {:?}",
        project_forge_gate_facts(&conflicted)
    );
}
