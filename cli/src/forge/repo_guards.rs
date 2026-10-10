//! THE REPO-WIDE GUARDS, in Rust — three rules that are about the whole repository rather than one
//! crate, enforced by tests that read the tree and fail.
//!
//! Ported from the deleted `workflow_app/tests/` guards that `AGENTS.md` names (the paths never
//! existed; `docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md` finding A measured it). The shape is copied
//! from `cli/src/forge/ts_sweep.rs` and the guardrail walk in `vendor_block.rs`: deterministic,
//! sorted output so two runs can be diffed, and a failure when the tree and the claim disagree.
//!
//! Each guard is scoped to the paths it names and lets the rest of the tree be, because a scan that
//! fails on a legitimate file teaches people to bypass it. Each one also FAILS when it reads nothing
//! at all: an empty scan is not a clean scan (Risk 2 in the story brief).
//!
//! The rules, with the handbook sentence each test carries:
//!   * `AGENTS.md:151` — "Create a worktree, a per-lane tree, or any file-based parallel to the
//!     database workflow. **NO TREES. EVER.**" The estate grew to 83 worktrees under
//!     `Documents/Culebraluxe-worktrees/` plus `.assay-workspaces/`, and on 2026-09-16 a lane
//!     produced verdicts about a tree instead of about the code. The scan freezes the set of files
//!     that can CREATE a worktree (the `git worktree add` command in either form), and fails any
//!     tracked file that names the estate in its PATH or a tree-era field in its CONTENT; a new file
//!     joining the set fails.
//!   * `AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer; if two ever
//!     disagree, that is a REFUSAL (HOLD) naming both...". A source-only scan cannot decide whether
//!     two files write the SAME COLUMN, so the decidable thing is pinned instead: the set of files
//!     that write each audited canonical table is frozen, anchored to
//!     `docs/agent/COLUMN-WRITER-AUDIT.md`, and a new file joining the set fails.
//!   * `AGENTS.md:166` — "Treat WhatsApp as a new identity type." The identity registry
//!     (`PersonIdentityKind`) must stay exactly Phone/Email/External, and a WhatsApp number must be
//!     attributed through the existing phone identity, never through a new kind.
//!   * `AGENTS.md:172` — the same one-writer sentence, applied to the DISPATCH RULE: "a story that
//!     becomes `Ready` gets exactly one open work item, scored and arbitrated" is written once, by
//!     `agent_work_item_dispatch()` in the database (025:101, restated 146:36). No production Rust file
//!     may insert one; the repair path restores the status change the trigger fires on instead.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;

use super::repo_root;

// ---------------------------------------------------------------------------
// AGENTS.md:151 — NO TREES. EVER.
// ---------------------------------------------------------------------------

/// Directory names that ARE the deleted per-lane worktree estate. A tracked source file naming one
/// is re-introducing the tree the rule exists to keep at zero. Matched in a file's PATH and in its
/// CONTENT: the path half catches a file checked in under the estate, the content half catches a
/// writer that names it.
const TREE_ESTATE_TOKENS: [&str; 2] = ["Culebraluxe-worktrees", ".assay-workspaces"];

/// The tree-era FIELD and CONSTANT names a writer emits when it records a per-lane worktree. These
/// are content-only — a field name is not a directory, so a path never carries one. Ported from the
/// deleted guard's `RECORD_WRITERS` token set (`/worktrees/`, `worktreePath`, `worktree=`), which
/// acceptance #1 names as "a writer that emits it".
const TREE_FIELD_TOKENS: [&str; 4] = [
    "DEFAULT_WORKTREES_DIRNAME",
    "/worktrees/",
    "worktreePath",
    "worktree=",
];

/// The files allowed to carry a worktree-CREATION capability, frozen. Growth fails the test: remove
/// the capability, or add the file here WITH the reason it may exist. `forge/src/engine/worktree.rs`
/// owns low-level worktree commands; `forge/src/engine/executor/drive.rs` is the Batch 2 orchestration
/// caller that provisions per-run isolation worktrees through that module. Both are under the
/// disposable worker-worktree exception, while this guard keeps the capability surface explicit.
///
/// `tests/src/git.rs` is the contract-test suite's disposable-worktree helper. It is
/// allowed because it is the one shape AGENTS.md:151 exempts by name — "Scratch that a command creates
/// and consumes inside itself is fine". `DisposableWorktree` adds a worktree under the system temp
/// directory and removes it in `Drop`, so it never outlives the test that made it, is never read by
/// another lane, and is not a per-lane tree. It adds no workflow the database does not already own.
///
/// `scripts/lane-new.sh` is the lane creator AGENTS.md names in its own words — "To add a lane:
/// `pnpm lane:new <name>` (`scripts/lane-new.sh`: worktree, two env symlinks, `--unset-upstream`,
/// modes)" — with the recipe it embodies in `docs/agent/LAYOUT.md`. It exists under the Captain's
/// exception of 2026-10-03 ("the agent lanes are the one standing set of trees"), which supersedes
/// "NO TREES, EVER" for exactly one tree per lane and nothing else: the script creates
/// `src/lane-<name>` and nothing more, a lane adds no second tree of its own, no lane reads or builds
/// in another lane's tree, and Neon stays the only workflow and control-plane authority. Naming it
/// here is the maintenance path this guard itself prescribes — the capability is sanctioned, so it is
/// recorded with its reason rather than removed.
/// `forge/src/pianola/worker_lanes.rs` is the TST worker-lane provisioning wrapper (commit
/// `93f46e74`). It resolves `<parent-of-repo>/Culebraluxe-worktrees` and creates one disposable
/// worktree per story — the shape the Captain exempted on 2026-10-01: "parallel Forge execution may
/// use one disposable Git worktree per story solely as an isolation sandbox. Neon remains the only
/// workflow/control-plane authority; no lane may read another story's worktree; the worktree is
/// removed when the child run ends" — and the module's own header cites that same exception. It is
/// recorded here by the Captain's call (`bless`, 2026-10-04) rather than removed, which is the
/// maintenance path this guard prescribes. On that date it had no live caller: `provision_tst_lane`
/// is referenced by nothing outside its own tests and the module is reachable only through
/// `pub mod worker_lanes;`. The baseline makes the capability visible instead of silent; retire the
/// module when a story owns it.
///
/// `scripts/lane-cargo-config.sh` creates no worktree. It matches this scan because its prose names
/// `git worktree add` while explaining which kind of checkout gets which target directory
/// (`docs/agent/LAYOUT.md`, "One target directory per checkout"): a lane gets
/// `/Users/Shared/dev/build/rust-lane-<name>`, a sandbox builds inside its own tree, the main checkout
/// is refused. That prose is load-bearing — its heredoc delimiter is quoted because an unquoted body
/// *executed* what it described, so on 2026-10-08 writing a sandbox's config ran `git worktree add`,
/// `pnpm ui:build` and `cargo check` on the way out. The scan is textual, so the file is recorded here
/// with its reason (`bless`, 2026-10-08) rather than reworded to hide the phrase.
///
/// `scripts/verify-checkout-build-dir.sh` does create worktrees: three detached probes under
/// `${TMPDIR:-/tmp}`, timed, asserted against, and removed in its own `cleanup` trap. That is the shape
/// AGENTS.md:151 exempts by name — "Scratch that a command creates and consumes inside itself is fine;
/// a directory that outlives the command, or that another lane reads, is a tree" — so each probe lives
/// and dies inside one command, and a probe that only printed would not be a receipt. Recorded here by
/// the Captain's call (`bless`, 2026-10-08) rather than removed: deleting the probes would delete the
/// regression guard for the 2026-10-08 checkout bug they were written to catch.
const WORKTREE_CAPABILITY_FILES: [&str; 7] = [
    "forge/src/engine/executor/drive.rs",
    "forge/src/engine/worktree.rs",
    "forge/src/pianola/worker_lanes.rs",
    "scripts/lane-cargo-config.sh",
    "scripts/lane-new.sh",
    "scripts/verify-checkout-build-dir.sh",
    "tests/src/git.rs",
];

/// The tracked roots the capability scan is allowed to read: the workspace's crate roots, which are the tiers
/// (`web/`, `middle/`, `db/`) and the entry points (`cli/`, `forge/`), plus `tests/`, where the contract suite
/// lives. Scoped deliberately: `gsd-core/` is an unrelated vendored tool and `experiments/` is outside
/// the workspace, so neither belongs to this rule's estate.
const RESIDUE_ROOTS: [&str; 9] = [
    "web",
    "middle",
    "db",
    "cli",
    "forge",
    "tests",
    "scripts",
    ".githooks",
    "package.json",
];

const RESIDUE_EXTS: [&str; 9] = ["rs", "sh", "mjs", "js", "ts", "toml", "json", "yml", "yaml"];

fn worktree_add_phrase() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\bworktree\s+add\b").expect("the worktree-add phrase is a valid pattern")
    })
}

/// `forge/src/engine/worktree.rs` writes the invocation as two array elements — `"worktree",`
/// then `"add",` — so a literal `worktree add` search cannot see the only creator in the tree. This
/// pattern catches the split form.
fn worktree_add_split() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)["']worktree["']\s*,\s*["']add["']"#)
            .expect("the split worktree-add pattern is valid")
    })
}

/// Every estate-name token in a tracked file's own PATH. Paths are not content: a file checked in
/// UNDER `Culebraluxe-worktrees/` or `.assay-workspaces/` re-creates the very tree the rule keeps at
/// zero, even when its bytes say nothing at all. Case-insensitive, because the same directory can be
/// spelled differently on a case-folding filesystem and it is still the same tree.
fn worktree_tokens_in_path(path: &str) -> Vec<String> {
    let lowered = path.to_ascii_lowercase();
    let mut found: Vec<String> = TREE_ESTATE_TOKENS
        .iter()
        .filter(|token| lowered.contains(&token.to_ascii_lowercase()))
        .map(|token| token.to_string())
        .collect();
    found.sort();
    found.dedup();
    found
}

/// Every token in `text` that names or creates a per-lane worktree. Sorted and de-duplicated, so the
/// same tree prints the same findings on two machines.
fn worktree_tokens_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for token in TREE_ESTATE_TOKENS.iter().chain(TREE_FIELD_TOKENS.iter()) {
        if text.contains(token) {
            found.push(token.to_string());
        }
    }
    if worktree_add_phrase().is_match(text) {
        found.push("worktree add".to_string());
    }
    if worktree_add_split().is_match(text) {
        found.push("\"worktree\",\"add\"".to_string());
    }
    found.sort();
    found.dedup();
    found
}

// ---------------------------------------------------------------------------
// AGENTS.md:172 — one fact has ONE writer.
// ---------------------------------------------------------------------------

/// The canonical tables the deleted `column-writer-audit.test.ts` audited.
const AUDITED_TABLES: [&str; 3] = [
    "storyboard_story",
    "storyboard_story_run",
    "forge_workflow_evidence",
];

/// The Rust source files that write each audited table, frozen. This is the pinned decidable fact:
/// a source-only scan cannot decide whether two files write the same COLUMN, but it can decide the
/// writer SET per table, and a new file joining that set fails here. Removal also fails, so the fence
/// cannot be quietly widened in the other direction.
const TABLE_WRITERS_BASELINE: [(&str, &[&str]); 3] = [
    (
        // `forge/src/engine/neon_sql.rs` left this set on 2026-09-29: it held a second, never
        // executed COPY of the repair/replan increments while `forge_engine.rs` held the ones that ran.
        // Removing it makes this fence report the writer that serves. Do not re-add a file that only
        // carries a dead copy of a statement it does not execute.
        "storyboard_story",
        &[
            "db/src/forge_control.rs",
            "db/src/forge_engine.rs",
            "db/src/forge_reset.rs",
            "db/src/tech.rs",
        ],
    ),
    (
        "storyboard_story_run",
        // The learning DAO removed its unused story-run writer during Batch 4. Model-attempt
        // settlement now has a dedicated submodule in the same Forge Engine DAO; it is an
        // intentional second source file because it owns the atomic, idempotent usage receipt
        // transaction. Keep this explicit so future writers still require a reviewed baseline.
        &[
            "db/src/forge_engine.rs",
            "db/src/forge_engine/model_attempt_budget.rs",
        ],
    ),
    ("forge_workflow_evidence", &["db/src/forge_engine.rs"]),
];

/// The canonical audit the table list is anchored to. If the doc's audited tables change, the
/// baselines above must follow.
const COLUMN_AUDIT_DOC: &str = "docs/agent/COLUMN-WRITER-AUDIT.md";

fn update_re(table: &str) -> Regex {
    Regex::new(&format!(r"(?i)\bupdate\s+{table}\b")).expect("the update pattern is valid")
}

fn insert_re(table: &str) -> Regex {
    Regex::new(&format!(r"(?i)\binsert\s+into\s+{table}\b")).expect("the insert pattern is valid")
}

/// Whether `text` writes `table`. The `\b` after the table name is load-bearing: it keeps
/// `storyboard_story` from also answering for `storyboard_story_run`.
fn writes_table(text: &str, table: &str) -> bool {
    update_re(table).is_match(text) || insert_re(table).is_match(text)
}

/// The audited table names as they appear as `## <table> (n)` sections in the audit doc.
fn audit_tables_in(doc: &str) -> Vec<String> {
    doc.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("## ")?;
            let name = rest.split(" (").next().unwrap_or("").trim();
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// AGENTS.md:166 — WhatsApp is not a new identity type.
// ---------------------------------------------------------------------------

/// The exact identity vocabulary. The enum staying this set is the rule; a fourth variant named
/// `WhatsApp` is the failure.
const IDENTITY_KINDS: [&str; 3] = ["Phone", "Email", "External"];

const IDENTITY_REGISTRY: &str = "middle/model/src/person.rs";
const WHATSAPP_ATTRIBUTION: &str = "db/src/whatsapp.rs";

/// The variants of `pub enum PersonIdentityKind`, in source order. Empty when the enum is gone, which
/// the test treats as a failure rather than a clean scan.
fn identity_kinds(source: &str) -> Vec<String> {
    let Some(at) = source.find("pub enum PersonIdentityKind") else {
        return Vec::new();
    };
    let Some(open) = source[at..].find('{').map(|offset| at + offset) else {
        return Vec::new();
    };
    let Some(close) = source[open..].find('}').map(|offset| open + offset) else {
        return Vec::new();
    };
    let body = &source[open + 1..close];
    let mut kinds = Vec::new();
    for segment in body.split(',') {
        let line = segment.lines().last().unwrap_or("").trim();
        let name = line.split_whitespace().next().unwrap_or("");
        if !name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            kinds.push(name.to_string());
        }
    }
    kinds
}

fn whatsapp_kind_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)"kind"\s*:\s*"whatsapp""#).expect("the whatsapp kind pattern is valid")
    })
}

/// True when the WhatsApp path attributes its participant through the existing phone identity:
/// `"kind": "phone"` in the participant/contact candidates AND `identity_type = 'phone'` in the
/// resolution query.
fn phone_attribution_present(text: &str) -> bool {
    static KIND: OnceLock<Regex> = OnceLock::new();
    static COLUMN: OnceLock<Regex> = OnceLock::new();
    let kind = KIND.get_or_init(|| {
        Regex::new(r#"(?i)"kind"\s*:\s*"phone""#).expect("the phone kind pattern is valid")
    });
    let column = COLUMN.get_or_init(|| {
        Regex::new(r"(?is)identity_type\s*=\s*'phone'").expect("the phone column pattern is valid")
    });
    kind.is_match(text) && column.is_match(text)
}

// ---------------------------------------------------------------------------
// AGENTS.md:172 — one fact has ONE writer, applied to the DISPATCH RULE.
// ---------------------------------------------------------------------------

/// The table whose INSERT belongs to the database and not to the port.
///
/// One rule owns the queue: *a story that BECOMES `Ready` gets exactly one open work item, scored by
/// `story_priority_score()` and arbitrated by the partial unique index*. It is written once, in
/// `agent_work_item_dispatch()` (`db/migrations/025_agent_work_queue.sql:101`, restated in
/// `db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql:36`), and it is the whole reason
/// `258_reopen_stranded_ready_work_items.sql` was needed at all — the rule has a single owner and a row
/// the owner did not create is a row that dispatches to nothing.
///
/// The port spelled the rule a second time in Rust: the same insert, the same score call, the same
/// arbiter predicate typed out again (`db/src/forge_engine.rs` until 2026-09-29). A second
/// spelling of an owned rule drifts — migration 146 exists because the arbiter was not restated when
/// 143 replaced the index underneath it — so the decidable fact is frozen here: **no production Rust
/// file inserts a work item.** Putting a `Ready` story back in the queue is done by restoring the
/// status CHANGE the trigger fires on, never by writing the row.
const DISPATCH_OWNED_TABLE: &str = "agent_work_item";

struct DispatchWriteScan {
    scanned: usize,
    writers: Vec<String>,
}

fn dispatch_write_scan(root: &Path) -> DispatchWriteScan {
    let pattern = insert_re(DISPATCH_OWNED_TABLE);
    let mut scanned = 0usize;
    let mut writers = Vec::new();
    for relative in tracked_files(root, &["web", "middle", "db", "cli", "forge", "tests"]) {
        if is_guard_source(&relative) || is_test_path(&relative) {
            continue;
        }
        // Production Rust only: a fixture in `tests/` may build a shape the database would never
        // create — that is what a test is for — but a code path that serves may not.
        if !relative.contains("/src/") || extension(&relative) != Some("rs") {
            continue;
        }
        let Some(text) = read(root, &relative) else {
            continue;
        };
        scanned += 1;
        if pattern.is_match(&text) {
            writers.push(relative);
        }
    }
    writers.sort();
    DispatchWriteScan { scanned, writers }
}

// ---------------------------------------------------------------------------
// Shared file access — tracked files only, so vendored/untracked trees (gsd-core, node_modules,
// target) are never in scope.
// ---------------------------------------------------------------------------

fn tracked_files(root: &Path, roots: &[&str]) -> Vec<String> {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .arg("ls-files")
        .arg("-z")
        .arg("--")
        .args(roots);
    let Ok(output) = command.output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    files.sort();
    files
}

fn extension(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|value| value.to_str())
}

fn is_test_path(path: &str) -> bool {
    path.contains("/tests/") || path.ends_with("_test.rs") || path.contains(".test.")
}

/// A crate whose every module exists to run a test, so it is not production code even where it lives
/// under `src/`.
///
/// `tests/` is the contract suite: its `tests/` hold the cases and its `src/` holds the
/// fixture client (`Database`, `ForgeHarness`) that writes and cleans up canonical rows on purpose —
/// asking the rows instead of a DAO is the whole method, so writing them is the harness's job, not a
/// second owner of a fact. `tests/src/git.rs` is already named in
/// `WORKTREE_CAPABILITY_FILES` for exactly this reason (a harness may create a disposable worktree).
///
/// The production-writer scan lacked the same line, so on 2026-09-30 `tests/src/forge.rs`
/// joined the `storyboard_story` writer set through its fixture `delete from storyboard_story` and turned
/// the one-writer guard red — for a crate that is linked into no binary and serves nobody.
fn is_test_support_crate(path: &str) -> bool {
    path.starts_with("tests/")
}

/// The guard's own source names every token it hunts, so a scan that counted itself would report its
/// own text as a capability. The deleted TypeScript guard excluded itself for the same reason: it
/// measures what production can do.
fn is_guard_source(path: &str) -> bool {
    path == "cli/src/forge/repo_guards.rs"
}

fn read(root: &Path, relative: &str) -> Option<String> {
    fs::read_to_string(root.join(relative)).ok()
}

struct ResidueScan {
    scanned: usize,
    hits: Vec<(String, Vec<String>)>,
}

fn residue_scan(root: &Path) -> ResidueScan {
    let files = tracked_files(root, &RESIDUE_ROOTS);
    let mut scanned = 0usize;
    let mut hits: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for relative in files {
        // A path hit counts for every tracked file, whatever its extension: a file living UNDER a
        // worktree-named directory is the residue itself, independent of what it contains.
        let path_tokens = worktree_tokens_in_path(&relative);
        if !path_tokens.is_empty() {
            hits.entry(relative.clone())
                .or_default()
                .extend(path_tokens);
        }
        if is_guard_source(&relative) || is_test_path(&relative) {
            continue;
        }
        let Some(ext) = extension(&relative) else {
            continue;
        };
        if !RESIDUE_EXTS.contains(&ext) {
            continue;
        }
        let Some(text) = read(root, &relative) else {
            continue;
        };
        scanned += 1;
        let tokens = worktree_tokens_in(&text);
        if !tokens.is_empty() {
            hits.entry(relative).or_default().extend(tokens);
        }
    }
    let hits = hits
        .into_iter()
        .map(|(file, tokens)| (file, tokens.into_iter().collect()))
        .collect();
    ResidueScan { scanned, hits }
}

struct WriterScan {
    scanned: usize,
    writers: BTreeMap<&'static str, BTreeSet<String>>,
}

fn writer_scan(root: &Path) -> WriterScan {
    let patterns: Vec<(&'static str, Regex, Regex)> = AUDITED_TABLES
        .iter()
        .map(|table| (*table, update_re(table), insert_re(table)))
        .collect();
    let files = tracked_files(root, &["web", "middle", "db", "cli", "forge", "tests"]);
    let mut scanned = 0usize;
    let mut writers: BTreeMap<&'static str, BTreeSet<String>> = AUDITED_TABLES
        .iter()
        .map(|table| (*table, BTreeSet::new()))
        .collect();
    for relative in files {
        if is_guard_source(&relative) || is_test_path(&relative) {
            continue;
        }
        // Production Rust only: the audit is about the code paths that serve, not their tests — and not
        // their test harness either, whose `src/` is a fixture client (see `is_test_support_crate`).
        if is_test_support_crate(&relative) {
            continue;
        }
        if !relative.contains("/src/") || extension(&relative) != Some("rs") {
            continue;
        }
        let Some(text) = read(root, &relative) else {
            continue;
        };
        scanned += 1;
        for (table, update, insert) in &patterns {
            if update.is_match(&text) || insert.is_match(&text) {
                writers
                    .get_mut(table)
                    .expect("every audited table is inserted above")
                    .insert(relative.clone());
            }
        }
    }
    WriterScan { scanned, writers }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- matcher unit tests: prove each scan can actually see a violation -------------------------

    /// `.guard: AGENTS.md:151` — "Create a worktree, a per-lane tree, or any file-based parallel to
    /// the database workflow. **NO TREES. EVER.**" Proves the matcher can actually see a violation.
    #[test]
    fn the_worktree_matcher_sees_the_phrase_the_split_invocation_and_the_estate_name() {
        let phrase = worktree_tokens_in("git worktree add -b x /tmp/t");
        println!("worktree matcher (phrase): {phrase:?}");
        assert!(phrase.iter().any(|token| token == "worktree add"));
        let split = worktree_tokens_in("[\n  \"worktree\",\n  \"add\",\n]");
        println!("worktree matcher (split): {split:?}");
        assert!(split.iter().any(|token| token == "\"worktree\",\"add\""));
        let estate = worktree_tokens_in("const DIR: &str = \"Culebraluxe-worktrees\";");
        println!("worktree matcher (estate): {estate:?}");
        assert!(estate.contains(&"Culebraluxe-worktrees".to_string()));
        let clean = worktree_tokens_in("let x = 1; // harmless\n");
        println!("worktree matcher (clean): {clean:?}");
        assert!(clean.is_empty());
    }

    /// `.guard: AGENTS.md:151` — "Create a worktree, a per-lane tree, or any file-based parallel to
    /// the database workflow. **NO TREES. EVER.**" Proves a WRITER that emits a tree-era field or the
    /// `.../worktrees/` estate path is caught in a file's bytes, not only a creator invoking the
    /// command. Ported from the deleted guard's `RECORD_WRITERS` token set.
    #[test]
    fn the_worktree_matcher_sees_the_tree_era_field_tokens_a_writer_emits() {
        for token in TREE_FIELD_TOKENS {
            let sample = match token {
                "/worktrees/" => "let dir = format!(\"{}\", \"/worktrees/eng-qa-01\");",
                "worktreePath" => "struct RunDetail { worktreePath: String }",
                "worktree=" => "let note = format!(\"worktree={cwd}\");",
                _ => "let dir = DEFAULT_WORKTREES_DIRNAME;",
            };
            let found = worktree_tokens_in(sample);
            println!("worktree matcher (field {token:?}): {found:?}");
            assert!(
                found.contains(&token.to_string()),
                "the matcher did not see {token:?} in {sample:?}"
            );
        }
    }

    /// `.guard: AGENTS.md:151` — "Create a worktree, a per-lane tree, or any file-based parallel to
    /// the database workflow. **NO TREES. EVER.**" Proves the scan sees the estate name in a PATH, not
    /// only in a file's bytes.
    #[test]
    fn the_worktree_matcher_sees_the_estate_name_in_a_tracked_path() {
        let assay = worktree_tokens_in_path("rust/.assay-workspaces/case.rs");
        println!("worktree path matcher (assay): {assay:?}");
        assert!(assay.contains(&".assay-workspaces".to_string()));
        let legacy = worktree_tokens_in_path("scripts/Culebraluxe-worktrees/run.sh");
        println!("worktree path matcher (legacy): {legacy:?}");
        assert!(legacy.contains(&"Culebraluxe-worktrees".to_string()));
        // The sole legitimate creator lives in a file NAMED worktree.rs; the bare word is not the
        // estate, so its path must not trip the guard.
        let creator = worktree_tokens_in_path("forge/src/engine/worktree.rs");
        println!("worktree path matcher (creator): {creator:?}");
        assert!(creator.is_empty());
    }

    /// `.guard: AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer; if two
    /// ever disagree, that is a REFUSAL (HOLD) naming both...". Proves the matcher can see a write.
    #[test]
    fn the_writer_matcher_does_not_confuse_the_run_table_with_the_story_table() {
        let text = "UPDATE storyboard_story_run SET result_status='x'";
        println!(
            "writer matcher: run={} story={}",
            writes_table(text, "storyboard_story_run"),
            writes_table(text, "storyboard_story")
        );
        assert!(!writes_table(text, "storyboard_story"));
        assert!(writes_table(text, "storyboard_story_run"));
        assert!(writes_table(
            "insert into storyboard_story(id) values ($1)",
            "storyboard_story"
        ));
    }

    /// `.guard: AGENTS.md:166` — "Treat WhatsApp as a new identity type." Proves the parser reads the
    /// real variant list and would notice a fourth kind.
    #[test]
    fn the_identity_parser_reads_the_variants_and_notices_a_new_one() {
        let known = "pub enum PersonIdentityKind {\n    Phone,\n    Email,\n    External,\n}\n";
        println!("identity parser (known): {:?}", identity_kinds(known));
        assert_eq!(identity_kinds(known), vec!["Phone", "Email", "External"]);
        let injected =
            "pub enum PersonIdentityKind {\n    Phone,\n    Email,\n    External,\n    WhatsApp,\n}\n";
        println!("identity parser (injected): {:?}", identity_kinds(injected));
        assert!(identity_kinds(injected)
            .iter()
            .any(|kind| kind.eq_ignore_ascii_case("whatsapp")));
    }

    // -- the guards against the real tree ---------------------------------------------------------

    /// `.guard: AGENTS.md:151` — "Create a worktree, a per-lane tree, or any file-based parallel to
    /// the database workflow. **NO TREES. EVER.**" The capability set is frozen; growth (or a removal
    /// that leaves the fence stale) fails.
    #[test]
    fn no_tree_residue_the_worktree_capability_set_is_frozen() {
        let root = repo_root();
        let scan = residue_scan(&root);

        println!(
            "no-tree-residue: read {} tracked source file(s)",
            scan.scanned
        );
        for (file, tokens) in &scan.hits {
            println!("  matched {file} <- {}", tokens.join(", "));
        }

        assert!(
            scan.scanned > 0,
            "the scan read no files at all — an empty scan is a failure, not a pass"
        );
        let matched: Vec<&str> = scan.hits.iter().map(|(file, _)| file.as_str()).collect();
        assert_eq!(
            matched, WORKTREE_CAPABILITY_FILES,
            "the set of files that can create a per-lane worktree changed. NO TREES, EVER: remove the \
             capability, or add the file to WORKTREE_CAPABILITY_FILES in this test WITH the reason it \
             may exist. Silence is not an option."
        );
    }

    /// `.guard: AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer..."
    /// A source-only scan cannot decide same-column; the decidable fact — the writer set per audited
    /// canonical table — is frozen here.
    #[test]
    fn one_writer_per_column_the_writer_set_per_canonical_table_is_frozen() {
        let root = repo_root();
        let scan = writer_scan(&root);

        println!(
            "one-writer-per-column: read {} tracked production Rust file(s)",
            scan.scanned
        );
        for (table, files) in &scan.writers {
            let files: Vec<&str> = files.iter().map(String::as_str).collect();
            println!("  writes {table} <- {}", files.join(", "));
        }

        assert!(
            scan.scanned > 0,
            "the scan read no files at all — an empty scan is a failure, not a pass"
        );

        let audit = read(&root, COLUMN_AUDIT_DOC).unwrap_or_else(|| {
            panic!("{COLUMN_AUDIT_DOC} is missing — the audit this test anchors to cannot be read")
        });
        assert_eq!(
            audit_tables_in(&audit),
            AUDITED_TABLES,
            "the audited table list in {COLUMN_AUDIT_DOC} changed; this test's baselines must follow it"
        );
        assert!(
            audit.lines().filter(|line| line.contains("| `")).count() > 0,
            "{COLUMN_AUDIT_DOC} carries no classified column rows — the audit went quiet"
        );

        for (table, expected) in TABLE_WRITERS_BASELINE {
            let found = scan
                .writers
                .get(table)
                .unwrap_or_else(|| panic!("{table} is not scanned"));
            let found: Vec<&str> = found.iter().map(String::as_str).collect();
            let expected: Vec<&str> = expected.to_vec();
            assert_eq!(
                found, expected,
                "the set of files writing {table} changed. One fact has ONE writer: name the writer \
                 here deliberately, or do not join the set."
            );
        }

        // The strongest single-writer fact a source scan can decide: forge_workflow_evidence has one
        // writer, and it stays one.
        let evidence = scan
            .writers
            .get("forge_workflow_evidence")
            .expect("forge_workflow_evidence is scanned");
        assert_eq!(
            evidence.len(),
            1,
            "forge_workflow_evidence must keep exactly one writer"
        );
    }

    /// `.guard: AGENTS.md:166` — "Treat WhatsApp as a new identity type." The registry stays exactly
    /// Phone/Email/External, and a WhatsApp number is attributed through the existing phone identity.
    #[test]
    fn whatsapp_is_not_a_new_identity_kind() {
        let root = repo_root();

        let registry = read(&root, IDENTITY_REGISTRY).unwrap_or_else(|| {
            panic!("{IDENTITY_REGISTRY} is missing — the identity registry cannot be read")
        });
        let kinds = identity_kinds(&registry);
        println!("whatsapp-attribution: PersonIdentityKind = {kinds:?}");

        assert_eq!(
            kinds, IDENTITY_KINDS,
            "the identity vocabulary changed. WhatsApp is NOT a new identity type: attribute it \
             through the existing phone identity (PersonIdentityKind::Phone), never a fourth kind."
        );
        assert!(
            !kinds
                .iter()
                .any(|kind| kind.eq_ignore_ascii_case("whatsapp")),
            "WhatsApp was introduced as an identity kind"
        );

        let attribution = read(&root, WHATSAPP_ATTRIBUTION).unwrap_or_else(|| {
            panic!("{WHATSAPP_ATTRIBUTION} is missing — the attribution path cannot be read")
        });
        assert!(
            !attribution.is_empty(),
            "{WHATSAPP_ATTRIBUTION} is empty — an empty scan is a failure, not a pass"
        );
        println!(
            "whatsapp-attribution: phone identity present = {}",
            phone_attribution_present(&attribution)
        );
        assert!(
            phone_attribution_present(&attribution),
            "the WhatsApp path no longer attributes through the existing phone identity \
             (\"kind\": \"phone\" and identity_type = 'phone')"
        );
        assert!(
            !whatsapp_kind_re().is_match(&attribution),
            "the WhatsApp path introduces a new \"kind\": \"whatsapp\" identity instead of the \
             existing phone identity"
        );
    }

    /// `.guard: AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer..." applied
    /// to the dispatch rule: proves the matcher can see the second spelling it exists to forbid, and
    /// does not confuse an item STATE change (which the engine legitimately owns) with a creation.
    #[test]
    fn the_dispatch_matcher_sees_a_work_item_insert_and_only_an_insert() {
        let second_owner =
            "insert into agent_work_item (story_id, state, priority) values ($1, 'Ready', $2)";
        println!(
            "dispatch matcher (second owner): {}",
            insert_re(DISPATCH_OWNED_TABLE).is_match(second_owner)
        );
        assert!(insert_re(DISPATCH_OWNED_TABLE).is_match(second_owner));

        let claim_recovery = "update agent_work_item set state='Ready', claimed_by=null, \
                              error_text='stale claim recovered; awaiting fresh attempt' \
                              where id=$1::uuid and state in ('Claimed','Running')";
        println!(
            "dispatch matcher (claim recovery): {}",
            insert_re(DISPATCH_OWNED_TABLE).is_match(claim_recovery)
        );
        assert!(
            !insert_re(DISPATCH_OWNED_TABLE).is_match(claim_recovery),
            "the engine owns the state of a claim it holds; this guard only forbids CREATING an item"
        );
    }

    /// `.guard: AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer..."
    /// The dispatch rule has exactly one writer and it is the database. Growth fails.
    #[test]
    fn the_database_owns_dispatch_no_production_rust_file_inserts_a_work_item() {
        let root = repo_root();
        let scan = dispatch_write_scan(&root);

        println!(
            "dispatch-owner: read {} tracked production Rust file(s)",
            scan.scanned
        );
        for file in &scan.writers {
            println!("  inserts {DISPATCH_OWNED_TABLE} <- {file}");
        }

        assert!(
            scan.scanned > 0,
            "the scan read no files at all — an empty scan is a failure, not a pass"
        );
        assert!(
            scan.writers.is_empty(),
            "the dispatch rule has ONE writer and it is the database. \
             `agent_work_item_dispatch()` creates the item, scores it with `story_priority_score()` and \
             arbitrates it with the partial unique index; a Rust file that inserts here is a second owner \
             of a rule that has already drifted once (migration 146). To put a `Ready` story back in the \
             queue, restore the status CHANGE the trigger fires on — off `Ready` and back, inside one \
             transaction — instead of writing the row. Found: {}. If you are looking at a legitimate \
             exception, that is a decision for the captain, not for this test.",
            scan.writers.join(", ")
        );
    }
}
