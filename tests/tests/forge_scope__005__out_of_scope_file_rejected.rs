//! FORGE.SCOPE — out-of-scope file rejected (TST-FORGE-SCOPE-005).
//!
//! Contract: a test-authoring story owns a TEST ARTIFACT. A candidate that reaches outside it — into a
//! production root — is not a candidate, and the refusal must come from a boundary that measures the file set
//! rather than from a reader's judgement of the prose.
//!
//! Two production seams enforce it, and they answer the same question differently on purpose:
//!
//!   1. **the lane gate** — `judge_delivered_candidate` (`forge/src/roles/smith.rs:74`) → `candidate_rejection`
//!      (`smith.rs:142-156`). Only under a declared `RUST_CONTRACT` test mode does it read the changed paths at
//!      all, and then: a candidate whose range touches a production path is refused by name, listing the range
//!      it touched it across. The refusal CLEARS `candidate_sha`, so the run cannot keep the candidate.
//!   2. **the QA gate** — `rust_contract_production_edits`
//!      (`forge/src/engine/assay.rs:87`). It asks the same question of the same range and returns the production
//!      paths as a LIST, or an error string when the range cannot be measured. An empty list is the pass; a
//!      non-empty list is the QA FAIL that stops the story.
//!
//! What makes the two agree is that neither re-derives the answer: both classify a path with the one
//! definition, `is_rust_contract_production_path` (`assay.rs:79`), over the same production roots
//! (`PRODUCTION_ROOTS`, `assay.rs:77`). A second opinion about "is this production?" is exactly how a gate
//! stops being a gate.
//!
//! Negative and fault cases are load-bearing. Without them the test could pass on a boundary that refuses every
//! candidate (section 3 is the control) or on one that measures nothing (section 5: an unmeasurable range must
//! be an error, never an empty list that reads as a pass).
//!
//! Deterministic and isolated: the repository is a scripted `CandidateProbe` and a scripted `CommandResult`
//! runner at the boundaries production injects. No git process, no filesystem, no network, no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_scope__005__out_of_scope_file_rejected

use std::collections::HashMap;

use forge::engine::assay::{is_rust_contract_production_path, rust_contract_production_edits, CommandResult};
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
        answers.insert(format!("merge-base --is-ancestor {BASE} {AFTER}"), String::new());
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
    }
}

/// A `CommandResult` for the one command `rust_contract_production_edits` runs, with a scripted answer.
fn answering(output: &str) -> impl Fn(&str) -> CommandResult + '_ {
    move |command: &str| CommandResult {
        command: command.to_string(),
        exit_code: 0,
        passed: true,
        excerpt: String::new(),
        unmeasurable: false,
        output: output.to_string(),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SCOPE-005); the file and the assay use it.
fn forge_scope_005__out_of_scope_file_rejected() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — A PRODUCTION PATH IN THE RANGE IS REJECTED. The test-authoring story added its test AND
    //    reached into `forge/`. The lane gate must refuse the candidate, clear it, and name the range.
    // -----------------------------------------------------------------------------------------------------------
    let out_of_scope = Repo::changing(
        &["tests/tests/forge_scope__005.rs", "forge/src/engine/scope.rs"],
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
    // 2. THE QA GATE ANSWERS THE SAME QUESTION AS A LIST. Same range, same classification function, and the
    //    production path is NAMED — not merely counted, so the failure is actionable.
    // -----------------------------------------------------------------------------------------------------------
    let measured = rust_contract_production_edits(
        Some(BASE),
        Some(AFTER),
        &answering("tests/tests/forge_scope__005.rs\nforge/src/engine/scope.rs\n"),
    )
    .expect("the range is measurable");
    assert_eq!(
        measured,
        vec!["forge/src/engine/scope.rs".to_string()],
        "{HARNESS}: QA names the production paths the candidate touched, and only those"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. CONTROL — A CANDIDATE THAT STAYS INSIDE ITS SCOPE IS ACCEPTED. Without this the whole proof would be
    //    satisfied by a gate that refuses every candidate, which is not the contract.
    // -----------------------------------------------------------------------------------------------------------
    let in_scope = Repo::changing(
        &["tests/tests/forge_scope__005.rs", "tests/tests/support/forge_scope.rs"],
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
        rust_contract_production_edits(
            Some(BASE),
            Some(AFTER),
            &answering("tests/tests/forge_scope__005.rs\ntests/tests/support/forge_scope.rs\n"),
        )
        .expect("the range is measurable")
        .is_empty(),
        "{HARNESS}: QA sees no production edit in a test-only range"
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
            &Repo::changing(&["tests/tests/forge_scope__005.rs", &path], Some("RUST_CONTRACT")),
            "smith",
            &mut into_root,
        );
        assert!(
            into_root.candidate_sha.is_none(),
            "{HARNESS}: reaching into {root} must be rejected, got a candidate"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 5. FAULT — AN UNMEASURABLE RANGE IS AN ERROR, NOT AN EMPTY LIST. This is the half that makes the gate a
    //    gate. `rust_contract_production_edits` answers `Ok(vec![])` for "nothing production was touched" and
    //    `Err(..)` for "I could not tell" — if a failed or unmeasurable git collapsed into an empty list, every
    //    story whose repository was unreadable would pass QA by default. The message says which range failed.
    // -----------------------------------------------------------------------------------------------------------
    let unmeasurable = |command: &str| CommandResult {
        command: command.to_string(),
        exit_code: 128,
        passed: false,
        excerpt: "fatal: bad object".into(),
        unmeasurable: true,
        output: String::new(),
    };
    let fault = rust_contract_production_edits(Some(BASE), Some(AFTER), &unmeasurable)
        .expect_err("an unmeasurable range must not read as a clean one");
    assert!(
        fault.contains("could not measure") && fault.contains(&format!("{BASE}..{AFTER}")),
        "{HARNESS}: the fault names the range it could not measure, got: {fault:?}"
    );

    // The same fault at the lane gate: git cannot list the changed paths, so the candidate is refused with the
    // reason named — not accepted because the list came back empty.
    let unreadable_diff = Repo::changing(&["tests/tests/forge_scope__005.rs"], Some("RUST_CONTRACT"))
        .answer(&format!("diff --name-only --no-renames {BASE}..{AFTER}"), None);
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

    // -----------------------------------------------------------------------------------------------------------
    // 6. FAULT — A MISSING BASE OR CANDIDATE IS AN ERROR TOO. `None` here means the run recorded no base or
    //    published no candidate, and both are QA FAILs rather than "nothing to check".
    // -----------------------------------------------------------------------------------------------------------
    let silent = answering("");
    assert!(
        rust_contract_production_edits(None, Some(AFTER), &silent)
            .expect_err("no base is a failure")
            .contains("execution base is missing"),
        "{HARNESS}: a missing base fails the gate"
    );
    assert!(
        rust_contract_production_edits(Some(BASE), None, &silent)
            .expect_err("no candidate is a failure")
            .contains("candidate SHA is missing"),
        "{HARNESS}: a missing candidate fails the gate"
    );
    // A base that is present but blank is the same shape, not a licence to measure nothing.
    assert!(
        rust_contract_production_edits(Some("   "), Some(AFTER), &silent)
            .expect_err("a blank base is a failure")
            .contains("execution base is missing"),
        "{HARNESS}: a blank base fails the gate rather than defaulting"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. THE SCOPE IS THE TEST MODE'S, NOT EVERY MODE'S. `candidate_rejection` only reads the changed paths when
    //    the packet declared `RUST_CONTRACT` (`smith.rs:147-149`). A story that is allowed to write production
    //    code is not judged by the test-artifact rule — so a boundary that applied it unconditionally would
    //    refuse every implementation story in the estate.
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
