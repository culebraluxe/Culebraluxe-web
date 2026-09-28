//! Citations — resolving what a packet claims about the tree. Ported from the retired
//! `lib/scope-manifest.ts` (`lineCitations`), deleted with the TypeScript application in `4cf98110`.
//!
//! Three habits decide whether a gate survives contact with real writing:
//!
//! 1. Evidence names a file AND a line range, so a stale range is a claim that reads as precision.
//! 2. A citation is repo-relative, doc-relative (`packets/README.md` meaning
//!    `docs/agent/packets/README.md`) or a bare basename (`apple_messages.rs:120`). Measured
//!    2026-09-15: 15 of the 16 hits from the first live run of the range rule were that shorthand, so
//!    the resolver exists instead of a stricter rule that people would switch off.
//! 3. Two files sharing a basename is not this gate's call — it stays quiet rather than guessing.
//!
//! `SOURCE_ROOTS` includes `rust/`: the tree is Rust now, so a packet's shorthand for a Rust file has
//! to be resolvable or the rule would fail every honest citation after the port. `target/` is skipped
//! alongside `node_modules/`, or the walk would index build output.

use regex::Regex;
use std::cell::OnceCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Source roots the basename resolver walks. Bounded on purpose: this is a lint, not an indexer.
const SOURCE_ROOTS: [&str; 12] = [
    "app",
    "components",
    "lib",
    "services",
    "db",
    "scripts",
    "agent-runtime",
    "workflow_app",
    "ui",
    "testv2",
    "legacy/db/migrations",
    "rust",
];

/// Directories never walked: build output, not source.
const SKIPPED_DIRS: [&str; 2] = ["node_modules", "target"];

pub struct LineCitation {
    pub path: String,
    /// The last line the citation claims. The start of the range is not kept: the only question this
    /// gate asks is whether the range runs past the end of the file, and a field nothing reads is how
    /// a struct learns to lie about what it is for.
    pub end: usize,
    pub raw: String,
}

fn citation_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        // `.rs` and `.toml` were added with the port: the tree is Rust now, and a pattern that cannot
        // see a Rust file means rule 10 checks nothing at all on the code it was written to check.
        Regex::new(
            r"`([A-Za-z0-9_./-]+\.(?:ts|tsx|mjs|js|json|sql|sh|md|css|rs|toml)):(\d+)(?:-(\d+))?`",
        )
        .expect("citation pattern is a constant")
    })
}

fn backtick_token_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"`([A-Za-z0-9_./*<>{}-]+)`").expect("token pattern is a constant")
    })
}

/// Every `path:line` or `path:line-line` citation in a document.
pub fn line_citations(text: &str) -> Vec<LineCitation> {
    citation_pattern()
        .captures_iter(text)
        .map(|captures| {
            let start = captures[2].parse::<usize>().unwrap_or(0);
            let end = captures
                .get(3)
                .and_then(|value| value.as_str().parse::<usize>().ok())
                .unwrap_or(start);
            LineCitation {
                path: captures[1].to_string(),
                end,
                raw: captures[0].to_string(),
            }
        })
        .collect()
}

/// `content.split('\n').length` is one too many for a file ending in a newline.
pub fn count_lines(content: &str) -> usize {
    let lines = content.split('\n').count();
    if content.ends_with('\n') {
        lines - 1
    } else {
        lines
    }
}

/// Repo paths a MAP cites. This is what separates a map from prose: a map that points at a file that
/// moved is worse than no map, because it wastes the reader's time and teaches them to distrust it.
pub fn cited_repo_paths(content: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for captures in backtick_token_pattern().captures_iter(content) {
        let token = &captures[1];
        if !token.contains('/') {
            continue; // not a path (table names, commands without paths)
        }
        if token.starts_with('/') || token.starts_with("http:") || token.starts_with("https:") {
            continue; // a route, not a repo path
        }
        if token.starts_with(".next/") || token.starts_with(".vercel/") || token.starts_with(".git/") {
            continue;
        }
        if token.contains('<') || token.contains('>') || token.contains('{') || token.contains('}') {
            continue; // placeholder like services/<domain>/
        }
        let looks_like_file = [
            ".md", ".ts", ".tsx", ".mjs", ".js", ".json", ".sql", ".xml", ".css", ".rs", ".toml",
        ]
        .iter()
        .any(|extension| token.ends_with(extension));
        if !looks_like_file && !token.ends_with('/') {
            continue;
        }
        if seen.insert(token.to_string()) {
            out.push(token.to_string());
        }
    }
    out
}

/// What a citation resolved to. `Ambiguous` is deliberate: two files share the name, and this gate
/// does not get to pick.
pub enum Resolution {
    File(String),
    None,
    Ambiguous,
}

/// Resolves citations against one repo root, with the basename index built once per run.
pub struct Resolver {
    root: PathBuf,
    index: OnceCell<HashMap<String, Vec<String>>>,
}

impl Resolver {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            index: OnceCell::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path_exists(&self, token: &str) -> bool {
        let cleaned = token.trim_end_matches('/');
        if cleaned.contains('*') {
            let (dir, base) = match cleaned.rfind('/') {
                Some(index) => (&cleaned[..index], &cleaned[index + 1..]),
                None => ("", cleaned),
            };
            let pattern = format!(
                "^{}$",
                base.split('*')
                    .map(regex::escape)
                    .collect::<Vec<_>>()
                    .join(".*")
            );
            let Ok(pattern) = Regex::new(&pattern) else {
                return false;
            };
            let Ok(entries) = fs::read_dir(self.root.join(dir)) else {
                return false;
            };
            return entries
                .flatten()
                .any(|entry| pattern.is_match(&entry.file_name().to_string_lossy()));
        }
        let full = self.root.join(cleaned);
        full.is_file() || full.is_dir()
    }

    /// Packets and manifests cite three ways; one resolver handles all three, shared by the manifest
    /// rule and the evidence rule, because two copies measured disagreement between themselves — the
    /// gate reported a path as missing that the generator had just resolved.
    pub fn resolve(&self, path: &str) -> Resolution {
        if self.path_exists(path) {
            return Resolution::File(path.to_string());
        }
        if path.contains('/') {
            let doc_relative = format!("docs/agent/{path}");
            if self.path_exists(&doc_relative) {
                return Resolution::File(doc_relative);
            }
        }
        let basename = path.rsplit('/').next().unwrap_or(path);
        let matches = self
            .basename_index()
            .get(basename)
            .cloned()
            .unwrap_or_default();
        match matches.len() {
            0 => Resolution::None,
            1 => Resolution::File(matches[0].clone()),
            _ => Resolution::Ambiguous,
        }
    }

    /// basename -> repo-relative paths, so a packet's shorthand citation can still be verified.
    fn basename_index(&self) -> &HashMap<String, Vec<String>> {
        self.index.get_or_init(|| {
            let mut index: HashMap<String, Vec<String>> = HashMap::new();
            for root in SOURCE_ROOTS {
                self.walk(&self.root.join(root), 0, &mut index);
            }
            for paths in index.values_mut() {
                paths.sort();
            }
            index
        })
    }

    fn walk(&self, dir: &Path, depth: usize, index: &mut HashMap<String, Vec<String>>) {
        if depth > 10 {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if SKIPPED_DIRS.contains(&name.as_str()) || name.starts_with('.') {
                continue;
            }
            let full = entry.path();
            if full.is_dir() {
                self.walk(&full, depth + 1, index);
                continue;
            }
            if let Ok(relative) = full.strip_prefix(&self.root) {
                index
                    .entry(name)
                    .or_default()
                    .push(relative.to_string_lossy().to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_citation_carries_a_path_and_a_range() {
        let citations = line_citations("see `rust/cli/src/main.rs:70-72` and `a/b.rs:9`");
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].path, "rust/cli/src/main.rs");
        assert_eq!(citations[0].end, 72);
        assert_eq!(citations[1].end, 9);
        assert_eq!(citations[0].raw, "`rust/cli/src/main.rs:70-72`");
    }

    #[test]
    fn prose_without_a_range_is_not_a_citation() {
        assert!(line_citations("see `rust/cli/src/main.rs` for the dispatch").is_empty());
    }

    #[test]
    fn a_trailing_newline_is_not_a_line() {
        assert_eq!(count_lines("a\nb\n"), 2);
        assert_eq!(count_lines("a\nb"), 2);
    }

    #[test]
    fn only_tokens_that_look_like_paths_are_cited() {
        let cited = cited_repo_paths("`forge_decision` and `rust/cli/src/main.rs` and `services/<d>/`");
        assert_eq!(cited, vec!["rust/cli/src/main.rs"]);
        assert_eq!(
            cited_repo_paths("`https://x.test/a.md` and `./rel/thing.md` and `docs/agent/`"),
            vec!["./rel/thing.md", "docs/agent/"]
        );
    }
}
