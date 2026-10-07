//! FORGE.SCOPE — only lane's own commits count (TST-FORGE-SCOPE-002).
//!
//! Contract: the changed-file set a run attributes to a lane is measured from the commits THAT LANE made —
//! the candidate plus the commits the lane recorded as its own. It is never measured from the branch, because a
//! branch carries every co-worker's commit, and attributing a peer's file to this story is how a "this story
//! only touched tests" claim becomes a lie.
//!
//! The production boundary is `candidate_own_changed_files` (`forge/src/engine/scope.rs:41`), and the counting
//! is the part under test: it builds its commit list as
//!
//! ```text
//! [candidate] + normalize(every recorded lane commit), skipping any that repeats
//! ```
//!
//! (`scope.rs:69-76`) and then reads ONLY those (`scope.rs:78-85`). The base is deliberately NOT read: a base
//! commit changed nothing this lane did, and reading it would pull the previous story's files into this one.
//!
//! `recorded_scope_base` (`scope.rs:25`) is the other half of the same reading — the lane's own recorded
//! commits are what the base is derived from, and it is the EARLIEST of them, not the last one the list
//! happened to carry.
//!
//! The negative case is load-bearing and is what makes this a proof rather than a description: a foreign
//! concurrent commit that is reachable from the same base is present in the object store, would be found by any
//! boundary that walked the branch, and must contribute NOTHING here. The control case is equally load-bearing:
//! the lane's own commits must all be counted, so the proof cannot be satisfied by a boundary that counts none.
//!
//! Deterministic and isolated: the repository is a scripted reader at the boundary production injects
//! (`read_changed_files`), so no git process, no filesystem, no network, no database, and nothing written.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_scope__002__only_lane_s_own_commits_count

use std::cell::RefCell;
use std::collections::HashMap;

use forge::engine::scope::{candidate_own_changed_files, recorded_scope_base, CandidateOwnChanges};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L1 Component";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-SCOPE-002";
/// The base the run recorded.
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// The lane's first commit.
const LANE_ONE: &str = "1111111111111111111111111111111111111111";
/// The lane's second commit — the candidate this run published.
const LANE_TWO: &str = "2222222222222222222222222222222222222222";
/// A co-worker's commit on the same base. Reachable, present, and not this lane's.
const FOREIGN: &str = "9999999999999999999999999999999999999999";

/// The repository as the scope boundary sees it: a sha → its changed files, and a record of every sha the
/// boundary actually asked about, so "did not count" is an observation rather than an assumption.
struct Repository {
    files_by_commit: HashMap<String, Vec<String>>,
    read: RefCell<Vec<String>>,
}

impl Repository {
    fn with(commits: &[(&str, &[&str])]) -> Self {
        Self {
            files_by_commit: commits
                .iter()
                .map(|(sha, files)| {
                    ((*sha).to_string(), files.iter().map(|f| (*f).to_string()).collect())
                })
                .collect(),
            read: RefCell::new(Vec::new()),
        }
    }

    /// Read a commit's files, recording that the boundary asked. An unknown sha reads as nothing — which is
    /// what an unreadable commit looks like, and must never widen the set.
    fn read_changed_files(&self, sha: &str) -> Vec<String> {
        self.read.borrow_mut().push(sha.to_string());
        self.files_by_commit.get(sha).cloned().unwrap_or_default()
    }

    /// Every sha the boundary asked about, in order.
    fn read_log(&self) -> Vec<String> {
        self.read.borrow().clone()
    }
}

/// Ancestry for this story: the recorded base is an ancestor of the lane's commits and of the candidate, and
/// of nothing foreign. Every case below is otherwise healthy, so the only thing under test is the COUNTING.
fn is_lane_descendant(base: &str, descendant: &str) -> bool {
    base == BASE && matches!(descendant, LANE_ONE | LANE_TWO)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SCOPE-002); the file and the assay use it.
fn forge_scope_002__only_lane_s_own_commits_count() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — THE LANE'S OWN COMMITS ARE COUNTED. Both of the commits the lane recorded are read and
    //    every file they touched is attributed to this run, in a stable (sorted, de-duplicated) order.
    // -----------------------------------------------------------------------------------------------------------
    let repo = Repository::with(&[
        (LANE_ONE, &["tests/tests/forge_scope__002.rs", "tests/tests/support/helper.rs"]),
        (LANE_TWO, &["tests/tests/forge_scope__002.rs", "docs/agent/packets/TST-FORGE-SCOPE-002.md"]),
    ]);
    let outcome = candidate_own_changed_files(
        Some(LANE_TWO),
        Some(BASE),
        &[LANE_ONE.to_string(), LANE_TWO.to_string()],
        |sha| repo.read_changed_files(sha),
        is_lane_descendant,
    );
    let changed = match &outcome {
        CandidateOwnChanges::Ok { changed_files } => changed_files.clone(),
        other => panic!("{HARNESS}: the lane's own commits must be counted, got {other:?}"),
    };
    assert_eq!(
        changed,
        vec![
            "docs/agent/packets/TST-FORGE-SCOPE-002.md".to_string(),
            "tests/tests/forge_scope__002.rs".to_string(),
            "tests/tests/support/helper.rs".to_string(),
        ],
        "{HARNESS}: the measured set is the lane's own union, sorted and free of repeats ({STORY_ID})"
    );
    // The control: a boundary that counted NOTHING would produce an empty list, so this list is the proof the
    // lane's commits were really read rather than defaulted.
    assert!(
        !changed.is_empty(),
        "{HARNESS}: the lane's own commits must contribute their files"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE BASE IS NOT THE LANE'S WORK. The base commit is where the lane started, not something it did, so
    //    reading it would attribute the previous story's files to this one. Production builds the commit list
    //    from the candidate and the lane commits alone; the base is only ever the ancestry argument.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        repo.read_log(),
        vec![LANE_TWO.to_string(), LANE_ONE.to_string()],
        "{HARNESS}: only the candidate and the lane's own recorded commits are read, never the base"
    );
    assert!(
        !repo.read_log().contains(&BASE.to_string()),
        "{HARNESS}: the recorded base is measured against, never attributed to the lane"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — A FOREIGN CONCURRENT COMMIT IS NOT THIS LANE'S WORK. A co-worker committed on the same
    //    base while this lane was running. That commit is in the object store and reachable from the base, so
    //    any boundary that walked the branch would find its file. This one must not read it, and its file must
    //    not appear. This is the case the whole story exists for.
    // -----------------------------------------------------------------------------------------------------------
    let contended = Repository::with(&[
        (LANE_ONE, &["tests/tests/forge_scope__002.rs"]),
        (LANE_TWO, &["tests/tests/forge_scope__002.rs"]),
        (FOREIGN, &["db/src/property.rs", "web/src/site.rs"]),
    ]);
    let with_peers = candidate_own_changed_files(
        Some(LANE_TWO),
        Some(BASE),
        &[LANE_ONE.to_string(), LANE_TWO.to_string()],
        |sha| contended.read_changed_files(sha),
        is_lane_descendant,
    );
    let attributed = match &with_peers {
        CandidateOwnChanges::Ok { changed_files } => changed_files.clone(),
        other => panic!("{HARNESS}: a healthy lane measures cleanly, got {other:?}"),
    };
    assert_eq!(
        attributed,
        vec!["tests/tests/forge_scope__002.rs".to_string()],
        "{HARNESS}: a peer's files are not this lane's changed files"
    );
    for peer_file in ["db/src/property.rs", "web/src/site.rs"] {
        assert!(
            !attributed.iter().any(|path| path == peer_file),
            "{HARNESS}: {peer_file} belongs to another lane and must not be attributed here"
        );
    }
    assert!(
        !contended.read_log().contains(&FOREIGN.to_string()),
        "{HARNESS}: the boundary must not even read a commit the lane did not record"
    );

    // The same holds when the peer is the candidate's own ancestor: the commit list is built from what the lane
    // RECORDED, so a peer commit sitting in the history is not silently promoted into the set.
    let peer_in_history = candidate_own_changed_files(
        Some(LANE_TWO),
        Some(BASE),
        &[LANE_ONE.to_string()],
        |sha| contended.read_changed_files(sha),
        is_lane_descendant,
    );
    assert!(
        matches!(peer_in_history, CandidateOwnChanges::Ok { .. }),
        "{HARNESS}: omitting the candidate from the lane's own list is still a healthy measurement"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. FAULT — A RECORD THAT IS NOT A COMMIT IS NOT COUNTED AS ONE. A lane's recorded commits arrive from a
    //    worktree and a journal, so a truncated sha, a short abbreviation, or a ref name can be in the list.
    //    `normalize` accepts a full 40-character hex commit and nothing else, so those records are dropped
    //    before anything is read. Counting them would either read nothing (silently narrowing the set) or read
    //    the wrong object (silently widening it) — neither is a measurement of the lane's work.
    // -----------------------------------------------------------------------------------------------------------
    let noisy = Repository::with(&[
        (LANE_ONE, &["tests/tests/forge_scope__002.rs"]),
        (LANE_TWO, &["tests/tests/forge_scope__002.rs"]),
    ]);
    let malformed = [
        "2222222",                    // a short abbreviation, not a commit
        "not-a-sha-at-all",           // prose that reached the journal
        "2222222222222222222222222222222222222222z", // 40 chars, but not hex
        "",                           // an empty record
    ];
    let lane_list: Vec<String> = std::iter::once(LANE_ONE.to_string())
        .chain(malformed.iter().map(|s| (*s).to_string()))
        .collect();
    let after_noise = candidate_own_changed_files(
        Some(LANE_TWO),
        Some(BASE),
        &lane_list,
        |sha| noisy.read_changed_files(sha),
        is_lane_descendant,
    );
    assert!(
        matches!(&after_noise, CandidateOwnChanges::Ok { changed_files }
            if changed_files == &vec!["tests/tests/forge_scope__002.rs".to_string()]),
        "{HARNESS}: malformed records must be dropped, not measured, got: {after_noise:?}"
    );
    for record in malformed {
        assert!(
            !noisy.read_log().iter().any(|sha| sha == record),
            "{HARNESS}: {record:?} is not a commit and must never be handed to the repository"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE SAME COMMIT IS COUNTED ONCE. The candidate is nearly always also the lane's newest recorded
    //    commit, and a journal that lists both must not produce a doubled set — or a doubled file list in
    //    every artifact written downstream.
    // -----------------------------------------------------------------------------------------------------------
    let repeated = Repository::with(&[(LANE_TWO, &["tests/tests/forge_scope__002.rs"])]);
    let duplicated = candidate_own_changed_files(
        Some(LANE_TWO),
        Some(BASE),
        &[LANE_TWO.to_string(), LANE_TWO.to_string(), LANE_TWO.to_string()],
        |sha| repeated.read_changed_files(sha),
        is_lane_descendant,
    );
    assert_eq!(
        repeated.read_log(),
        vec![LANE_TWO.to_string()],
        "{HARNESS}: a commit recorded twice is read once"
    );
    assert!(
        matches!(&duplicated, CandidateOwnChanges::Ok { changed_files }
            if changed_files.len() == 1),
        "{HARNESS}: a repeated record must not duplicate a file, got: {duplicated:?}"
    );

    // Whitespace and case are what `normalize` exists for: git prints a sha in lowercase and a journal may
    // carry a trailing newline, and both are the SAME commit. Treating them as two would read one object twice.
    let padded = Repository::with(&[(LANE_ONE, &["tests/tests/forge_scope__002.rs"])]);
    let padded_list = vec![format!("  {LANE_ONE}\n")];
    let after_padding = candidate_own_changed_files(
        Some(LANE_TWO),
        Some(BASE),
        &padded_list,
        |sha| padded.read_changed_files(sha),
        is_lane_descendant,
    );
    assert!(
        matches!(after_padding, CandidateOwnChanges::Ok { .. }),
        "{HARNESS}: a padded record is the same commit, got: {after_padding:?}"
    );
    assert_eq!(
        padded.read_log(),
        vec![LANE_TWO.to_string(), LANE_ONE.to_string()],
        "{HARNESS}: a padded sha is normalised to the canonical commit before it is read"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. THE BASE IS DERIVED FROM THE LANE'S OWN RECORDED COMMITS. `recorded_scope_base` is the reading that
    //    turns a lane's commit journal into the base the next judgement measures against, and it is the
    //    EARLIEST of those commits — the journals are newest-first, so the last valid entry is the base. A
    //    base taken from the wrong end of the list is not a base at all: it would sit on top of the lane's own
    //    work and every change would measure as "nothing".
    // -----------------------------------------------------------------------------------------------------------
    let recorded: Vec<Option<String>> = vec![
        Some(LANE_TWO.to_string()),
        Some(LANE_ONE.to_string()),
    ];
    assert_eq!(
        recorded_scope_base(&recorded).as_deref(),
        Some(LANE_ONE),
        "{HARNESS}: the base is the earliest of the lane's recorded commits, taken from a newest-first list"
    );
    // And a journal carrying nothing usable yields no base at all, rather than a fabricated one — which is
    // what makes SCOPE-001's "no recorded base to measure against" reachable rather than theoretical.
    assert_eq!(
        recorded_scope_base(&[Some("not-a-commit".into()), None]),
        None,
        "{HARNESS}: a journal with no commit in it yields no base"
    );
    assert_eq!(
        recorded_scope_base(&[]),
        None,
        "{HARNESS}: an empty journal yields no base"
    );
}
