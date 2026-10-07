//! FORGE.SCOPE — deletion behavior (TST-FORGE-SCOPE-007).
//!
//! Contract: DELETING a production file is a change to production code, and the scope boundary must see it as
//! one. Deletion is the quiet half of this boundary: a `git diff --name-only` listing is a list of PATHS, and a
//! path carries no record of what happened to it — so a deleted file and an added file look identical in the
//! answer, and a gate that reads "no production path in the list" reads a deletion of `db/src/pool.rs` as a
//! clean run.
//!
//! Why it matters more than it looks: a RUST_CONTRACT story's whole deliverable is a test artifact, and
//! "deleting the code the test was supposed to describe" is the cheapest way to make an impossible assertion
//! pass. It is also a real shape in this estate — the retired TypeScript estate exists because production code
//! was removed and the removal had to be recorded.
//!
//! Three production seams read a range's paths, and all three treat a deletion as a path:
//!
//!   1. `candidate_rejection` (`forge/src/roles/smith.rs:142`) —
//!      `git diff --name-only --no-renames {base}..{after}`, then `.any(is_rust_contract_production_path)` over
//!      the lines (`smith.rs:147-155`). It never asks git for a status, so it cannot tell a deletion from an
//!      edit, and it must not: both are production changes.
//!   2. `rust_contract_production_edits` (`forge/src/engine/assay.rs:87`) — the QA gate, which lists the
//!      production paths and sorts/dedups them (`assay.rs:111-120`). A deletion is named like any other.
//!   3. `candidate_own_changed_files` (`forge/src/engine/scope.rs:41`) — the measured file set, which collects
//!      the paths of every commit on the lane (`scope.rs:78-85`) with no status attached.
//!
//! This proof runs the real commands against a disposable git repository (`test_harness::git::DisposableRepo`),
//! so "git reports a deleted file's path" is observed rather than assumed, and then drives both gates with what
//! git actually said.
//!
//! Negative and control cases: a deletion inside the TEST tree is not a production edit (otherwise every
//! cleanup a test-authoring story does would Hold), and an edit of a production file is still caught — so the
//! rule is about crossing the root line, not about deletions being special.
//!
//! Deterministic and isolated: a throwaway git repository under the system temp root, no network, no database,
//! no PROD, no environment mutation. The repository is removed when the test returns.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_scope__007__deletion_behavior

use std::collections::HashMap;
use std::process::Command;

use forge::engine::assay::{rust_contract_production_edits, CommandResult};
use forge::engine::runner::{CandidateProbe, HarnessOutput, RoleHarness};
use forge::roles::smith::judge_delivered_candidate;
use test_harness::git::{git_available, DisposableRepo};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L1 Component";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-SCOPE-007";

/// A repository described by its answers, with the test mode the packet declared.
struct Repo {
    answers: HashMap<String, String>,
}

impl Repo {
    /// A clean, descending, RUST_CONTRACT candidate whose range git reports as `changed`.
    fn changing(base: &str, after: &str, changed: &str) -> Self {
        let mut answers = HashMap::new();
        answers.insert(format!("merge-base --is-ancestor {base} {after}"), String::new());
        answers.insert("status --porcelain".into(), String::new());
        answers.insert(
            format!("diff --name-only --no-renames {base}..{after}"),
            changed.to_string(),
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

/// What git prints for `base..candidate`, run for real in the disposable repository.
fn diff_names(repo: &DisposableRepo, base: &str, after: &str) -> Vec<String> {
    repo.git(&[
        "diff",
        "--name-only",
        "--no-renames",
        &format!("{base}..{after}"),
    ])
    .expect("git diff --name-only runs in the disposable repository")
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .map(str::to_string)
    .collect()
}

/// What git reports about a single path in `base..candidate`, read from `git diff --name-status`.
fn diff_status(repo: &DisposableRepo, base: &str, after: &str) -> Vec<(String, String)> {
    repo.git(&[
        "diff",
        "--name-status",
        "--no-renames",
        &format!("{base}..{after}"),
    ])
    .expect("git diff --name-status runs in the disposable repository")
    .lines()
    .filter_map(|line| {
        let mut parts = line.split_whitespace();
        let status = parts.next()?.to_string();
        let path = parts.next()?.to_string();
        Some((status, path))
    })
    .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SCOPE-007); the file and the assay use it.
fn forge_scope_007__deletion_behavior() {
    if !git_available() {
        eprintln!("skipping: git is not on PATH, and this proof is a contract WITH git");
        return;
    }

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE DELETION. A test-authoring story adds its canonical test and DELETES a production file. Git
    //    records the deletion as a `D` against the production path, and `--name-only` reports that path like any
    //    other — which is the whole reason a deletion can slip past a path-only gate.
    // -----------------------------------------------------------------------------------------------------------
    let repo = DisposableRepo::init().expect("a disposable repository");
    repo.write("db/src/pool.rs", "pub fn shared() {}\n").expect("write the production file");
    repo.write("forge/src/engine/scope.rs", "pub fn candidate_own_changed_files() {}\n")
        .expect("write the second production file");
    let base = repo.commit_all("db: the pool and the scope boundary").expect("commit the base");

    repo.git(&["rm", "-q", "db/src/pool.rs"]).expect("delete the production file");
    repo.write("tests/tests/forge_scope__007.rs", "// canonical test\n").expect("write the test");
    let after = repo
        .commit_all("test: the canonical deletion-behaviour test")
        .expect("commit the candidate");

    // Observed, not assumed: git really does report the deleted production path in a name-only listing.
    let statuses = diff_status(&repo, &base, &after);
    assert!(
        statuses
            .iter()
            .any(|(status, path)| status == "D" && path == "db/src/pool.rs"),
        "{HARNESS}: the deletion really is a D against the production path, got: {statuses:?}"
    );
    let listing = diff_names(&repo, &base, &after);
    assert!(
        listing.contains(&"db/src/pool.rs".to_string()),
        "{HARNESS}: git reports a DELETED production file's path in a name-only listing — this is what makes the deletion visible to the gate, got: {listing:?}"
    );
    assert!(
        listing.contains(&"tests/tests/forge_scope__007.rs".to_string()),
        "{HARNESS}: the added test is in the same listing, got: {listing:?}"
    );
    // And the file really is gone from the tree — the listing is not a rename being read as a deletion.
    assert!(
        !repo.path().join("db/src/pool.rs").exists(),
        "{HARNESS}: the production file is deleted from the working tree"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE LANE GATE REJECTS IT, DRIVEN WITH WHAT GIT SAID. The refusal is a consequence of the listing above.
    // -----------------------------------------------------------------------------------------------------------
    let mut out = delivered_turn(&base, &after);
    judge_delivered_candidate(
        &Repo::changing(&base, &after, &listing.join("\n")),
        "smith",
        &mut out,
    );
    assert!(
        out.candidate_sha.is_none(),
        "{HARNESS}: deleting production code is not a test artifact and leaves no candidate ({STORY_ID})"
    );
    assert!(
        out.refusal
            .as_deref()
            .unwrap_or_default()
            .contains("RUST_CONTRACT candidate modified production code"),
        "{HARNESS}: the refusal names the rule, got: {:?}",
        out.refusal
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. QA NAMES THE DELETED FILE. The QA gate runs the same command and returns the production paths as a
    //    list; a deletion appears in it identically to an edit, which is what makes it actionable — a person
    //    sees WHICH file went.
    // -----------------------------------------------------------------------------------------------------------
    let qa = rust_contract_production_edits(
        Some(&base),
        Some(&after),
        &{
            // The closure runs git itself, so QA's answer comes from the command production names rather than
            // from the listing this test also read. The range is cloned because the shas are used below.
            let repo_path = repo.path().to_path_buf();
            let range = format!("{base}..{after}");
            move |command: &str| {
                let reported = Command::new("git")
                    .args(["diff", "--name-only", "--no-renames", &range])
                    .current_dir(&repo_path)
                    .output()
                    .expect("git runs in the disposable repository");
                CommandResult {
                    command: command.to_string(),
                    exit_code: 0,
                    passed: reported.status.success(),
                    excerpt: String::new(),
                    unmeasurable: false,
                    output: String::from_utf8_lossy(&reported.stdout).into_owned(),
                }
            }
        },
    )
    .expect("the range is measurable");
    assert_eq!(
        qa,
        vec!["db/src/pool.rs".to_string()],
        "{HARNESS}: QA names the DELETED production path, and the untouched sibling is not reported"
    );

    // The measured scope set carries the deletion too — the run's "files this story touched" is not blind to it.
    let measured = forge::engine::scope::candidate_own_changed_files(
        Some(&after),
        Some(&base),
        std::slice::from_ref(&after),
        |sha| {
            assert_eq!(sha, &after, "{HARNESS}: only the candidate commit is read");
            listing.clone()
        },
        |_, _| true,
    );
    let measured_files = match &measured {
        forge::engine::scope::CandidateOwnChanges::Ok { changed_files } => changed_files.clone(),
        other => panic!("{HARNESS}: the lane's own deletion measures cleanly, got {other:?}"),
    };
    assert!(
        measured_files.contains(&"db/src/pool.rs".to_string()),
        "{HARNESS}: the deletion is in the measured file set, got: {measured_files:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — A DELETION INSIDE THE TEST TREE IS NOT A PRODUCTION EDIT. Otherwise every cleanup a
    //    test-authoring story does — retiring a fixture, dropping a helper — would Hold with no product defect
    //    anywhere, and the rule would be switched off within a week.
    // -----------------------------------------------------------------------------------------------------------
    let inside = DisposableRepo::init().expect("a disposable repository");
    inside.write("tests/tests/support/retired.rs", "// fixture\n").expect("write");
    inside.write("tests/tests/keep.rs", "// fixture\n").expect("write");
    let inside_base = inside.commit_all("test: fixtures").expect("commit base");
    inside.git(&["rm", "-q", "tests/tests/support/retired.rs"]).expect("delete the fixture");
    let inside_after = inside.commit_all("test: retire the fixture").expect("commit candidate");
    let inside_listing = diff_names(&inside, &inside_base, &inside_after);
    assert!(
        inside_listing.contains(&"tests/tests/support/retired.rs".to_string()),
        "{HARNESS}: the deleted fixture is reported, got: {inside_listing:?}"
    );
    let qa_inside = rust_contract_production_edits(
        Some(&inside_base),
        Some(&inside_after),
        &|command: &str| CommandResult {
            command: command.to_string(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output: inside_listing.join("\n"),
        },
    )
    .expect("the range is measurable");
    assert!(
        qa_inside.is_empty(),
        "{HARNESS}: deleting a test fixture is not a production edit, got: {qa_inside:?}"
    );
    let mut inside_out = delivered_turn(&inside_base, &inside_after);
    judge_delivered_candidate(
        &Repo::changing(&inside_base, &inside_after, &inside_listing.join("\n")),
        "smith",
        &mut inside_out,
    );
    assert_eq!(
        inside_out.candidate_sha.as_deref(),
        Some(inside_after.as_str()),
        "{HARNESS}: a test-tree-only deletion leaves the candidate intact ({:?})",
        inside_out.refusal
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. CONTROL — AN EDIT OF A PRODUCTION FILE IS CAUGHT BY THE SAME LISTING RULE, so the deletion is not a
    //    special case that happens to work: both are caught because both are PATHS under a production root.
    // -----------------------------------------------------------------------------------------------------------
    let edited = DisposableRepo::init().expect("a disposable repository");
    edited.write("forge/src/engine/scope.rs", "pub fn candidate_own_changed_files() {}\n").expect("write");
    let edited_base = edited.commit_all("forge: the boundary").expect("commit base");
    edited
        .write("forge/src/engine/scope.rs", "pub fn candidate_own_changed_files() { /* changed */ }\n")
        .expect("edit");
    let edited_after = edited.commit_all("forge: change the boundary").expect("commit candidate");
    let edited_statuses = diff_status(&edited, &edited_base, &edited_after);
    assert!(
        edited_statuses
            .iter()
            .any(|(status, path)| status == "M" && path == "forge/src/engine/scope.rs"),
        "{HARNESS}: the edit really is an M against the production path, got: {edited_statuses:?}"
    );
    let edited_listing = diff_names(&edited, &edited_base, &edited_after);
    let qa_edited = rust_contract_production_edits(
        Some(&edited_base),
        Some(&edited_after),
        &|command: &str| CommandResult {
            command: command.to_string(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output: edited_listing.join("\n"),
        },
    )
    .expect("the range is measurable");
    assert_eq!(
        qa_edited,
        vec!["forge/src/engine/scope.rs".to_string()],
        "{HARNESS}: an edit and a deletion are caught by one rule: the path is under a production root"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CONTROL — A CANDIDATE THAT DELETES NOTHING AND TOUCHES NOTHING IS NOT A CANDIDATE EITHER. An empty
    //    name-only listing is refused with its own reason, so "git said nothing changed" is never read as
    //    "this story did the work".
    // -----------------------------------------------------------------------------------------------------------
    let empty = Repo::changing(&base, &after, "");
    let mut nothing = delivered_turn(&base, &after);
    judge_delivered_candidate(&empty, "smith", &mut nothing);
    assert!(
        nothing.candidate_sha.is_none(),
        "{HARNESS}: a candidate that changes no files is not a candidate"
    );
    assert!(
        nothing
            .refusal
            .as_deref()
            .unwrap_or_default()
            .contains("changes no files"),
        "{HARNESS}: the refusal names the empty range, got: {:?}",
        nothing.refusal
    );
}
