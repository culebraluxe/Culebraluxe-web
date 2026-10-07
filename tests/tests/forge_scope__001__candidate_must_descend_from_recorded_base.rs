//! FORGE.SCOPE — candidate must descend from recorded base (TST-FORGE-SCOPE-001).
//!
//! Contract: a delivered candidate is only a candidate because it descends from the base the lane recorded for
//! the run. Two production seams enforce it and they may not disagree:
//!
//!   1. **the pure scope boundary** — `candidate_own_changed_files`
//!      (`forge/src/engine/scope.rs:41`). A candidate that is not a descendant of the recorded base is
//!      `CandidateOwnChanges::Fail`, never a changed-file list measured from an unrelated history. The same
//!      function also fails closed on a candidate that is not a full commit and on a run with no recorded base
//!      at all, so "cannot tell" is never read as "in scope".
//!   2. **the lane gate that consumes it** — `judge_delivered_candidate`
//!      (`forge/src/roles/smith.rs:74`) → `candidate_rejection` (`smith.rs:102`). Before the scope boundary is
//!      even reached, Smith's own reading asks git `merge-base --is-ancestor <base> <after>`. A git that will
//!      not answer refuses the candidate (`is_none()` → refuse), which is the fail-closed half: an unreadable
//!      ancestry is a refusal, not a pass.
//!
//! The story's word is "recorded", and that is load-bearing: the base is the one the run recorded
//! (`HarnessOutput.execution_base`, `forge/src/engine/runner.rs:23`), not whatever `origin/main` happens to be
//! when the gate runs. A boundary that quietly re-resolved the base would accept work built on a tree this run
//! never read.
//!
//! Negative and fault cases are load-bearing here — without them a test could pass on a gate that refuses
//! everything. So the same lane gate is also driven with a healthy descending candidate (accepted), with HEAD
//! unchanged (refused as "no new commit"), and with a git whose ancestry query fails (refused, not accepted).
//!
//! Deterministic and isolated: no database, no network, no external provider, no environment mutation. The
//! repository is a scripted `CandidateProbe` at the boundary production injects; nothing is read from disk and
//! nothing is written anywhere.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_scope__001__candidate_must_descend_from_recorded_base

use std::collections::HashMap;

use forge::engine::assay::CommandResult;
use forge::engine::runner::{CandidateProbe, HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::scope::{candidate_own_changed_files, CandidateOwnChanges};
use forge::roles::smith::{delivers_code, judge_delivered_candidate};
use workflow::TaskStatus;

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L1 Component";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-SCOPE-001";
/// The base the run RECORDED. Not a branch name: a recorded base is a commit.
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// The candidate the lane produced on top of it.
const AFTER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
/// A commit that exists in the object store but is on neither the base nor the candidate's history.
const FOREIGN: &str = "ffffffffffffffffffffffffffffffffffffffff";

/// A repository described by its answers: each git invocation (args joined by spaces) maps to its stdout, and
/// an invocation with no answer is a git that failed. This is the boundary production injects
/// (`CandidateProbe`), so the rules under test are Smith's, never the transport's.
struct RecordedRepo {
    answers: HashMap<String, String>,
}

impl RecordedRepo {
    /// A clean, descending candidate that changes one test file.
    fn descending() -> Self {
        let mut answers = HashMap::new();
        answers.insert(format!("merge-base --is-ancestor {BASE} {AFTER}"), String::new());
        answers.insert("status --porcelain".into(), String::new());
        answers.insert(
            format!("diff --name-only --no-renames {BASE}..{AFTER}"),
            "tests/tests/forge_scope__001.rs".into(),
        );
        Self { answers }
    }

    /// The same run, with the ancestry query answered as "not an ancestor".
    fn not_descending() -> Self {
        Self::descending().answer(
            &format!("merge-base --is-ancestor {BASE} {AFTER}"),
            None,
        )
    }

    /// The same run, with the ancestry query failing outright — the fault case.
    fn ancestry_unreadable() -> Self {
        Self::descending().answer(
            &format!("merge-base --is-ancestor {BASE} {AFTER}"),
            None,
        )
    }

    fn answer(mut self, args: &str, stdout: Option<&str>) -> Self {
        match stdout {
            Some(out) => self.answers.insert(args.into(), out.into()),
            None => self.answers.remove(args),
        };
        self
    }
}

impl CandidateProbe for RecordedRepo {
    fn git(&self, args: &[&str]) -> Option<String> {
        self.answers.get(&args.join(" ")).cloned()
    }
    fn declared_test_mode(&self) -> Option<&str> {
        None
    }
}

impl RoleHarness for RecordedRepo {
    fn run_role(
        &self,
        _: &str,
        _: &forge::engine::runtime::ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        unreachable!("the judgement never runs a turn")
    }
    fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn run_command(&self, _: &str) -> CommandResult {
        unreachable!("the judgement runs no command")
    }
    fn candidate_probe(&self) -> Option<&dyn CandidateProbe> {
        Some(self)
    }
}

/// The Smith turn as a code-delivering node leaves it: the recorded base, the HEAD it produced, and nothing
/// else the gate might take comfort in.
fn delivered_turn(base: &str, after: &str) -> HarnessOutput {
    HarnessOutput {
        raw: format!("{STORY_ID}: authored the canonical contract test and committed it"),
        candidate_sha: Some(after.into()),
        assay_commands: Vec::new(),
        acceptance_mapped: true,
        refusal: None,
        execution_base: Some(base.into()),
        usage: None,
    }
}

/// The Smith task-node as the engine lists it: this story, a claimed lane, an identity to write against. The
/// gate is asked the question production asks it, for the story this file is filed under.
fn smith_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-scope-001".into(),
        process_instance_id: "instance-scope-001".into(),
        story_id: STORY_ID.into(),
        token_id: Some("token-scope-001".into()),
        node_id: Some("smith".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["smith".into()],
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SCOPE-001); the file and the assay use it.
fn forge_scope_001__candidate_must_descend_from_recorded_base() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — A FOREIGN CANDIDATE IS NOT A DESCENDANT OF THE RECORDED BASE. The scope boundary is
    //    handed a candidate on unrelated history and asked what it changed. It must refuse, and it must say
    //    why in terms of the recorded base.
    // -----------------------------------------------------------------------------------------------------------
    let foreign = candidate_own_changed_files(
        Some(AFTER),
        Some(BASE),
        &[],
        |_| vec!["tests/tests/forge_scope__001.rs".into()],
        // git says the base is not an ancestor of the candidate.
        |_, _| false,
    );
    let refusal = match &foreign {
        CandidateOwnChanges::Fail { reason } => reason.clone(),
        other => panic!(
            "{HARNESS}: a candidate that does not descend from its recorded base must be refused, got {other:?}"
        ),
    };
    assert!(
        refusal.contains("not a descendant") && refusal.contains(BASE) && refusal.contains(AFTER),
        "{HARNESS}: the refusal must name the candidate and the base it failed to descend from, got: {refusal:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1b. THE QUESTION IS ASKED OF A NODE THAT DELIVERS CODE. A gate that only judged non-code nodes would
    //     satisfy every other assertion in this file while never running on Smith's own task.
    // -----------------------------------------------------------------------------------------------------------
    let task = smith_task();
    assert_eq!(
        task.story_id, STORY_ID,
        "{HARNESS}: this proof runs against the story it is filed under"
    );
    assert!(
        delivers_code(task.node_id.as_deref().unwrap_or_default()),
        "{HARNESS}: smith is a code-delivering node, so this gate is the one that judges candidates"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE LANE GATE CONSUMES THE SAME RULE. Smith's own reading asks git the ancestry question directly and
    //    refuses before the scope boundary is reached. The refusal is attached to the output and the candidate
    //    is cleared, so nothing downstream can mistake it for a delivery.
    // -----------------------------------------------------------------------------------------------------------
    let harness = RecordedRepo::not_descending();
    let mut out = delivered_turn(BASE, AFTER);
    judge_delivered_candidate(&harness, "smith", &mut out);
    assert!(
        out.candidate_sha.is_none(),
        "{HARNESS}: a candidate that does not descend from the execution base must not remain the run's candidate"
    );
    let lane_refusal = out.refusal.as_deref().unwrap_or_default();
    assert!(
        lane_refusal.contains("not a descendant of execution base"),
        "{HARNESS}: the lane gate must refuse on ancestry, got: {lane_refusal:?}"
    );
    assert!(
        out.raw.contains("SMITH_CANDIDATE_REJECTED"),
        "{HARNESS}: the refusal must be written into the turn's own output, got: {:?}",
        out.raw
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. CONTROL — A DESCENDING CANDIDATE IS ACCEPTED. Same gate, same rules, an ancestry git answers
    //    affirmatively. Without this the whole proof would be satisfied by a gate that refuses every candidate,
    //    which is not the contract.
    // -----------------------------------------------------------------------------------------------------------
    let healthy = RecordedRepo::descending();
    let mut accepted = delivered_turn(BASE, AFTER);
    judge_delivered_candidate(&healthy, "smith", &mut accepted);
    assert_eq!(
        accepted.candidate_sha.as_deref(),
        Some(AFTER),
        "{HARNESS}: a clean descending candidate is accepted"
    );
    assert!(
        accepted.refusal.is_none(),
        "{HARNESS}: an accepted candidate carries no refusal, got: {:?}",
        accepted.refusal
    );

    // The pure boundary agrees: the same candidate on the same recorded base measures cleanly.
    let measured = candidate_own_changed_files(
        Some(AFTER),
        Some(BASE),
        &[AFTER.to_string()],
        |sha| {
            assert_eq!(sha, AFTER, "{HARNESS}: only the candidate is read here");
            vec!["tests/tests/forge_scope__001.rs".into()]
        },
        |ancestor, descendant| {
            // git's own rule: a commit is an ancestor of itself, and the recorded base is this one's parent.
            ancestor == BASE && descendant == AFTER
        },
    );
    assert!(
        matches!(measured, CandidateOwnChanges::Ok { .. }),
        "{HARNESS}: a descending candidate measures cleanly, got: {measured:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — HEAD THAT NEVER MOVED IS NOT A CANDIDATE. `after == base` is the degenerate ancestry: git
    //    would answer "is an ancestor" affirmatively, so a gate that only asked the ancestry question would let
    //    a turn that delivered nothing through. Production refuses on the equality first.
    // -----------------------------------------------------------------------------------------------------------
    let no_new_commit = RecordedRepo::descending()
        .answer(
            &format!("merge-base --is-ancestor {BASE} {BASE}"),
            Some(""),
        )
        .answer(
            &format!("diff --name-only --no-renames {BASE}..{BASE}"),
            Some(""),
        );
    let mut unchanged = delivered_turn(BASE, BASE);
    judge_delivered_candidate(&no_new_commit, "smith", &mut unchanged);
    assert!(
        unchanged.candidate_sha.is_none(),
        "{HARNESS}: a turn that created no commit leaves no candidate"
    );
    assert!(
        unchanged
            .refusal
            .as_deref()
            .unwrap_or_default()
            .contains("no new commit was created"),
        "{HARNESS}: the refusal must name the missing commit, got: {:?}",
        unchanged.refusal
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. FAULT — AN UNREADABLE ANCESTRY IS A REFUSAL, NOT A PASS. `CandidateProbe::git` answers `None` both
    //    for "git said no" and for "git could not answer", and the two must not be conflated: a repository whose
    //    object store is unreadable would otherwise certify every candidate it cannot check.
    // -----------------------------------------------------------------------------------------------------------
    let faulted = RecordedRepo::ancestry_unreadable();
    let mut faulted_out = delivered_turn(BASE, AFTER);
    judge_delivered_candidate(&faulted, "smith", &mut faulted_out);
    assert!(
        faulted_out.candidate_sha.is_none(),
        "{HARNESS}: an unreadable ancestry must not certify a candidate"
    );
    assert!(
        faulted_out
            .refusal
            .as_deref()
            .unwrap_or_default()
            .contains("not a descendant"),
        "{HARNESS}: the fault surfaces as the ancestry refusal, got: {:?}",
        faulted_out.refusal
    );

    // The pure boundary fails closed on the same two shapes a lineage can be missing in: a candidate that is
    // not a commit, and a run that recorded no base at all.
    for (label, candidate, base) in [
        ("a candidate that is not a commit", Some("HEAD"), Some(BASE)),
        ("a run with no recorded base", Some(AFTER), None),
        ("a recorded base that is blank", Some(AFTER), Some("   ")),
    ] {
        let outcome = candidate_own_changed_files(
            candidate,
            base,
            &[],
            |_| vec!["tests/tests/forge_scope__001.rs".into()],
            // Ancestry would happily say yes. These cases must be refused BEFORE it is consulted.
            |_, _| true,
        );
        assert!(
            matches!(outcome, CandidateOwnChanges::Fail { .. }),
            "{HARNESS}: {label} must be refused, got: {outcome:?}"
        );
    }

    // A candidate on a history that is not the recorded base's at all is still a foreign candidate: the
    // foreign sha proves the object store was reachable and the ancestry was genuinely false.
    let unreachable_history = candidate_own_changed_files(
        Some(FOREIGN),
        Some(BASE),
        &[FOREIGN.to_string()],
        |_| vec!["tests/tests/forge_scope__001.rs".into()],
        |_, _| false,
    );
    assert!(
        matches!(unreachable_history, CandidateOwnChanges::Fail { .. }),
        "{HARNESS}: a candidate unrelated to the recorded base must be refused, got: {unreachable_history:?}"
    );
}
