//! SCOPE MANIFEST — "here are the exact files to read for this scope", generated and gate-able, instead
//! of "here is what grep happened to match".
//!
//! Ported from the retired `lib/scope-manifest.ts` + `scripts/forge-manifest.ts`, both deleted with the
//! TypeScript application in `4cf98110`. The split is the one the port keeps: the RULES live here (pure,
//! no filesystem, no git, no clock), and the gather-and-print half lives in `rust/cli/src/forge/manifest.rs`.
//!
//! Pirated (idea, not code) from OpenContext's `oc context manifest` (0xranx/OpenContext, MIT) and rebuilt
//! in our shape. What we did NOT copy is the part that made theirs weak: their query is `ORDER BY
//! rel_path`, so the manifest is a directory listing and its only signal is the one-line description
//! someone remembered to write. We measured that against our corpus on 2026-09-15:
//!
//!   grep -ril "how does a story move get written to the database" docs/agent
//!     -> 124 of 133 files, alphabetically (so the first five are arbitrary)
//!   the same query ranked by structural signals
//!     -> the packet, the paths it cites, the files the story's own commits touched
//!
//! Lanes are STRUCTURAL FIRST, because we have signals a document store cannot have: a packet that cites
//! paths, git commits that name the story, and story state that lives in Neon. Lexical ranking is the
//! second lane and runs over the grep-restricted candidate set rather than the whole corpus, because an
//! identifier like `forge_batch_item` is already an exact-match query.

use std::collections::{HashMap, HashSet};

/// The six lanes, in the order they are pushed. Ordering is by LANE, never by score across lanes: a file
/// the packet cites outranks a file that merely talks about the same words, however often it says them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lane {
    Handbook,
    Packet,
    Cited,
    Commit,
    Lexical,
    Index,
}

impl Lane {
    pub fn as_str(self) -> &'static str {
        match self {
            Lane::Handbook => "handbook",
            Lane::Packet => "packet",
            Lane::Cited => "cited",
            Lane::Commit => "commit",
            Lane::Lexical => "lexical",
            Lane::Index => "index",
        }
    }
}

/// One row: a path you can OPEN, and the why that puts it there.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Repo-relative path.
    pub path: String,
    pub lane: Lane,
    /// Why this row is here, in the words a reader would use.
    pub detail: String,
    /// Last commit date touching the path (`YYYY-MM-DD`), or `None` when unknown.
    pub last_touched: Option<String>,
    /// The path was named (packet, commit) but is not on disk. Always a finding.
    pub missing: bool,
    /// Lexical relevance; 0 for every structural lane.
    pub score: f64,
}

/// Always-read files. Not ranked, not scored: if you work in this repo you read these, and a manifest that
/// omits them is how someone works from a stale picture of the rules.
pub const HANDBOOK_PATHS: [&str; 4] = [
    "AGENTS.md",
    "docs/agent/ORIENTATION.md",
    "docs/agent/MEMORY.md",
    "docs/agent/CURRENT.md",
];

const STOPWORDS: &str = "the a an and or of to in on for is are was were be been am it its this that those these with from as at by if then than into not no do does \
did how what when where which who why you your we our they their them there here all any some more most other such only own same so too very \
can will just should now also must may might shall about over under again further once during before after above below out off while each \
add adds added new use uses used using make makes made get gets got run runs ran set sets back onto per via";

/// Words from a query or packet: lowercased, stopworded, identifiers kept intact.
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .map(|token| token.trim_matches(|c| c == '-' || c == '_').to_string())
        .filter(|token| {
            token.len() >= 3 && !STOPWORDS.split_whitespace().any(|word| word == token)
        })
        .collect()
}

/// Identifiers are what grep already does perfectly, so the lexical lane must not dilute them: a token
/// containing a digit or underscore is treated as an exact term (`forge_batch_item`, `v5-23`,
/// `storyboard_story`).
pub fn is_identifier_token(token: &str) -> bool {
    token.contains(|c: char| c.is_ascii_digit() || c == '_')
}

/// One candidate document for the lexical lane, already the grep-restricted set.
pub struct CorpusFile {
    pub path: String,
    pub content: String,
}

/// Everything the ranking reads. Existence is injected as a closure so a row can be judged against a
/// fixture root in a test and against the real tree in the CLI — the rule is the same in both.
pub struct RankInput<'a> {
    pub scope: &'a str,
    /// Repo-relative path of the packet, when the scope is a story.
    pub packet_path: Option<&'a str>,
    /// The paths the packet cites, resolved to the same contract the packet lint uses.
    pub cited_paths: Vec<String>,
    /// The packet's text, used for its vocabulary.
    pub packet_text: &'a str,
    /// Paths named by commits whose message mentions the scope.
    pub commit_paths: &'a [String],
    /// How many such commits there were, for the "why" line.
    pub commit_count: usize,
    /// path -> `YYYY-MM-DD` of the last commit touching it.
    pub last_touched: &'a HashMap<String, String>,
    pub corpus: &'a [CorpusFile],
    /// Cap on lexical rows. Structural rows are never dropped.
    pub lexical_limit: Option<usize>,
}

fn corpus_stats(corpus: &[&CorpusFile]) -> (HashMap<String, usize>, HashMap<String, Vec<String>>) {
    let mut document_frequency: HashMap<String, usize> = HashMap::new();
    let mut tokens_by_file: HashMap<String, Vec<String>> = HashMap::new();
    for file in corpus {
        let tokens = tokenize(&file.content);
        for token in tokens.iter().collect::<HashSet<_>>() {
            *document_frequency.entry(token.clone()).or_insert(0) += 1;
        }
        tokens_by_file.insert(file.path.clone(), tokens);
    }
    (document_frequency, tokens_by_file)
}

/// The structural lanes only: everything a reader must open regardless of the corpus's mood.
fn push_structural(input: &RankInput<'_>, exists: &dyn Fn(&str) -> bool) -> (Vec<Entry>, HashSet<String>) {
    let mut rows: Vec<Entry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut push = |path: &str, lane: Lane, detail: String, rows: &mut Vec<Entry>| {
        if path.is_empty() || seen.contains(path) {
            return;
        }
        seen.insert(path.to_string());
        rows.push(Entry {
            path: path.to_string(),
            lane,
            detail,
            last_touched: input.last_touched.get(path).cloned(),
            missing: !exists(path),
            score: 0.0,
        });
    };

    for path in HANDBOOK_PATHS {
        push(path, Lane::Handbook, "always-read handbook".to_string(), &mut rows);
    }
    if let Some(packet) = input.packet_path {
        push(
            packet,
            Lane::Packet,
            format!("the packet for {}", input.scope),
            &mut rows,
        );
    }
    for path in &input.cited_paths {
        if Some(path.as_str()) == input.packet_path {
            continue;
        }
        push(
            path,
            Lane::Cited,
            format!("cited by {}", input.scope),
            &mut rows,
        );
    }

    let mut seen_commit: HashSet<&String> = HashSet::new();
    for path in input.commit_paths {
        if !seen_commit.insert(path) {
            continue;
        }
        push(
            path,
            Lane::Commit,
            format!(
                "touched by {} commit(s) naming {}",
                input.commit_count.max(1),
                input.scope
            ),
            &mut rows,
        );
    }

    (rows, seen)
}

/// The lexical tail: `docs/agent` prose sharing vocabulary with the packet, scored by TF-IDF over the
/// grep-restricted corpus. Never outranks a structural row — it is appended after them.
fn lexical_rows(
    input: &RankInput<'_>,
    seen: &HashSet<String>,
    exists: &dyn Fn(&str) -> bool,
) -> Vec<Entry> {
    let terms: HashSet<String> = tokenize(input.packet_text).into_iter().collect();
    // The lexical lane only ever sees corpus files no structural lane already claimed, and the document
    // frequency is measured over THAT set — the corpus is restricted, so its statistics must be too.
    let files: Vec<&CorpusFile> = input
        .corpus
        .iter()
        .filter(|file| !seen.contains(&file.path))
        .collect();
    if terms.is_empty() || files.is_empty() {
        return Vec::new();
    }
    let (document_frequency, tokens_by_file) = corpus_stats(&files);
    let total = files.len().max(1) as f64;
    let mut scored: Vec<(String, f64, Vec<String>)> = Vec::new();
    for file in &files {
        let tokens = tokens_by_file.get(&file.path).cloned().unwrap_or_default();
        let mut counts: HashMap<String, usize> = HashMap::new();
        for token in &tokens {
            if terms.contains(token) {
                *counts.entry(token.clone()).or_insert(0) += 1;
            }
        }
        if counts.is_empty() {
            continue;
        }
        let mut score = 0.0f64;
        for (token, count) in &counts {
            let frequency = *document_frequency.get(token).unwrap_or(&input.corpus.len());
            let idf = (1.0 + total / frequency.max(1) as f64).ln();
            let weight = if is_identifier_token(token) { 3.0 } else { 1.0 };
            score += weight * (1.0 + (*count as f64).ln()) * idf;
        }
        // A file that matches more distinct terms is about more of the question.
        score *= 1.0 + 0.35 * (counts.len() as f64 - 1.0);
        // Length normalization: a 60KB log must not win a term-count argument.
        score /= (1.0 + tokens.len() as f64 / 1000.0).sqrt();
        let mut matched: Vec<String> = counts.keys().cloned().collect();
        matched.sort();
        scored.push((file.path.clone(), score, matched));
    }
    scored.retain(|(_, score, _)| *score > 0.0);
    // Ties break on the path so two runs over the same tree render the same file.
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    scored
        .into_iter()
        .take(input.lexical_limit.unwrap_or(8))
        .map(|(path, score, matched)| Entry {
            last_touched: input.last_touched.get(&path).cloned(),
            missing: !exists(&path),
            path,
            lane: Lane::Lexical,
            detail: format!(
                "term match: {}",
                matched.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
            ),
            score,
        })
        .collect()
}

/// Build the ranked rows. Structural lanes first, then the lexical tail.
pub fn rank_entries(input: &RankInput<'_>, exists: &dyn Fn(&str) -> bool) -> Vec<Entry> {
    let (mut rows, seen) = push_structural(input, exists);
    rows.extend(lexical_rows(input, &seen, exists));
    rows
}

/// Does this packet DECLARE THE PATH AS NEW — a deliverable of this story that does not exist yet?
///
/// The marker is mechanical: the path and `(new)` on the same line, `(\s*new\s*)` case-insensitive. A
/// story's own deliverables do not exist on the base ref — that is what "build this" means — and reporting
/// them as MISSING rows made `manifest-cites-missing-path` red-light the harness on exactly the stories that
/// had not been built yet, the ones that need the gates most (measured 2026-09-15 on ENG-FORGE-DOCTOR-01:
/// three rows, every one of them a file the story exists to create).
///
/// One definition, here, rather than a second opinion wherever rows are read: the rows for pending
/// deliverables are dropped by the generator, so the lint is never handed a path it must resolve and cannot.
pub fn declares_new(packet_text: &str, path: &str) -> bool {
    if packet_text.is_empty() || path.is_empty() || path.contains('\n') {
        return false;
    }
    // The marker is matched against the line with spaces removed, so `( new )` counts and the path itself is
    // still matched against the line as written.
    packet_text.split('\n').any(|line| {
        line.contains(path)
            && line
                .to_ascii_lowercase()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .contains("(new)")
    })
}

/// The header of every rendered manifest: a generated file, and the one line that identifies it.
pub const MANIFEST_HEADER: &str = "# Scope manifest";

/// What the header records about the render. `dirty` is a fact about the tree, not a failure.
pub struct Meta {
    pub scope: String,
    pub generated_at: String,
    pub commit: String,
    pub branch: String,
    pub dirty: bool,
    /// Command that reproduces this file, printed in the header so nobody hand-edits it.
    pub command: String,
}

/// The manifest as markdown: `path · lane · why · last touched`, with a MISSING flag on the row itself,
/// because a manifest that quietly drops a deleted file is how a reader concludes the file still exists.
pub fn render(entries: &[Entry], meta: &Meta) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("{MANIFEST_HEADER} — {}", meta.scope));
    lines.push(String::new());
    lines.push(
        "<!-- GENERATED FILE. Do not hand-edit. The freshness gate compares the structural rows"
            .to_string(),
    );
    lines.push(
        "     (handbook, packet, cited, commit, index); the lexical tail is informational. -->"
            .to_string(),
    );
    lines.push(String::new());
    lines.push(format!("- generated: {}", meta.generated_at));
    lines.push(format!(
        "- commit: `{}`{} on `{}`",
        meta.commit,
        if meta.dirty { " (working tree dirty)" } else { "" },
        meta.branch
    ));
    lines.push(format!("- regenerate: `{}`", meta.command));
    lines.push(format!(
        "- rows: {} — packet, cited paths and story commits first, lexical matches after",
        entries.len()
    ));
    lines.push(String::new());
    lines.push(
        "Read top-down. A row is a file to open, and the why column says why it is here.".to_string(),
    );
    lines.push(String::new());
    for entry in entries {
        lines.push(format!(
            "- `{}`{} — {} · {} · last touched {}",
            entry.path,
            if entry.missing { " **MISSING**" } else { "" },
            entry.lane.as_str(),
            entry.detail,
            entry
                .last_touched
                .clone()
                .unwrap_or_else(|| "untracked".to_string())
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Scope names become file names; keep a story id or a path from escaping the directory.
pub fn manifest_file_name(scope: &str) -> String {
    let mut safe = String::new();
    for character in scope.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            safe.push(character);
        } else if !safe.ends_with('-') {
            safe.push('-');
        }
    }
    let trimmed = safe.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "all".to_string()
    } else {
        trimmed
    }
}

/// Rows the lint must fail on: a manifest row naming a path that is not on disk.
pub fn missing_paths(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| entry.missing)
        .map(|entry| entry.path.clone())
        .collect()
}

/// A manifest row whose lane is `lexical` — the corpus-dependent tail.
pub fn is_lexical_row(line: &str) -> bool {
    line.contains(" — lexical · ")
}

/// Which lanes a comparison counts.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lanes {
    Structural,
    All,
}

/// Row-level diff between the file on disk and a fresh render.
///
/// STRUCTURAL LANES ONLY, by default. Measured the first time it mattered: a co-worker landed a new packet
/// (282 lines, one file, no overlap with any file this story touched) and the lexical tail of another
/// story's manifest moved — so the gate went red on a commit that had nothing to do with that story. A gate
/// that fails on a co-worker's docs commit is a gate someone switches off, and the value of the freshness
/// check is the part that actually rots: did the packet's cited paths change, did a row's file disappear, did
/// the story's own commits touch something new. The lexical tail is a convenience list and is reported
/// separately, as information.
///
/// Safety is unchanged either way: a row pointing at a path that is gone still fails the packet lint
/// (rule 8), which parses rows rather than comparing renders.
pub fn manifest_drift(on_disk: &str, fresh: &str, lanes: Lanes) -> (Vec<String>, Vec<String>) {
    let rows = |markdown: &str| -> Vec<String> {
        markdown
            .split('\n')
            .filter(|line| line.starts_with("- `"))
            .filter(|line| matches!(lanes, Lanes::All) || !is_lexical_row(line))
            .map(|line| {
                let cut = line.rfind(" · last touched").unwrap_or(line.len());
                line[..cut].to_string()
            })
            .collect()
    };
    let before: Vec<String> = rows(on_disk);
    let after: Vec<String> = rows(fresh);
    let added = after
        .iter()
        .filter(|row| !before.contains(row))
        .cloned()
        .collect();
    let removed = before
        .iter()
        .filter(|row| !after.contains(row))
        .cloned()
        .collect();
    (added, removed)
}

/// Did ONLY the run stamp move between two renders of the same manifest?
///
/// A generated file whose header carries `generated: <now>` changes on every run, so a whole-body comparison
/// makes every run write the file — and `pnpm forge:harness` runs `forge:manifest --check-all`, so the chain
/// left a dirty worktree every time it ran. That contradicts the writer's own purpose ("an idempotent writer
/// makes a re-run free, which is what lets a gate re-run the generator to compare"), so the header's run stamp
/// is compared separately from the body: when everything else is identical, the file on disk is kept, and its
/// `generated:` line then means "when this CONTENT was generated" — which is what a reader wants it to mean.
///
/// The stamp is three fields, all of them facts about the RUN rather than about the rows: the clock, the
/// `(working tree dirty)` marker (it flips as somebody works), and the commit sha (it moves on every commit,
/// including commits that touch nothing this manifest lists — so treating it as content made all eight
/// committed manifests stale after every push, and re-rendering them meant another commit: a treadmill).
pub fn only_the_run_stamp_moved(on_disk: &str, fresh: &str) -> bool {
    const STAMP_PREFIXES: [&str; 2] = ["- generated: ", "- commit: "];
    const DIRTY_MARK: &str = " (working tree dirty)";
    fn body_without_stamp(markdown: &str) -> String {
        markdown
            .split('\n')
            .filter(|line| !STAMP_PREFIXES.iter().any(|prefix| line.starts_with(prefix)))
            .map(|line| line.replace(DIRTY_MARK, ""))
            .collect::<Vec<_>>()
            .join("\n")
    }
    on_disk != fresh && body_without_stamp(on_disk) == body_without_stamp(fresh)
}
pub fn lexical_drift_count(on_disk: &str, fresh: &str) -> usize {
    let (added, removed) = manifest_drift(on_disk, fresh, Lanes::All);
    added.len() + removed.len()
}

/// How many STRUCTURAL rows a fresh render would change: the part a gate may fail on.
pub fn structural_drift_count(on_disk: &str, fresh: &str) -> usize {
    let (added, removed) = manifest_drift(on_disk, fresh, Lanes::Structural);
    added.len() + removed.len()
}

/// REFUSE TO OVERWRITE SOMETHING THAT IS NOT A MANIFEST.
///
/// The harness treats EVERY `*.md` in `docs/agent/manifest/` as a manifest and renders a fresh one to
/// compare, so the directory is only for manifests — and this tool, given a name, will happily write
/// `docs/agent/manifest/<name>.md` over whatever is already there. On 2026-09-18 that destroyed a 128-column
/// audit table: `pnpm forge:manifest COLUMN-WRITER-AUDIT` replaced it with a 34-row index skeleton, and only
/// git brought it back. A tool that overwrites a file it cannot recognise is a tool that will do it again, so
/// the write is now guarded by the file's own first line.
///
/// Returns the refusal to print, or `None` when the write may proceed (a missing file is fine: a new manifest
/// has to be creatable).
pub fn non_manifest_refusal(existing: Option<&str>, target: &str) -> Option<String> {
    let existing = existing?;
    if existing.starts_with(MANIFEST_HEADER) {
        return None;
    }
    Some(format!(
        "refusing to overwrite {target}: it is not a manifest (it does not start with \"{MANIFEST_HEADER}\").\n  \
         If it is an audit, a report or a plan, it does not belong in docs/agent/manifest/ — move it out and \
         point its tool at the new path."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_history() -> HashMap<String, String> {
        HashMap::new()
    }

    fn entry(path: &str, lane: Lane) -> Entry {
        Entry {
            path: path.to_string(),
            lane,
            detail: "why".to_string(),
            last_touched: None,
            missing: false,
            score: 0.0,
        }
    }

    fn meta() -> Meta {
        Meta {
            scope: "TEST-01".to_string(),
            generated_at: "2026-09-28 00:00:00Z".to_string(),
            commit: "abc1234".to_string(),
            branch: "main".to_string(),
            dirty: false,
            command: "pnpm forge:manifest TEST-01".to_string(),
        }
    }

    #[test]
    fn the_handbook_is_always_first_and_never_scored() {
        let last_touched = no_history();
        let input = RankInput {
            scope: "TEST-01",
            packet_path: Some("docs/agent/packets/TEST-01.md"),
            cited_paths: vec!["rust/cli/src/main.rs".to_string()],
            packet_text: "",
            commit_paths: &[],
            commit_count: 0,
            last_touched: &last_touched,
            corpus: &[],
            lexical_limit: None,
        };
        let rows = rank_entries(&input, &|_| true);
        let lanes: Vec<&str> = rows.iter().map(|row| row.lane.as_str()).collect();
        assert_eq!(
            lanes,
            vec!["handbook", "handbook", "handbook", "handbook", "packet", "cited"]
        );
        assert_eq!(rows[0].path, "AGENTS.md");
        assert!(rows.iter().all(|row| row.score == 0.0));
    }

    #[test]
    fn an_identifier_match_outweighs_a_plain_word_at_equal_frequency() {
        // The measured reason the identifier weight exists (2026-09-15): `grep -ril` returns 124 of 133 files
        // alphabetically. An identifier is an exact query, so at equal frequency it must score three times a
        // plain word — otherwise "which forge_batch_item row…" ranks the doc that merely says "batch".
        let last_touched = no_history();
        let corpus = vec![
            CorpusFile {
                path: "docs/agent/a.md".to_string(),
                content: "forge_batch_item".to_string(),
            },
            CorpusFile {
                path: "docs/agent/b.md".to_string(),
                content: "batch".to_string(),
            },
        ];
        let input = RankInput {
            scope: "TEST-01",
            packet_path: None,
            cited_paths: vec![],
            packet_text: "which forge_batch_item row holds a batch",
            commit_paths: &[],
            commit_count: 0,
            last_touched: &last_touched,
            corpus: &corpus,
            lexical_limit: None,
        };
        let rows = rank_entries(&input, &|_| true);
        let lexical: Vec<&Entry> = rows.iter().filter(|row| row.lane == Lane::Lexical).collect();
        assert_eq!(lexical.len(), 2);
        assert_eq!(lexical[0].path, "docs/agent/a.md");
        assert!((lexical[0].score / lexical[1].score - 3.0).abs() < 0.01);
    }

    #[test]
    fn a_row_that_is_not_on_disk_is_missing_rather_than_dropped() {
        let last_touched = no_history();
        let input = RankInput {
            scope: "TEST-01",
            packet_path: None,
            cited_paths: vec!["legacy/db/gone.ts".to_string()],
            packet_text: "",
            commit_paths: &[],
            commit_count: 0,
            last_touched: &last_touched,
            corpus: &[],
            lexical_limit: None,
        };
        let rows = rank_entries(&input, &|path| !path.starts_with("legacy/"));
        assert_eq!(missing_paths(&rows), vec!["legacy/db/gone.ts"]);
        assert!(rows.iter().any(|row| row.path == "legacy/db/gone.ts"));
    }

    #[test]
    fn a_declared_new_deliverable_is_dropped_by_the_generator_not_flagged() {
        let packet = "- `rust/cli/src/forge/manifest.rs` (new) — the command\n- `rust/forge/src/scope_manifest.rs` ( new )";
        assert!(declares_new(packet, "rust/cli/src/forge/manifest.rs"));
        assert!(declares_new(packet, "rust/forge/src/scope_manifest.rs"));
        assert!(!declares_new(packet, "rust/cli/src/main.rs"));
        assert!(!declares_new("", "rust/cli/src/main.rs"));
    }

    #[test]
    fn a_rendered_row_carries_path_lane_why_and_age() {
        let markdown = render(
            &[
                entry("AGENTS.md", Lane::Handbook),
                Entry {
                    last_touched: Some("2026-09-27".to_string()),
                    ..entry("legacy/db/gone.ts", Lane::Cited)
                },
            ],
            &meta(),
        );
        assert!(markdown.starts_with("# Scope manifest — TEST-01\n"));
        assert!(markdown.contains("- `AGENTS.md` — handbook · why · last touched untracked\n"));
        assert!(markdown.contains("- regenerate: `pnpm forge:manifest TEST-01`\n"));
        assert!(markdown.contains("- rows: 2 —"));
        assert!(markdown.ends_with('\n'));
    }

    #[test]
    fn a_missing_row_says_so_on_the_row_itself() {
        let markdown = render(
            &[Entry {
                missing: true,
                ..entry("legacy/db/gone.ts", Lane::Cited)
            }],
            &meta(),
        );
        // The lint parses this shape (`^- \`([^\`]+)\`( \*\*MISSING\*\*)? — `), so the marker has to sit
        // before the em dash, not after it.
        assert!(markdown.contains("- `legacy/db/gone.ts` **MISSING** — cited · why ·"));
    }

    #[test]
    fn the_age_of_a_row_does_not_decide_whether_it_has_drifted() {
        // A co-worker touching a file moves its date. The gate compares path, lane and why, never the age.
        let row = entry("AGENTS.md", Lane::Handbook);
        let before = render(std::slice::from_ref(&row), &meta());
        let aged = render(
            &[Entry {
                last_touched: Some("2026-09-28".to_string()),
                ..row
            }],
            &meta(),
        );
        assert_ne!(before, aged);
        let (added, removed) = manifest_drift(&before, &aged, Lanes::Structural);
        assert!(added.is_empty() && removed.is_empty());
    }

    #[test]
    fn the_lexical_tail_moves_the_gate_only_when_asked() {
        let handbook = entry("AGENTS.md", Lane::Handbook);
        let before = render(std::slice::from_ref(&handbook), &meta());
        let after = render(
            &[handbook, entry("docs/agent/OTHER.md", Lane::Lexical)],
            &meta(),
        );
        assert_eq!(structural_drift_count(&before, &after), 0);
        assert_eq!(lexical_drift_count(&before, &after), 1);
    }

    #[test]
    fn a_scope_name_cannot_escape_the_manifest_directory() {
        assert_eq!(manifest_file_name("PIRATE-01"), "PIRATE-01");
        assert_eq!(manifest_file_name("docs/agent/"), "docs-agent");
        assert_eq!(manifest_file_name("   "), "all");
        // A scope is a story id or a directory. No path separator survives the sanitising, so the worst a
        // hostile scope can produce is an odd FILE NAME inside the manifest directory — never a path out of it.
        let escaped = manifest_file_name("../../etc/passwd");
        assert!(!escaped.contains('/'));
        assert!(!escaped.contains('\\'));
        assert_eq!(escaped, "..-..-etc-passwd");
    }

    #[test]
    fn an_audit_table_in_the_manifest_directory_is_never_overwritten() {
        assert!(
            non_manifest_refusal(Some("# Scope manifest — X\n"), "docs/agent/manifest/X.md").is_none()
        );
        assert!(non_manifest_refusal(None, "docs/agent/manifest/X.md").is_none());
        let refusal =
            non_manifest_refusal(Some("# Column writer audit\n"), "docs/agent/manifest/X.md")
                .expect("an audit is refused");
        assert!(refusal.contains("refusing to overwrite"));
        assert!(refusal.contains("docs/agent/manifest/X.md"));
    }
}

