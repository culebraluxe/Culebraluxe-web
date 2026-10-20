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
///
/// X-5 removed Forge from this repository: a contract whose subject is Forge source reads the checkout
/// [`forge_root`] resolves, never a path under this root.
pub fn workspace_root() -> PathBuf {
    repo_root()
}

/// The Forge checkout, when the suite is running beside one.
///
/// X-5 moved Forge to its own repository (`github.com/culebraluxe/forge`), so Forge source no longer lives in
/// this tree. Resolution order:
///
/// 1. `FORGE_HOME`, when set — an explicit checkout (a Forge worktree while changing Forge itself).
/// 2. The cargo git checkout of the Forge revision this app's `Cargo.lock` pins. That is the Forge source the
///    app is compiled against, so a contract about Forge code reads the same bytes the compiler does; a sibling
///    checkout that has moved on cannot make the app suite assert a different Forge than the one it links.
/// 3. The local cutover layout — a `forge/` directory beside this app checkout.
///
/// `None` means no Forge checkout is available (the app's GitHub CI clone and the Vercel build have none, and
/// they do not build this crate); callers must use [`forge_root_or_skip`] rather than read a missing path, scan
/// nothing, or pass on an empty scan.
///
/// The directory is validated as a Forge workspace (`forge/Cargo.toml`), not merely as an existing directory: a
/// wrong path would make every Forge contract below read nothing and report a clean tree.
pub fn forge_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var("FORGE_HOME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
    {
        return is_forge_checkout(&root).then_some(root);
    }
    if let Some(root) = pinned_git_rev("forge")
        .and_then(|rev| cargo_git_checkout("forge-", &rev, is_forge_checkout))
    {
        return Some(root);
    }
    let sibling = repo_root().parent().map(|parent| parent.join("forge"))?;
    is_forge_checkout(&sibling).then_some(sibling)
}

/// A Forge repository root carries both crates X-5 moved there (`forge/` and `forge-cli/`).
fn is_forge_checkout(root: &Path) -> bool {
    root.join("forge").join("Cargo.toml").is_file()
}

/// The git revision of `package` this workspace pins, read from `Cargo.lock`
/// (`source = "git+<url>?tag=…#<rev>"`).
fn pinned_git_rev(package: &str) -> Option<String> {
    let lock = std::fs::read_to_string(repo_root().join("Cargo.lock")).ok()?;
    let wanted = format!("name = \"{package}\"");
    let mut in_package = false;
    for line in lock.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name = ") {
            in_package = trimmed == wanted;
            continue;
        }
        if !in_package || !trimmed.starts_with("source = ") {
            continue;
        }
        let source = trimmed.trim_start_matches("source = ").trim_matches('"');
        if !source.starts_with("git+") {
            return None;
        }
        let rev = source.rsplit('#').next()?;
        if rev.len() >= 7 && rev.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(rev.to_string());
        }
    }
    None
}

/// Cargo's git checkout of `rev`, under `$CARGO_HOME/git/checkouts/<database-prefix>/<short-rev>/`, validated by
/// `is_checkout_we_want`.
fn cargo_git_checkout(
    database_prefix: &str,
    rev: &str,
    is_checkout_we_want: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let cargo_home = std::env::var("CARGO_HOME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".cargo"))
        })?;
    let checkouts = cargo_home.join("git").join("checkouts");
    for database in std::fs::read_dir(&checkouts).ok()?.flatten() {
        if !database
            .file_name()
            .to_string_lossy()
            .starts_with(database_prefix)
        {
            continue;
        }
        for candidate in std::fs::read_dir(database.path()).ok()?.flatten() {
            let short = candidate.file_name().to_string_lossy().to_string();
            if !rev.starts_with(&short) {
                continue;
            }
            if is_checkout_we_want(&candidate.path()) {
                return Some(candidate.path());
            }
        }
    }
    None
}

/// The workflow-project checkout (W-1): `workflow-core` and `workflow-neon` live in
/// `github.com/culebraluxe/workflow`, pinned by `Cargo.lock`. Unlike Forge there is no `FORGE_HOME`-style override
/// convention: the pinned checkout is the only source.
pub fn workflow_root() -> Option<PathBuf> {
    let rev = pinned_git_rev("workflow-core")?;
    cargo_git_checkout("workflow-", &rev, is_workflow_checkout)
}

/// A workflow-project root carries both crates W-1 moved there under `crates/`.
fn is_workflow_checkout(root: &Path) -> bool {
    root.join("crates")
        .join("workflow-core")
        .join("Cargo.toml")
        .is_file()
}

/// [`workflow_root`], or an explicit, printed skip. The kernel no longer lives in this checkout, so a contract whose
/// subject is the kernel cannot be silently green without the pinned workflow checkout.
pub fn workflow_root_or_skip() -> Option<PathBuf> {
    let root = workflow_root();
    if root.is_none() {
        let test = std::thread::current()
            .name()
            .unwrap_or("(unknown test)")
            .to_string();
        eprintln!(
            "SKIP {test}: no workflow-project checkout in the cargo git cache for the pinned \
             workflow-core revision. Run `cargo fetch`/a build so the pinned checkout exists; the kernel's subject \
             lives in github.com/culebraluxe/workflow."
        );
    }
    root
}

/// [`forge_root`], or an explicit, printed skip.
///
/// A Forge-source contract is not green without a Forge checkout, and it must not be a silent pass: this says
/// which test did nothing and why. The Forge repository (and Forge CI, which checks out this app as
/// `FORGE_TARGET_REPO`) runs the same contracts with a checkout present.
pub fn forge_root_or_skip() -> Option<PathBuf> {
    let root = forge_root();
    if root.is_none() {
        let test = std::thread::current()
            .name()
            .unwrap_or("(unknown test)")
            .to_string();
        eprintln!(
            "SKIP {test}: no Forge checkout. Set FORGE_HOME (or keep a `forge` sibling of this checkout) to \
             run this Forge-source contract; the subject lives in github.com/culebraluxe/forge."
        );
    }
    root
}

/// Resolve a path this suite spells from the repository root: app paths from this checkout, `forge/…` and
/// `forge-cli/…` paths from the Forge checkout (the two crates X-5 moved there). Panics when a Forge path is
/// requested without a checkout — call [`forge_root_or_skip`] first, so the absence is a named skip rather than
/// a false finding.
pub fn repo_path(relative: &str) -> PathBuf {
    if forges_path(relative) {
        return forge_root()
            .map(|root| root.join(relative))
            .unwrap_or_else(|| {
                panic!(
                    "{relative} resolves into the Forge checkout, and no Forge checkout is present: set \
                     FORGE_HOME or keep a `forge` sibling. Call forge_root_or_skip first to make this a skip."
                )
            });
    }
    repo_root().join(relative)
}

/// Whether a repository-relative path names a crate in the Forge checkout (`forge/`, `forge-cli/`).
fn forges_path(relative: &str) -> bool {
    relative.starts_with("forge/") || relative.starts_with("forge-cli/")
}

/// A path as the repository writes it — relative to the root, for failure messages a reader can act on.
pub fn relative(path: &Path) -> String {
    relative_to(&repo_root(), path)
}

/// [`relative`], against an explicit root: the app checkout or the Forge checkout, whichever owns `path`.
pub fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Every `.rs` file under a repository-relative directory, as (repository-relative path, path on disk).
///
/// The directory is resolved the way the contracts spell it: a plain path under this app checkout, or a
/// `forge/…` path into the Forge checkout. A Forge file keeps the `forge/` prefix it had before X-5, so a pin or
/// exemption written against the old one-checkout layout still names the same file. Callers that name a Forge
/// directory must resolve [`forge_root_or_skip`] first; the panic here is the programming-error backstop.
pub fn sources_in(relative_dir: &str) -> Vec<(String, PathBuf)> {
    if forges_path(relative_dir) {
        let root = forge_root().unwrap_or_else(|| {
            panic!(
                "{relative_dir} resolves into the Forge checkout, and no Forge checkout is present: set \
                 FORGE_HOME or keep a `forge` sibling. Call forge_root_or_skip first to make this a skip."
            )
        });
        let dir = root.join(relative_dir);
        return sources_under(&dir)
            .into_iter()
            .map(|path| (relative_to(&root, &path), path))
            .collect();
    }
    let dir = repo_root().join(relative_dir);
    sources_under(&dir)
        .into_iter()
        .map(|path| (relative(&path), path))
        .collect()
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
