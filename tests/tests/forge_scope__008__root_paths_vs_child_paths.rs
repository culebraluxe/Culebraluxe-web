//! FORGE.SCOPE — root paths vs child paths (TST-FORGE-SCOPE-008).
//!
//! Contract: "is this path production code?" is answered on PATH SEGMENTS, not on string prefixes. The roots
//! are `web/`, `middle/`, `db/`, `cli/` and `forge/` (`forge/src/engine/assay.rs:336`), and the classifier is
//!
//! ```rust
//! let path = path.trim();
//! PRODUCTION_ROOTS.iter().any(|root| path.starts_with(root))
//! ```
//!
//! (`assay.rs:338-341`). The trailing slash inside each root is the whole contract: it makes the test a test of
//! the first path SEGMENT rather than of the first few characters.
//!
//! The two halves that matter, and both are load-bearing:
//!
//!   * a CHILD path — `forge/src/engine/scope.rs` — is production, at any depth, because a directory's contents
//!     are its contents however deep they sit;
//!   * a NEIGHBOUR whose name merely starts with the same letters — `forge-tools/x.rs`, `website/x.rs`,
//!     `middlewares/x.rs`, `db-backup/x.sql`, `cli-docs/x.md` — is NOT production, because the boundary between
//!     `forge` and `forge-tools` is a `/` and the roots carry one.
//!
//! A classifier written as `path.starts_with("forge")` would pass every "child path is production" test and
//! still swallow four sibling trees, so the sibling half is the only part of this story that can fail.
//!
//! This classification is the load-bearing input to the lane gate — `candidate_rejection_in_surface`
//! (`forge/src/roles/smith/mod.rs:276-291`) — and to the QA-side reading of the very same listing, so a sibling
//! tree read as production would refuse legitimate test-authoring work (a false Hold), and a deeper production
//! path read as a sibling would let a product edit through as "test only". Neither direction is safe, and both
//! are asserted below through the gate itself, not only through the classifier.
//!
//! Deterministic and isolated: pure path classification plus a scripted `CandidateProbe`. No git process, no
//! filesystem, no network, no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_scope__008__root_paths_vs_child_paths

use std::collections::HashMap;

use forge::engine::assay::{is_rust_contract_production_path, CommandResult};
use forge::engine::runner::{CandidateProbe, HarnessOutput, RoleHarness};
use forge::roles::smith::judge_delivered_candidate;

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L1 Component";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-SCOPE-008";
/// The base the run recorded.
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// The candidate the lane produced.
const AFTER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// A repository described by its answers, with the test mode the packet declared.
struct Repo {
    answers: HashMap<String, String>,
}

impl Repo {
    /// A clean, descending, RUST_CONTRACT candidate whose range changes the given paths.
    fn changing(paths: &[&str]) -> Self {
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

/// Judge a RUST_CONTRACT candidate whose range changes `paths`, returning the lane's verdict.
fn judge(paths: &[&str]) -> (Option<String>, Option<String>) {
    let mut out = delivered_turn();
    judge_delivered_candidate(&Repo::changing(paths), "smith", &mut out);
    (out.candidate_sha, out.refusal)
}

/// The production paths of a `git diff --name-only` listing, by the one rule production applies.
///
/// This is the surviving half of the retired `rust_contract_production_edits` aggregate: the same
/// `is_rust_contract_production_path` the lane gate classifies its listing with, applied to the listing the gate
/// judged rather than to a second `git diff` QA used to run over the same range. An empty answer is the pass, and
/// a named path is the failure.
fn production_edits(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(str::trim)
        .filter(|path| !path.is_empty() && is_rust_contract_production_path(path))
        .map(str::to_string)
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SCOPE-008); the file and the assay use it.
fn forge_scope_008__root_paths_vs_child_paths() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. A CHILD PATH IS PRODUCTION, AT ANY DEPTH. The root is a directory: what is inside it is inside it,
    //    however deep. These are the real files of the five production roots in this workspace.
    // -----------------------------------------------------------------------------------------------------------
    for path in [
        "forge/src/lib.rs",
        "forge/src/engine/scope.rs",
        "forge/src/pianola/worker_authoring.rs",
        "forge/definitions/FORGE_SDLC-v6.xml",
        "db/src/pool.rs",
        "db/src/signature/signature_row.rs",
        "db/migrations/301_add_column.sql",
        "web/src/site.rs",
        "web/ui/src/app/mod.rs",
        "cli/src/smoke.rs",
        "middle/model/src/document_sign.rs",
        "middle/workflow/src/types.rs",
    ] {
        assert!(
            is_rust_contract_production_path(path),
            "{HARNESS}: {path} is a child of a production root and is production code ({STORY_ID})"
        );
    }
    // Depth is not a defence: the deepest production file in the estate is refused like the shallowest.
    assert!(
        is_rust_contract_production_path("forge/src/pianola/worker_authoring.rs")
            == is_rust_contract_production_path("forge/src/lib.rs"),
        "{HARNESS}: the classifier must not depend on depth"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. A NEIGHBOUR WHOSE NAME SHARES PREFIX LETTERS IS NOT PRODUCTION. This is the half a prefix-only
    //    classifier fails, and it is why each root carries its own `/`.
    // -----------------------------------------------------------------------------------------------------------
    for path in [
        // Same letters, no separator: not a child of `forge/`.
        "forge-tools/src/x.rs",
        "forgery/src/x.rs",
        "forge.yml",
        "forge.sh",
        "forge",
        // Same for every other root.
        "website/src/lib.rs",
        "middleware/src/x.rs",
        "db-backup/dump.sql",
        "database/schema.sql",
        "cli-docs/README.md",
        "client/src/x.rs",
        // And a NESTED sibling does not become a child by living inside something else.
        "tools/forge/src/x.rs",
        "docs/agent/notes-about-forge.md",
    ] {
        assert!(
            !is_rust_contract_production_path(path),
            "{HARNESS}: {path} shares letters with a production root but is NOT inside it"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 3. WHAT THE CLASSIFIER TRIMS, AND WHAT IT DOES NOT. `path.trim()` (`assay.rs:339`) is the whole of the
    //    normalisation, and it is worth pinning exactly, because a `./`-prefixed path is the one shape that
    //    reads like production and is not.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        is_rust_contract_production_path("  forge/src/engine/scope.rs \n"),
        "{HARNESS}: whitespace around a path must not hide it from the classifier"
    );
    // `git diff --name-only` prints repo-relative paths with no `./`, so the shape below does not arise from the
    //    listing the gate reads. It is recorded here because it is the boundary of this rule and not an
    //    accident of it: trim() removes whitespace, and `./db/` is a DIFFERENT first path segment from `db/`.
    assert!(
        !is_rust_contract_production_path("./db/src/pool.rs"),
        "{HARNESS}: `./` is a distinct first segment; trim() normalises whitespace, not path syntax"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE GATE AGREES WITH THE CLASSIFIER — THE CHILD HALF. A deep production path in the range is rejected
    //    by the lane gate, and the same classification names it when QA reads the same listing.
    // -----------------------------------------------------------------------------------------------------------
    let deep = "forge/src/pianola/worker_authoring.rs";
    let (candidate, refusal) = judge(&["tests/tests/forge_scope__008.rs", deep]);
    assert!(
        candidate.is_none(),
        "{HARNESS}: a deep production child path must be rejected ({STORY_ID})"
    );
    assert!(
        refusal
            .as_deref()
            .unwrap_or_default()
            .contains("RUST_CONTRACT candidate modified production code"),
        "{HARNESS}: the refusal names the rule, got: {refusal:?}"
    );
    let qa = production_edits(&format!("tests/tests/forge_scope__008.rs\n{deep}\n"));
    assert_eq!(
        qa,
        vec![deep.to_string()],
        "{HARNESS}: the deep production path is named, and only it"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE GATE AGREES WITH THE CLASSIFIER — THE SIBLING HALF. This is the direction that bites in
    //    production: a sibling tree read as production refuses legitimate test-authoring work, so a test suite
    //    living under `tools/` or a doc under `docs/` would come back Held with no product defect anywhere.
    // -----------------------------------------------------------------------------------------------------------
    for sibling in [
        "forge-tools/src/helper.rs",
        "website/src/theme.rs",
        "db-backup/restore.sh",
        "cli-docs/README.md",
        "docs/agent/notes-about-forge.md",
    ] {
        let (candidate, refusal) = judge(&["tests/tests/forge_scope__008.rs", sibling]);
        assert_eq!(
            candidate.as_deref(),
            Some(AFTER),
            "{HARNESS}: {sibling} is outside every production root and must not hold a test candidate ({refusal:?})"
        );
        assert!(
            refusal.is_none(),
            "{HARNESS}: {sibling} must not produce a refusal, got: {refusal:?}"
        );
        let qa = production_edits(&format!("tests/tests/forge_scope__008.rs\n{sibling}\n"));
        assert!(
            qa.is_empty(),
            "{HARNESS}: no production edit is reported for {sibling}, got: {qa:?}"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 6. A MIXED RANGE IS JUDGED PER PATH, NOT PER RANGE. One production sibling inside a range of legitimate
    //    work does not condemn the whole candidate silently, and it does not get lost either: the refusal names
    //    the range, and the classification names the path. This is what makes the boundary usable — a person can
    //    see WHICH file to move.
    // -----------------------------------------------------------------------------------------------------------
    let mixed = [
        "tests/tests/forge_scope__008.rs",
        "forge-tools/src/helper.rs",
        "middle/workflow/src/types.rs",
        "docs/agent/MEMORY.md",
    ];
    let qa_mixed = production_edits(&format!("{}\n", mixed.join("\n")));
    assert_eq!(
        qa_mixed,
        vec!["middle/workflow/src/types.rs".to_string()],
        "{HARNESS}: exactly the production child is reported, out of a range of legitimate work"
    );
    let (mixed_candidate, _) = judge(&mixed);
    assert!(
        mixed_candidate.is_none(),
        "{HARNESS}: one production child in the range still rejects the candidate"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. A BARE ROOT WITH NO CHILD IS NOT A PATH GIT WOULD EVER PRINT. `forge/` on its own is a directory, not
    //    a file, so it is neither production nor a sibling: the honest answer is that the roots are prefixes
    //    WITH a separator, and `forge/` alone is the only string they match. Pinning it keeps the shape visible
    //    rather than accidental.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        !is_rust_contract_production_path("forge"),
        "{HARNESS}: a bare directory name is not a file path and is not production code"
    );
}
