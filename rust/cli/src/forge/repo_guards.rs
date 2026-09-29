//! THE REPO-WIDE GUARDS, in Rust — three rules that are about the whole repository rather than one
//! crate, enforced by tests that read the tree and fail.
//!
//! Ported from the deleted `workflow_app/tests/` guards that `AGENTS.md` names (the paths never
//! existed; `docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md` finding A measured it). The shape is copied
//! from `rust/cli/src/forge/ts_sweep.rs` and the guardrail walk in `vendor_block.rs`: deterministic,
//! sorted output so two runs can be diffed, and a failure when the tree and the claim disagree.
//!
//! Each guard is scoped to the paths it names and lets the rest of the tree be, because a scan that
//! fails on a legitimate file teaches people to bypass it. Each one also FAILS when it reads nothing
//! at all: an empty scan is not a clean scan (Risk 2 in the story brief).
//!
//! The three rules, with the handbook sentence each test carries:
//!   * `AGENTS.md:151` — "Create a worktree, a per-lane tree, or any file-based parallel to the
//!     database workflow. **NO TREES. EVER.**" The estate grew to 83 worktrees under
//!     `Documents/Culebraluxe-worktrees/` plus `.assay-workspaces/`, and on 2026-09-16 a lane
//!     produced verdicts about a tree instead of about the code. The scan freezes the set of files
//!     that can CREATE a worktree; a new file joining it fails.
//!   * `AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer; if two ever
//!     disagree, that is a REFUSAL (HOLD) naming both...". A source-only scan cannot decide whether
//!     two files write the SAME COLUMN, so the decidable thing is pinned instead: the set of files
//!     that write each audited canonical table is frozen, anchored to
//!     `docs/agent/COLUMN-WRITER-AUDIT.md`, and a new file joining the set fails.
//!   * `AGENTS.md:166` — "Treat WhatsApp as a new identity type." The identity registry
//!     (`PersonIdentityKind`) must stay exactly Phone/Email/External, and a WhatsApp number must be
//!     attributed through the existing phone identity, never through a new kind.

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
/// is re-introducing the tree the rule exists to keep at zero.
const TREE_PATH_TOKENS: [&str; 3] = [
    "Culebraluxe-worktrees",
    "DEFAULT_WORKTREES_DIRNAME",
    ".assay-workspaces",
];

/// The files allowed to carry a worktree-CREATION capability, frozen. Growth fails the test: remove
/// the capability, or add the file here WITH the reason it may exist. `rust/forge/src/engine/worktree.rs`
/// is the sole Rust file that invokes `git worktree add`; it is under an active engine audit
/// (`H3` in `docs/agent/HANDOFF-ts-guards-to-rust-2026-09-29.md`), so this guard baselines it rather
/// than editing it.
const WORKTREE_CAPABILITY_FILES: [&str; 1] = ["rust/forge/src/engine/worktree.rs"];

/// The tracked roots the capability scan is allowed to read. Scoped deliberately: `gsd-core/` is an
/// unrelated, untracked vendored tool and `legacy/` is retired, so neither belongs to this rule's
/// estate.
const RESIDUE_ROOTS: [&str; 4] = ["rust", "scripts", ".githooks", "package.json"];

const RESIDUE_EXTS: [&str; 9] = ["rs", "sh", "mjs", "js", "ts", "toml", "json", "yml", "yaml"];

fn worktree_add_phrase() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\bworktree\s+add\b").expect("the worktree-add phrase is a valid pattern")
    })
}

/// `rust/forge/src/engine/worktree.rs` writes the invocation as two array elements — `"worktree",`
/// then `"add",` — so a literal `worktree add` search cannot see the only creator in the tree. This
/// pattern catches the split form.
fn worktree_add_split() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)["']worktree["']\s*,\s*["']add["']"#)
            .expect("the split worktree-add pattern is valid")
    })
}

/// Every token in `text` that names or creates a per-lane worktree. Sorted and de-duplicated, so the
/// same tree prints the same findings on two machines.
fn worktree_tokens_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for token in TREE_PATH_TOKENS {
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
        "storyboard_story",
        &[
            "rust/core/db/src/forge_control.rs",
            "rust/core/db/src/forge_engine.rs",
            "rust/core/db/src/forge_reset.rs",
            "rust/core/db/src/tech.rs",
            "rust/forge/src/engine/neon_sql.rs",
        ],
    ),
    (
        "storyboard_story_run",
        &[
            "rust/core/db/src/forge_control.rs",
            "rust/core/db/src/forge_engine.rs",
        ],
    ),
    (
        "forge_workflow_evidence",
        &["rust/core/db/src/forge_engine.rs"],
    ),
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

const IDENTITY_REGISTRY: &str = "rust/core/domain/src/person.rs";
const WHATSAPP_ATTRIBUTION: &str = "rust/core/db/src/whatsapp.rs";

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
        if !name.is_empty() && name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            kinds.push(name.to_string());
        }
    }
    kinds
}

fn whatsapp_kind_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)"kind"\s*:\s*"whatsapp""#)
            .expect("the whatsapp kind pattern is valid")
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

/// The guard's own source names every token it hunts, so a scan that counted itself would report its
/// own text as a capability. The deleted TypeScript guard excluded itself for the same reason: it
/// measures what production can do.
fn is_guard_source(path: &str) -> bool {
    path == "rust/cli/src/forge/repo_guards.rs"
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
    let mut hits = Vec::new();
    for relative in files {
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
            hits.push((relative, tokens));
        }
    }
    hits.sort();
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
    let files = tracked_files(root, &["rust"]);
    let mut scanned = 0usize;
    let mut writers: BTreeMap<&'static str, BTreeSet<String>> = AUDITED_TABLES
        .iter()
        .map(|table| (*table, BTreeSet::new()))
        .collect();
    for relative in files {
        if is_guard_source(&relative) || is_test_path(&relative) {
            continue;
        }
        // Production Rust only: the audit is about the code paths that serve, not their tests.
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
        assert!(worktree_tokens_in("git worktree add -b x /tmp/t")
            .iter()
            .any(|token| token == "worktree add"));
        assert!(worktree_tokens_in("[\n  \"worktree\",\n  \"add\",\n]")
            .iter()
            .any(|token| token == "\"worktree\",\"add\""));
        assert!(worktree_tokens_in("const DIR: &str = \"Culebraluxe-worktrees\";")
            .contains(&"Culebraluxe-worktrees".to_string()));
        assert!(worktree_tokens_in("let x = 1; // harmless\n").is_empty());
    }

    /// `.guard: AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer; if two
    /// ever disagree, that is a REFUSAL (HOLD) naming both...". Proves the matcher can see a write.
    #[test]
    fn the_writer_matcher_does_not_confuse_the_run_table_with_the_story_table() {
        let text = "UPDATE storyboard_story_run SET result_status='x'";
        assert!(!writes_table(text, "storyboard_story"));
        assert!(writes_table(text, "storyboard_story_run"));
        assert!(writes_table("insert into storyboard_story(id) values ($1)", "storyboard_story"));
    }

    /// `.guard: AGENTS.md:166` — "Treat WhatsApp as a new identity type." Proves the parser reads the
    /// real variant list and would notice a fourth kind.
    #[test]
    fn the_identity_parser_reads_the_variants_and_notices_a_new_one() {
        let known = "pub enum PersonIdentityKind {\n    Phone,\n    Email,\n    External,\n}\n";
        assert_eq!(identity_kinds(known), vec!["Phone", "Email", "External"]);
        let injected =
            "pub enum PersonIdentityKind {\n    Phone,\n    Email,\n    External,\n    WhatsApp,\n}\n";
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
            !kinds.iter().any(|kind| kind.eq_ignore_ascii_case("whatsapp")),
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
}
