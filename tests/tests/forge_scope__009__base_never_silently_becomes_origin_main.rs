//! FORGE.SCOPE — base never silently becomes origin/main (TST-FORGE-SCOPE-009).
//!
//! Contract: the base a lane is measured against is the one THE RUN RECORDED. It never becomes `origin/main`,
//! and it never silently becomes any other revision — including when the recorded base cannot be read.
//!
//! Why the substitution is dangerous rather than merely wrong: `origin/main` MOVES. Every judgement in this
//! taxonomy is a diff against a base — did the candidate descend from it, which files did the lane change, did
//! the range touch production code. Re-resolve the base at judgement time and a run's evidence describes a
//! range nobody chose: a candidate that descended from the recorded base can be refused because `origin/main`
//! has since moved past it, and a candidate that did NOT descend can be accepted because `origin/main` now
//! contains the peer commit it was built on. Both are silent, and both are wrong.
//!
//! Four production seams hold the base, and none of them falls back:
//!
//!   1. **the lane gate** — `candidate_rejection` (`forge/src/roles/smith.rs:109-112`) takes the base from
//!      `HarnessOutput.execution_base` (`forge/src/engine/runner.rs:23`) and REFUSES when it is absent:
//!      `"execution base is unreadable"`. There is no `unwrap_or("origin/main")` on that path, and this test is
//!      what proves it.
//!   2. **the scope boundary** — `candidate_own_changed_files` (`forge/src/engine/scope.rs:56-61`) refuses a
//!      missing or blank recorded base: `"candidate {sha} has no recorded base to measure against"`.
//!   3. **the QA gate** — `rust_contract_production_edits` (`forge/src/engine/assay.rs:92-95`) answers
//!      `"QA FAIL: RUST_CONTRACT execution base is missing."` for an absent or blank base. It cannot report
//!      "no production edits" for a range it never measured.
//!   4. **the workspace provisioner** — `resolve_base_commit` (`forge/src/engine/worktree.rs:106-118`) resolves
//!      a ref to a commit and FAILS if what comes back is not 40 hex characters, and
//!      `provision_worker_workspace` (`worktree.rs:144-150`) REFUSES an empty `base_ref` outright rather than
//!      defaulting. That is where `origin/main` would come from if anything let it: `resolve_approved_base_ref`
//!      (`worktree.rs:38-44`) defaults to `origin/main` when `AGENT_WORKSPACE_BASE_REF` is unset — so the
//!      protection is that a default ref must still RESOLVE to a real commit at provisioning time, and a run
//!      whose base was recorded from it carries that commit, not the moving name.
//!
//! Deterministic and isolated: section 4 runs real git in a disposable repository
//! (`test_harness::git::DisposableRepo`) so "a ref that cannot be resolved is refused" is observed. Nothing
//! reads or writes the operator's repository, the network, or PROD, and no environment variable is set or
//! unset — `resolve_approved_base_ref` is asserted by SHAPE (a non-empty ref), never by mutating the process
//! environment a parallel test may also be reading.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_scope__009__base_never_silently_becomes_origin_main

use std::collections::HashMap;

use forge::engine::assay::{rust_contract_production_edits, CommandResult};
use forge::engine::runner::{CandidateProbe, HarnessOutput, RoleHarness};
use forge::engine::scope::{candidate_own_changed_files, CandidateOwnChanges};
use forge::engine::worktree::{
    provision_worker_workspace, resolve_approved_base_ref, resolve_base_commit,
};
use forge::roles::smith::judge_delivered_candidate;
use test_harness::git::{git_available, DisposableRepo};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L1 Component";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-SCOPE-009";
/// The base the run RECORDED. A commit, never a branch name.
const RECORDED_BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// The candidate the lane produced on top of it.
const AFTER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// A repository described by its answers. `origin/main` is never an answer it can give: the probe answers only
/// the exact revisions the recorded base names, so any fallback to the moving name would show up as a refusal.
struct Repo {
    answers: HashMap<String, String>,
}

impl Repo {
    fn healthy() -> Self {
        let mut answers = HashMap::new();
        answers.insert(
            format!("merge-base --is-ancestor {RECORDED_BASE} {AFTER}"),
            String::new(),
        );
        answers.insert("status --porcelain".into(), String::new());
        answers.insert(
            format!("diff --name-only --no-renames {RECORDED_BASE}..{AFTER}"),
            "tests/tests/forge_scope__009.rs".into(),
        );
        Self { answers }
    }
}

impl CandidateProbe for Repo {
    fn git(&self, args: &[&str]) -> Option<String> {
        self.answers.get(&args.join(" ")).cloned()
    }
    fn declared_test_mode(&self) -> Option<&str> {
        Some("RUST_CONTRACT")
    }
}

impl RoleHarness for Repo {
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

/// A delivered turn whose `execution_base` is whatever the caller says it is.
fn turn_with_base(base: Option<&str>) -> HarnessOutput {
    HarnessOutput {
        raw: format!("{STORY_ID}: authored the canonical contract test and committed it"),
        candidate_sha: Some(AFTER.into()),
        assay_commands: Vec::new(),
        acceptance_mapped: true,
        refusal: None,
        execution_base: base.map(str::to_string),
        usage: None,
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SCOPE-009); the file and the assay use it.
fn forge_scope_009__base_never_silently_becomes_origin_main() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — THE RECORDED BASE IS THE BASE. A run that recorded a base is judged against exactly
    //    that revision, and the candidate is accepted. The probe is incapable of answering for `origin/main`,
    //    so this passing proves the judgement used the recorded base rather than re-resolving a name.
    // -----------------------------------------------------------------------------------------------------------
    let mut accepted = turn_with_base(Some(RECORDED_BASE));
    judge_delivered_candidate(&Repo::healthy(), "smith", &mut accepted);
    assert_eq!(
        accepted.candidate_sha.as_deref(),
        Some(AFTER),
        "{HARNESS}: a candidate on the recorded base is a candidate ({STORY_ID})"
    );
    assert!(
        accepted.refusal.is_none(),
        "{HARNESS}: an accepted candidate carries no refusal, got: {:?}",
        accepted.refusal
    );
    // The scope boundary agrees, and measures against the recorded base.
    let measured = candidate_own_changed_files(
        Some(AFTER),
        Some(RECORDED_BASE),
        &[AFTER.to_string()],
        |_| vec!["tests/tests/forge_scope__009.rs".into()],
        |ancestor, descendant| {
            assert_eq!(
                (ancestor, descendant),
                (RECORDED_BASE, AFTER),
                "{HARNESS}: the ancestry question is asked about the RECORDED base, never about origin/main"
            );
            true
        },
    );
    assert!(
        matches!(measured, CandidateOwnChanges::Ok { .. }),
        "{HARNESS}: the scope boundary measures against the recorded base, got: {measured:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — NO RECORDED BASE IS A REFUSAL, NOT A FALLBACK. This is the clause the whole story exists
    //    for. `origin/main` is always resolvable in a healthy checkout, so a boundary that defaulted to it
    //    would never be short of a base — it would just be measuring the wrong range, silently. The refusal
    //    must say the base is unreadable.
    // -----------------------------------------------------------------------------------------------------------
    let mut no_base = turn_with_base(None);
    judge_delivered_candidate(&Repo::healthy(), "smith", &mut no_base);
    assert!(
        no_base.candidate_sha.is_none(),
        "{HARNESS}: a run with no recorded base has no candidate — origin/main is not a substitute"
    );
    assert!(
        no_base
            .refusal
            .as_deref()
            .unwrap_or_default()
            .contains("execution base is unreadable"),
        "{HARNESS}: the refusal names the missing base, got: {:?}",
        no_base.refusal
    );

    // A blank base is the same shape as no base: `Some("   ")` is not a revision.
    let mut blank_base = turn_with_base(Some("   "));
    judge_delivered_candidate(&Repo::healthy(), "smith", &mut blank_base);
    assert!(
        blank_base.candidate_sha.is_none(),
        "{HARNESS}: a blank execution base has no candidate"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE SCOPE BOUNDARY REFUSES THE SAME SHAPES. It never measures "from HEAD" or "from the branch" when
    //    the run recorded nothing, and it refuses before the ancestry question is consulted at all — so a
    //    boundary that asked `is_ancestor("origin/main", candidate)` could not answer, and that is the point.
    // -----------------------------------------------------------------------------------------------------------
    for (label, base) in [("absent", None), ("blank", Some("   "))] {
        // `is_ancestor` is an `Fn`, so the record of whether it was consulted is a cell rather than a local.
        let ancestry_consulted = std::cell::Cell::new(false);
        let outcome = candidate_own_changed_files(
            Some(AFTER),
            base,
            &[AFTER.to_string()],
            |_| vec!["tests/tests/forge_scope__009.rs".into()],
            |_, _| {
                ancestry_consulted.set(true);
                true
            },
        );
        let reason = match &outcome {
            CandidateOwnChanges::Fail { reason } => reason.clone(),
            other => panic!("{HARNESS}: a {label} recorded base must be refused, got {other:?}"),
        };
        assert!(
            reason.contains("no recorded base to measure against"),
            "{HARNESS}: the refusal names the missing base, got: {reason:?}"
        );
        assert!(
            !ancestry_consulted.get(),
            "{HARNESS}: a {label} base is refused BEFORE any ancestry question, so no substitute can be consulted"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. QA REFUSES THE SAME SHAPES. `rust_contract_production_edits` answers an ERROR for a missing or blank
    //    base, never an empty list: an empty list is the PASS, and a range that was never measured must not be
    //    able to produce one.
    // -----------------------------------------------------------------------------------------------------------
    let silent = |command: &str| CommandResult {
        command: command.to_string(),
        exit_code: 0,
        passed: true,
        excerpt: String::new(),
        unmeasurable: false,
        output: "forge/src/engine/scope.rs\n".into(),
    };
    for (label, base) in [("absent", None), ("blank", Some("   "))] {
        let error = rust_contract_production_edits(base, Some(AFTER), &silent)
            .expect_err("a {label} base must not report a clean range");
        assert!(
            error.contains("execution base is missing"),
            "{HARNESS}: QA names the missing base for a {label} base, got: {error:?}"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE PROVISIONER REFUSES A BASE REF IT WAS NOT GIVEN. `provision_worker_workspace` requires an explicit
    //    approved base and returns an error for an empty one — it does not reach for `origin/main` itself. This
    //    is the last place a substitution could happen, so it is the last one tested.
    // -----------------------------------------------------------------------------------------------------------
    let refused = provision_worker_workspace(None, STORY_ID, Some("run-scope-009"), Some("  "), None)
        .expect_err("an empty baseRef must be refused, not defaulted");
    assert!(
        refused.contains("baseRef is required"),
        "{HARNESS}: the provisioner names the missing approval, got: {refused:?}"
    );
    let no_ref_at_all =
        provision_worker_workspace(None, STORY_ID, Some("run-scope-009"), None, None)
            .expect_err("an absent baseRef must be refused, not defaulted");
    assert!(
        no_ref_at_all.contains("baseRef is required"),
        "{HARNESS}: the provisioner names the missing approval, got: {no_ref_at_all:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. A REF THAT CANNOT BE RESOLVED TO A COMMIT IS REFUSED, NOT COERCED. Real git, in a disposable
    //    repository. Two shapes: a ref that does not exist at all, and a ref that resolves to something that is
    //    not a commit. Both must fail rather than yield a plausible-looking base.
    // -----------------------------------------------------------------------------------------------------------
    if git_available() {
        let repo = DisposableRepo::init().expect("a disposable repository");
        let head = repo.head_sha().expect("head");

        // A real ref resolves to the recorded COMMIT, not to the name — the stamp on the run is a sha.
        assert_eq!(
            resolve_base_commit(repo.path(), "main").expect("main resolves"),
            head,
            "{HARNESS}: a resolved ref yields the COMMIT it names, so a moving name is pinned at provisioning"
        );
        // An unresolvable ref is an error naming the ref.
        let missing = resolve_base_commit(repo.path(), "refs/heads/no-such-branch")
            .expect_err("an unresolvable ref must not yield a base");
        assert!(
            missing.contains("could not be resolved to a commit"),
            "{HARNESS}: the refusal names the resolution failure, got: {missing:?}"
        );

        // A ref that resolves to a tree rather than a commit is refused too: `resolve_base_commit` requires
        // `^{commit}`, and a tag pointing straight at a tree object has no base to diff from. (A tag pointing at
        // a COMMIT resolves fine and must — `^{commit}` peels — so the shape that is refused is a genuinely
        // non-commit object, not a tag per se.)
        repo.git(&["tag", "commit-tag"]).expect("tag the commit");
        assert_eq!(
            resolve_base_commit(repo.path(), "commit-tag").expect("a commit tag peels to its commit"),
            head,
            "{HARNESS}: a ref that names a commit resolves to that commit"
        );
        let tree = repo
            .git(&["mktree"])
            .map(|out| out.trim().to_string())
            .unwrap_or_default();
        if !tree.is_empty() {
            repo.git(&["tag", "tree-tag", &tree]).expect("tag the tree object");
            let not_a_commit = resolve_base_commit(repo.path(), "tree-tag")
                .expect_err("a ref that is not a commit must not yield a base");
            assert!(
                not_a_commit.contains("could not be resolved to a commit"),
                "{HARNESS}: a ref that is not a commit is refused, got: {not_a_commit:?}"
            );
        }
    } else {
        eprintln!("skipping section 6: git is not on PATH");
    }

    // -----------------------------------------------------------------------------------------------------------
    // 7. THE DEFAULT REF IS A REF, NOT A BASE. `resolve_approved_base_ref` is where `origin/main` enters the
    //    system at all, so this asserts its SHAPE without touching the process environment: whatever it returns
    //    is a name to be resolved (section 6), never a revision a judgement can be taken against directly.
    // -----------------------------------------------------------------------------------------------------------
    let approved = resolve_approved_base_ref();
    assert!(
        !approved.trim().is_empty(),
        "{HARNESS}: the approved base ref is never empty"
    );
    assert!(
        approved.len() < 40 || !approved.bytes().all(|b| b.is_ascii_hexdigit()),
        "{HARNESS}: the approved base ref is a REF to resolve, not a revision used as one; got: {approved:?}"
    );
}
