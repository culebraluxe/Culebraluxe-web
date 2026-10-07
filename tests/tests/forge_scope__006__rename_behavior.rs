//! FORGE.SCOPE — rename behavior (TST-FORGE-SCOPE-006).
//!
//! Contract: a file RENAMED OUT of a production root is still a change to production code, and the boundary
//! must see it as one. Rename detection is what breaks this: with detection on, `git diff --name-only` prints
//! the move's DESTINATION only, so `forge/src/engine/scope.rs` → `tests/tests/support/scope.rs` reads as a
//! brand-new test file and sails through a test-authoring gate that has just been told "this story authors a
//! test artifact". The production code was deleted, not moved, and nothing in the story says so.
//!
//! The production answer is the `--no-renames` flag, and it is the answer in BOTH places that read a range:
//!
//!   * `rust_contract_production_edits` (`forge/src/engine/assay.rs:103`) builds
//!     `git diff --name-only --no-renames {base}..{candidate}` — the QA gate, and the comment at `assay.rs:101-102`
//!     names this exact bypass as the reason the flag is there;
//!   * `candidate_rejection` (`forge/src/roles/smith.rs:142`) builds
//!     `git diff --name-only --no-renames {base}..{after}` — the lane gate;
//!   * `git_changed_files` (`forge/src/engine/worktree.rs:329-337`) uses it for the measured file set, with the
//!     same reason in its own comment.
//!
//! The flag is not decoration, and a test that only asserted the string would be asserting a constant. So this
//! proof runs the flag against REAL GIT in a disposable repository (`test_harness::git::DisposableRepo` — the
//! harness's own fixture, which creates a throwaway repo under the temp root and removes it on drop) and shows
//! what each shape actually prints: with the flag, a rename prints BOTH paths; without it, git prints one and
//! the production path disappears from the answer. The boundary is then driven with the real listing, so the
//! refusal is a consequence of what git said rather than of a scripted string.
//!
//! Negative case: a rename that stays INSIDE the test tree is not a production edit, and a rename that stays
//! inside production is caught — so the rule is about where the move ENDS UP crossing the root line, not about
//! renames being forbidden.
//!
//! Deterministic and isolated: a throwaway git repository under the system temp root, no network, no database,
//! no PROD, no environment mutation. The repository is removed when the test returns.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_scope__006__rename_behavior

use std::collections::HashMap;
use std::process::Command;

use forge::engine::assay::{rust_contract_production_edits, CommandResult};
use forge::engine::runner::{CandidateProbe, HarnessOutput, RoleHarness};
use forge::roles::smith::judge_delivered_candidate;
use test_harness::git::{git_available, DisposableRepo};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L1 Component";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-SCOPE-006";
// The base the run recorded and the candidate the lane produced are both REAL commits, resolved from the
// disposable repository inside the proof — this story's contract is a contract with git, and a fabricated sha
// would prove that a string classifier works, not that a rename is caught.

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

/// What git prints for `base..candidate`, with and without `--no-renames`, run in the disposable repository.
fn diff_names(repo: &DisposableRepo, base: &str, after: &str, no_renames: bool) -> Vec<String> {
    let mut args = vec!["diff", "--name-only"];
    if no_renames {
        args.push("--no-renames");
    }
    let range = format!("{base}..{after}");
    args.push(range.as_str());
    let out = repo
        .git(&args)
        .expect("git diff --name-only runs in the disposable repository");
    out.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SCOPE-006); the file and the assay use it.
fn forge_scope_006__rename_behavior() {
    if !git_available() {
        eprintln!("skipping: git is not on PATH, and this proof is a contract WITH git");
        return;
    }

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE MOVE. A production file is renamed into the test tree — the exact bypass the `--no-renames`
    //    comments name: with rename detection, git prints only the destination and the production path is
    //    gone from the answer.
    // -----------------------------------------------------------------------------------------------------------
    let repo = DisposableRepo::init().expect("a disposable repository");
    repo.write("forge/src/engine/scope.rs", "pub fn candidate_own_changed_files() {}\n")
        .expect("write the production file");
    let base = repo.commit_all("forge: the production scope boundary").expect("commit the base");

    // Rename it out of production, with `git mv` so the commit really records a rename. `git mv` will not
    // create the destination directory, so the support tree exists before the move.
    repo.write("tests/tests/support/.keep", "").expect("create the destination directory");
    std::fs::remove_file(repo.path().join("tests/tests/support/.keep")).expect("remove the placeholder");
    repo.git(&["mv", "forge/src/engine/scope.rs", "tests/tests/support/scope.rs"])
        .expect("git mv out of production");
    repo.write("tests/tests/forge_scope__006.rs", "// canonical test\n").expect("write the test");
    let after = repo.commit_all("test: move the boundary into the test tree").expect("commit the candidate");

    // Without the flag, git reports ONE path — the destination — and the production root is not mentioned.
    let with_detection = diff_names(&repo, &base, &after, false);
    assert!(
        with_detection.contains(&"tests/tests/support/scope.rs".to_string()),
        "{HARNESS}: git's destination listing is present, got: {with_detection:?}"
    );
    assert!(
        !with_detection.contains(&"forge/src/engine/scope.rs".to_string()),
        "{HARNESS}: with rename detection git reports the DESTINATION only — this is the bypass {STORY_ID} exists to close, got: {with_detection:?}"
    );

    // With the flag — the flag both production gates pass — git reports BOTH sides of the move.
    let without_detection = diff_names(&repo, &base, &after, true);
    assert!(
        without_detection.contains(&"forge/src/engine/scope.rs".to_string()),
        "{HARNESS}: `--no-renames` must surface the production path of a move out of production, got: {without_detection:?}"
    );
    assert!(
        without_detection.contains(&"tests/tests/support/scope.rs".to_string()),
        "{HARNESS}: `--no-renames` still reports the destination, got: {without_detection:?}"
    );
    assert!(
        without_detection.len() >= with_detection.len(),
        "{HARNESS}: disabling rename detection can only ADD paths, never remove them"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE LANE GATE, DRIVEN WITH WHAT GIT ACTUALLY SAID. The refusal is a consequence of the listing above,
    //    not of a scripted string: a rename out of production is a production edit.
    // -----------------------------------------------------------------------------------------------------------
    let listing = without_detection.join("\n");
    let mut out = delivered_turn(&base, &after);
    judge_delivered_candidate(&Repo::changing(&base, &after, &listing), "smith", &mut out);
    assert!(
        out.candidate_sha.is_none(),
        "{HARNESS}: a candidate that MOVED production code into the test tree is not a test artifact ({STORY_ID})"
    );
    assert!(
        out.refusal
            .as_deref()
            .unwrap_or_default()
            .contains("RUST_CONTRACT candidate modified production code"),
        "{HARNESS}: the refusal names the rule it applied, got: {:?}",
        out.refusal
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. QA ANSWERS THE SAME QUESTION FROM THE SAME LISTING, AND NAMES THE MOVED FILE. `rust_contract_production_edits`
    //    runs the same `--no-renames` command, so its answer is the same one — and it names the production path
    //    so a person can see which file moved.
    // -----------------------------------------------------------------------------------------------------------
    let qa = rust_contract_production_edits(
        Some(&base),
        Some(&after),
        &{
            // The closure runs git itself rather than replaying the listing, so QA's answer is produced by the
            // same command production names. The range is cloned because the shas are asserted on below.
            let repo_path = repo.path().to_path_buf();
            let range = format!("{base}..{after}");
            move |command: &str| {
                assert!(
                    command.contains("--no-renames"),
                    "{HARNESS}: QA must ask git for BOTH sides of a move; it asked: {command:?}"
                );
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
        vec!["forge/src/engine/scope.rs".to_string()],
        "{HARNESS}: QA names the production path the move took out of, and only it"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — A RENAME THAT STAYS INSIDE THE TEST TREE IS NOT A PRODUCTION EDIT. The rule is about
    //    crossing the root line, not about renames: refusing every rename would hold every legitimate
    //    refactor a test-authoring story does.
    // -----------------------------------------------------------------------------------------------------------
    let inside = DisposableRepo::init().expect("a disposable repository");
    inside.write("tests/tests/support/old_name.rs", "// fixture\n").expect("write");
    let inside_base = inside.commit_all("test: fixture under the old name").expect("commit base");
    inside
        .git(&["mv", "tests/tests/support/old_name.rs", "tests/tests/support/new_name.rs"])
        .expect("git mv inside the test tree");
    let inside_after = inside.commit_all("test: rename the fixture").expect("commit candidate");
    let inside_listing = diff_names(&inside, &inside_base, &inside_after, true);
    for path in &inside_listing {
        assert!(
            !path.starts_with("forge/") && !path.starts_with("db/"),
            "{HARNESS}: a rename inside the test tree touches no production root, got: {inside_listing:?}"
        );
    }
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
        "{HARNESS}: a rename inside the test tree is not a production edit, got: {qa_inside:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — A RENAME WITHIN PRODUCTION IS STILL A PRODUCTION EDIT. Both sides are under `forge/`, so
    //    the rule catches it twice over; pinning it keeps the rule from being read as "moves out of production
    //    only", which would leave a reorganisation of production code invisible to a test-authoring gate.
    // -----------------------------------------------------------------------------------------------------------
    let within = DisposableRepo::init().expect("a disposable repository");
    within.write("forge/src/old.rs", "// fixture\n").expect("write");
    let within_base = within.commit_all("forge: fixture under the old name").expect("commit base");
    within
        .git(&["mv", "forge/src/old.rs", "forge/src/new.rs"])
        .expect("git mv within production");
    let within_after = within.commit_all("forge: rename the fixture").expect("commit candidate");
    let within_listing = diff_names(&within, &within_base, &within_after, true);
    let qa_within = rust_contract_production_edits(
        Some(&within_base),
        Some(&within_after),
        &|command: &str| CommandResult {
            command: command.to_string(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output: within_listing.join("\n"),
        },
    )
    .expect("the range is measurable");
    assert_eq!(
        qa_within,
        vec!["forge/src/new.rs".to_string(), "forge/src/old.rs".to_string()],
        "{HARNESS}: a rename within production lists BOTH sides, so the move is caught whichever side git reports"
    );

    // The two revisions used here are real commits, resolved from the disposable repository.
    assert_eq!(base.len(), 40, "{HARNESS}: the base is a real commit sha, got: {base:?}");
    assert_eq!(after.len(), 40, "{HARNESS}: the candidate is a real commit sha, got: {after:?}");
    assert_ne!(base, after, "{HARNESS}: the candidate is not the base");
}
