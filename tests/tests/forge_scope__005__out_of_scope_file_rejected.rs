//! FORGE.SCOPE — out-of-scope file rejected (TST-FORGE-SCOPE-005).
//!
//! Contract: a test-authoring story owns a TEST ARTIFACT. A candidate that reaches outside it — into a
//! production root — is not a candidate, and the refusal must come from a boundary that measures the file set
//! rather than from a reader's judgement of the prose.
//!
//! The rule is production's, and it is applied where the changed file set is measured:
//!
//!   1. **the lane gate** — `judge_delivered_candidate` (`forge/src/roles/smith/mod.rs:77`) →
//!      `candidate_rejection_in_surface` (`smith/mod.rs:238-248`). Only under a declared `RUST_CONTRACT` test
//!      mode does it read the changed paths at all, and then: a candidate whose range touches a production path
//!      is refused by name, listing the range it touched it across (`smith/mod.rs:276-291`). The refusal CLEARS
//!      `candidate_sha`, so the run cannot keep the candidate.
//!   2. **the classification** — `is_rust_contract_production_path` (`forge/src/engine/assay.rs:338`) over the
//!      five production roots (`PRODUCTION_ROOTS`, `assay.rs:336`), which turns a `git diff --name-only` listing
//!      into the LIST of production paths the candidate touched. An empty list is the pass; a named path is the
//!      failure that stops the story.
//!
//! QA no longer runs a range of its own: the retired `rust_contract_production_edits` aggregate re-asked git for
//! a `{base}..{candidate}` diff the lane gate had ALREADY measured, and source ownership is Smith's. The
//! surviving classification is the one above, so this proof applies it to the same listing the gate judged —
//! one definition, one listing, two readers instead of two commands.
//!
//! What keeps the gate honest is that the answer is never re-derived: a path is production iff
//! `is_rust_contract_production_path` says so. A second opinion about "is this production?" is exactly how a
//! gate stops being a gate.
//!
//! Negative and fault cases are load-bearing. Without them the test could pass on a boundary that refuses every
//! candidate (section 3 is the control) or on one that measures nothing (section 5: an unmeasurable range is a
//! REFUSAL that names its reason, never a silent empty answer that reads as a pass).
//!
//! Deterministic and isolated: the repository is a scripted `CandidateProbe` at the boundary production injects,
//! and the classification is applied to the listing the probe answers with. No git process, no filesystem, no
//! network, no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_scope__005__out_of_scope_file_rejected

use std::collections::HashMap;

use forge::engine::assay::{is_rust_contract_production_path, CommandResult};
use forge::engine::runner::{CandidateProbe, HarnessOutput, RoleHarness};
use forge::roles::smith::judge_delivered_candidate;

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L1 Component";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-SCOPE-005";
/// The base the run recorded.
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// The candidate the lane produced.
const AFTER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// A repository described by its answers, with the test mode the packet declared.
struct Repo {
    answers: HashMap<String, String>,
    test_mode: Option<&'static str>,
}

impl Repo {
    /// A candidate whose range changes the given paths, on a clean tree, descending from the base.
    fn changing(paths: &[&str], test_mode: Option<&'static str>) -> Self {
        let mut answers = HashMap::new();
        answers.insert(
            format!("merge-base --is-ancestor {BASE} {AFTER}"),
            String::new(),
        );
        answers.insert("status --porcelain".into(), String::new());
        answers.insert(
            format!("diff --name-only --no-renames {BASE}..{AFTER}"),
            paths.join("\n"),
        );
        Self { answers, test_mode }
    }

    /// Make git fail for one invocation — the fault case.
    fn answer(mut self, args: &str, stdout: Option<&str>) -> Self {
        match stdout {
            Some(out) => self.answers.insert(args.into(), out.into()),
            None => self.answers.remove(args),
        };
        self
    }
}

impl CandidateProbe for Repo {
    fn git(&self, args: &[&str]) -> Option<String> {
        self.answers.get(&args.join(" ")).cloned()
    }
    fn declared_test_mode(&self) -> Option<&str> {
        self.test_mode
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

fn delivered_turn() -> HarnessOutput {
    HarnessOutput {
        raw: format!("{STORY_ID}: authored the canonical contract test and committed it"),
        candidate_sha: Some(AFTER.into()),
        assay_commands: Vec::new(),
        acceptance_mapped: true,
        refusal: None,
        execution_base: Some(BASE.into()),
        usage: None,
        trajectory: None,
    }
}

/// The production paths of a `git diff --name-only` listing, by the one rule production applies.
///
/// This is the surviving half of the retired `rust_contract_production_edits` aggregate: the same
/// `is_rust_contract_production_path` the lane gate classifies its listing with, applied to the listing itself
/// rather than to a second `git diff` QA used to run over the range the gate had already measured. An empty
/// answer is the pass, and a named path is the failure.
fn production_edits(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(str::trim)
        .filter(|path| !path.is_empty() && is_rust_contract_production_path(path))
        .map(str::to_string)
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SCOPE-005); the file and the assay use it.
fn forge_scope_005__out_of_scope_file_rejected() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — A PRODUCTION PATH IN THE RANGE IS REJECTED. The test-authoring story added its test AND
    //    reached into `forge/`. The lane gate must refuse the candidate, clear it, and name the range.
    // -----------------------------------------------------------------------------------------------------------
    let out_of_scope = Repo::changing(
        &[
            "tests/tests/forge_scope__005.rs",
            "forge/src/engine/scope.rs",
        ],
        Some("RUST_CONTRACT"),
    );
    let mut refused = delivered_turn();
    judge_delivered_candidate(&out_of_scope, "smith", &mut refused);
    assert!(
        refused.candidate_sha.is_none(),
        "{HARNESS}: a candidate that touched production code must not remain the run's candidate ({STORY_ID})"
    );
    let reason = refused.refusal.as_deref().unwrap_or_default();
    assert!(
        reason.contains("RUST_CONTRACT candidate modified production code"),
        "{HARNESS}: the refusal names the rule it applied, got: {reason:?}"
    );
    assert!(
        reason.contains(&format!("{BASE}..{AFTER}")),
        "{HARNESS}: the refusal names the range it measured, got: {reason:?}"
    );
    assert!(
        refused.raw.contains("SMITH_CANDIDATE_REJECTED"),
        "{HARNESS}: the refusal is written into the turn's own output, got: {:?}",
        refused.raw
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE SAME LISTING, CLASSIFIED. The gate's refusal above and this answer read ONE listing through ONE
    //    function, and the production path is NAMED — not merely counted, so the failure is actionable.
    // -----------------------------------------------------------------------------------------------------------
    let measured = production_edits("tests/tests/forge_scope__005.rs\nforge/src/engine/scope.rs\n");
    assert_eq!(
        measured,
        vec!["forge/src/engine/scope.rs".to_string()],
        "{HARNESS}: the production paths the candidate touched are named, and only those"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. CONTROL — A CANDIDATE THAT STAYS INSIDE ITS SCOPE IS ACCEPTED. Without this the whole proof would be
    //    satisfied by a gate that refuses every candidate, which is not the contract.
    // -----------------------------------------------------------------------------------------------------------
    let in_scope = Repo::changing(
        &[
            "tests/tests/forge_scope__005.rs",
            "tests/tests/support/forge_scope.rs",
        ],
        Some("RUST_CONTRACT"),
    );
    let mut accepted = delivered_turn();
    judge_delivered_candidate(&in_scope, "smith", &mut accepted);
    assert_eq!(
        accepted.candidate_sha.as_deref(),
        Some(AFTER),
        "{HARNESS}: a test-only candidate is a candidate"
    );
    assert!(
        accepted.refusal.is_none(),
        "{HARNESS}: an in-scope candidate carries no refusal, got: {:?}",
        accepted.refusal
    );
    assert!(
        production_edits("tests/tests/forge_scope__005.rs\ntests/tests/support/forge_scope.rs\n")
            .is_empty(),
        "{HARNESS}: a test-only range classifies as no production edit"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — EVERY PRODUCTION ROOT IS OUT OF SCOPE, not just the one this story happened to reach. A
    //    boundary that listed `forge/` and forgot `db/` would let a schema change ride along as "test work".
    // -----------------------------------------------------------------------------------------------------------
    for root in ["web/", "middle/", "db/", "cli/", "forge/"] {
        let path = format!("{root}src/somewhere.rs");
        assert!(
            is_rust_contract_production_path(&path),
            "{HARNESS}: {path} is production code and out of scope for a test-authoring story"
        );
        let mut into_root = delivered_turn();
        judge_delivered_candidate(
            &Repo::changing(
                &["tests/tests/forge_scope__005.rs", &path],
                Some("RUST_CONTRACT"),
            ),
            "smith",
            &mut into_root,
        );
        assert!(
            into_root.candidate_sha.is_none(),
            "{HARNESS}: reaching into {root} must be rejected, got a candidate"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 5. FAULT — AN UNMEASURABLE RANGE IS A REFUSAL, NOT AN EMPTY LIST. This is the half that makes the gate a
    //    gate: the classification answers an EMPTY VEC for "nothing production was touched", so a failed or
    //    unmeasurable `git diff` must not collapse into that same empty answer — every story whose repository was
    //    unreadable would pass by default. The gate refuses with the reason NAMED, which is what tells the two
    //    apart.
    // -----------------------------------------------------------------------------------------------------------

    // The same fault at the lane gate: git cannot list the changed paths, so the candidate is refused with the
    // reason named — not accepted because the list came back empty.
    let unreadable_diff =
        Repo::changing(&["tests/tests/forge_scope__005.rs"], Some("RUST_CONTRACT")).answer(
            &format!("diff --name-only --no-renames {BASE}..{AFTER}"),
            None,
        );
    let mut unknown = delivered_turn();
    judge_delivered_candidate(&unreadable_diff, "smith", &mut unknown);
    assert!(
        unknown.candidate_sha.is_none(),
        "{HARNESS}: an unreadable changed-path list must not certify a candidate"
    );
    assert!(
        unknown
            .refusal
            .as_deref()
            .unwrap_or_default()
            .contains("changed paths are unreadable"),
        "{HARNESS}: the fault is named, not swallowed, got: {:?}",
        unknown.refusal
    );

    // The same fault read through the classification instead of through the gate: nothing measured classifies as
    // nothing production touched, and that empty answer is exactly the shape a PASS takes — which is why the
    // refusal above carries a reason rather than returning this list.
    assert!(
        production_edits("").is_empty(),
        "{HARNESS}: an unmeasured listing and a clean one look identical — silence is never a pass on its own"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. FAULT — A MISSING BASE OR CANDIDATE IS A REFUSAL TOO. `None` here means the run recorded no base or
    //    published no candidate, and both are refusals rather than "nothing to check". The boundary reads the
    //    base FIRST, so a candidate whose base was never recorded is refused before any diff exists to read — and
    //    a base that is present but blank takes the same path, because the recorded value is used as recorded
    //    instead of being defaulted to a branch name.
    // -----------------------------------------------------------------------------------------------------------
    let in_scope_paths = ["tests/tests/forge_scope__005.rs"];

    let mut no_base = delivered_turn();
    no_base.execution_base = None;
    judge_delivered_candidate(
        &Repo::changing(&in_scope_paths, Some("RUST_CONTRACT")),
        "smith",
        &mut no_base,
    );
    assert!(
        no_base.candidate_sha.is_none(),
        "{HARNESS}: a run with no recorded base has no candidate"
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

    let mut no_candidate = delivered_turn();
    no_candidate.candidate_sha = None;
    judge_delivered_candidate(
        &Repo::changing(&in_scope_paths, Some("RUST_CONTRACT")),
        "smith",
        &mut no_candidate,
    );
    assert!(
        no_candidate
            .refusal
            .as_deref()
            .unwrap_or_default()
            .contains("HEAD was unreadable after the role turn"),
        "{HARNESS}: the refusal names the missing candidate, got: {:?}",
        no_candidate.refusal
    );

    // A base that is present but BLANK is the same shape, not a licence to measure nothing: the recorded base is
    // used as recorded rather than defaulted to a branch name, so nothing answers for it and the candidate is
    // refused instead of certified.
    let mut blank_base = delivered_turn();
    blank_base.execution_base = Some("   ".into());
    judge_delivered_candidate(
        &Repo::changing(&in_scope_paths, Some("RUST_CONTRACT")),
        "smith",
        &mut blank_base,
    );
    let blank_reason = blank_base.refusal.as_deref().unwrap_or_default();
    assert!(
        blank_base.candidate_sha.is_none(),
        "{HARNESS}: a blank base must not be defaulted into a measured range"
    );
    assert!(
        blank_reason.contains("is not a descendant of execution base"),
        "{HARNESS}: the blank base is judged on the base it was handed, got: {blank_reason:?}"
    );
    assert!(
        blank_base.execution_base.as_deref() == Some("   "),
        "{HARNESS}: the recorded base is left as recorded — nothing rewrites it mid-judgement"
    );
    assert!(
        !blank_reason.contains(&format!("{BASE}..{AFTER}")),
        "{HARNESS}: the blank base must not be quietly replaced by a real revision"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. THE SCOPE IS THE TEST MODE'S, NOT EVERY MODE'S. `candidate_rejection` only reads the changed paths when
    //    the packet declared `RUST_CONTRACT` (`forge/src/roles/smith/mod.rs:279-291`). A story allowed to write
    //    production code is not judged by the test-artifact rule — so a boundary that applied it unconditionally
    //    would refuse every implementation story in the estate.
    // -----------------------------------------------------------------------------------------------------------
    let implementation = Repo::changing(&["forge/src/engine/scope.rs"], None);
    let mut allowed = delivered_turn();
    judge_delivered_candidate(&implementation, "smith", &mut allowed);
    assert_eq!(
        allowed.candidate_sha.as_deref(),
        Some(AFTER),
        "{HARNESS}: a story that did not declare RUST_CONTRACT is not judged by the test-artifact rule"
    );
    assert!(
        allowed.refusal.is_none(),
        "{HARNESS}: the RUST_CONTRACT rule must not leak onto other stories, got: {:?}",
        allowed.refusal
    );
}
