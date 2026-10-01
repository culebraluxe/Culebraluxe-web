//! Two handbook "Never" rules, as Rust source scans in `rust/forge` — the crate whose code must obey
//! them. They are the Rust equivalents of the deleted `legacy/workflow_app/tests/forge-qa-no-git.test.ts`
//! and `worker-commit-identity.test.ts`, and `AGENTS.md`'s `guard:` lines point at this file.
//!
//! They are scans, not behaviour tests, on purpose: the defect each guards is a line of code being added
//! back (a git call in the QA lane, a push outside the publish path), and a scan fails the moment it is.
//! Each scan reads code with comments stripped, prints what it matched, and fails when it read nothing —
//! "an empty scan is a failure, not a pass" (`docs/agent/BROKEN-TS-INVENTORY.md`'s discipline).

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("rust/forge sits under rust/ under the repository root")
        .to_path_buf()
}

/// Read CODE, never the prose that NAMES the rule: strip `//` line comments and `/* … */` blocks. The
/// deleted TypeScript guard was comment-aware for the same reason — the modules say "no git, no sha" in
/// their headers, and a naive scan would be red before it guarded anything.
fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_line = false;
    let mut in_block = false;
    while let Some(c) = chars.next() {
        if in_line {
            if c == '\n' {
                in_line = false;
                out.push(c);
            }
            continue;
        }
        if in_block {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            }
            continue;
        }
        if c == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    in_line = true;
                    continue;
                }
                Some('*') => {
                    chars.next();
                    in_block = true;
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// AGENTS.md:160 — "Let git decide anything about work that exists. **PAID CODE > GIT SHA** — the work is
/// the asset, the sha is a label. A git fact may never gate, void or replay work that has been paid for."
///
/// The QA/Assay release path answers only "did the tests pass"; it has no relationship to git. This fails
/// if any QA-named engine module runs a git command or checks lineage.
///
/// The blind spot, named: it does not fail on a sha-shaped identifier. `qa_classify.rs` carries
/// `evaluatedSha` into the failure-classifier directive, which is naming the candidate, not gating on it;
/// a name scan would be red before it guarded anything. The decidable defect is the git CALL. Reach: the
/// scan is by module NAME (`qa_*.rs` plus `assay.rs`), so a new QA module is covered the day it exists.
#[test]
fn the_qa_release_path_runs_no_git_command_and_checks_no_lineage() {
    let dir = repo_root().join("rust/forge/src/engine");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.ends_with(".rs") && (name.starts_with("qa_") || name == "assay.rs")
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "the QA family scan read no module — an empty scan is a failure, not a pass"
    );

    let git_tokens = [
        "Command::new(\"git\")",
        "rev-parse",
        "rev-list",
        "merge-base",
        "is-ancestor",
        "descendant",
    ];
    let mut findings: Vec<String> = Vec::new();
    for file in &files {
        let code = strip_comments(&fs::read_to_string(file).expect("module is readable"));
        for token in git_tokens {
            if code.contains(token) {
                findings.push(format!("{}: {token}", file.display()));
            }
        }
    }
    assert!(
        findings.is_empty(),
        "a QA module reached for git or lineage: {findings:#?}"
    );
}

/// AGENTS.md — "Push `main`, or push any branch that is not your own `agent/*` worker branch."
///
/// The engine has exactly one pusher: the DevOps publish path (`git_publish.rs`), and it pushes only
/// behind `FORGE_ALLOW_PUBLISH=1`. A worker checks in its own `agent/*` branch through git, not through
/// engine source; this fails if any engine source names a `push`, `merge` or `rebase` git subcommand.
/// `merge-base` is a lineage read, not a merge, and is not matched.
#[test]
fn only_the_publish_path_may_push_merge_or_rebase() {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_rs(&repo_root().join("rust/forge/src"), &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "the engine source scan read no file — an empty scan is a failure, not a pass"
    );

    let mut findings: Vec<(String, &str)> = Vec::new();
    for file in &files {
        let code = strip_comments(&fs::read_to_string(file).expect("source is readable"));
        for subcommand in ["\"push\"", "\"merge\"", "\"rebase\""] {
            if code.contains(subcommand) {
                findings.push((file.display().to_string(), subcommand));
            }
        }
    }

    let allowed = |path: &str, subcommand: &str| {
        subcommand == "\"push\"" && path.ends_with("engine/git_publish.rs")
    };
    assert!(
        findings.iter().any(|(_, sub)| *sub == "\"push\""),
        "the live scan must find the publish path's push, or it read nothing: {findings:#?}"
    );
    let violations: Vec<&(String, &str)> = findings
        .iter()
        .filter(|(path, sub)| !allowed(path, sub))
        .collect();
    assert!(
        violations.is_empty(),
        "a worker-reachable path pushes, merges or rebases: {violations:#?}"
    );
}
