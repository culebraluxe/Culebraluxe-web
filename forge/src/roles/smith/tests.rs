use super::*;
use crate::engine::assay::CommandResult;
use crate::engine::harness::HarnessUsage;
use crate::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use std::sync::Arc;

/// The bug this test exists for: every previous record of a candidate was a *pointer* into git
/// (`candidate_sha`), so the night the objects went, the work went with them. The row has to carry the
/// patch text itself, and it has to carry it under the base it was taken against — a diff without its base
/// is not replayable.
#[test]
fn the_candidate_is_stored_as_code_and_not_as_a_pointer_to_it() {
    let sha = "a".repeat(40);
    let base = "b".repeat(40);
    let patch = "diff --git a/tests/tests/t.rs b/tests/tests/t.rs\n\
                 +fn the_new_assertion() {}\n";
    let artifact = smith_candidate_artifact(
        "TST-RUNTIME-POOL-003",
        Some("11111111-2222-3333-4444-555555555555"),
        &sha,
        &base,
        patch,
        &["tests/tests/t.rs".to_string()],
    );

    assert_eq!(artifact.tool, "smith");
    assert_eq!(artifact.kind, "candidate-code");
    assert_eq!(artifact.sha.as_deref(), Some(sha.as_str()));
    assert_eq!(
        artifact.story_run_id.as_deref(),
        Some("11111111-2222-3333-4444-555555555555"),
        "the capture hangs on the run that produced it"
    );
    // A verdict here would be read as a claim about the run. This row reports what Smith wrote, which is a
    // fact whether the run passes or fails — and a failed run's code is the code someone wants back.
    assert_eq!(artifact.verdict, None);

    let detail = artifact.detail.expect("the code is the payload");
    assert_eq!(detail["patch"], serde_json::Value::from(patch));
    assert_eq!(detail["base"], serde_json::Value::from(base));
    assert_eq!(detail["candidateSha"], serde_json::Value::from(sha));
    assert_eq!(detail["patchBytes"], serde_json::Value::from(patch.len()));
    assert_eq!(
        detail["changedFiles"][0],
        serde_json::Value::from("tests/tests/t.rs")
    );
    let summary = artifact.summary.expect("a summary an operator can read");
    assert!(summary.contains("1 file(s)"), "{summary}");
    assert!(summary.contains(&patch.len().to_string()), "{summary}");
}

/// The bug this test exists for: the capture was gated on the story-run row, and the lane a captain runs by
/// hand (`forge --story …`, no `--work-item`) opens no run row at all — so the fail-safe stayed silent on the
/// one lane where nothing else records the code. A capture with no claim is still a capture: the story is the
/// anchor, and NULL in `story_run_id` says what actually happened instead of leaving a gap.
#[test]
fn a_capture_without_a_claim_still_carries_the_code() {
    let sha = "c".repeat(40);
    let base = "d".repeat(40);
    let patch = "diff --git a/x b/x\n+fn t() {}\n";
    let artifact = smith_candidate_artifact(
        "TST-ACCOUNTING-CORE-007",
        None,
        &sha,
        &base,
        patch,
        &["x".to_string()],
    );

    assert_eq!(
        artifact.story_id, "TST-ACCOUNTING-CORE-007",
        "the story is the anchor, because the story always exists"
    );
    assert_eq!(
        artifact.story_run_id, None,
        "no claim opened this run, and the row records that rather than refusing to be written"
    );
    assert_eq!(artifact.kind, "candidate-code");
    assert_eq!(artifact.verdict, None);
    assert_eq!(artifact.sha.as_deref(), Some(sha.as_str()));

    let detail = artifact.detail.expect("the code is the payload");
    assert_eq!(detail["patch"], serde_json::Value::from(patch));
    assert_eq!(
        detail["candidateSha"],
        serde_json::Value::from(sha.as_str())
    );
    assert_eq!(detail["base"], serde_json::Value::from(base.as_str()));
}

/// The guard that keeps a shell string honest. `sha` and `base` are interpolated into `git diff`, so a
/// value that is not a full commit — a branch name, a short sha, an empty string, anything with a space in
/// it — must be refused before it reaches the shell rather than trusted for having come from `rev-parse`.
#[test]
fn only_a_full_commit_reaches_the_diff() {
    assert!(is_full_commit(&"0".repeat(40)));
    assert!(is_full_commit("ABCDEF0123456789abcdef0123456789ABCDEF01"));
    assert!(
        is_full_commit(&format!("{}\n", "a".repeat(40))),
        "git prints a newline, and the value is trimmed rather than rejected for it"
    );
    assert!(!is_full_commit("HEAD"));
    assert!(!is_full_commit("main"));
    assert!(!is_full_commit(""));
    assert!(!is_full_commit(&"a".repeat(39)));
    assert!(!is_full_commit(&"a".repeat(41)));
    assert!(!is_full_commit(&format!("{}; rm -rf /", "a".repeat(40))));
    assert!(!is_full_commit(&format!("{} HEAD", "a".repeat(40))));
    assert!(
        !is_full_commit(&"z".repeat(40)),
        "40 characters is not 40 hex digits"
    );
}

/// A harness over a REAL git repository, so the snapshot's shell is exercised and not mocked.
struct GitHarness {
    repo: std::path::PathBuf,
    candidate: Option<String>,
    refusal: Option<String>,
    base: String,
}

impl RoleHarness for GitHarness {
    fn run_role(&self, _: &str, _: &ActiveForgeRoleTask, _: Option<&str>) -> Result<HarnessOutput> {
        Ok(HarnessOutput {
            trajectory: None,
            raw: String::new(),
            candidate_sha: self.candidate.clone(),
            assay_commands: vec![],
            acceptance_mapped: false,
            refusal: self.refusal.clone(),
            // Reported by the harness, NOT declared through `execution_base_commit` — the run that used to
            // capture nothing because no base was declared.
            execution_base: Some(self.base.clone()),
            usage: Some(HarnessUsage {
                session_id: "ses_turn".into(),
                tokens_input: 26_714,
                tokens_output: 701,
                cost_usd: 0.005352,
            }),
        })
    }
    fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        &self.repo
    }
    fn run_command(&self, command: &str) -> CommandResult {
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.repo)
            .output()
            .expect("sh runs");
        let code = out.status.code().unwrap_or(1);
        CommandResult {
            cancelled: false,
            command: command.into(),
            exit_code: code,
            passed: code == 0,
            excerpt: String::from_utf8_lossy(&out.stderr)
                .chars()
                .take(240)
                .collect(),
            unmeasurable: false,
            output: String::from_utf8_lossy(&out.stdout).to_string(),
        }
    }
}

fn git(repo: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A repo with a base commit, one Smith commit on top, an uncommitted edit and an untracked file.
fn smith_left_a_mess(name: &str) -> (std::path::PathBuf, String, String) {
    let repo = std::env::temp_dir().join(format!("forge-capture-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(&repo).expect("temp repo");
    git(&repo, &["init", "-q", "--initial-branch=main", "."]);
    git(&repo, &["config", "user.email", "forge@test.invalid"]);
    git(&repo, &["config", "user.name", "forge test"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    std::fs::write(repo.join("tracked.rs"), "base\n").expect("seed");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);
    let base = git(&repo, &["rev-parse", "HEAD"]);
    std::fs::write(repo.join("committed.rs"), "fn committed() {}\n").expect("commit");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "smith"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    std::fs::write(repo.join("tracked.rs"), "base\nuncommitted edit\n").expect("dirty");
    std::fs::write(repo.join("untracked.rs"), "fn untracked() {}\n").expect("untracked");
    (repo, base, head)
}

fn smith_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "t".into(),
        process_instance_id: "p".into(),
        story_id: "TST-CAPTURE-001".into(),
        token_id: None,
        node_id: Some("smith".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec!["smith".into()],
        write_surface: None,
    }
}

/// The bug this test exists for: a REFUSED candidate ("uncommitted work remains after candidate …") left no
/// `candidate-code` row, so the code a refusal held — committed, uncommitted and untracked — had no copy off
/// the machine. It is captured now, marked refused, and the lane's real index is left exactly as it was.
#[test]
fn refused_smith_work_is_captured_whole_without_touching_the_index() {
    let (repo, base, head) = smith_left_a_mess("refused");
    let status_before = git(&repo, &["status", "--porcelain"]);
    let harness = GitHarness {
        repo: repo.clone(),
        candidate: None,
        refusal: Some(format!("uncommitted work remains after candidate {head}")),
        base: base.clone(),
    };
    let writer = Arc::new(crate::engine::writer::RecordingWriter::default());
    let runner = ProductionRoleRunner::new(Arc::new(harness), ForgeGateEvidence::default())
        .with_writer(writer.clone());
    let _ = SmithService::new(&runner).execute("smith", &smith_task());

    let artifacts = writer.artifacts.lock().unwrap();
    let capture = artifacts
        .iter()
        .find(|a| a.kind == "candidate-code")
        .expect("refused work is still captured");
    let detail = capture.detail.as_ref().expect("the code is the payload");
    let patch = detail["patch"].as_str().unwrap_or_default();
    assert!(
        patch.contains("fn committed() {}"),
        "the commit is in the patch"
    );
    assert!(
        patch.contains("uncommitted edit"),
        "the uncommitted edit is in the patch"
    );
    assert!(
        patch.contains("fn untracked() {}"),
        "the untracked file is in the patch"
    );
    assert!(detail["refusal"]
        .as_str()
        .unwrap_or_default()
        .contains("uncommitted work remains"));
    assert_eq!(detail["base"].as_str(), Some(base.as_str()));
    assert_eq!(capture.sha.as_deref(), Some(head.as_str()));
    assert!(capture
        .summary
        .as_deref()
        .unwrap_or_default()
        .starts_with("REFUSED"));
    assert_eq!(
        git(&repo, &["status", "--porcelain"]),
        status_before,
        "the snapshot must not stage anything in the lane's real index"
    );
    let _ = std::fs::remove_dir_all(&repo);
}

/// The refusal a lane is handed must reach the gate NAMED, not as the lifecycle's boilerplate. "role did not
/// deliver smith-candidate" is true of a turn that delivered nothing and WRONG about a candidate that was
/// delivered and refused — and that string is what the board's hold record and the next attempt's repair
/// directive carry, so the lane's own words have to be the ones sitting there.
#[test]
fn a_refusal_reaches_the_gate_as_the_refusal_and_not_as_boilerplate() {
    let (repo, base, head) = smith_left_a_mess("refusal-named");
    let refusal = format!("uncommitted work remains after candidate {head}");
    let harness = GitHarness {
        repo: repo.clone(),
        candidate: None,
        refusal: Some(refusal.clone()),
        base: base.clone(),
    };
    let writer = Arc::new(crate::engine::writer::RecordingWriter::default());
    let runner = ProductionRoleRunner::new(Arc::new(harness), ForgeGateEvidence::default())
        .with_writer(writer.clone());

    let outcome = SmithService::new(&runner)
        .execute("smith", &smith_task())
        .expect("the lane runs");

    assert!(
        outcome.evidence.candidate_sha.is_none(),
        "a refused candidate is not frozen, so no QA node has anything to be handed"
    );
    assert_eq!(
        outcome.evidence.deliverable_rejection.as_deref(),
        Some(refusal.as_str()),
        "the gate and the hold record name the refusal the lane made, not the fallback"
    );
    let _ = std::fs::remove_dir_all(&repo);
}

/// A writer whose run stamp always fails, so the test proves what a stamp DB error costs.
struct StampFailingWriter {
    inner: Arc<crate::engine::writer::RecordingWriter>,
}

impl crate::engine::writer::ForgeStateWriter for StampFailingWriter {
    fn mark_story_human_hold(&self, s: &str, r: &str) -> std::result::Result<(), String> {
        self.inner.mark_story_human_hold(s, r)
    }
    fn mark_story_complete(&self, s: &str) -> std::result::Result<(), String> {
        self.inner.mark_story_complete(s)
    }
    fn mark_story_in_progress(&self, s: &str) -> std::result::Result<(), String> {
        self.inner.mark_story_in_progress(s)
    }
    fn stamp_run_candidate(&self, _r: &str, _s: &str) -> std::result::Result<(), String> {
        Err("injected stamp failure: storyboard_story_run unavailable".into())
    }
    fn append_run_detail(&self, r: &str, d: &str) -> std::result::Result<(), String> {
        self.inner.append_run_detail(r, d)
    }
    fn open_hold(
        &self,
        input: &crate::engine::hold::OpenHold,
    ) -> std::result::Result<String, String> {
        self.inner.open_hold(input)
    }
    fn record_tool_artifact(
        &self,
        input: &db::NewToolArtifact,
    ) -> std::result::Result<Option<String>, String> {
        self.inner.record_tool_artifact(input)
    }
    fn record_run_usage(
        &self,
        r: &str,
        u: &crate::engine::harness::HarnessUsage,
    ) -> std::result::Result<(), String> {
        self.inner.record_run_usage(r, u)
    }
}

/// The bug this test exists for: the run stamp (`stamp_run_candidate`) ran BEFORE the fail-safe
/// capture, so a stamp DB error propagated with `?` and the candidate's code was never written
/// down. The capture must run first, so a stamp failure still leaves the code behind.
#[test]
fn a_stamp_failure_still_leaves_the_capture_behind() {
    let (repo, base, head) = smith_left_a_mess("stamp-fails");
    git(&repo, &["checkout", "-q", "--", "tracked.rs"]);
    std::fs::remove_file(repo.join("untracked.rs")).expect("clean tree");
    let harness = GitHarness {
        repo: repo.clone(),
        candidate: Some(head.clone()),
        refusal: None,
        base: base.clone(),
    };
    let inner = Arc::new(crate::engine::writer::RecordingWriter::default());
    let writer: Arc<dyn crate::engine::writer::ForgeStateWriter> = Arc::new(StampFailingWriter {
        inner: inner.clone(),
    });
    let runner = ProductionRoleRunner::new(Arc::new(harness), ForgeGateEvidence::default())
        .with_writer(writer)
        .with_story_run(Some("22222222-3333-4444-5555-666666666666".into()));
    let outcome = SmithService::new(&runner).execute("smith", &smith_task());

    assert!(
        outcome.is_err(),
        "the stamp really did fail: without the injected failure this test proves nothing"
    );
    let artifacts = inner.artifacts.lock().unwrap();
    assert!(
        artifacts.iter().any(|a| a.kind == "candidate-code"),
        "a stamp DB error must still leave a capture behind"
    );
    let _ = std::fs::remove_dir_all(&repo);
}

/// The bug this test exists for: a harness that declared no execution base (`FORGE_WORKTREE` & co. unset) got
/// no capture at all, silently, even for an ACCEPTED candidate. The base the harness measured against is used.
#[test]
fn an_accepted_candidate_is_captured_without_a_declared_base() {
    let (repo, base, head) = smith_left_a_mess("accepted");
    git(&repo, &["checkout", "-q", "--", "tracked.rs"]);
    std::fs::remove_file(repo.join("untracked.rs")).expect("clean tree");
    let harness = GitHarness {
        repo: repo.clone(),
        candidate: Some(head.clone()),
        refusal: None,
        base: base.clone(),
    };
    let writer = Arc::new(crate::engine::writer::RecordingWriter::default());
    let runner = ProductionRoleRunner::new(Arc::new(harness), ForgeGateEvidence::default())
        .with_writer(writer.clone());
    let _ = SmithService::new(&runner).execute("smith", &smith_task());

    let artifacts = writer.artifacts.lock().unwrap();
    let capture = artifacts
        .iter()
        .find(|a| a.kind == "candidate-code")
        .expect("an accepted candidate is captured even with no declared base");
    let detail = capture.detail.as_ref().expect("the code is the payload");
    assert!(detail["patch"]
        .as_str()
        .unwrap_or_default()
        .contains("fn committed() {}"));
    assert_eq!(detail["changedFiles"][0].as_str(), Some("committed.rs"));
    assert!(
        detail.get("refusal").is_none(),
        "an accepted candidate is not marked refused"
    );
    let _ = std::fs::remove_dir_all(&repo);
}
