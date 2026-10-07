//! FORGE.SCOPE — scope additions accepted (TST-FORGE-SCOPE-004).
//!
//! Contract: work a lane adds BEYOND the candidate commit is in scope. A lane is not one commit — it is a
//! branch of attempts against one recorded base, and the last commit is only the tip. If the measured set were
//! the candidate's diff alone, every earlier attempt's work would vanish from the run's evidence: the artifact
//! would carry a patch that does not replay, and the "files this story touched" list would under-report by
//! exactly the amount that matters.
//!
//! The production boundary is `candidate_own_changed_files` (`forge/src/engine/scope.rs:41`). Additions are
//! accepted by construction: the commit list is
//!
//! ```text
//! [candidate] + normalize(every recorded lane commit), skipping any that repeats
//! ```
//!
//! (`scope.rs:69-76`), every file of every one of those commits is collected (`scope.rs:78-85`), and the
//! result is sorted and de-duplicated (`scope.rs:86`). `CandidateOwnChanges::Ok { changed_files }` is the
//! acceptance: the additions are IN the set, not reported as an exception to it.
//!
//! The same acceptance is observable one layer up, where the acceptance becomes the artifact: Smith's capture
//! writes the changed-file list and the patch as CODE under the base it was taken against
//! (`smith_candidate_artifact`, `forge/src/roles/smith.rs:334`), so an addition that the scope boundary
//! accepted is carried in `detail.changedFiles` and the patch is taken over the FULL range. A refusal, by
//! contrast, is a different artifact kind with `detail.refusal` set (`smith_refused_work_artifact`,
//! `smith.rs:370`) — so "accepted" and "refused" are distinguishable in the row itself, not by prose.
//!
//! Negative case: the union must not become a grab bag. A commit that is not on the lane's list, and a record
//! that is not a commit, are both excluded — which is TST-FORGE-SCOPE-002's contract seen from the other side.
//! The control: with NO additions the measured set is the candidate's own files, so this test cannot be
//! satisfied by a boundary that always reports a fixed set.
//!
//! Deterministic and isolated: the repository is a scripted reader at the boundary production injects. No git
//! process, no filesystem, no network, no database, nothing written.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_scope__004__scope_additions_accepted

use std::cell::RefCell;
use std::collections::HashMap;

use forge::engine::scope::{candidate_own_changed_files, CandidateOwnChanges};
use forge::roles::smith::{smith_candidate_artifact, smith_refused_work_artifact};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L1 Component";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-SCOPE-004";
/// The base the run recorded.
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// The first attempt, whose work the candidate does not contain on its own.
const ATTEMPT_ONE: &str = "1111111111111111111111111111111111111111";
/// The repair attempt: the candidate this run published.
const ATTEMPT_TWO: &str = "2222222222222222222222222222222222222222";

/// The repository as the scope boundary sees it, recording every commit it was asked about.
struct Repo {
    files: HashMap<String, Vec<String>>,
    read: RefCell<Vec<String>>,
}

impl Repo {
    fn with(commits: &[(&str, &[&str])]) -> Self {
        Self {
            files: commits
                .iter()
                .map(|(sha, paths)| {
                    ((*sha).to_string(), paths.iter().map(|p| (*p).to_string()).collect())
                })
                .collect(),
            read: RefCell::new(Vec::new()),
        }
    }
    fn read_changed_files(&self, sha: &str) -> Vec<String> {
        self.read.borrow_mut().push(sha.to_string());
        self.files.get(sha).cloned().unwrap_or_default()
    }
}

/// Ancestry for a healthy run: the recorded base is the ancestor of every commit this lane made.
fn descends(base: &str, descendant: &str) -> bool {
    base == BASE && matches!(descendant, ATTEMPT_ONE | ATTEMPT_TWO)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SCOPE-004); the file and the assay use it.
fn forge_scope_004__scope_additions_accepted() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — THE ADDITION IS IN THE MEASURED SET. Attempt one added the fixture and the packet;
    //    attempt two (the candidate) added the canonical test and amended the fixture. Every file of both is
    //    in the run's changed-file set — the addition is accepted, not reported as an exception.
    // -----------------------------------------------------------------------------------------------------------
    let repo = Repo::with(&[
        (ATTEMPT_ONE, &["tests/tests/support/forge_scope__004.rs", "docs/agent/packets/TST-FORGE-SCOPE-004.md"]),
        (
            ATTEMPT_TWO,
            &["tests/tests/forge_scope__004.rs", "tests/tests/support/forge_scope__004.rs"],
        ),
    ]);
    let outcome = candidate_own_changed_files(
        Some(ATTEMPT_TWO),
        Some(BASE),
        &[ATTEMPT_ONE.to_string(), ATTEMPT_TWO.to_string()],
        |sha| repo.read_changed_files(sha),
        descends,
    );
    let changed = match &outcome {
        CandidateOwnChanges::Ok { changed_files } => changed_files.clone(),
        other => panic!(
            "{HARNESS}: additions beyond the candidate commit must be accepted, got {other:?} ({STORY_ID})"
        ),
    };
    assert_eq!(
        changed,
        vec![
            "docs/agent/packets/TST-FORGE-SCOPE-004.md".to_string(),
            "tests/tests/forge_scope__004.rs".to_string(),
            "tests/tests/support/forge_scope__004.rs".to_string(),
        ],
        "{HARNESS}: the accepted set is the union over the lane's own commits, sorted"
    );

    // The addition is genuinely there because it was really read, not because it was guessed at.
    assert_eq!(
        repo.read.borrow().clone(),
        vec![ATTEMPT_TWO.to_string(), ATTEMPT_ONE.to_string()],
        "{HARNESS}: the candidate AND the earlier attempt were both read, so the addition is measured"
    );

    // And the addition the fixture proves: a file ONLY the earlier attempt touched is still in the set.
    assert!(
        changed
            .iter()
            .any(|path| path == "docs/agent/packets/TST-FORGE-SCOPE-004.md"),
        "{HARNESS}: a file added by an earlier attempt is in scope for the run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. CONTROL — WITH NO ADDITIONS THE SET IS THE CANDIDATE'S OWN. A boundary that reported a fixed list, or
    //    that always unioned in something, would fail here: this is the same boundary with one commit.
    // -----------------------------------------------------------------------------------------------------------
    let single = Repo::with(&[(ATTEMPT_TWO, &["tests/tests/forge_scope__004.rs"])]);
    let without = candidate_own_changed_files(
        Some(ATTEMPT_TWO),
        Some(BASE),
        &[ATTEMPT_TWO.to_string()],
        |sha| single.read_changed_files(sha),
        descends,
    );
    assert!(
        matches!(&without, CandidateOwnChanges::Ok { changed_files }
            if changed_files == &vec!["tests/tests/forge_scope__004.rs".to_string()]),
        "{HARNESS}: with no additions the measured set is exactly the candidate's files, got: {without:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — THE UNION IS NOT A GRAB BAG. A commit the lane did not record, and a record that is not a
    //    commit, are both outside the union. Without these the "addition accepted" rule would be satisfied by a
    //    boundary that reads everything on the branch, which is the failure TST-FORGE-SCOPE-002 names.
    // -----------------------------------------------------------------------------------------------------------
    const PEER: &str = "9999999999999999999999999999999999999999";
    let crowded = Repo::with(&[
        (ATTEMPT_ONE, &["tests/tests/support/forge_scope__004.rs"]),
        (ATTEMPT_TWO, &["tests/tests/forge_scope__004.rs"]),
        (PEER, &["db/src/property.rs"]),
    ]);
    let bounded = candidate_own_changed_files(
        Some(ATTEMPT_TWO),
        Some(BASE),
        &[ATTEMPT_ONE.to_string(), ATTEMPT_TWO.to_string(), "not-a-commit".to_string()],
        |sha| crowded.read_changed_files(sha),
        descends,
    );
    assert!(
        matches!(&bounded, CandidateOwnChanges::Ok { changed_files }
            if changed_files == &vec![
                "tests/tests/forge_scope__004.rs".to_string(),
                "tests/tests/support/forge_scope__004.rs".to_string(),
            ]),
        "{HARNESS}: the union covers this lane's commits and nothing else, got: {bounded:?}"
    );
    assert!(
        !crowded
            .read
            .borrow()
            .iter()
            .any(|sha| sha == PEER || sha == "not-a-commit"),
        "{HARNESS}: neither a peer's commit nor a non-commit record is read, read: {:?}",
        crowded.read.borrow()
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE ACCEPTANCE IS CARRIED IN THE ARTIFACT, NOT ASSERTED IN PROSE. An accepted candidate and a refused
    //    one produce the same row kind with opposite contents: the accepted one names the base the patch was
    //    taken against and the full changed-file set; the refused one names why it is not a candidate and says
    //    the patch is a worktree snapshot. A reader recovering paid code from this row must be able to tell
    //    which it is without reading a model's sentence.
    // -----------------------------------------------------------------------------------------------------------
    let accepted = smith_candidate_artifact(
        STORY_ID,
        Some("11111111-2222-3333-4444-555555555555"),
        ATTEMPT_TWO,
        BASE,
        "diff --git a/tests/tests/forge_scope__004.rs b/tests/tests/forge_scope__004.rs\n+#[test]\n",
        &changed,
    );
    assert_eq!(accepted.kind, "candidate-code", "{HARNESS}: an accepted candidate is captured as code");
    assert_eq!(accepted.sha.as_deref(), Some(ATTEMPT_TWO), "{HARNESS}: the row names the candidate");
    let detail = accepted.detail.expect("the detail carries the measurement");
    assert_eq!(
        detail["base"].as_str(),
        Some(BASE),
        "{HARNESS}: the patch is recorded against the base it replays on"
    );
    assert_eq!(
        detail["changedFiles"].as_array().map(|files| files.len()),
        Some(changed.len()),
        "{HARNESS}: the accepted row carries the FULL measured set, additions included"
    );
    assert!(
        detail["changedFiles"]
            .as_array()
            .expect("changedFiles is an array")
            .iter()
            .any(|path| path.as_str() == Some("tests/tests/support/forge_scope__004.rs")),
        "{HARNESS}: the addition is in the recorded changed-file set, got: {}",
        detail["changedFiles"]
    );

    // The refused row says so in its own fields — a refusal must not read as an accepted candidate to anyone
    // who recovers the work.
    let refused = smith_refused_work_artifact(
        STORY_ID,
        None,
        Some(ATTEMPT_TWO),
        BASE,
        "diff --git a/tests/tests/forge_scope__004.rs b/tests/tests/forge_scope__004.rs\n",
        &["tests/tests/forge_scope__004.rs".to_string()],
        "RUST_CONTRACT candidate modified production code across base..candidate",
    );
    let refused_detail = refused.detail.expect("the refused row carries its reason");
    assert_eq!(
        refused_detail["worktreeSnapshot"].as_bool(),
        Some(true),
        "{HARNESS}: a refused candidate's patch is a whole-tree snapshot, not the candidate's range"
    );
    assert!(
        refused
            .summary
            .as_deref()
            .unwrap_or_default()
            .contains("REFUSED, not a candidate"),
        "{HARNESS}: a refusal is labelled in the row itself, got: {:?}",
        refused.summary
    );
    assert!(
        refused_detail["refusal"].as_str().unwrap_or_default().contains("production code"),
        "{HARNESS}: the refusal names the reason, got: {}",
        refused_detail["refusal"]
    );
}
