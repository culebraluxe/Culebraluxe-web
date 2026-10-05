//! ARCH.ONE_WRITER — `storyboard_story.status` has exactly one owning layer (TST-ARCH-ONE-WRITER-001).
//!
//! CONTRACT. The handbook's one-writer rule is that "one fact has ONE writer; if two ever disagree, that is a
//! REFUSAL (HOLD) naming both, never a resolution that picks a winner". `storyboard_story.status` is the most
//! consequential fact in the control plane — every lane reads a story's status to decide whether it may run — so
//! its writer is pinned here, and the pin is checked against the tree rather than restated.
//!
//! THERE IS ALREADY A FENCE FOR THIS, and this test does not replace it: `cli/src/forge/repo_guards.rs` freezes
//! `TABLE_WRITERS_BASELINE`, the per-table writer set, and `AGENTS.md` names that file as the guard for the
//! one-writer rule. That module is entirely private, so a test cannot call it — and a second hand-copied copy of
//! the writer list would be exactly the duplication this taxonomy is supposed to prevent. So the baseline is read
//! **out of the production source** and the tree is measured against it. One source of truth, and the direction of
//! the check is what makes it worth having: adding a writer turns both fences red, and editing the baseline to
//! match a tree that never grew a writer turns THIS one red, because a baseline that no longer describes the tree
//! is a fence that has stopped fencing.
//!
//! THE OWNING LAYER IS `db`, and that is the narrower, decidable fact. Four `db` modules write the row
//! (`forge_control`, `forge_engine`, `forge_reset`, `tech`) because four genuinely different control-plane verbs
//! write it — batching, claiming, resetting and operator repair. Collapsing them into one function would be a
//! refactor of production code, which a test-authoring story may not do (`forge/src/engine/assay.rs:77`,
//! `PRODUCTION_ROOTS`). What must be unique is the *layer*, so no second crate grows a private door onto the
//! column and becomes the second adjudicator the handbook refuses.
//!
//! THE HONEST STATE OF THE TREE, recorded rather than hidden:
//!
//!   * `UPDATE storyboard_story_run` is NOT a write to `storyboard_story`. The trailing word boundary is
//!     load-bearing, exactly as `repo_guards.rs` documents it ("The `\b` after the table name is load-bearing: it
//!     keeps `storyboard_story` from also answering for `storyboard_story_run`"). An earlier draft of this file
//!     omitted it and the sweep immediately reported `cli/src/forge/repo_guards.rs` as an out-of-layer writer —
//!     the defect was in the detector, not the tree, and fact 4 below is the control that now holds it.
//!   * A source scan decides the writer SET per table, never per COLUMN: two files can name `status` in one and
//!     `completion` in the other and a text scan cannot tell them apart. `docs/agent/COLUMN-WRITER-AUDIT.md` is
//!     where the column-level question is answered by hand.
//!   * `tests/` is excluded on purpose. A test may write the row to arrange a fixture — several do — and a fixture
//!     write is not production ownership. Production is the five tiers. `legacy/` holds no Rust and
//!     `experiments/` is outside the workspace.
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: it reads source text, not behaviour. It
//! does not prove the pinned statements are correct, that they update the row they name, or that the layers above
//! call the DAO rather than opening their own connection — a `Database` handed down out of `db` into `web` is
//! still a `web`-owned write and a text scan cannot see it. It proves the narrower, decidable fact: the SQL that
//! writes the row is written in one crate, and the frozen baseline still describes that crate.
//!
//! Level: L0 Pure — filesystem reads only. No database, no network, no process spawned.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__001__storyboard_story_status

use std::collections::BTreeSet;
use std::path::PathBuf;

use test_harness::source;

/// The two files a fence must not read as writers, because each names the tokens it forbids.
///
/// `repo_guards.rs` is excluded for the same reason the production fence excludes itself
/// (`is_guard_source`, `repo_guards.rs:408`): its `#[cfg(test)]` block carries planted samples like
/// `"insert into storyboard_story(id) values ($1)"` that are assertions about the detector, not writes. A
/// fence that reads its own negative controls reports itself as a violator and fences nothing.
const NOT_A_WRITER: [&str; 2] = [
    "tests/tests/arch_one_writer__001__storyboard_story_status.rs",
    BASELINE_FILE,
];

/// The production tiers. A write outside these is not production at all.
const PRODUCTION_ROOTS: [&str; 5] = ["db", "web", "middle", "forge", "cli"];

/// The fact is the `storyboard_story` row, and the one layer permitted to write it.
const OWNING_LAYER: &str = "db/";

/// Where the frozen baseline lives — the production fence this test measures against.
const BASELINE_FILE: &str = "cli/src/forge/repo_guards.rs";

/// These files are the read-proof that the sweep is not reading the wrong thing: without them a broken path or
/// an empty file would let a vacuous pass through, which is the failure mode `test_harness::source` warns about.
const STATUS_WRITE_PROOF: [(&str, &str); 2] = [
    (
        "db/src/tech.rs",
        "completion=case when $2='Complete' then 100 else completion end",
    ),
    ("db/src/forge_control.rs", "status='Batched'"),
];

/// Whether `text` contains a write against the `storyboard_story` row: `update storyboard_story` or
/// `insert into storyboard_story`, case-insensitively, whitespace-tolerant, and bounded by a word boundary on
/// BOTH sides of the table name.
fn writes_storyboard_story(text: &str) -> bool {
    const TABLE: &str = "storyboard_story";
    let lowered = text.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(found) = lowered[cursor..].find(TABLE) {
        let at = cursor + found;
        let after = at + TABLE.len();
        // The trailing boundary is load-bearing: `storyboard_story_run` is a different table.
        let trailing_ok = lowered[after..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
        // The leading token must be the statement keyword, so a reader (`select … from storyboard_story`) and a
        // helper name (`update_batched_storyboard_story_label`) are both not writes.
        let leading_ok = lowered[..at]
            .trim_end()
            .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .next()
            .is_some_and(|token| token == "update" || token == "into");
        if trailing_ok && leading_ok {
            return true;
        }
        cursor = after;
    }
    false
}

/// Every production source path under the five tiers.
fn production_sources() -> Vec<PathBuf> {
    let root = source::workspace_root();
    let mut all = Vec::new();
    for dir in PRODUCTION_ROOTS {
        let start = root.join(dir);
        if start.is_dir() {
            all.extend(source::sources_under(&start));
        }
    }
    all
}

/// Read the `storyboard_story` writer set out of the frozen baseline in `repo_guards.rs`.
///
/// The baseline entry is the one `"storyboard_story"` literal immediately followed by `&[`, which distinguishes it
/// from the `AUDITED_TABLES` list (whose entries are bare literals with no slice). Returns the quoted `.rs` paths
/// inside that slice.
fn baseline_writers(text: &str) -> Vec<String> {
    let needle = "\"storyboard_story\"";
    let bytes = text.as_bytes();
    let mut start = 0usize;
    while let Some(found) = text[start..].find(needle) {
        let at = start + found;
        let after = at + needle.len();
        // Only the baseline entry is followed by a slice. The tuple is `("storyboard_story", &[...]),` so the
        // comma between the key and the slice has to be stepped over before the slice can be recognised.
        let rest = text[after..].trim_start();
        let rest = rest.strip_prefix(',').unwrap_or(rest).trim_start();
        if let Some(tail) = rest.strip_prefix("&[") {
            let close = tail
                .find(']')
                .expect("the baseline slice is closed in the production source");
            return tail[..close]
                .split('"')
                .skip(1)
                .step_by(2)
                .map(|s| s.trim().to_string())
                .filter(|s| s.ends_with(".rs"))
                .collect();
        }
        start = after;
    }
    panic!("no TABLE_WRITERS_BASELINE entry for storyboard_story in {BASELINE_FILE}");
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ONE-WRITER-001); the file and the assay use it.
fn arch_one_writer__001__storyboard_story_status() {
    let root = source::workspace_root();

    // A walker that silently found nothing would report a clean tree and pass, so the sweep is floored: the five
    // tiers hold hundreds of files. A broken `workspace_root()` fails here instead of flattering the contract.
    let files = production_sources();
    assert!(
        files.len() > 400,
        "the production sweep found only {} files under {PRODUCTION_ROOTS:?} — the scan is broken, not clean",
        files.len()
    );

    // FACT 1 — measure the tree.
    let mut measured: BTreeSet<String> = BTreeSet::new();
    for file in &files {
        let relative = source::relative(file);
        if NOT_A_WRITER.contains(&relative.as_str()) {
            continue; // A fence never counts itself or its own negative controls as writers.
        }
        if writes_storyboard_story(&source::read(file)) {
            measured.insert(relative);
        }
    }

    // FACT 2 — every measured writer sits in the one owning layer.
    for writer in &measured {
        assert!(
            writer.starts_with(OWNING_LAYER),
            "{writer} writes the storyboard_story row outside {OWNING_LAYER}. The one-writer contract for \
             storyboard_story.status admits exactly one owning layer: batching, claiming, resetting and operator \
             repair are different verbs and legitimately different db modules, but a second CRATE holding its own \
             door onto the row is the second adjudicator the handbook refuses. Move the statement into the db DAO, \
             or amend the frozen baseline in {BASELINE_FILE} as a deliberate act."
        );
    }

    // FACT 3 — the frozen baseline still describes the tree, in both directions.
    let baseline_path = root.join(BASELINE_FILE);
    assert!(
        baseline_path.is_file(),
        "{BASELINE_FILE} does not exist; the one-writer fence this test measures against is gone"
    );
    let declared: BTreeSet<String> = baseline_writers(&source::read(&baseline_path))
        .into_iter()
        .collect();
    assert!(
        !declared.is_empty(),
        "the baseline entry for storyboard_story parsed empty, so fact 3 would compare nothing"
    );
    assert_eq!(
        measured, declared,
        "the tree and the frozen baseline in {BASELINE_FILE} disagree about who writes storyboard_story.\n  \
         measured: {measured:?}\n  declared: {declared:?}\nA new writer belongs in the baseline; a baseline entry \
         with no writer behind it is a fence that has stopped fencing."
    );

    // FACT 4 — the pin is not vacuous, and the column in question is really among the writes.
    for (path, needle) in STATUS_WRITE_PROOF {
        let full = root.join(path);
        assert!(full.is_file(), "pinned writer {path} does not exist");
        assert!(
            source::read(&full).contains(needle),
            "{path} no longer contains `{needle}`, so the status-column proof is stale"
        );
    }

    // FACT 5 — the detector is self-checked against planted samples. A detector that cannot fail proves nothing.
    assert!(
        writes_storyboard_story(
            "sqlx::query(\"update storyboard_story set status='Ready' where id=$1\")"
        ),
        "detector missed a same-line write"
    );
    assert!(
        writes_storyboard_story("sqlx::query(\"UPDATE STORYBOARD_STORY SET status='Ready'\")"),
        "detector missed an upper-case write"
    );
    assert!(
        writes_storyboard_story(
            "let sql = \"update\n  storyboard_story\n  set status = 'Ready'\";"
        ),
        "detector missed a multi-line write"
    );
    assert!(
        writes_storyboard_story("sqlx::query(\"insert into storyboard_story(id) values ($1)\")"),
        "detector missed an insert"
    );
    // THE CONTROL THAT CAUGHT THE DEFECT: the trailing boundary. Without it this detector reported
    // `cli/src/forge/repo_guards.rs` as an out-of-layer writer, which is how the omission was found.
    assert!(
        !writes_storyboard_story(
            "let text = \"UPDATE storyboard_story_run SET result_status='x'\";"
        ),
        "detector matched storyboard_story inside storyboard_story_run"
    );
    // A reader is not a writer: most of this suite reads the row, and counting reads would void the contract.
    assert!(
        !writes_storyboard_story("select id, status from storyboard_story order by id"),
        "detector counted a read as a write"
    );
    assert!(
        !writes_storyboard_story("fn update_batched_storyboard_story_label() {}"),
        "detector counted a function name as a write"
    );
    assert!(
        !writes_storyboard_story("insert into storyboard_story_run(id) values ($1)"),
        "detector matched an insert into a different table"
    );
}
