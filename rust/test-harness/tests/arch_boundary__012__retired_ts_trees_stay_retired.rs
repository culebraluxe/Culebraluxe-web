//! ARCH.BOUNDARY — the retired TypeScript trees stay retired (TST-ARCH-BOUNDARY-012).
//!
//! Contract, in two halves, because either alone can be satisfied while the retirement is broken:
//!
//!   1. `legacy/agent-runtime`, `legacy/services` and `legacy/lib` are not in the tree. They held 45 files and
//!      6,643 lines, every one of them a `*.test.ts` for code that was already deleted — orphaned tests that
//!      could not load (`docs/agent/BROKEN-TS-INVENTORY.md`), 372 of the `tsc` error sites
//!      (`pnpm typecheck`), and that no gate even scanned. Removed 2026-10-02; the record is
//!      `docs/agent/DEAD-TS-DOWNSIZE.md` §5, which authorized exactly this as "one `git rm`".
//!   2. No live JavaScript or TypeScript imports anything from under `legacy/`. The engine was dropped because
//!      agents re-integrated live production code with the retired TypeScript libraries, and "wiring live code
//!      to it is not [allowed]" is the rule that came out of it (`BROKEN-TS-INVENTORY.md`). Nothing enforced
//!      that rule: `eslint.config.mjs` names the paths in a banned-import group, and this repository's own
//!      `.github/workflows/gates.yml:7-8` says an unenforced rule is not a rule. This is the enforcement.
//!
//! The import detector is shape-based, not a bare string search: `eslint.config.mjs` mentions `@/legacy/*` in
//! order to forbid it, and a rule naming a path is not an import of it. Only `from '…'`, `import('…')` and
//! `require('…')` count.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test arch_boundary__012__retired_ts_trees_stay_retired

use std::path::{Path, PathBuf};

use test_harness::source;

/// The three trees removed on 2026-10-02. A path is retired as a *property*, not as an event: if one comes back,
/// this fails and the reader is sent to the page that records what it was and why it went.
const RETIRED: [&str; 3] = ["legacy/agent-runtime", "legacy/services", "legacy/lib"];

/// The extensions a file in this repository can import something from. `.mjs` and `.js` are included because the
/// live JavaScript here is exactly the shell-and-agent scripts (`scripts/*.mjs`, `e2e/*.mjs`) plus
/// `eslint.config.mjs` — the files most likely to reach for a deleted helper.
const SCRIPT_EXTS: [&str; 6] = ["ts", "tsx", "mts", "mjs", "js", "jsx"];

/// A floor on the walk, not a census: today the tree holds five such files (three `scripts/*.mjs`, one
/// `e2e/*.mjs`, `eslint.config.mjs`). Deleting one of them must not read as a broken walker, while a walker that
/// found nothing — an empty root, a wrong path — fails instead of reporting a clean tree.
const LIVE_SCRIPT_FLOOR: usize = 4;

/// Every live script in the repository: `repo_root()` depth-first, skipping build output, VCS metadata and the
/// retired tree itself (`legacy/`, whose files are the *subject* of the retirement, not callers of it).
fn live_scripts_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                "node_modules" | ".git" | ".next" | "dist" | "target" | "legacy"
            ) {
                continue;
            }
            live_scripts_under(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if SCRIPT_EXTS.contains(&ext) {
                out.push(path);
            }
        }
    }
}

/// The specifier of an import that reaches under `legacy/`, if this line carries one.
fn legacy_import(line: &str) -> Option<String> {
    let code = source::code_of(line);
    for opening in [
        "from '",
        "from \"",
        "import('",
        "import(\"",
        "require('",
        "require(\"",
    ] {
        let Some(start) = code.find(opening) else {
            continue;
        };
        let rest = &code[start + opening.len()..];
        let quote = opening
            .chars()
            .next_back()
            .expect("the opening carries its quote");
        let Some(end) = rest.find(quote) else {
            continue;
        };
        let spec = &rest[..end];
        if spec.contains("legacy/") {
            return Some(spec.to_string());
        }
    }
    None
}

#[test]
fn the_retired_trees_are_gone() {
    for retired in RETIRED {
        let path = source::repo_root().join(retired);
        assert!(
            !path.exists(),
            "{retired} is back on disk; it was retired on 2026-10-02 (docs/agent/DEAD-TS-DOWNSIZE.md §5) — \
             restore it with `git show <commit>^:{retired}` if the intent is wanted again, but do not resurrect \
             it as live code"
        );
    }
}

#[test]
fn no_live_script_imports_a_retired_tree() {
    let mut files = Vec::new();
    live_scripts_under(&source::repo_root(), &mut files);
    files.sort();
    assert!(
        files.len() >= LIVE_SCRIPT_FLOOR,
        "the walk found {} live scripts (floor {LIVE_SCRIPT_FLOOR}); a walker that sees nothing reports a clean \
         tree, so this contract would pass while proving nothing",
        files.len()
    );

    let mut offenders: Vec<String> = Vec::new();
    for path in &files {
        for (index, line) in source::read(path).lines().enumerate() {
            if let Some(spec) = legacy_import(line) {
                offenders.push(format!(
                    "{}:{} imports {spec}",
                    source::relative(path),
                    index + 1
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "live code imports the retired TypeScript: {offenders:#?}. The TypeScript engine was dropped because \
         live code was wired back into its dead libraries; translate the behaviour into Rust instead."
    );

    // The detector, against planted samples — a scan that cannot fail cannot pass. Each shape a real import takes,
    // and the near-miss that must not count: the banned-import *rule* in `eslint.config.mjs` names these paths, and
    // naming a path in order to forbid it is not importing it.
    for (sample, expected) in [
        ("import { x } from '@/legacy/lib/y'", true),
        (
            "export { y } from \"../legacy/services/regrid/client\"",
            true,
        ),
        (
            "const m = await import('./legacy/agent-runtime/factory')",
            true,
        ),
        ("const r = require(\"@/legacy/db/workflow-trace\")", true),
        ("import { x } from '@/lib/worker-workspace'", false),
        ("group: ['@/legacy/*', '**/legacy/services/*']", false),
        ("// legacy/agent-runtime is documented, not imported", false),
    ] {
        assert_eq!(
            legacy_import(sample).is_some(),
            expected,
            "the detector read {sample:?} as the wrong shape"
        );
    }
}
