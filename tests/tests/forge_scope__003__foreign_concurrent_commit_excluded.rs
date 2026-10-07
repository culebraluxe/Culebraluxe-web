//! FORGE.SCOPE — foreign concurrent commit excluded (TST-FORGE-SCOPE-003).
//!
//! Contract: a commit that lands on the shared history WHILE this lane is running is not this lane's work, and
//! the scope boundary must exclude it — from the file set it attributes to the run, and from the candidate it
//! will accept.
//!
//! Two production seams carry that fact:
//!
//!   1. **the ancestry gate** — `candidate_own_changed_files` (`forge/src/engine/scope.rs:41`) asks
//!      `is_ancestor(recorded_base, candidate)` BEFORE reading anything. A candidate that is not a descendant
//!      of the recorded base is `CandidateOwnChanges::Fail` (`scope.rs:62-68`) and no file is read at all. That
//!      is the whole defence against a foreign head: a peer who pushed first and moved the tip cannot have
//!      their history adopted as this run's candidate.
//!   2. **the lane gate** — `judge_delivered_candidate` (`forge/src/roles/smith.rs:74`) → `candidate_rejection`
//!      (`smith.rs:121-128`) asks git `merge-base --is-ancestor <base> <after>` and refuses the candidate when
//!      git does not confirm the ancestry. Note the shape: `probe.git(..).is_none()` is a REFUSAL. A repository
//!      that cannot answer the ancestry question certifies nothing.
//!
//! The negative case is the concurrency itself, made deterministic: the peer commits on the same base, the
//! candidate is then built on top of the PEER'S commit rather than on the recorded base, and the whole boundary
//! must refuse. The controls matter as much — a lane whose candidate genuinely descends from the recorded base
//! is accepted even while a foreign commit sits in the same object store, so the proof cannot be satisfied by a
//! boundary that refuses every contended run.
//!
//! Deterministic and isolated: the repository is a scripted `CandidateProbe` and a scripted reader at the two
//! boundaries production injects. No git process, no filesystem, no network, no database, nothing written.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_scope__003__foreign_concurrent_commit_excluded

use std::collections::HashMap;

use forge::engine::assay::CommandResult;
use forge::engine::runner::{CandidateProbe, HarnessOutput, RoleHarness};
use forge::engine::scope::{candidate_own_changed_files, CandidateOwnChanges};
use forge::roles::smith::judge_delivered_candidate;

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L1 Component";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-SCOPE-003";
/// The base the run recorded before the peer started committing.
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// This lane's own commit, built on the recorded base.
const LANE_COMMIT: &str = "1111111111111111111111111111111111111111";
/// The peer's concurrent commit, also built on the recorded base — a sibling, not an ancestor.
const PEER_COMMIT: &str = "9999999999999999999999999999999999999999";
/// A candidate built on the PEER's commit, so it descends from the peer and never from the recorded base.
const CANDIDATE_ON_PEER: &str = "8888888888888888888888888888888888888888";

/// A repository described by its answers, plus a record of every commit the boundary asked about.
///
/// The lane gate asks three questions and all three are answered here, because a probe that could only answer
/// the ancestry question would refuse every candidate for an unrelated reason and prove nothing: `merge-base
/// --is-ancestor` (is the candidate on the recorded base's history), `status --porcelain` (is the tree clean),
/// and `diff --name-only --no-renames <base>..<candidate>` (does it change anything).
struct ContendedRepo {
    ancestry: HashMap<(String, String), bool>,
    files: HashMap<String, Vec<String>>,
    /// The `status --porcelain` answer: `None` is an unreadable status, `Some(text)` is that text.
    status: Option<String>,
    /// The `diff --name-only --no-renames <base>..<candidate>` answer, keyed by the range.
    diffs: HashMap<String, Option<String>>,
    /// `RoleHarness` is `Send + Sync`, so the record of what was read is behind a mutex rather than a `RefCell`.
    read: std::sync::Mutex<Vec<String>>,
}

impl ContendedRepo {
    /// A repository with a clean tree, and diffs that list the commits' own files.
    fn new(ancestry: &[(&str, &str, bool)], files: &[(&str, &[&str])]) -> Self {
        let mut repo = Self {
            ancestry: ancestry
                .iter()
                .map(|(a, d, yes)| (((*a).to_string(), (*d).to_string()), *yes))
                .collect(),
            files: files
                .iter()
                .map(|(sha, paths)| {
                    ((*sha).to_string(), paths.iter().map(|p| (*p).to_string()).collect())
                })
                .collect(),
            status: Some(String::new()),
            diffs: HashMap::new(),
            read: std::sync::Mutex::new(Vec::new()),
        };
        // `BASE..<sha>` lists what that commit added to the base, which is exactly what `files` records.
        let base = BASE.to_string();
        for (sha, paths) in &repo.files {
            let range = format!("{base}..{sha}");
            let listed = paths.join("\n");
            repo.diffs.insert(range, Some(listed));
        }
        repo
    }

    /// Make `git status --porcelain` answer `answer` — `None` for an unreadable status.
    fn with_status(mut self, answer: Option<&str>) -> Self {
        self.status = answer.map(str::to_string);
        self
    }

    /// git's answer for the three questions the lane gate asks. An answer of `None` means git failed or said
    /// no; production treats both as a refusal, which is the fail-closed half this story depends on.
    fn probe(&self, args: &[&str]) -> Option<String> {
        match args.first().copied()? {
            "merge-base" => {
                let ancestor = args.get(2)?;
                let descendant = args.get(3)?;
                self.ancestry
                    .get(&(ancestor.to_string(), descendant.to_string()))
                    .filter(|yes| **yes)
                    .map(|_| String::new())
            }
            "status" => self.status.clone(),
            "diff" => {
                // `diff --name-only --no-renames <base>..<candidate>`
                let range = *args.last()?;
                self.diffs.get(range).cloned().flatten()
            }
            _ => None,
        }
    }

    fn read_changed_files(&self, sha: &str) -> Vec<String> {
        self.read.lock().expect("the read log is never poisoned").push(sha.to_string());
        self.files.get(sha).cloned().unwrap_or_default()
    }

    /// Every commit the boundary asked about, in order.
    fn read_log(&self) -> Vec<String> {
        self.read.lock().expect("the read log is never poisoned").clone()
    }
}

impl CandidateProbe for ContendedRepo {
    fn git(&self, args: &[&str]) -> Option<String> {
        self.probe(args)
    }
    fn declared_test_mode(&self) -> Option<&str> {
        None
    }
}

impl RoleHarness for ContendedRepo {
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

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SCOPE-003); the file and the assay use it.
fn forge_scope_003__foreign_concurrent_commit_excluded() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE SHARED HISTORY. Recorded base, this lane's commit on it, and a peer's commit on the SAME base —
    //    the exact shape of two lanes dispatched together. Both are present and reachable; only one is ours.
    // -----------------------------------------------------------------------------------------------------------
    let contended = ContendedRepo::new(
        &[
            (BASE, LANE_COMMIT, true),
            (BASE, PEER_COMMIT, true),
            // The contended candidate descends from the PEER, so the recorded base is NOT its ancestor.
            (BASE, CANDIDATE_ON_PEER, false),
            (PEER_COMMIT, CANDIDATE_ON_PEER, true),
        ],
        &[
            (LANE_COMMIT, &["tests/tests/forge_scope__003.rs"]),
            (PEER_COMMIT, &["db/src/property.rs"]),
            (
                CANDIDATE_ON_PEER,
                &["db/src/property.rs", "tests/tests/forge_scope__003.rs"],
            ),
        ],
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE CONTRACT — A CANDIDATE BUILT ON A FOREIGN COMMIT IS EXCLUDED. It descends from the peer, so its
    //    diff against the recorded base contains the peer's file. Adopting it would make this story's "changed
    //    files" include a database migration it never wrote. The boundary refuses, and it refuses BEFORE
    //    reading a single file — the file set is never even computed.
    // -----------------------------------------------------------------------------------------------------------
    let outcome = candidate_own_changed_files(
        Some(CANDIDATE_ON_PEER),
        Some(BASE),
        &[CANDIDATE_ON_PEER.to_string()],
        |sha| contended.read_changed_files(sha),
        |ancestor, descendant| {
            contended
                .ancestry
                .get(&(ancestor.to_string(), descendant.to_string()))
                .copied()
                .unwrap_or(false)
        },
    );
    let refusal = match &outcome {
        CandidateOwnChanges::Fail { reason } => reason.clone(),
        other => panic!(
            "{HARNESS}: a candidate built on a foreign commit must be excluded, got {other:?} ({STORY_ID})"
        ),
    };
    assert!(
        refusal.contains("not a descendant") && refusal.contains(BASE),
        "{HARNESS}: the exclusion must name the recorded base the candidate failed to descend from, got: {refusal:?}"
    );
    assert!(
        contended.read_log().is_empty(),
        "{HARNESS}: an excluded candidate must not have its files read at all, read: {:?}",
        contended.read_log()
    );

    // The lane gate refuses the same candidate, for the same reason, with the same wording the refusal carries.
    let mut out = delivered_turn(BASE, CANDIDATE_ON_PEER);
    judge_delivered_candidate(&contended, "smith", &mut out);
    assert!(
        out.candidate_sha.is_none(),
        "{HARNESS}: a candidate on a foreign history must not remain the run's candidate"
    );
    assert!(
        out.refusal
            .as_deref()
            .unwrap_or_default()
            .contains("not a descendant of execution base"),
        "{HARNESS}: the lane gate must exclude the foreign candidate, got: {:?}",
        out.refusal
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. CONTROL — A LANE CANDIDATE IS ACCEPTED WHILE A PEER SITS IN THE SAME OBJECT STORE. Concurrency is not
    //    the fault; descending from the recorded base is. Without this half, a boundary that refused every
    //    contended run would satisfy section 2 and this story would be proved by a gate that simply refuses.
    // -----------------------------------------------------------------------------------------------------------
    let ours = candidate_own_changed_files(
        Some(LANE_COMMIT),
        Some(BASE),
        &[LANE_COMMIT.to_string()],
        |sha| contended.read_changed_files(sha),
        |ancestor, descendant| {
            contended
                .ancestry
                .get(&(ancestor.to_string(), descendant.to_string()))
                .copied()
                .unwrap_or(false)
        },
    );
    assert!(
        matches!(&ours, CandidateOwnChanges::Ok { changed_files }
            if changed_files == &vec!["tests/tests/forge_scope__003.rs".to_string()]),
        "{HARNESS}: this lane's own commit measures to its own file only, got: {ours:?}"
    );

    let mut accepted = delivered_turn(BASE, LANE_COMMIT);
    judge_delivered_candidate(&contended, "smith", &mut accepted);
    assert_eq!(
        accepted.candidate_sha.as_deref(),
        Some(LANE_COMMIT),
        "{HARNESS}: a candidate descending from the recorded base is accepted while a peer commits alongside"
    );
    assert!(
        accepted.refusal.is_none(),
        "{HARNESS}: an accepted candidate carries no refusal, got: {:?}",
        accepted.refusal
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — A CANDIDATE THAT IS THE PEER'S COMMIT ITSELF. Not a merge, not a descendant of our base:
    //    the peer simply got there first and its sha became HEAD. Same answer, same reason.
    // -----------------------------------------------------------------------------------------------------------
    let peer_head = candidate_own_changed_files(
        Some(PEER_COMMIT),
        Some(BASE),
        &[PEER_COMMIT.to_string()],
        |sha| contended.read_changed_files(sha),
        |_, _| false,
    );
    assert!(
        matches!(peer_head, CandidateOwnChanges::Fail { .. }),
        "{HARNESS}: the peer's commit is not this run's candidate, got: {peer_head:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. FAULT — A GIT THAT CANNOT ANSWER THE ANCESTRY EXCLUDES EVERYTHING. `probe.git(..)` returns `None` for
    //    both "git said no" and "git could not answer", and production treats both as a refusal. That is the
    //    safe direction: a shallow clone with no base object, a corrupted object store, or a repository that has
    //    been re-created under the lane all produce a candidate this boundary cannot vouch for, and a boundary
    //    that read "unknown" as "yes" would adopt whatever HEAD happened to be.
    // -----------------------------------------------------------------------------------------------------------
    // Only the ancestry table is emptied: the tree is still clean and the diff still lists, so the refusal that
    // comes back is provably the ancestry question and not one of the other two.
    let mut no_ancestry = ContendedRepo::new(
        &[],
        &[
            (LANE_COMMIT, &["tests/tests/forge_scope__003.rs"]),
            (PEER_COMMIT, &["db/src/property.rs"]),
        ],
    );
    no_ancestry.ancestry.clear();
    let mut unknown = delivered_turn(BASE, LANE_COMMIT);
    judge_delivered_candidate(&no_ancestry, "smith", &mut unknown);
    assert!(
        unknown.candidate_sha.is_none(),
        "{HARNESS}: an unreadable ancestry must not certify a candidate"
    );
    assert!(
        unknown
            .refusal
            .as_deref()
            .unwrap_or_default()
            .contains("not a descendant"),
        "{HARNESS}: the fault surfaces as the ancestry exclusion, got: {:?}",
        unknown.refusal
    );

    // The same fail-closed shape for the tree: an unreadable `git status` is a refusal too, never a pass. A
    // dirty tree means the commit is not the whole work, and adopting it as a candidate would void the rest.
    let unreadable_status =
        ContendedRepo::new(&[(BASE, LANE_COMMIT, true)], &[(LANE_COMMIT, &["tests/tests/forge_scope__003.rs"])])
            .with_status(None);
    let mut status_unknown = delivered_turn(BASE, LANE_COMMIT);
    judge_delivered_candidate(&unreadable_status, "smith", &mut status_unknown);
    assert!(
        status_unknown.candidate_sha.is_none(),
        "{HARNESS}: an unreadable tree state must not certify a candidate"
    );
    assert!(
        status_unknown
            .refusal
            .as_deref()
            .unwrap_or_default()
            .contains("git status is unreadable"),
        "{HARNESS}: the fault is named, not swallowed, got: {:?}",
        status_unknown.refusal
    );

    let unknown_scope = candidate_own_changed_files(
        Some(LANE_COMMIT),
        Some(BASE),
        &[LANE_COMMIT.to_string()],
        |sha| contended.read_changed_files(sha),
        |_, _| false,
    );
    assert!(
        matches!(unknown_scope, CandidateOwnChanges::Fail { .. }),
        "{HARNESS}: the pure boundary excludes the same candidate, got: {unknown_scope:?}"
    );
}
