//! THE DEAD-TYPESCRIPT LOADER SWEEP, in Rust — the health metric behind
//! `docs/agent/BROKEN-TS-INVENTORY.md`.
//!
//! Rust replacement for the retired `scripts/broken-ts-sweep.mjs` (behind `pnpm broken:ts:sweep`).
//! Same question, same two classes, same output shape, same exit contract; there is no Node runtime
//! left in this repository, so the gate is Rust now.
//!
//! It answers ONE question by static resolution, with no database and no network: which files in this
//! repository cannot load because a module they import is gone?
//!
//! Two classes, because the distinction is real and was measured the hard way:
//!   CANNOT LOAD — a value import (directly, or through another broken file) resolves to nothing.
//!                 `import type` is deliberately ignored: tsx erases it, so it cannot break loading.
//!   CANNOT WORK — the file loads, but a module it imports lazily is gone, so the code path it needs
//!                 does not exist. It is still broken; it is not the same sentence.
//!
//! Exits 1 when a file is broken but unmarked, or marked but loadable — i.e. when the inventory in
//! `docs/agent/BROKEN-TS-INVENTORY.md` has drifted from the tree. Exits 0 when the two agree.
//!
//! ONE DELIBERATE CHANGE FROM THE TYPESCRIPT: the drift lists are printed sorted by path. The
//! TypeScript printed them in `readdir` order — the filesystem's — so the same tree could print its
//! findings in a different order on two machines, and a gate whose output is not stable cannot be
//! diffed.
//!
//! Usage:
//!   cargo run -p cli -- forge ts-sweep [roots...]     (default roots: scripts agent-runtime)

use crate::forge::{repo_root, Failure};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const DEFAULT_ROOTS: [&str; 2] = ["scripts", "agent-runtime"];

/// Resolution suffixes, in the order Node would try them: a bare file first, then a directory index.
const EXTS: [&str; 8] = ["", ".ts", ".tsx", ".mts", ".mjs", ".js", ".jsx", ".json"];

/// The retirement banner — ASSEMBLED, never written literally in one piece. A file containing the
/// literal would mark itself as bannered, which is exactly what this tool did to itself on its first
/// run in TypeScript.
pub fn marker() -> String {
    format!("{} {} {}", "//", "⚠", "BROKEN ON PURPOSE")
}

fn import_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?m)(?:^|[^.\w])import\s+(?:type\s+)?(?:[\s\S]*?)\s*from\s*['"]([^'"]+)['"]|^\s*import\s+['"]([^'"]+)['"]"#,
        )
        .expect("the import pattern is valid")
    })
}

fn dynamic_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"\bimport\(\s*['"]([^'"]+)['"]\s*\)"#).expect("pattern is valid")
    })
}

fn type_only_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\s*import\s+type\s").expect("pattern is valid"))
}

fn source_ext_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\.(?:ts|mts|mjs|cjs)$").expect("pattern is valid"))
}

/// Where a specifier points. `External` is a runtime builtin or an installed package: it cannot break.
#[derive(Debug, PartialEq, Eq)]
pub enum Resolution {
    External,
    File(PathBuf),
    Missing,
}

/// A specifier and the line of the file it was found on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    pub spec: String,
    pub line: usize,
}

fn with_ext(base: &Path, ext: &str) -> PathBuf {
    let mut raw = base.as_os_str().to_os_string();
    raw.push(ext);
    PathBuf::from(raw)
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Resolve one specifier the way Node's loader would, for the purpose of asking whether it exists.
pub fn resolve_spec(root: &Path, spec: &str, from_dir: &Path) -> Resolution {
    if spec.starts_with("node:") || spec.starts_with("bun:") {
        return Resolution::External;
    }
    let base = if let Some(rest) = spec.strip_prefix("@/") {
        root.join(rest)
    } else if spec.starts_with('.') {
        from_dir.join(spec)
    } else {
        let package = if spec.starts_with('@') {
            spec.split('/').take(2).collect::<Vec<_>>().join("/")
        } else {
            spec.split('/').next().unwrap_or_default().to_string()
        };
        if root.join("node_modules").join(&package).exists() {
            return Resolution::External;
        }
        root.join(spec)
    };
    for ext in EXTS {
        let candidate = with_ext(&base, ext);
        if candidate.is_file() {
            return Resolution::File(canonical(&candidate));
        }
    }
    for ext in &EXTS[1..] {
        let candidate = base.join(format!("index{ext}"));
        if candidate.is_file() {
            return Resolution::File(canonical(&candidate));
        }
    }
    Resolution::Missing
}

/// Every value import in a file, with the line it was found on. `import type` is skipped: tsx erases
/// it, so it can never be the reason a file cannot load.
pub fn value_specs(text: &str) -> Vec<Spec> {
    let mut out = Vec::new();
    for captures in import_re().captures_iter(text) {
        let whole = captures.get(0).expect("group 0 always exists");
        let spec = captures
            .get(1)
            .or_else(|| captures.get(2))
            .map(|group| group.as_str())
            .unwrap_or_default();
        if spec.is_empty() {
            continue;
        }
        let line_start = text[..whole.start()].rfind('\n').map_or(0, |at| at + 1);
        let statement_end = text[whole.end()..]
            .find('\n')
            .map_or(text.len(), |offset| whole.end() + offset);
        if type_only_re().is_match(&text[line_start..statement_end]) {
            continue;
        }
        let line = text[..whole.start()].matches('\n').count() + 1;
        out.push(Spec {
            spec: spec.to_string(),
            line,
        });
    }
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        if is_dir {
            if name == "node_modules" || name.starts_with('.') {
                continue;
            }
            walk(&path, out);
        } else if source_ext_re().is_match(&name) {
            out.push(canonical(&path));
        }
    }
}

/// Recursive, memoised: a file is dead if any value import of it is missing or dead. A cycle in the
/// graph is not death — `visiting` answers "not dead" and each caller is judged on its own imports.
fn analyse(
    root: &Path,
    file: &Path,
    dead: &mut HashMap<PathBuf, String>,
    alive: &mut HashSet<PathBuf>,
    visiting: &mut HashSet<PathBuf>,
) -> bool {
    if alive.contains(file) {
        return false;
    }
    if dead.contains_key(file) {
        return true;
    }
    if visiting.contains(file) {
        return false;
    }
    visiting.insert(file.to_path_buf());
    let Ok(text) = fs::read_to_string(file) else {
        visiting.remove(file);
        alive.insert(file.to_path_buf());
        return false;
    };
    let from_dir = file.parent().unwrap_or(root);
    for Spec { spec, line } in value_specs(&text) {
        match resolve_spec(root, &spec, from_dir) {
            Resolution::External => continue,
            Resolution::Missing => {
                visiting.remove(file);
                dead.insert(
                    file.to_path_buf(),
                    format!("missing module '{spec}' (line {line})"),
                );
                return true;
            }
            Resolution::File(target) => {
                let target = canonical(&target);
                if analyse(root, &target, dead, alive, visiting) {
                    visiting.remove(file);
                    dead.insert(
                        file.to_path_buf(),
                        format!(
                            "imports dead '{spec}' -> {} (line {line})",
                            target.strip_prefix(root).unwrap_or(&target).display()
                        ),
                    );
                    return true;
                }
            }
        }
    }
    visiting.remove(file);
    alive.insert(file.to_path_buf());
    false
}

/// The answer, in the shape the report needs. Collected without printing, so it can be asserted on.
pub struct Report {
    pub root: PathBuf,
    pub scanned: Vec<PathBuf>,
    pub dead: Vec<(PathBuf, String)>,
    pub runtime: Vec<(PathBuf, String)>,
    pub marked: Vec<PathBuf>,
    pub unmarked: Vec<(PathBuf, String)>,
    pub overmarked: Vec<PathBuf>,
}

impl Report {
    pub fn collect(root: &Path, roots: &[String]) -> Self {
        let root = &canonical(root);
        let mut scanned = Vec::new();
        for name in roots {
            let dir = root.join(name);
            if dir.exists() {
                walk(&dir, &mut scanned);
            }
        }
        let mut dead: HashMap<PathBuf, String> = HashMap::new();
        let mut alive: HashSet<PathBuf> = HashSet::new();
        let mut visiting: HashSet<PathBuf> = HashSet::new();
        for file in &scanned {
            analyse(root, file, &mut dead, &mut alive, &mut visiting);
        }

        // The second class: the file loads, but the module it reaches for lazily is gone.
        let mut runtime: Vec<(PathBuf, String)> = Vec::new();
        for file in &scanned {
            if dead.contains_key(file) {
                continue;
            }
            let Ok(text) = fs::read_to_string(file) else {
                continue;
            };
            let from_dir = file.parent().unwrap_or(root);
            for captures in dynamic_re().captures_iter(&text) {
                let spec = captures
                    .get(1)
                    .map(|group| group.as_str())
                    .unwrap_or_default();
                if spec.is_empty() {
                    continue;
                }
                let gone = match resolve_spec(root, spec, from_dir) {
                    Resolution::External => continue,
                    Resolution::Missing => true,
                    Resolution::File(target) => dead.contains_key(&target),
                };
                if gone {
                    runtime.push((file.clone(), format!("dynamic import '{spec}' is gone")));
                    break;
                }
            }
        }

        let mut marked: Vec<PathBuf> = Vec::new();
        for file in &scanned {
            if let Ok(text) = fs::read_to_string(file) {
                if text.contains(&marker()) {
                    marked.push(file.clone());
                }
            }
        }

        let reason_of = |file: &Path| -> Option<String> {
            dead.get(file)
                .or_else(|| {
                    runtime
                        .iter()
                        .find(|(path, _)| path == file)
                        .map(|(_, why)| why)
                })
                .cloned()
        };
        let mut unmarked: Vec<(PathBuf, String)> = Vec::new();
        let mut overmarked: Vec<PathBuf> = Vec::new();
        for file in &scanned {
            match (reason_of(file), marked.contains(file)) {
                (Some(why), false) => unmarked.push((file.clone(), why)),
                (None, true) => overmarked.push(file.clone()),
                _ => {}
            }
        }

        let mut dead: Vec<(PathBuf, String)> = dead.into_iter().collect();
        dead.sort();
        let mut runtime = runtime;
        runtime.sort();
        let mut marked = marked;
        marked.sort();
        unmarked.sort();
        let mut overmarked = overmarked;
        overmarked.sort();

        Self {
            root: root.to_path_buf(),
            scanned,
            dead,
            runtime,
            marked,
            unmarked,
            overmarked,
        }
    }

    fn relative(&self, file: &Path) -> String {
        file.strip_prefix(&self.root)
            .unwrap_or(file)
            .display()
            .to_string()
    }

    pub fn drifted(&self) -> bool {
        !self.unmarked.is_empty() || !self.overmarked.is_empty()
    }
}

pub fn run(args: &[String]) -> Result<u8, Failure> {
    let mut roots: Vec<String> = Vec::new();
    for arg in args {
        if arg.starts_with('-') {
            return Err(Failure::usage(format!(
                "unknown argument `{arg}`; usage: forge ts-sweep [roots...]"
            )));
        }
        roots.push(arg.clone());
    }
    if roots.is_empty() {
        roots = DEFAULT_ROOTS.iter().map(|root| root.to_string()).collect();
    }

    let report = Report::collect(&repo_root(), &roots);

    println!(
        "scanned                        : {} files",
        report.scanned.len()
    );
    println!("cannot load                    : {}", report.dead.len());
    println!("loads, but lazy target gone     : {}", report.runtime.len());
    println!("marked \"{}\"    : {}", marker(), report.marked.len());

    if !report.unmarked.is_empty() {
        println!("\nDRIFT — broken but NOT marked:");
        for (file, why) in &report.unmarked {
            println!("  {}  <- {why}", report.relative(file));
        }
    }
    if !report.overmarked.is_empty() {
        println!("\nDRIFT — marked but loads fine:");
        for file in &report.overmarked {
            println!("  {}", report.relative(file));
        }
    }
    if !report.drifted() {
        println!("\nthe tree and the inventory agree");
        return Ok(0);
    }
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scratch inside the test, removed inside the test: nothing outlives the run.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("culebraluxe-ts-sweep-{name}"));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("scratch directory");
            Self(dir)
        }

        fn write(&self, relative: &str, body: &str) {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("scratch parent");
            }
            fs::write(&path, body).expect("scratch file");
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn report(&self) -> Report {
            let roots = DEFAULT_ROOTS
                .iter()
                .map(|root| root.to_string())
                .collect::<Vec<_>>();
            Report::collect(self.path(), &roots)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn why<'a>(report: &'a Report, relative: &str) -> Option<&'a String> {
        let wanted = canonical(&report.root.join(relative));
        report
            .dead
            .iter()
            .chain(report.runtime.iter())
            .find(|(path, _)| canonical(path) == wanted)
            .map(|(_, why)| why)
    }

    #[test]
    fn a_value_import_of_a_vanished_module_cannot_load() {
        let scratch = Scratch::new("missing");
        scratch.write(
            "scripts/a.mjs",
            "import x from './gone.mjs'\nexport default x\n",
        );

        let report = scratch.report();

        assert_eq!(report.dead.len(), 1, "{:?}", report.dead);
        let reason = why(&report, "scripts/a.mjs").expect("a.mjs is dead");
        assert!(reason.contains("missing module './gone.mjs'"), "{reason}");
        assert!(reason.contains("(line 1)"), "{reason}");
        assert!(report.drifted(), "and nothing marks it, so the gate fails");
    }

    #[test]
    fn a_file_that_imports_a_dead_file_cannot_load_either() {
        let scratch = Scratch::new("transitive");
        scratch.write("scripts/a.mjs", "import x from './gone.mjs'\n");
        scratch.write("scripts/b.mjs", "import a from './a.mjs'\n");

        let report = scratch.report();

        let reason = why(&report, "scripts/b.mjs").expect("b.mjs is dead");
        assert!(reason.starts_with("imports dead './a.mjs'"), "{reason}");
        assert!(reason.contains("scripts/a.mjs"), "{reason}");
    }

    #[test]
    fn import_type_cannot_break_loading_and_a_side_effect_import_is_a_value_import() {
        let scratch = Scratch::new("type-only");
        scratch.write("scripts/real.mjs", "export const real = 1\n");
        scratch.write(
            "scripts/a.mjs",
            "import type { Shape } from './gone'\nimport './real.mjs'\n",
        );

        let report = scratch.report();

        assert_eq!(report.dead.len(), 0, "{:?}", report.dead);
        assert_eq!(report.runtime.len(), 0, "{:?}", report.runtime);
        assert!(!report.drifted());
    }

    #[test]
    fn a_lazy_import_of_a_vanished_module_loads_but_cannot_work() {
        let scratch = Scratch::new("lazy");
        scratch.write(
            "scripts/a.mjs",
            "export async function go() {\n  const m = await import('./gone.mjs')\n  return m\n}\n",
        );

        let report = scratch.report();

        assert_eq!(report.dead.len(), 0, "{:?}", report.dead);
        assert_eq!(
            why(&report, "scripts/a.mjs").map(String::as_str),
            Some("dynamic import './gone.mjs' is gone")
        );
        // It is still broken, so a marker is still demanded for it.
        assert_eq!(report.unmarked.len(), 1);
    }

    #[test]
    fn a_marked_file_that_loads_fine_is_drift_in_the_other_direction() {
        let scratch = Scratch::new("overmarked");
        scratch.write("scripts/healthy.mjs", "export const healthy = 1\n");
        scratch.write(
            "scripts/marked.mjs",
            &format!("{}\nexport const x = 1\n", marker()),
        );

        let report = scratch.report();

        assert_eq!(report.marked.len(), 1);
        assert_eq!(report.overmarked.len(), 1, "{:?}", report.overmarked);
        assert_eq!(report.unmarked.len(), 0);
        assert!(report.drifted());
    }

    #[test]
    fn node_modules_dot_directories_and_other_extensions_are_not_scanned() {
        let scratch = Scratch::new("scope");
        scratch.write("scripts/node_modules/dep/index.mjs", "import 'gone'\n");
        scratch.write("scripts/.hidden/a.mjs", "import 'gone'\n");
        scratch.write("scripts/notes.md", "import 'gone'\n");
        scratch.write("scripts/keepers.mjs", "export const keep = 1\n");

        let report = scratch.report();

        assert_eq!(report.scanned.len(), 1, "{:?}", report.scanned);
        assert_eq!(report.dead.len(), 0, "{:?}", report.dead);
    }

    #[test]
    fn a_specifier_finds_the_file_the_index_and_the_alias() {
        let scratch = Scratch::new("resolve");
        scratch.write("scripts/pkg.ts", "export const pkg = 1\n");
        scratch.write("scripts/pkg/index.ts", "export const index = 1\n");
        scratch.write("scripts/from.mjs", "export const from = 1\n");
        scratch.write("lib/deep.mjs", "export const deep = 1\n");
        let root = scratch.path();
        let scripts = root.join("scripts");

        match resolve_spec(root, "./pkg", &scripts) {
            Resolution::File(found) => assert!(found.ends_with("pkg.ts"), "{found:?}"),
            other => panic!("expected the file, got {other:?}"),
        }
        match resolve_spec(root, "./from", &scripts) {
            Resolution::File(found) => assert!(found.ends_with("from.mjs"), "{found:?}"),
            other => panic!("expected the file, got {other:?}"),
        }
        match resolve_spec(root, "@/lib/deep.mjs", &scripts) {
            Resolution::File(found) => assert!(found.ends_with("deep.mjs"), "{found:?}"),
            other => panic!("expected the alias target, got {other:?}"),
        }
        assert_eq!(
            resolve_spec(root, "node:fs", &scripts),
            Resolution::External
        );
        assert_eq!(
            resolve_spec(root, "./nowhere", &scripts),
            Resolution::Missing
        );
    }

    /// The bug this test exists for: the marker is assembled from parts, because the first run of the
    /// literal version found the literal inside this tool and declared the tool bannered.
    #[test]
    fn the_marker_is_the_banner_the_inventory_documents() {
        assert!(marker().contains("BROKEN ON PURPOSE"));
        assert!(marker().starts_with("//"));
    }
}
