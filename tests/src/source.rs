//! Source-tree contracts: the helpers for tests whose subject is the repository itself.
//!
//! Some architectural contracts are not a function's behaviour but the shape of the tree — "a screen never opens a
//! connection", "the UI cannot link the database crate", "the workflow kernel does not know a domain noun". Those are
//! proven by reading the sources, and the reading has to be as honest as any other assertion in this harness: a
//! walker that silently found no files would report a clean tree and pass. Every caller is therefore expected to
//! assert a floor on [`sources_under`]'s length (and its own detector against a planted sample), so a broken path
//! fails the test instead of flattering it.
//!
//! Nothing here is a second implementation of production behaviour: these functions read files and compare text.

use std::path::{Path, PathBuf};

/// The repository root — which is also the workspace's root, and now the directory this suite's manifest sits in.
/// Every Rust crate is a tier directory under it (`web/`, `middle/`, `db/`) or an entry point beside them (`cli/`,
/// `forge/`), and the tests are in `tests/`.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the suite lives in tests/, one level below the repository root")
        .to_path_buf()
}

/// Where the Rust sources are: the tier directories and the entry points, i.e. the repository root itself.
///
/// This used to be `rust/`, the single directory the whole workspace lived in. The walkers below skip build output,
/// VCS metadata and `node_modules`, and nothing else in the tree holds a `.rs` file, so the root is the honest bound.
pub fn workspace_root() -> PathBuf {
    repo_root()
}

/// A path as the repository writes it — relative to the root, for failure messages a reader can act on.
pub fn relative(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Every `.rs` file under `dir`, depth-first and **sorted**, skipping build output and VCS metadata.
///
/// Sorted so a failure lists the same findings in the same order on every machine.
pub fn sources_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(dir, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), "target" | "node_modules" | ".git" | "dist") {
                continue;
            }
            collect(&path, out);
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// The text of `path`, failing loudly rather than skipping a file this contract is supposed to inspect.
pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} must be readable: {error}", relative(path)))
}

/// Whole-word containment, so `selected` is not `select` and `update` is not `updated_at`.
pub fn contains_word(haystack: &str, needle: &str) -> bool {
    contains_at_boundary(haystack, needle, false)
}

/// Identifier-segment containment: `property_id` names `property`, but `propertied` does not.
///
/// The stricter form of [`contains_word`], for contracts about a *name* rather than about a word: a business noun
/// hidden inside a snake_case identifier is still the noun being named, while a longer English word merely containing
/// it is not.
pub fn contains_segment(haystack: &str, needle: &str) -> bool {
    contains_at_boundary(haystack, needle, true)
}

fn contains_at_boundary(haystack: &str, needle: &str, underscore_is_boundary: bool) -> bool {
    let is_boundary = |c: char| !c.is_alphanumeric() && (underscore_is_boundary || c != '_');
    haystack.match_indices(needle).any(|(start, _)| {
        let before = haystack[..start]
            .chars()
            .next_back()
            .map(is_boundary)
            .unwrap_or(true);
        let after = haystack[start + needle.len()..]
            .chars()
            .next()
            .map(is_boundary)
            .unwrap_or(true);
        before && after
    })
}

/// The code part of a source line: everything before a `//` comment.
///
/// The tests must not read a sentence about `select` as a statement, so every content check starts here.
pub fn code_of(line: &str) -> &str {
    line.split("//").next().unwrap_or("")
}

/// The keys a `Cargo.toml` names inside `table`, e.g. `manifest_keys(text, "dependencies")`.
///
/// A key is the text left of the first `=`, exactly as written — `db = { path = "../core/db" }` yields `db`, and a
/// dotted key yields its dotted path (`serde.workspace`), which [`crate_name_of`] reduces to the crate the manifest
/// actually names. A keyless continuation line (a feature list spread over lines) is returned whole, which can only
/// add a harmless string to the result. The caller asserts the table it expected to find was found.
pub fn manifest_keys(manifest: &str, table: &str) -> Vec<String> {
    let header = format!("[{table}]");
    let mut keys = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == header;
            continue;
        }
        if !inside || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let key = trimmed.split('=').next().unwrap_or(trimmed).trim();
        keys.push(key.to_string());
    }
    keys
}

/// The crate name a `Cargo.toml` entry refers to: `db` for `db = ...`, `db` for `db = { path = "../core/db" }`.
pub fn crate_name_of(entry: &str) -> String {
    entry
        .split(['=', '.', ' '])
        .next()
        .unwrap_or(entry)
        .trim()
        .to_string()
}

/// The body of a free function `fn name(…) { … }`, as the lines from the signature to the closing
/// brace at the signature's indentation.
///
/// The signature may span lines until the `{` that opens the body; the body ends at the first line
/// whose code is exactly `}` at the signature's indentation. A trait declaration (`fn name(…) …;`)
/// has no body and is refused rather than returned as an empty one.
pub fn fn_body(text: &str, name: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| code_of(line).contains(&format!("fn {name}(")))
        .unwrap_or_else(|| panic!("fn {name} must exist"));
    body_from(&lines, start)
}

/// The body of a method `fn name(…) { … }` inside the `impl … impl_type …` block.
///
/// Anchoring on the impl block matters where a method is declared in a trait, defined in the DAO's
/// impl and again in the service's impl: the service's definition is the one that authorizes, and it
/// is the one this reads rather than the DAO's pass-through.
pub fn impl_body(text: &str, impl_type: &str, name: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let impl_start = lines
        .iter()
        .position(|line| {
            let code = code_of(line);
            code.starts_with("impl") && code.contains(impl_type)
        })
        .unwrap_or_else(|| panic!("the impl of {impl_type} must exist"));
    let mut index = impl_start;
    while index < lines.len() {
        if code_of(lines[index]).contains(&format!("fn {name}(")) {
            return body_from(&lines, index);
        }
        index += 1;
    }
    panic!("fn {name} must exist in the impl of {impl_type}")
}

/// The lines of the function whose signature starts at `start`, through the closing brace.
fn body_from(lines: &[&str], start: usize) -> String {
    // The signature may span lines until the `{` that opens the body.
    let mut cursor = start;
    let mut signature = String::new();
    loop {
        let code = code_of(lines[cursor]);
        signature.push_str(code);
        if signature.contains('{') || code.contains(';') {
            break;
        }
        cursor += 1;
    }
    assert!(
        signature.contains('{'),
        "fn at line {} must be a definition with a body",
        start + 1
    );
    let indent = lines[start]
        .chars()
        .take_while(|character| *character == ' ')
        .count();
    let closing = format!("{}}}", " ".repeat(indent));
    let mut body = String::new();
    for line in lines.iter().skip(start) {
        body.push_str(line);
        body.push('\n');
        if code_of(line) == closing {
            return body;
        }
    }
    panic!("fn at line {} must have a closing brace", start + 1)
}

/// The `.route(…)` block a path is wired to: the lines from the path literal to the closing `)`.
///
/// The block is the unit a route assertion reads — the path and the handler that serves it are
/// wired together in one `.route(…)` call, and this returns that call's lines.
pub fn route_block(routes: &str, path: &str) -> String {
    let lines: Vec<&str> = routes.lines().collect();
    let start = lines
        .iter()
        .position(|line| code_of(line).contains(path))
        .unwrap_or_else(|| panic!("the route {path} must exist"));
    let mut block = String::new();
    for line in lines.iter().skip(start) {
        let code = code_of(line);
        block.push_str(line);
        block.push('\n');
        if code.trim() == ")" {
            break;
        }
    }
    block
}
