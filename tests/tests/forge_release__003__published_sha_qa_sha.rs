//! FORGE.RELEASE — the published SHA is the QA SHA (TST-FORGE-RELEASE-003).
//!
//! Contract: what the release publishes is the SHA QA approved — the
//! candidate — and a published SHA that cannot be traced to that
//! candidate never satisfies the release. The proof walks the whole
//! chain, at both boundaries that carry it:
//!
//!   1. **the release executor** — `DbForgeReleaseExecutor::publish`
//!      (`forge/src/engine/release.rs:182`), the command boundary
//!      `forge.publish_candidate` dispatches to. The executor runs the
//!      QA lineage gate FIRST (`forge_lineage_error(evidence, \"qa\")`):
//!      a story without a QA-passed candidate is refused before any
//!      publish is attempted. On success it records the operation's
//!      published hash as `publishedSha` — and the publish-stage lineage
//!      gate (`forge_lineage_error(evidence, \"publish\")`) plus the
//!      fact projection are what the workflow's `publish_result`
//!      decision actually reads: an operation that claims to have
//!      published an unusable SHA is recorded, but the projected
//!      `publishSucceeded` fact is FALSE — the decision cannot be
//!      fooled by a claim.
//!   2. **the real operation** — `publish_candidate`
//!      (`forge/src/engine/git_publish.rs:48`) on a disposable git
//!      repository with a local bare origin: a fast-forward publishes
//!      the EXACT candidate (`Published { published_main_hash: candidate }`),
//!      a candidate nobody knows is `NoCandidate`, and a main that has
//!      diverged from the candidate produces an integration commit that
//!      must be proven by the story's QA commands — with no proofs
//!      there is no proof, and an untested merge is refused
//!      (`IntegrationUnverified`), never published.
//!
//! The kill switch is its own outcome, never a conflict:
//! `publish_switch_off` (`git_publish.rs:238`) is a pure predicate over
//! the raw `FORGE_ALLOW_PUBLISH` value, and `PublishDisabled` is filed
//! under its own class so an operator greps the cause instead of hunting
//! a merge conflict that does not exist.
//!
//! External providers are faked at the adapter boundary: the executor's
//! operations and evidence store are the ports it is generic over; the
//! git operation runs against a disposable repository this test creates
//! and removes. No network, no database, no PROD mutation.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__003__published_sha_qa_sha

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{forge_lineage_error, project_forge_gate_facts, ForgeGateEvidence};
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
/// The commit QA approved: the candidate.
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";

/// The release operations, faked at the adapter boundary. The publish
/// outcome is scripted per case; the migration and derived operations
/// answer success so the proof stays on the publish seam.
struct ScriptedPublish {
    outcome: PublishOutcome,
}

impl ForgeReleaseOperations for ScriptedPublish {
    fn apply_migrations(
        &self,
        _target: &str,
        _files: &[String],
        _command_id: &str,
    ) -> ForgeOperationResult {
        ForgeOperationResult {
            success: true,
            detail: "migrations applied".into(),
        }
    }
    fn verify_migrations(&self, target: &str, files: &[String]) -> ForgeOperationResult {
        self.apply_migrations(target, files, "")
    }
    fn refresh_derived(&self, _models: &[String], _command_id: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: true,
            detail: "derived models refreshed".into(),
        }
    }
    fn verify_derived(
        &self,
        _models: &[String],
        _attempt_command_id: &str,
    ) -> ForgeOperationResult {
        ForgeOperationResult {
            success: true,
            detail: "derived models verified".into(),
        }
    }
    fn publish(&self, _candidate_sha: Option<&str>, _frozen_proofs: &[String]) -> PublishOutcome {
        self.outcome.clone()
    }
}

/// The evidence store, faked at the adapter boundary: the story's
/// evidence in, and every patch the executor merged recorded for the
/// proof to read. The port takes `&self`, so the records live behind
/// the mutex — the same one-writer-per-process-instance discipline the
/// production `DbReleaseEvidenceStore` runs under.
#[derive(Default)]
struct RecordingStoreInner {
    stored: ForgeGateEvidence,
    merged: Vec<ForgeGateEvidence>,
}

#[derive(Clone, Default)]
struct RecordingStore(Arc<Mutex<RecordingStoreInner>>);

impl RecordingStore {
    fn patches(&self) -> Vec<ForgeGateEvidence> {
        self.0
            .lock()
            .expect("the store is not poisoned")
            .merged
            .clone()
    }
    fn stored(&self) -> ForgeGateEvidence {
        self.0
            .lock()
            .expect("the store is not poisoned")
            .stored
            .clone()
    }
}

impl EvidenceStore for RecordingStore {
    fn read(&self, _story_id: &str) -> std::result::Result<ForgeGateEvidence, String> {
        Ok(self.stored())
    }
    fn merge(
        &self,
        _process_instance_id: &str,
        _story_id: &str,
        patch: ForgeGateEvidence,
    ) -> std::result::Result<(), String> {
        self.0
            .lock()
            .expect("the store is not poisoned")
            .merged
            .push(patch);
        Ok(())
    }
    fn operation_receipt(
        &self,
        _operation_command_id: &str,
    ) -> std::result::Result<Option<db::ForgeReleaseOperationReceipt>, String> {
        Ok(None)
    }
    fn record_operation_receipt(
        &self,
        _env: &ForgeCommandEnvelope,
        _operation_command_id: &str,
        _result: &serde_json::Value,
    ) -> std::result::Result<(), String> {
        Ok(())
    }
    fn settle_operation(
        &self,
        _env: &ForgeCommandEnvelope,
        _operation_command_id: &str,
        patch: ForgeGateEvidence,
    ) -> std::result::Result<(), String> {
        self.0
            .lock()
            .expect("the store is not poisoned")
            .merged
            .push(patch);
        Ok(())
    }
    fn latest_refresh_command_id(&self, _process_instance_id: &str) -> Option<String> {
        None
    }
    fn frozen_proofs(&self, _story_id: &str) -> Vec<String> {
        Vec::new()
    }
}

/// A release command envelope for this story.
fn envelope(command_type: &str) -> ForgeCommandEnvelope {
    ForgeCommandEnvelope {
        command_type: command_type.into(),
        command_id: format!("forge-command:{command_type}"),
        causation_id: None,
        process_instance_id: "instance-release-003".into(),
        story_id: STORY_ID.into(),
    }
}

/// The evidence of a story whose QA pass on the candidate is on record.
fn qa_passed_story() -> ForgeGateEvidence {
    ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        qa_passed: Some(true),
        ..ForgeGateEvidence::default()
    }
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

/// Run the release executor's publish command once, returning the command
/// result, the patch it merged, and the evidence as it stands after the merge.
fn run_publish(
    operations: ScriptedPublish,
    story: ForgeGateEvidence,
) -> (
    workflow::ApplicationCommandResult,
    ForgeGateEvidence,
    ForgeGateEvidence,
) {
    let store = RecordingStore(Arc::new(Mutex::new(RecordingStoreInner {
        stored: story.clone(),
        merged: Vec::new(),
    })));
    let executor = DbForgeReleaseExecutor {
        operations,
        evidence: store.clone(),
        pending: None,
    };
    let result = executor.execute(&envelope("forge.publish_candidate"));
    let patch = store
        .patches()
        .last()
        .expect("{HARNESS}: the executor merged the publish evidence")
        .clone();
    let merged = patch.merge_over(&store.stored());
    (result, patch, merged)
}

// ---------------------------------------------------------------------------------------------
// The real publish operation, on a disposable git repository with a local bare origin.
// ---------------------------------------------------------------------------------------------

/// `git` in `repo`, returning trimmed stdout or the trimmed stderr.
fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// A disposable repository: a work clone with a local bare "origin",
/// an initial commit on `main`, and a candidate commit on top of it.
/// Returns (root, repo, origin, initial, candidate).
fn disposable_release_repo() -> (PathBuf, PathBuf, PathBuf, String, String) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("forge-release-003-{stamp}"));
    let origin = root.join("origin.git");
    let repo = root.join("repo");
    std::fs::create_dir_all(&root).expect("the disposable root is created");

    // The bare origin and the work clone, on `main` from the start.
    git(
        &root,
        &["init", "--bare", "-b", "main", origin.to_str().unwrap()],
    )
    .expect("the bare origin is initialized");
    git(&root, &["init", "-b", "main", repo.to_str().unwrap()])
        .expect("the work clone is initialized");
    git(
        &repo,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    )
    .expect("the origin remote is added");
    git(&repo, &["config", "user.email", "forge@test.local"]).expect("the identity is set");
    git(&repo, &["config", "user.name", "Forge Release Test"]).expect("the identity is set");

    // The initial commit, pushed: main's first published state.
    std::fs::write(repo.join("release.txt"), "initial\n").expect("the initial file is written");
    git(&repo, &["add", "release.txt"]).expect("the initial file is staged");
    git(&repo, &["commit", "-m", "initial"]).expect("the initial commit is created");
    let initial = git(&repo, &["rev-parse", "HEAD"]).expect("the initial SHA is read");
    git(&repo, &["push", "origin", "main"]).expect("the initial commit is published");

    // The candidate commit: the QA-approved change, not yet published.
    std::fs::write(repo.join("release.txt"), "candidate\n").expect("the candidate file is written");
    git(&repo, &["add", "release.txt"]).expect("the candidate file is staged");
    git(&repo, &["commit", "-m", "candidate"]).expect("the candidate commit is created");
    let candidate = git(&repo, &["rev-parse", "HEAD"]).expect("the candidate SHA is read");

    (root, repo, origin, initial, candidate)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-003); the file and the assay use it.
fn forge_release_003__published_sha_qa_sha() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE GATE — THE PUBLISH COMMAND RUNS THE QA LINEAGE FIRST. A
    //    story without a QA pass on a valid candidate is refused before
    //    any publish is attempted: the published SHA can only ever be
    //    the QA-approved one.
    // -----------------------------------------------------------------------------------------------------------
    let qa_failed = ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        qa_passed: Some(false),
        ..ForgeGateEvidence::default()
    };
    let (result, patch, _merged) = run_publish(
        ScriptedPublish {
            outcome: PublishOutcome::Published {
                published_main_hash: CANDIDATE.into(),
            },
        },
        qa_failed,
    );
    assert_eq!(
        result.outcome,
        ApplicationCommandOutcome::Success,
        "{HARNESS}: a refused publish is a recorded release outcome, not a lost command"
    );
    assert!(
        result
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("QA has not passed"),
        "{HARNESS}: the refusal names the QA gate, got: {:?}",
        result.message
    );
    assert_eq!(
        patch.publish_succeeded,
        Some(false),
        "{HARNESS}: a story QA has not passed does not publish"
    );
    assert_eq!(
        patch.failure_class.as_deref(),
        Some("PUBLISH_CONFLICT"),
        "{HARNESS}: the refusal is classified, got: {:?}",
        patch.failure_class
    );
    assert_eq!(
        patch.failed_release_stage.as_deref(),
        Some("PUBLISH"),
        "{HARNESS}: the refusal is pinned to the PUBLISH stage, got: {:?}",
        patch.failed_release_stage
    );

    let no_candidate = ForgeGateEvidence {
        qa_passed: Some(true),
        ..ForgeGateEvidence::default()
    };
    let (result, patch, _merged) = run_publish(
        ScriptedPublish {
            outcome: PublishOutcome::Published {
                published_main_hash: CANDIDATE.into(),
            },
        },
        no_candidate,
    );
    assert_eq!(
        patch.publish_succeeded,
        Some(false),
        "{HARNESS}: a story without a candidate SHA does not publish"
    );
    assert!(
        result
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("candidateSha"),
        "{HARNESS}: the refusal names the missing candidate, got: {:?}",
        result.message
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE CONTRACT — THE PUBLISHED SHA IS THE QA SHA. The operation
    //    published the candidate, and the executor recorded exactly
    //    that SHA: the published SHA equals the QA-verified candidate,
    //    the publish-stage lineage gate clears, and the projected
    //    `publishSucceeded` fact the workflow decision reads is true.
    // -----------------------------------------------------------------------------------------------------------
    let (result, patch, merged) = run_publish(
        ScriptedPublish {
            outcome: PublishOutcome::Published {
                published_main_hash: CANDIDATE.into(),
            },
        },
        qa_passed_story(),
    );
    assert_eq!(
        patch.publish_succeeded,
        Some(true),
        "{HARNESS}: the operation's success is recorded"
    );
    assert_eq!(
        patch.published_sha.as_deref(),
        Some(CANDIDATE),
        "{HARNESS}: the published SHA is the QA-approved candidate, got: {:?}",
        patch.published_sha
    );
    assert!(
        forge_lineage_error(&merged, "publish").is_none(),
        "{HARNESS}: the published candidate clears the publish-stage lineage gate"
    );
    let facts = project_forge_gate_facts(&merged);
    assert!(
        projected_bool(&facts, "publishSucceeded"),
        "{HARNESS}: the decision reads publishSucceeded = true, facts: {facts:?}"
    );
    assert_eq!(
        result.outcome,
        ApplicationCommandOutcome::Success,
        "{HARNESS}: the publish command records its outcome"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. FAULT — AN UNUSABLE PUBLISHED SHA NEVER SATISFIES THE RELEASE.
    //    An operation that claims to have published garbage is recorded
    //    as it claimed, but the publish-stage lineage gate flags the
    //    recorded SHA and the projected `publishSucceeded` fact is
    //    FALSE: the workflow decision cannot be fooled by a claim.
    // -----------------------------------------------------------------------------------------------------------
    let (_result, patch, merged) = run_publish(
        ScriptedPublish {
            outcome: PublishOutcome::Published {
                published_main_hash: "not-a-sha".into(),
            },
        },
        qa_passed_story(),
    );
    assert_eq!(
        patch.publish_succeeded,
        Some(true),
        "{HARNESS}: the executor records the operation's own claim"
    );
    assert_eq!(
        forge_lineage_error(&merged, "publish"),
        Some("publishedSha is missing or invalid".into()),
        "{HARNESS}: the lineage gate catches the unusable published SHA"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&merged), "publishSucceeded"),
        "{HARNESS}: the decision reads publishSucceeded = false for an unusable published SHA"
    );

    // A conflict outcome is recorded as a failed publish, classified.
    let (_result, patch, _merged) = run_publish(
        ScriptedPublish {
            outcome: PublishOutcome::PublishConflict {
                reason: "origin/main moved or refused the candidate".into(),
            },
        },
        qa_passed_story(),
    );
    assert_eq!(patch.publish_succeeded, Some(false));
    assert_eq!(
        patch.failure_class.as_deref(),
        Some("PUBLISH_CONFLICT"),
        "{HARNESS}: a publish conflict is classified PUBLISH_CONFLICT"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE REAL OPERATION — A FAST-FORWARD PUBLISHES THE EXACT
    //    CANDIDATE. On a disposable repository with a local origin,
    //    `publish_candidate` fast-forwards main to the candidate: the
    //    published main hash IS the QA-approved SHA. Re-publishing the
    //    same candidate is idempotent, an unknown candidate is refused,
    //    and a diverged main yields an integration commit that is
    //    refused without QA proofs — an untested merge is never
    //    published.
    // -----------------------------------------------------------------------------------------------------------
    let (root, repo, _origin, _initial, candidate) = disposable_release_repo();
    let outcome = publish_candidate(&repo, &candidate, &[]);
    match outcome {
        PublishOutcome::Published {
            published_main_hash,
        } => assert_eq!(
            published_main_hash, candidate,
            "{HARNESS}: a fast-forward publishes the exact candidate SHA"
        ),
        other => {
            panic!("{HARNESS}: the first publish of the candidate fast-forwards, got: {other:?}")
        }
    }

    // Re-publishing the already-published candidate is idempotent.
    let again = publish_candidate(&repo, &candidate, &[]);
    match again {
        PublishOutcome::Published {
            published_main_hash,
        } => assert_eq!(
            published_main_hash, candidate,
            "{HARNESS}: re-publishing the published candidate is idempotent"
        ),
        other => panic!("{HARNESS}: the published candidate republishes as itself, got: {other:?}"),
    }

    // A candidate nobody knows is refused, never published.
    let unknown = publish_candidate(&repo, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", &[]);
    match unknown {
        PublishOutcome::NoCandidate { reason } => assert!(
            reason.contains("is not a commit"),
            "{HARNESS}: an unknown candidate is refused by name, got: {reason}"
        ),
        other => panic!("{HARNESS}: an unknown candidate is NoCandidate, got: {other:?}"),
    }

    // Diverge origin/main from the candidate with a change that does
    // NOT touch the candidate's file, so the merge is clean and the
    // publish must build an integration commit — which, with no QA
    // proofs to prove it with, is refused as untested.
    let base = git(&repo, &["rev-parse", "--verify", "main~1"]).expect("the base is readable");
    let diverge =
        git(&repo, &["checkout", "-b", "diverge", &base]).expect("the divergent branch is created");
    assert_eq!(diverge, "");
    std::fs::write(repo.join("parallel.txt"), "diverged\n").expect("the diverged file is written");
    git(&repo, &["add", "parallel.txt"]).expect("the diverged file is staged");
    git(&repo, &["commit", "-m", "diverged"]).expect("the diverged commit is created");
    let diverged = git(&repo, &["rev-parse", "HEAD"]).expect("the diverged SHA is read");
    git(
        &repo,
        &[
            "push",
            "--force",
            "origin",
            &format!("{diverged}:refs/heads/main"),
        ],
    )
    .expect("origin/main is moved to the divergent commit");
    git(&repo, &["checkout", "main"]).expect("the candidate branch is restored");

    let untested = publish_candidate(&repo, &candidate, &[]);
    match untested {
        PublishOutcome::IntegrationUnverified {
            integrated_commit,
            reason,
        } => {
            assert!(
                !integrated_commit.is_empty(),
                "{HARNESS}: the integration commit is named"
            );
            assert!(
                reason.contains("no QA command to prove it with"),
                "{HARNESS}: the untested merge is refused by name, got: {reason}"
            );
        }
        other => panic!(
            "{HARNESS}: a diverged main with no proofs refuses the untested merge, got: {other:?}"
        ),
    }

    // The disposable repository is removed: the proof leaves no tree behind.
    std::fs::remove_dir_all(&root).expect("the disposable repository is removed");

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE KILL SWITCH IS ITS OWN OUTCOME, NOT A CONFLICT.
    //    `publish_switch_off` is a pure predicate over the raw value:
    //    only an explicit off-word refuses, and it is filed as
    //    `PublishDisabled`, never folded into `PublishConflict`.
    // -----------------------------------------------------------------------------------------------------------
    for off in [Some("0"), Some("false"), Some("OFF"), Some("no")] {
        assert!(
            publish_switch_off(off),
            "{HARNESS}: {off:?} holds the publish kill switch open"
        );
    }
    for on in [None, Some("1"), Some("true"), Some("yes")] {
        assert!(
            !publish_switch_off(on),
            "{HARNESS}: {on:?} does not hold the kill switch closed"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 6. CONTROL — THE DEFINITION'S PUBLISH DECISION READS THE FACT.
    //    `publish_result` continues only on `publishSucceeded == true`
    //    and fails otherwise, so the projected fact — not the
    //    operation's claim — is what moves the release.
    // -----------------------------------------------------------------------------------------------------------
    let definition = forge_sdlc_definition();
    let publish_result = definition
        .definition
        .nodes
        .get("publish_result")
        .expect("{HARNESS}: the production definition owns a publish_result decision");
    let arms = publish_result
        .decisions
        .as_ref()
        .expect("{HARNESS}: publish_result is a decision");
    assert!(
        arms.iter()
            .any(|arm| arm.condition == "publishSucceeded == true" && arm.transition == "continue"),
        "{HARNESS}: a successful publish continues the release, arms: {arms:?}"
    );
    assert!(
        arms.iter()
            .any(|arm| arm.condition == "publishSucceeded == false" && arm.transition == "fail"),
        "{HARNESS}: a failed publish fails the story, arms: {arms:?}"
    );
    let publish_candidate = definition
        .definition
        .nodes
        .get("publish_candidate")
        .expect("{HARNESS}: the production definition owns a publish_candidate node");
    assert_eq!(
        publish_candidate.command_type.as_deref(),
        Some("forge.publish_candidate"),
        "{HARNESS}: the publish node is the release executor's command"
    );
}
