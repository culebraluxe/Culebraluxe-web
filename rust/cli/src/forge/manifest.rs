//! `forge manifest` — "here are the exact files to read for this scope", as a GENERATED file the harness
//! compares.
//!
//! Rust replacement for `scripts/forge-manifest.ts`, which imported the deleted `lib/scope-manifest.ts`,
//! `lib/git/sync-conflict.ts` and `lib/artifact-file.ts`, so `pnpm forge:manifest` and
//! `pnpm forge:manifest:check` both exited `ERR_MODULE_NOT_FOUND`. That mattered more than one dead command:
//! `pnpm forge:harness` runs `forge:sync-agents --check && forge:manifest --check-all && forge:packet-lint &&
//! test:harness`, so the whole harness chain was dead at its second link.
//!
//! The split is the port's: the RULES (lanes, scoring, rendering, drift, the write refusal) are pure and live
//! in `forge::scope_manifest`; this file gathers from the tree and git, and prints. No rule is decided here.
//!
//! Usage:
//!   cargo run -p cli -- forge manifest <scope> [--check] [--format json] [--lexical <n>]
//!   cargo run -p cli -- forge manifest --check-all
//!
//! Exit codes: 0 fresh (or a `--check-all` self-heal), 1 drift or an unresolvable row, 2 usage.

use super::citations::{cited_repo_paths, Resolution, Resolver};
use super::Failure;
use forge::scope_manifest::{
    declares_new, lexical_drift_count, manifest_drift, manifest_file_name, missing_paths,
    non_manifest_refusal, only_the_clock_moved, rank_entries, render, CorpusFile, Entry, Lane,
    Lanes, Meta, RankInput,
};
use forge::sync_conflict::is_conflict_copy_name;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

const MANIFEST_DIR: &str = "docs/agent/manifest";
/// One `git log` pass, deep enough to cover a story's whole life and cheap enough to run every time.
const HISTORY_DEPTH: usize = 400;

pub struct Options {
    pub check: bool,
    pub check_all: bool,
    pub json: bool,
    pub lexical_limit: Option<usize>,
    pub scope: String,
}

/// Pure, so the flag surface is asserted without touching the tree. `--check-all` and a scope are the two
/// whole-directory and one-file modes; with neither, the scope is the whole harness (`all`).
pub fn parse_args(argv: &[String]) -> Options {
    let mut options = Options {
        check: false,
        check_all: false,
        json: false,
        lexical_limit: None,
        scope: String::new(),
    };
    let mut index = 0;
    while index < argv.len() {
        let argument = argv[index].as_str();
        match argument {
            "--check" => options.check = true,
            "--check-all" => options.check_all = true,
            "--format" => {
                options.json = argv.get(index + 1).map(String::as_str) == Some("json");
                index += 1;
            }
            "--lexical" => {
                options.lexical_limit = argv.get(index + 1).and_then(|value| value.parse().ok());
                index += 1;
            }
            other if !other.starts_with("--") => options.scope = other.to_string(),
            _ => {}
        }
        index += 1;
    }
    if options.scope.is_empty() && !options.check_all {
        options.scope = "all".to_string();
    }
    options
}

/// `git ...` with the failure swallowed to an empty string, as the TypeScript did: a tree without git (or a
/// shallow clone) degrades to "no history" rather than refusing to render a manifest.
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git").args(args).current_dir(root).output();
    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout).to_string(),
        _ => String::new(),
    }
}

/// The three questions one `git log` pass answers: when each path was last touched, which files the scope's own
/// commits changed, and how many such commits there were. A per-path `git log -1` would be ~130 subprocesses
/// for the same answer.
struct History {
    last_touched: HashMap<String, String>,
    scope_paths: Vec<String>,
    scope_commits: usize,
}

fn read_history(root: &Path, scope: &str) -> History {
    let raw = git(
        root,
        &[
            "log",
            "--pretty=format:__C__%cs\x1f%s",
            "--name-only",
            "-n",
            &HISTORY_DEPTH.to_string(),
        ],
    );
    let mut history = History {
        last_touched: HashMap::new(),
        scope_paths: Vec::new(),
        scope_commits: 0,
    };
    let mut date = String::new();
    let mut matches_scope = false;
    let upper_scope = scope.to_uppercase();
    for line in raw.split('\n') {
        if let Some(rest) = line.strip_prefix("__C__") {
            let mut parts = rest.split('\u{1f}');
            date = parts.next().unwrap_or("").trim().to_string();
            let subject = parts.next().unwrap_or("").to_uppercase();
            matches_scope = !upper_scope.is_empty() && subject.contains(&upper_scope);
            if matches_scope {
                history.scope_commits += 1;
            }
            continue;
        }
        let path = line.trim();
        if path.is_empty() {
            continue;
        }
        history
            .last_touched
            .entry(path.to_string())
            .or_insert_with(|| date.clone());
        if matches_scope && !history.scope_paths.iter().any(|seen| seen == path) {
            history.scope_paths.push(path.to_string());
        }
    }
    history
}

/// Every markdown file under `docs/agent`, minus the debris and the generated pages.
///
/// A CONFLICT COPY IS NOT A DOCUMENT. On 2026-09-18 this walk indexed `RUNLOG 2.md` — a OneDrive
/// known-folder-move artifact — into six manifest pages, and then `pnpm health --fix` deleted the copy,
/// leaving those pages citing a path that no longer exists and failing the harness lint. A generated artifact
/// must never be built out of sync debris: the file was never written by a person, and every page that cites
/// it becomes wrong the moment it is cleaned.
fn walk_markdown(root: &Path, directory: &str, out: &mut Vec<CorpusFile>) {
    let full = root.join(directory);
    let Ok(entries) = fs::read_dir(&full) else {
        return;
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for name in names {
        if is_conflict_copy_name(&name) {
            continue;
        }
        let child = full.join(&name);
        let relative = format!("{directory}/{name}");
        if child.is_dir() {
            // The manifest directory holds the generated pages themselves, and `scratch` is not a document.
            if name == "manifest" || name == "scratch" || name.starts_with('.') {
                continue;
            }
            walk_markdown(root, &relative, out);
            continue;
        }
        if !name.ends_with(".md") {
            continue;
        }
        if let Ok(content) = fs::read_to_string(&child) {
            out.push(CorpusFile {
                path: relative,
                content,
            });
        }
    }
}

/// The lexical candidate set. Deliberately a plain lexical restriction before any scoring: identifiers are
/// exact-match queries, and the corpus is small enough that the whole harness tree is a fine candidate set.
fn load_corpus(root: &Path, scope: &str) -> Vec<CorpusFile> {
    let mut files: Vec<CorpusFile> = Vec::new();
    walk_markdown(root, "docs/agent", &mut files);
    if let Ok(content) = fs::read_to_string(root.join("AGENTS.md")) {
        files.push(CorpusFile {
            path: "AGENTS.md".to_string(),
            content,
        });
    }
    let directory_scope = if scope.contains('/') {
        Some(scope.trim_end_matches('/'))
    } else {
        None
    };
    match directory_scope {
        Some(prefix) => files
            .into_iter()
            .filter(|file| file.path.starts_with(prefix))
            .collect(),
        None => files,
    }
}

/// The handbook and the index pages, for the "what is in the harness" scope that has no packet.
fn top_level_pages(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root.join("docs/agent")) else {
        return Vec::new();
    };
    let mut pages: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".md"))
        .map(|name| format!("docs/agent/{name}"))
        .collect();
    pages.sort();
    pages
}

fn list_manifest_files(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root.join(MANIFEST_DIR)) else {
        return Vec::new();
    };
    let mut files: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".md"))
        .map(|name| format!("{MANIFEST_DIR}/{name}"))
        .collect();
    files.sort();
    files
}

fn scope_from_manifest_file(file: &str) -> String {
    let name = file.rsplit('/').next().unwrap_or(file);
    name.trim_end_matches(".md").to_string()
}

/// Write only when the content differs. These files are committed, and a writer that rewrites a byte-identical
/// file leaves a dirty worktree and a meaningless diff in every release. An idempotent writer makes a re-run
/// free, which is what lets a gate re-run the generator to compare.
///
/// The header's clock is not "content": when the only difference is `generated:` (which changes every second),
/// the file on disk is kept. Without that, `forge:manifest --check-all` — which the harness chain runs —
/// rewrote eight committed files on every run.
fn write_if_changed(path: &Path, content: &str) -> std::io::Result<bool> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == content || only_the_clock_moved(&existing, content) {
            return Ok(false);
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(true)
}

/// A built manifest: the rows, the header facts, and the markdown a reader opens.
pub struct Built {
    pub scope: String,
    pub file: String,
    pub entries: Vec<Entry>,
    pub meta: Meta,
    pub markdown: String,
}

/// Build the manifest for one scope: packet if there is one, the story's own commits, the cited paths, then
/// the lexical tail. Nothing here decides a rule — it gathers and hands the pure ranker its inputs.
pub fn build_manifest(root: &Path, scope: &str, lexical_limit: Option<usize>) -> Built {
    let packet_path = format!("docs/agent/packets/{scope}.md");
    let has_packet = root.join(&packet_path).is_file();
    let packet_content = if has_packet {
        fs::read_to_string(root.join(&packet_path)).unwrap_or_default()
    } else {
        String::new()
    };
    let history = read_history(root, scope);
    let resolver = Resolver::new(root);
    let own_file = format!("{MANIFEST_DIR}/{}.md", manifest_file_name(scope));

    // A cited line may be a COMMAND, not a path (`cargo test -p cli -- forge manifest`). The lint already
    // extracts the path out of it; if the generator does not, the two disagree about the same row — one says
    // MISSING, the other says fine (2026-09-15, on the Assay line of a new packet).
    let exists = |path: &str| -> bool {
        if path == own_file {
            // A packet that documents its own manifest must not report that file missing on the first run: we
            // are writing it in this command.
            return true;
        }
        if !matches!(resolver.resolve(path), Resolution::None) {
            return true;
        }
        path.split_whitespace()
            .any(|token| token.contains('/') && !matches!(resolver.resolve(token), Resolution::None))
    };

    let entries = if has_packet || scope.contains('/') {
        let input = RankInput {
            scope,
            packet_path: if has_packet {
                Some(packet_path.as_str())
            } else {
                None
            },
            cited_paths: cited_repo_paths(&packet_content),
            packet_text: &packet_content,
            commit_paths: &history.scope_paths,
            commit_count: history.scope_commits,
            last_touched: &history.last_touched,
            corpus: &load_corpus(root, scope),
            lexical_limit,
        };
        let mut entries = rank_entries(&input, &exists);
        // A row is a path you can OPEN. A deliverable the packet declares `(new)` cannot be opened yet, so it
        // is not a row — and because it is not a row, the lint is never handed a path it must resolve and
        // cannot. One definition of that marker (`declares_new`), not two opinions.
        entries.retain(|entry| !declares_new(&packet_content, &entry.path));
        entries
    } else {
        let input = RankInput {
            scope,
            packet_path: None,
            cited_paths: Vec::new(),
            packet_text: "",
            commit_paths: &[],
            commit_count: 0,
            last_touched: &history.last_touched,
            corpus: &[],
            lexical_limit,
        };
        let mut entries = rank_entries(&input, &exists);
        // No packet and no directory: this is the "what is in the harness" scope. List the handbook and the
        // index pages rather than pretending a ranking exists.
        let seen: Vec<String> = entries.iter().map(|entry| entry.path.clone()).collect();
        for path in top_level_pages(root) {
            if seen.contains(&path) {
                continue;
            }
            entries.push(Entry {
                last_touched: history.last_touched.get(&path).cloned(),
                path,
                lane: Lane::Index,
                detail: "top-level harness page".to_string(),
                missing: false,
                score: 0.0,
            });
        }
        entries
    };

    let commit = git(root, &["rev-parse", "--short", "HEAD"]).trim().to_string();
    let branch = git(root, &["rev-parse", "--abbrev-ref", "HEAD"]).trim().to_string();
    let dirty = !git(root, &["status", "--porcelain"]).trim().is_empty();
    let meta = Meta {
        scope: scope.to_string(),
        generated_at: format!(
            "{}Z",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
        ),
        commit: if commit.is_empty() { "unknown".to_string() } else { commit },
        branch: if branch.is_empty() { "unknown".to_string() } else { branch },
        dirty,
        command: format!("pnpm forge:manifest {scope}"),
    };
    let markdown = render(&entries, &meta);
    Built {
        scope: scope.to_string(),
        file: format!("{MANIFEST_DIR}/{}.md", manifest_file_name(scope)),
        entries,
        meta,
        markdown,
    }
}

/// One scope, compared rather than written: is the file on disk what a fresh render says?
fn check_one(root: &Path, built: &Built, json_output: bool) -> bool {
    let path = root.join(&built.file);
    let on_disk = fs::read_to_string(&path).unwrap_or_default();
    let (added, removed) = manifest_drift(&on_disk, &built.markdown, Lanes::Structural);
    let fresh = !on_disk.is_empty() && added.is_empty() && removed.is_empty();
    // Reported, never failed: the lexical tail follows the whole harness corpus, so every docs commit in this
    // repository can move it. Regenerate when you want the tail current.
    let lexical = if on_disk.is_empty() {
        0
    } else {
        lexical_drift_count(&on_disk, &built.markdown)
    };
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "file": built.file,
                "fresh": fresh,
                "drift": { "added": added, "removed": removed },
                "lexicalRowsBehind": lexical,
            }))
            .unwrap_or_else(|_| "{}".to_string())
        );
        return fresh;
    }
    if fresh {
        println!(
            "ok    {} ({} rows){}",
            built.file,
            built.entries.len(),
            if lexical > 0 {
                format!("\n      lexical tail: {lexical} row(s) behind a fresh render (informational)")
            } else {
                String::new()
            }
        );
        return true;
    }
    println!(
        "FAIL  {}{}\n      {} row(s) would be added, {} removed\n      regenerate: {}",
        built.file,
        if on_disk.is_empty() {
            " does not exist"
        } else {
            " has drifted from a fresh render"
        },
        added.len(),
        removed.len(),
        built.meta.command
    );
    for row in added.iter().take(5) {
        println!("      + {row}");
    }
    for row in removed.iter().take(5) {
        println!("      - {row}");
    }
    false
}

/// Refuse to write over a file that is not a manifest: the directory is only for manifests, and every `*.md`
/// in it is read as one. Returns the exit code when the write must not happen.
fn refusal_for(root: &Path, file: &str) -> Option<u8> {
    let existing = fs::read_to_string(root.join(file)).ok();
    let refusal = non_manifest_refusal(existing.as_deref(), file)?;
    eprintln!("{refusal}");
    Some(1)
}

/// `--check-all`: every manifest on disk, re-rendered and self-healed.
///
/// SELF-HEALING, DELIBERATELY. A manifest is a GENERATED file: every row is a function of the tree and the
/// commit log, so "drift" does not mean a person made a mistake — it means the render is one build behind.
/// Treating that as a failure turned a generated artifact into hand-maintenance, and it bit twice: each release
/// that touched a manifest changed the history the next render read. Render it, say so, move on.
fn check_all(root: &Path, options: &Options) -> Result<u8, Failure> {
    let files = list_manifest_files(root);
    if files.is_empty() {
        println!("forge:manifest — no manifests on disk yet (run: pnpm forge:manifest PIRATE-01)");
        return Ok(0);
    }
    let mut total_rows = 0usize;
    let mut rewritten = 0usize;
    let mut missing = 0usize;
    for file in &files {
        let built = build_manifest(root, &scope_from_manifest_file(file), options.lexical_limit);
        total_rows += built.entries.len();
        if let Some(code) = refusal_for(root, file) {
            return Ok(code);
        }
        match write_if_changed(&root.join(file), &built.markdown) {
            Ok(true) => rewritten += 1,
            Ok(false) => {}
            Err(error) => {
                return Err(Failure::failed(format!(
                    "cannot write {file}: {error}"
                )))
            }
        }
        let stale = missing_paths(&built.entries);
        missing += stale.len();
        for path in stale {
            // A row that lies is worth saying out loud, but it is prose citing code, not code: it is reported
            // here and in `forge harness-lint`, and it does not stop a release.
            println!("  note  {file} cites {path}, which does not exist");
        }
    }
    if !options.json {
        println!(
            "\nforge:manifest — {} manifest(s), {} rows, {} re-rendered{}",
            files.len(),
            total_rows,
            rewritten,
            if missing > 0 {
                format!(", {missing} stale citation(s) noted")
            } else {
                String::new()
            }
        );
    }
    Ok(0)
}

/// The command. `forge manifest [scope] [--check] [--check-all] [--format json] [--lexical <n>]`.
pub fn run(args: &[String]) -> Result<u8, Failure> {
    let options = parse_args(args);
    let root = super::repo_root();
    if options.check_all {
        return check_all(&root, &options);
    }
    let built = build_manifest(&root, &options.scope, options.lexical_limit);
    if options.check {
        return Ok(if check_one(&root, &built, options.json) {
            0
        } else {
            1
        });
    }
    if options.json {
        let missing = missing_paths(&built.entries);
        let value: Value = json!({
            "scope": built.scope,
            "file": built.file,
            "meta": {
                "scope": built.meta.scope,
                "generatedAt": built.meta.generated_at,
                "commit": built.meta.commit,
                "branch": built.meta.branch,
                "dirty": built.meta.dirty,
                "command": built.meta.command,
            },
            "rows": built.entries.len(),
            "missing": missing,
            "entries": built.entries.iter().map(|entry| json!({
                "path": entry.path,
                "lane": entry.lane.as_str(),
                "detail": entry.detail,
                "lastTouched": entry.last_touched,
                "missing": entry.missing,
                "score": entry.score,
            })).collect::<Vec<Value>>(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&value)
                .map_err(|error| Failure::failed(format!("cannot render as JSON: {error}")))?
        );
        return Ok(if missing.is_empty() { 0 } else { 1 });
    }

    if let Some(code) = refusal_for(&root, &built.file) {
        return Ok(code);
    }
    let changed = write_if_changed(&root.join(&built.file), &built.markdown).map_err(|error| {
        Failure::failed(format!("cannot write {}: {error}", built.file))
    })?;
    let mut by_lane: Vec<(&'static str, usize)> = Vec::new();
    for entry in &built.entries {
        match by_lane.iter_mut().find(|(lane, _)| *lane == entry.lane.as_str()) {
            Some((_, count)) => *count += 1,
            None => by_lane.push((entry.lane.as_str(), 1)),
        }
    }
    println!(
        "{}  {}",
        if changed { "wrote" } else { "unchanged" },
        built.file
    );
    println!(
        "  rows: {}  ({})",
        built.entries.len(),
        by_lane
            .iter()
            .map(|(lane, count)| format!("{lane} {count}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let missing = missing_paths(&built.entries);
    if !missing.is_empty() {
        println!("  MISSING paths — a row that lies is a bug; fix or remove the reference:");
        for path in &missing {
            println!("    {path}");
        }
        return Ok(1);
    }
    println!("  open the rows top-down; the why column says why each one is here.");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn no_arguments_means_the_whole_harness() {
        let options = parse_args(&args(&[]));
        assert_eq!(options.scope, "all");
        assert!(!options.check && !options.check_all && !options.json);
    }

    #[test]
    fn a_scope_and_the_gate_flags_are_read_together() {
        let options = parse_args(&args(&["PIRATE-01", "--check", "--format", "json"]));
        assert_eq!(options.scope, "PIRATE-01");
        assert!(options.check && options.json);
        let options = parse_args(&args(&["--format", "json", "--lexical", "3", "FORGE-GATES-01"]));
        assert_eq!(options.lexical_limit, Some(3));
        assert_eq!(options.scope, "FORGE-GATES-01");
        assert!(!options.check);
    }

    #[test]
    fn check_all_does_not_need_a_scope_and_ignores_a_stray_flag() {
        let options = parse_args(&args(&["--check-all", "--nonsense"]));
        assert!(options.check_all);
        assert_eq!(options.scope, "");
        // `--format` CONSUMES its value, as the TypeScript did: `--format PIRATE-01` is not a scope, it is a
        // format nobody recognises. Reading the scope out of it would be a second opinion about one argument.
        let options = parse_args(&args(&["--format", "PIRATE-01"]));
        assert!(!options.json);
        assert_eq!(options.scope, "all");
    }

    #[test]
    fn a_manifest_file_name_round_trips_back_to_its_scope() {
        assert_eq!(scope_from_manifest_file("docs/agent/manifest/PIRATE-01.md"), "PIRATE-01");
        assert_eq!(scope_from_manifest_file("all.md"), "all");
    }

    #[test]
    fn a_re_run_does_not_rewrite_a_file_that_did_not_change() {
        let root = std::env::temp_dir().join(format!("forge-manifest-write-{}", std::process::id()));
        let path = root.join("docs/agent/manifest/TEST-01.md");
        assert!(write_if_changed(&path, "# Scope manifest — TEST-01\n").expect("write"));
        assert!(!write_if_changed(&path, "# Scope manifest — TEST-01\n").expect("no write"));
        assert!(write_if_changed(&path, "# Scope manifest — TEST-01\n\n").expect("write"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_re_run_that_moved_only_the_clock_leaves_the_file_alone() {
        // `forge:manifest --check-all` is in the harness chain, so a rewrite per run dirtied eight committed
        // files every time it ran. Only a real change may touch them.
        let root = std::env::temp_dir().join(format!("forge-manifest-clock-{}", std::process::id()));
        let path = root.join("docs/agent/manifest/TEST-01.md");
        let first = "# Scope manifest — TEST-01\n\n- generated: 2026-09-28 08:00:00Z\n- rows: 1\n";
        let second = "# Scope manifest — TEST-01\n\n- generated: 2026-09-28 09:30:00Z\n- rows: 1\n";
        assert!(write_if_changed(&path, first).expect("write"));
        assert!(!write_if_changed(&path, second).expect("the clock alone is not a change"));
        assert_eq!(fs::read_to_string(&path).expect("read"), first);
        // A row that changed is a change, clock or no clock.
        let third = "# Scope manifest — TEST-01\n\n- generated: 2026-09-28 09:30:00Z\n- rows: 2\n";
        assert!(write_if_changed(&path, third).expect("write"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_history_reader_answers_three_questions_in_one_pass() {
        // Against the real repository: `git` may be absent in a sandbox, in which case the reader degrades to
        // "no history" rather than refusing — a manifest still has to render.
        //
        // The scope is a WORD that appears in commit subjects, not a story id: measured 2026-09-28, none of the
        // last 400 commit subjects in this repository names a story id, so the commit lane is empty for every
        // story until commits name their story (reported, not fixed here).
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .map(Path::to_path_buf)
            .expect("the repository root above rust/cli");
        let history = read_history(&root, "forge");
        if history.last_touched.is_empty() {
            return; // no git in this sandbox
        }
        assert!(history.scope_commits > 0, "the word is in the log");
        assert!(!history.scope_paths.is_empty(), "those commits changed files");
        assert!(history.last_touched.values().all(|date| date.len() == 10));
    }

    #[test]
    fn the_generator_never_indexes_a_conflict_copy() {
        let root = std::env::temp_dir().join(format!("forge-manifest-debris-{}", std::process::id()));
        let docs = root.join("docs/agent");
        fs::create_dir_all(&docs).expect("fixture tree");
        fs::write(docs.join("RUNLOG.md"), "# runlog\n").expect("fixture");
        fs::write(docs.join("RUNLOG 2.md"), "# runlog copy\n").expect("fixture");
        let corpus = load_corpus(&root, "TEST-01");
        assert!(corpus.iter().any(|file| file.path.ends_with("RUNLOG.md")));
        assert!(
            !corpus.iter().any(|file| file.path.contains("RUNLOG 2.md")),
            "a numbered sibling is debris, never a document: {:?}",
            corpus.iter().map(|file| &file.path).collect::<Vec<_>>()
        );
        let _ = fs::remove_dir_all(&root);
    }
}
