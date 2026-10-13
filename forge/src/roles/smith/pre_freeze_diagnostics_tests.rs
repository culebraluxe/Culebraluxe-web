//! C5: a candidate's own work must compile before the candidate is frozen.
//!
//! These tests judge a candidate in a DECLARED worktree, whose answers the double holds: the git facts the structural
//! rules read, cargo's answer for the workspace's crates, and what each planned check answers. Every command the gate
//! ran is recorded, so each test can assert what was NOT run — the confinement of the check is the property worth
//! pinning, not just its verdict.

use super::*;
use crate::engine::assay::CommandResult;
use crate::engine::packet::ExecutionWorkspace;
use crate::engine::runtime::ActiveForgeRoleTask;
use std::sync::Mutex;

const BASE: &str = "1111111111111111111111111111111111111111";
const AFTER: &str = "2222222222222222222222222222222222222222";
/// A temp path, the way every other `ExecutionWorkspace` fixture in this tree names one. It deliberately does NOT
/// name the deleted per-lane worktree estate's directory: `cli/src/forge/repo_guards.rs` scans for that shape and
/// fails any file that re-introduces it, and a fixture that merely needs *a* path has no business naming it.
const WORKTREE: &str = "/tmp/wt-story-1";
const METADATA: &str =
    "cargo metadata --no-deps --format-version 1 --manifest-path /tmp/wt-story-1/Cargo.toml";
/// What a compiler says, so a refusal that quotes it can be checked for quoting it.
const COMPILER_SAYS: &str = "error[E0425]: cannot find value `x` in this scope";

/// A candidate in a declared worktree, described by its answers.
struct Worktree {
    /// The declared worktree; `None` for a harness that declares none (every double, and a run with no worktree).
    workspace: Option<ExecutionWorkspace>,
    /// The candidate's changed paths as git reports them; `None` is a git that cannot be read.
    changed: Option<&'static str>,
    /// Cargo's answer for the crate list; `None` is a workspace cargo cannot read.
    crates: Option<String>,
    /// Whether the read of that list reported failure, whatever it left on stdout.
    metadata_fails: bool,
    /// Whether the tree is dirty, so a structural refusal can be proved to precede any compile.
    dirty: bool,
    /// What each planned command answers; a command with no answer fails the way a compiler fails.
    answers: Vec<(&'static str, bool, &'static str)>,
    /// Every command the gate actually ran, in order.
    ran: Mutex<Vec<String>>,
}

impl Worktree {
    /// A clean, descending candidate that changed `changed`, in a readable workspace.
    fn candidate(changed: &'static str) -> Self {
        Self {
            workspace: Some(ExecutionWorkspace {
                worktree_path: WORKTREE.into(),
                branch_name: "forge/story-1".into(),
                base_ref: "main".into(),
                base_commit: BASE.into(),
            }),
            changed: Some(changed),
            crates: Some(workspace_json()),
            metadata_fails: false,
            dirty: false,
            answers: Vec::new(),
            ran: Mutex::new(Vec::new()),
        }
    }

    fn answer(mut self, command: &'static str, passed: bool, excerpt: &'static str) -> Self {
        self.answers.push((command, passed, excerpt));
        self
    }

    /// A candidate that left uncommitted work behind: refused before it is ever compiled.
    fn dirty(mut self) -> Self {
        self.dirty = true;
        self
    }

    fn without_worktree(mut self) -> Self {
        self.workspace = None;
        self
    }

    fn without_workspace(mut self) -> Self {
        self.crates = None;
        self
    }

    /// A workspace whose read FAILED while still writing a document to stdout.
    fn failed_read_of_a_readable_workspace(mut self) -> Self {
        self.metadata_fails = true;
        self
    }

    fn ran(&self) -> Vec<String> {
        self.ran.lock().expect("the command log").clone()
    }

    fn result(&self, command: &str, passed: bool, excerpt: &str, output: &str) -> CommandResult {
        CommandResult {
            cancelled: false,
            command: command.into(),
            exit_code: if passed { 0 } else { 101 },
            passed,
            excerpt: excerpt.into(),
            unmeasurable: false,
            output: output.into(),
        }
    }
}

/// A workspace of three crates as cargo states it, so a plan can name them and a test can prove the ones OUTSIDE the
/// change were never checked.
fn workspace_json() -> String {
    let crates = [
        ("app", "app"),
        ("lib", "middle/lib"),
        ("docs", "tools/docs"),
    ];
    let packages: Vec<serde_json::Value> = crates
        .iter()
        .map(|(name, dir)| {
            serde_json::json!({
                "id": format!("path+file://{WORKTREE}/{dir}#0.1.0"),
                "name": name,
                "manifest_path": format!("{WORKTREE}/{dir}/Cargo.toml"),
            })
        })
        .collect();
    let members: Vec<String> = packages
        .iter()
        .map(|package| package["id"].as_str().unwrap_or_default().to_string())
        .collect();
    serde_json::json!({
        "workspace_root": WORKTREE,
        "workspace_members": members,
        "packages": packages,
    })
    .to_string()
}

impl CandidateProbe for Worktree {
    fn git(&self, args: &[&str]) -> Option<String> {
        match args.join(" ").as_str() {
            "status --porcelain" if self.dirty => Some(" M app/src/lib.rs".into()),
            "status --porcelain" => Some(String::new()),
            _ if args.first() == Some(&"diff") => self.changed.map(str::to_string),
            _ => Some(String::new()),
        }
    }
    fn declared_test_mode(&self) -> Option<&str> {
        None
    }
}

impl RoleHarness for Worktree {
    fn run_role(&self, _: &str, _: &ActiveForgeRoleTask, _: Option<&str>) -> Result<HarnessOutput> {
        unreachable!("the judgement never runs a turn")
    }
    fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(WORKTREE)
    }
    fn run_command(&self, command: &str) -> CommandResult {
        self.ran
            .lock()
            .expect("the command log")
            .push(command.into());
        if command == METADATA {
            if self.metadata_fails {
                // A failed read that still wrote a parseable document: the exit status is the answer, and a caller
                // that read only stdout would plan checks out of a command that said it failed.
                return self.result(
                    command,
                    false,
                    "error: failed to parse manifest",
                    &workspace_json(),
                );
            }
            return match self.crates.as_deref() {
                Some(json) => self.result(command, true, "", json),
                None => self.result(
                    command,
                    false,
                    "error: could not find `Cargo.toml` in `/tmp/wt-story-1` or any parent directory",
                    "",
                ),
            };
        }
        let (passed, excerpt) = self
            .answers
            .iter()
            .find(|(answered, _, _)| *answered == command)
            .map(|(_, passed, excerpt)| (*passed, *excerpt))
            .unwrap_or((false, COMPILER_SAYS));
        self.result(command, passed, excerpt, "")
    }
    fn candidate_probe(&self) -> Option<&dyn CandidateProbe> {
        Some(self)
    }
    fn execution_workspace(&self) -> Option<&ExecutionWorkspace> {
        self.workspace.as_ref()
    }
}

fn output(after: Option<&str>) -> HarnessOutput {
    HarnessOutput {
        trajectory: None,
        raw: "smith answered".into(),
        candidate_sha: after.map(str::to_string),
        assay_commands: vec![],
        acceptance_mapped: false,
        refusal: None,
        execution_base: Some(BASE.into()),
        usage: None,
    }
}

fn judge(harness: &dyn RoleHarness) -> HarnessOutput {
    let mut out = output(Some(AFTER));
    judge_delivered_candidate_in_surface(harness, "smith", &mut out, None);
    out
}

#[test]
fn a_candidate_that_does_not_compile_is_refused_and_the_compiler_is_quoted() {
    let worktree =
        Worktree::candidate("app/src/lib.rs").answer("cargo check -p app", false, COMPILER_SAYS);
    let out = judge(&worktree);

    assert_eq!(
        out.candidate_sha, None,
        "a candidate that does not compile is not the run's candidate"
    );
    let refusal = out.refusal.clone().expect("the refusal is recorded");
    assert!(
        refusal.starts_with("Smith candidate refused for smith: "),
        "every refusal is worded the same way: {refusal}"
    );
    assert!(refusal.contains("cargo check -p app"), "{refusal}");
    assert!(
        refusal.contains(COMPILER_SAYS),
        "the compiler's own words are Smith's feedback, not a summary of them: {refusal}"
    );
    assert!(
        out.raw
            .contains(&format!("SMITH_CANDIDATE_REJECTED: {refusal}")),
        "the reply carries the marker the next attempt is prompted with: {}",
        out.raw
    );
    assert_eq!(
        worktree.ran(),
        vec![METADATA.to_string(), "cargo check -p app".to_string()],
        "the crate list is read once and each planned check runs once"
    );
}

#[test]
fn a_candidate_that_compiles_is_frozen_with_only_its_own_check_run() {
    let worktree = Worktree::candidate("app/src/lib.rs").answer("cargo check -p app", true, "");
    let out = judge(&worktree);

    assert_eq!(out.candidate_sha.as_deref(), Some(AFTER));
    assert_eq!(out.refusal, None);
    assert_eq!(
        worktree.ran(),
        vec![METADATA.to_string(), "cargo check -p app".to_string()],
        "no crate outside the change is checked, and no check is run twice"
    );
    assert!(
        !worktree
            .ran()
            .iter()
            .any(|command| command.contains("test") || command.contains("--workspace")),
        "pre-freeze diagnostics never run the suite and never widen beyond the change: {:?}",
        worktree.ran()
    );
}

#[test]
fn the_check_is_confined_to_the_crates_the_candidate_touched() {
    let worktree = Worktree::candidate("app/src/lib.rs\nmiddle/lib/src/x.rs")
        .answer("cargo check -p app", true, "")
        .answer("cargo check -p lib", true, "");
    let out = judge(&worktree);

    assert_eq!(out.candidate_sha.as_deref(), Some(AFTER));
    assert_eq!(
        worktree.ran(),
        vec![
            METADATA.to_string(),
            "cargo check -p app".to_string(),
            "cargo check -p lib".to_string(),
        ],
        "the two crates the candidate touched are checked, in the workspace's own order, and the third is not"
    );
}

#[test]
fn a_change_that_compiles_nothing_is_not_checked() {
    let worktree = Worktree::candidate("docs/agent/notes.md");
    let out = judge(&worktree);

    assert_eq!(out.candidate_sha.as_deref(), Some(AFTER));
    assert_eq!(out.refusal, None);
    assert!(
        worktree.ran().is_empty(),
        "a change that compiles nothing owes no diagnostic, and asking cargo for the crate list to learn that \
         would be a process spent for nothing: {:?}",
        worktree.ran()
    );
}

#[test]
fn a_workspace_level_change_is_checked_across_the_workspace() {
    let worktree = Worktree::candidate("Cargo.lock").answer("cargo check --workspace", true, "");
    let out = judge(&worktree);

    assert_eq!(out.candidate_sha.as_deref(), Some(AFTER));
    assert_eq!(
        worktree.ran(),
        vec![METADATA.to_string(), "cargo check --workspace".to_string()],
        "an unconfined change gets an unconfined check rather than a guessed crate"
    );
}

#[test]
fn a_candidate_refused_on_the_structural_rules_is_never_compiled() {
    let worktree = Worktree::candidate("app/src/lib.rs").dirty();
    let out = judge(&worktree);

    let refusal = out.refusal.clone().expect("the refusal is recorded");
    assert!(refusal.contains("uncommitted work remains"), "{refusal}");
    assert!(
        worktree.ran().is_empty(),
        "a candidate already refused for its tree is not compiled: the reason in hand is the stronger one, and \
         the minutes are not spent: {:?}",
        worktree.ran()
    );
}

#[test]
fn a_harness_that_declares_no_worktree_is_never_asked_to_compile() {
    let worktree = Worktree::candidate("app/src/lib.rs").without_worktree();
    let out = judge(&worktree);

    assert_eq!(out.candidate_sha.as_deref(), Some(AFTER));
    assert_eq!(out.refusal, None);
    assert!(
        worktree.ran().is_empty(),
        "compiling is meaningful only inside the candidate's own checkout: {:?}",
        worktree.ran()
    );
}

#[test]
fn an_unreadable_workspace_is_not_a_verdict() {
    let worktree = Worktree::candidate("app/src/lib.rs").without_workspace();
    let out = judge(&worktree);

    assert_eq!(
        out.candidate_sha.as_deref(),
        Some(AFTER),
        "failing to ASK the question must never be recorded as the candidate failing it"
    );
    assert_eq!(out.refusal, None);
    assert_eq!(
        worktree.ran(),
        vec![METADATA.to_string()],
        "the crate list was read, and no check could be planned from it"
    );
}

#[test]
fn a_failed_read_of_the_crate_list_is_not_read_as_a_crate_list() {
    // The read failed and still wrote a document. The exit status is the answer: planning checks from stdout alone
    // would run a candidate's compile off a command that said it could not read the workspace at all.
    let worktree = Worktree::candidate("app/src/lib.rs").failed_read_of_a_readable_workspace();
    let out = judge(&worktree);

    assert_eq!(out.candidate_sha.as_deref(), Some(AFTER));
    assert_eq!(out.refusal, None);
    assert_eq!(
        worktree.ran(),
        vec![METADATA.to_string()],
        "nothing was planned out of a failed read"
    );
}
