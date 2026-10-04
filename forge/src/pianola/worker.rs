//! TST story inspection and test placement decision logic.
//!
//! A TST worker inspects one story's canonical fields, matches the
//! neighboring Rust test convention, and decides where the authored test
//! belongs. It never invents a new taxonomy: placement follows the target
//! path plus the neighboring test files.
//!
//! Lane posture: lanes are the captain-exception (2026-10-01) disposable
//! per-story Git worktrees. Neon (`storyboard_story`, `storyboard_story_run`,
//! `forge_tool_artifact`) remains the only workflow authority; a lane holds
//! code, commits locally, never pushes, and is removed when the run ends.
//! This reconciles the AGENTS.md trunk rule (work lands on `main`, short-lived
//! branches only) with the lane-isolation experiment: the lane is a sandbox,
//! not a second workflow.
//!
//! Path note: the playbook names this file `rust/forge/src/pianola/...`.
//! The workspace root is the repo root (`members = ["forge", ...]`), so the
//! canonical path is `forge/src/pianola/worker.rs`.
//!
//! Schema note: `StoryPacketRow` (`db::forge_engine`) carries `id, title,
//! goal, architect_brief, acceptance_criteria, test_mode, assay_commands`.
//! The `story_packet()` query folds `scope`, `dependencies`, `preconditions`,
//! `context_refs`, `operating_surface`, and `postconditions` into the
//! `architect_brief` text as `SCOPE:` / `OPERATING SURFACE:` sections rather
//! than separate columns. `TstStoryView` therefore parses `scope` and
//! `operating_surface` back out of those sections instead of guessing a
//! schema the DAO does not expose.

use std::path::{Path, PathBuf};

use db::StoryPacketRow;

use super::escalation::EscalationReason;
use crate::engine::packet::StoryPacket;

// ---------------------------------------------------------------------------
// Story view
// ---------------------------------------------------------------------------

/// The canonical fields of one TST story as the worker sees them.
///
/// Built from a [`StoryPacketRow`] (no DB round trip) or from Neon via
/// [`StoryPacket::load_from_neon`] (which itself uses
/// `ForgeEngineDao::story_packet`).
#[derive(Debug, Clone, Default)]
pub struct TstStoryView {
    pub id: String,
    pub title: String,
    pub goal: Option<String>,
    pub architect_brief: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub assay_commands: Vec<String>,
    pub test_mode: Option<String>,
    /// Parsed out of the `SCOPE:` section the `story_packet()` query folds
    /// into `architect_brief`. `None` when the packet carries no scope.
    pub scope: Option<String>,
    /// Parsed out of the `OPERATING SURFACE:` section folded into
    /// `architect_brief`. `None` when the packet carries none.
    pub operating_surface: Option<String>,
}

impl TstStoryView {
    /// Build the view from an already-loaded packet row. Pure: no I/O.
    pub fn from_row(row: &StoryPacketRow) -> Self {
        let brief = row.architect_brief.clone().filter(|v| !v.trim().is_empty());
        let scope = brief
            .as_deref()
            .and_then(|b| extract_section(b, "SCOPE:"));
        let operating_surface = brief
            .as_deref()
            .and_then(|b| extract_section(b, "OPERATING SURFACE:"));
        Self {
            id: row.id.clone(),
            title: row.title.clone(),
            goal: row.goal.clone().filter(|v| !v.trim().is_empty()),
            architect_brief: brief,
            acceptance_criteria: row
                .acceptance_criteria
                .clone()
                .filter(|v| !v.trim().is_empty()),
            assay_commands: row
                .assay_commands
                .as_deref()
                .map(|v| {
                    v.lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            test_mode: row.test_mode.clone().filter(|v| !v.trim().is_empty()),
            scope,
            operating_surface,
        }
    }

    /// Build the view from a [`StoryPacket`] (already loaded from Neon).
    pub fn from_packet(packet: &StoryPacket) -> Self {
        let brief = packet
            .architect_brief
            .clone()
            .filter(|v| !v.trim().is_empty());
        let scope = brief
            .as_deref()
            .and_then(|b| extract_section(b, "SCOPE:"));
        let operating_surface = brief
            .as_deref()
            .and_then(|b| extract_section(b, "OPERATING SURFACE:"));
        Self {
            id: packet.id.clone(),
            title: packet.title.clone(),
            goal: packet.goal.clone().filter(|v| !v.trim().is_empty()),
            architect_brief: brief,
            acceptance_criteria: packet
                .acceptance_criteria
                .clone()
                .filter(|v| !v.trim().is_empty()),
            assay_commands: packet.assay_commands.clone(),
            test_mode: packet.test_mode.clone().filter(|v| !v.trim().is_empty()),
            scope,
            operating_surface,
        }
    }

    /// Load the view from Neon via `StoryPacket::load_from_neon`, which uses
    /// `ForgeEngineDao::story_packet`. This is the only constructor that
    /// touches the database.
    pub fn load_from_neon(story_id: &str) -> Result<Self, String> {
        StoryPacket::load_from_neon(story_id).map(|packet| Self::from_packet(&packet))
    }
}

/// Pull the body of a `HEADER:` section out of concatenated brief text.
///
/// Sections are `HEADER:\n<body>` runs terminated by the next `ALL-CAPS:`
/// header or the end of the text. Matching is line-oriented so prose that
/// merely mentions "scope:" mid-sentence is not treated as a section.
fn extract_section(text: &str, header: &str) -> Option<String> {
    let mut capturing = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !capturing {
            if trimmed.eq_ignore_ascii_case(header) || trimmed.to_uppercase() == header {
                capturing = true;
            }
            continue;
        }
        if is_section_header(trimmed) {
            break;
        }
        out.push(line);
    }
    if !capturing {
        return None;
    }
    let body = out.join("\n").trim().to_string();
    if body.is_empty() {
        None
    } else {
        Some(body)
    }
}

/// A line like `SCOPE:`, `DEPENDENCIES:`, `OPERATING SURFACE:` — all caps
/// (plus spaces), ending in a colon.
fn is_section_header(line: &str) -> bool {
    let t = line.trim();
    if !t.ends_with(':') || t.len() < 3 {
        return false;
    }
    let stem = t.trim_end_matches(':');
    !stem.is_empty()
        && stem
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == ' ' || c == '_' || c == '-')
}

// ---------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------

/// Where the authored Rust test belongs. Never a new taxonomy: each variant
/// names a convention that already exists in the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestPlacement {
    /// Extend the file's inline `#[cfg(test)] mod tests` (the colocated
    /// convention: every file owns its unit tests at the bottom).
    CrateLocalMod {
        crate_path: String,
        module_file: String,
    },
    /// Extend or add a `<crate>/tests/*.rs` integration file (one concern
    /// per file; `_dev` suffix when a real Neon DEV socket is needed).
    IntegrationFile {
        crate_path: String,
        test_file: String,
    },
    /// Author under the workspace contract suite (`tests/tests/*.rs`).
    ForgeContractSuite { suite_file: String },
}

/// The idiom read off 2-3 neighboring test files, so the authored test uses
/// the correct harness markers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NeighborIdiom {
    pub uses_super_star: bool,
    pub has_cfg_test_mod: bool,
    pub uses_plain_test: bool,
    pub uses_tokio_test: bool,
    pub uses_contract_harness: bool,
    pub files_read: usize,
}

/// Workspace members that own code (the `tests` member is the contract
/// suite, not a crate target).
const WORKSPACE_CRATES: [&str; 10] = [
    "web/ui",
    "web/auth",
    "middle/model",
    "middle/services",
    "middle/workflow",
    "middle/apis",
    "db",
    "cli",
    "forge",
    "web",
];

/// Decide test placement from the story's target path hint plus the
/// neighboring test files on disk.
///
/// * `target_path`: the file hinted by the story (`scope` /
///   `operating_surface` / brief text), e.g. `forge/src/engine/scope.rs`.
///   The legacy `rust/` prefix (`rust/forge/src/...`) is accepted and
///   stripped.
/// * `workspace_root`: the lane checkout root (the worktree path).
///
/// Decision rule (target + neighbor, never a new taxonomy):
/// * target inside `<crate>/src/...` and the module file or a sibling
///   carries `#[cfg(test)]` -> [`TestPlacement::CrateLocalMod`].
/// * target inside `<crate>/...` and `<crate>/tests/*.rs` exists but no
///   inline tests neighbor it -> [`TestPlacement::IntegrationFile`].
/// * no target, or the target maps to no known crate -> contract suite
///   (`tests/tests/*.rs`) via [`TestPlacement::ForgeContractSuite`].
/// * target inside `<crate>/src/...` with neither inline tests nor a
///   `<crate>/tests/` dir -> `CrateLocalMod` (colocated is the default for
///   pure logic).
pub fn inspect_neighboring_tests(
    target_path: Option<&str>,
    workspace_root: &Path,
) -> TestPlacement {
    let Some(raw) = target_path.map(str::trim).filter(|s| !s.is_empty()) else {
        return TestPlacement::ForgeContractSuite {
            suite_file: default_suite_file(workspace_root),
        };
    };
    let normalized = normalize_target(raw);
    let Some(crate_dir) = match_workspace_crate(&normalized) else {
        return TestPlacement::ForgeContractSuite {
            suite_file: default_suite_file(workspace_root),
        };
    };
    let _ = read_neighbor_idiom(&normalized, workspace_root);

    let module_abs = workspace_root.join(&normalized);
    let under_src = is_under_src(&normalized, crate_dir);
    let has_inline = under_src && neighbors_have_inline_tests(&module_abs);
    if has_inline {
        return TestPlacement::CrateLocalMod {
            crate_path: crate_dir.to_string(),
            module_file: normalized,
        };
    }
    if let Some(integration) = nearest_integration_file(crate_dir, &normalized, workspace_root) {
        return TestPlacement::IntegrationFile {
            crate_path: crate_dir.to_string(),
            test_file: integration,
        };
    }
    if under_src {
        return TestPlacement::CrateLocalMod {
            crate_path: crate_dir.to_string(),
            module_file: normalized,
        };
    }
    TestPlacement::ForgeContractSuite {
        suite_file: default_suite_file(workspace_root),
    }
}

/// Read up to 3 nearest test files to capture the neighboring idiom:
/// `use super::*`, `#[test]`, `#[tokio::test]`, `#[cfg(test)] mod tests`,
/// and the contract/in-memory harness markers. Files are capped at 32 KiB
/// each. Missing files simply contribute nothing.
pub fn read_neighbor_idiom(target_path: &str, workspace_root: &Path) -> NeighborIdiom {
    let normalized = normalize_target(target_path);
    let mut candidates: Vec<PathBuf> = Vec::new();
    let module_abs = workspace_root.join(&normalized);
    if module_abs.is_file() {
        candidates.push(module_abs.clone());
    }
    if let Some(parent) = module_abs.parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            let mut siblings: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().map(|e| e == "rs").unwrap_or(false) && *p != module_abs
                })
                .collect();
            siblings.sort();
            for sib in siblings.into_iter().take(2) {
                candidates.push(sib);
            }
        }
    }
    if candidates.len() < 3 {
        if let Some(crate_dir) = match_workspace_crate(&normalized) {
            let tests_dir = workspace_root.join(crate_dir).join("tests");
            if let Ok(entries) = std::fs::read_dir(&tests_dir) {
                let mut integration: Vec<PathBuf> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|e| e == "rs").unwrap_or(false))
                    .collect();
                integration.sort();
                for file in integration.into_iter().take(3 - candidates.len()) {
                    candidates.push(file);
                }
            }
        }
    }
    candidates.truncate(3);

    let mut idiom = NeighborIdiom::default();
    for path in &candidates {
        let Ok(content) = std::fs::read(path) else {
            continue;
        };
        let content: Vec<u8> = content.into_iter().take(32 * 1024).collect();
        let text = String::from_utf8_lossy(&content);
        idiom.files_read += 1;
        if text.contains("use super::*") {
            idiom.uses_super_star = true;
        }
        if text.contains("#[cfg(test)]") {
            idiom.has_cfg_test_mod = true;
        }
        if text.contains("#[test]") {
            idiom.uses_plain_test = true;
        }
        if text.contains("tokio::test") {
            idiom.uses_tokio_test = true;
        }
        if text.contains("RecordingWriter")
            || text.contains("in_memory")
            || text.contains("RoleHarness")
        {
            idiom.uses_contract_harness = true;
        }
    }
    idiom
}

/// Strip `./` and the legacy `rust/` prefix the playbook uses
/// (`rust/forge/src/...` -> `forge/src/...`). The workspace root is the
/// repo root here, so `rust/` is not a real directory.
fn normalize_target(raw: &str) -> String {
    let mut t = raw.trim().trim_start_matches("./").to_string();
    if t == "rust" {
        return t;
    }
    if let Some(rest) = t.strip_prefix("rust/") {
        t = rest.to_string();
    }
    t
}

/// Longest workspace-crate prefix match, so `web/ui/...` wins over `web/...`.
fn match_workspace_crate(normalized: &str) -> Option<&'static str> {
    let mut best: Option<&'static str> = None;
    for krate in WORKSPACE_CRATES {
        if normalized == krate || normalized.starts_with(&format!("{krate}/")) {
            if best.map(|b: &str| krate.len() > b.len()).unwrap_or(true) {
                best = Some(krate);
            }
        }
    }
    best
}

/// True when the target sits under `<crate>/src/` (unit-test territory).
fn is_under_src(normalized: &str, crate_dir: &str) -> bool {
    normalized.starts_with(&format!("{crate_dir}/src/"))
        || normalized.starts_with(&format!("{crate_dir}/src\\"))
}

/// True when the module file itself or any sibling `.rs` file carries an
/// inline `#[cfg(test)]` module.
fn neighbors_have_inline_tests(module_abs: &Path) -> bool {
    let mut files = vec![module_abs.to_path_buf()];
    if let Some(parent) = module_abs.parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten().take(50) {
                let path = entry.path();
                if path.extension().map(|e| e == "rs").unwrap_or(false) && path != module_abs
                {
                    files.push(path);
                }
            }
        }
    }
    files.iter().any(|path| {
        std::fs::read(path)
            .map(|bytes| {
                let capped: Vec<u8> = bytes.into_iter().take(64 * 1024).collect();
                String::from_utf8_lossy(&capped).contains("#[cfg(test)]")
            })
            .unwrap_or(false)
    })
}

/// Nearest `<crate>/tests/*.rs` file: prefer the one sharing the module
/// stem, else the lexicographically first. `None` when the dir is missing
/// or holds no `.rs` files.
fn nearest_integration_file(
    crate_dir: &str,
    normalized: &str,
    workspace_root: &Path,
) -> Option<String> {
    let tests_dir = workspace_root.join(crate_dir).join("tests");
    let Ok(entries) = std::fs::read_dir(&tests_dir) else {
        return None;
    };
    let mut files: Vec<String> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "rs").unwrap_or(false))
        .filter_map(|p| {
            p.strip_prefix(workspace_root)
                .ok()
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    if files.is_empty() {
        return None;
    }
    files.sort();
    let stem = Path::new(normalized)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if !stem.is_empty() {
        if let Some(hit) = files.iter().find(|f| {
            Path::new(f)
                .file_stem()
                .map(|s| s.to_string_lossy().contains(&stem as &str))
                .unwrap_or(false)
        }) {
            return Some(hit.clone());
        }
    }
    Some(files.remove(0))
}

/// Default contract-suite file: reuse the story-derived name when the lane
/// checked out under `tests/tests/`, else the generic worker contract file.
fn default_suite_file(workspace_root: &Path) -> String {
    let suite_dir = workspace_root.join("tests").join("tests");
    if suite_dir.is_dir() {
        return "tests/tests/tst_worker_contract.rs".to_string();
    }
    "tests/tests/tst_worker_contract.rs".to_string()
}

// ---------------------------------------------------------------------------
// Intent validation
// ---------------------------------------------------------------------------

/// Validate that the story says enough to author a test, and that the brief
/// does not fight the architecture.
///
/// * Empty goal/spec -> `Err(AmbiguousSpec)`.
/// * Brief touching Abstract Service boundaries or MVI patterns ->
///   `Err(ArchConflict)` (marker set shared with
///   `escalation::should_escalate`).
/// * A story that intentionally exposes an app defect (goal names a failing
///   test, a reproduction, a defect to find) is **valid**: the failing test
///   is the deliverable, and the worker must not auto-fix the test to make
///   the app green. This always returns `Ok` for such stories.
pub fn validate_story_intent(view: &TstStoryView) -> Result<(), EscalationReason> {
    if view.goal.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err(EscalationReason::AmbiguousSpec);
    }
    if view
        .acceptance_criteria
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        return Err(EscalationReason::AmbiguousSpec);
    }
    if touches_architecture_text(
        view.goal.as_deref().unwrap_or(""),
        view.architect_brief.as_deref().unwrap_or(""),
        view.acceptance_criteria.as_deref().unwrap_or(""),
    ) {
        return Err(EscalationReason::ArchConflict);
    }
    Ok(())
}

/// Same marker set as the escalation guard: service-boundary, Abstract
/// Service / MVI, or architecture-record touch.
fn touches_architecture_text(goal: &str, brief: &str, criteria: &str) -> bool {
    const MARKERS: [&str; 7] = [
        "rust/core/service",
        "abstract service",
        "abstract_service",
        "mvi",
        "architecture decision",
        "architecture-decision",
        "adr-",
    ];
    let lower = format!("{goal}\n{brief}\n{criteria}").to_lowercase();
    MARKERS.iter().any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        id: &str,
        goal: Option<&str>,
        brief: Option<&str>,
        criteria: Option<&str>,
    ) -> StoryPacketRow {
        StoryPacketRow {
            id: id.to_string(),
            title: format!("story {id}"),
            goal: goal.map(str::to_string),
            architect_brief: brief.map(str::to_string),
            acceptance_criteria: criteria.map(str::to_string),
            test_mode: None,
            assay_commands: Some("cargo test -p forge --lib worker".to_string()),
        }
    }

    fn tmp_workspace(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pianola-worker-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("fixture workspace");
        dir
    }

    #[test]
    fn view_parses_scope_and_surface_from_brief_sections() {
        let r = row(
            "TST-1",
            Some("author a test"),
            Some("SCOPE:\nforge/src/engine/scope.rs\n\nOPERATING SURFACE:\nTECH\n\nPOSTCONDITIONS:\nnone"),
            Some("test exists and passes"),
        );
        let view = TstStoryView::from_row(&r);
        assert_eq!(view.id, "TST-1");
        assert_eq!(
            view.scope.as_deref().map(str::trim),
            Some("forge/src/engine/scope.rs")
        );
        assert_eq!(
            view.operating_surface.as_deref().map(str::trim),
            Some("TECH")
        );
        assert_eq!(view.assay_commands, vec!["cargo test -p forge --lib worker"]);
    }

    #[test]
    fn view_tolerates_missing_sections() {
        let r = row("TST-2", Some("goal"), Some("plain brief"), Some("criteria"));
        let view = TstStoryView::from_row(&r);
        assert!(view.scope.is_none());
        assert!(view.operating_surface.is_none());
    }

    #[test]
    fn rejects_empty_goal_as_ambiguous() {
        let view = TstStoryView::from_row(&row("TST-3", None, Some("brief"), Some("criteria")));
        assert_eq!(
            validate_story_intent(&view),
            Err(EscalationReason::AmbiguousSpec)
        );
        let blank = TstStoryView::from_row(&row("TST-3", Some("  "), Some("brief"), Some("x")));
        assert_eq!(
            validate_story_intent(&blank),
            Err(EscalationReason::AmbiguousSpec)
        );
    }

    #[test]
    fn rejects_empty_acceptance_criteria_as_ambiguous() {
        let view = TstStoryView::from_row(&row("TST-4", Some("goal"), Some("brief"), None));
        assert_eq!(
            validate_story_intent(&view),
            Err(EscalationReason::AmbiguousSpec)
        );
    }

    #[test]
    fn rejects_arch_conflict_brief() {
        let view = TstStoryView::from_row(&row(
            "TST-5",
            Some("author a test for the boundary"),
            Some("refactor the Abstract Service boundary for this story"),
            Some("test exists"),
        ));
        assert_eq!(
            validate_story_intent(&view),
            Err(EscalationReason::ArchConflict)
        );
    }

    #[test]
    fn failing_test_intent_is_valid_not_escalated() {
        // The story exists to EXPOSE a defect: the failing test is the
        // deliverable, never a reason to escalate or to rewrite the test
        // green.
        let view = TstStoryView::from_row(&row(
            "TST-6",
            Some("author a regression test that exposes the scope-ranking defect; the test is expected to FAIL against current code and that failure is the finding"),
            Some("colocated unit test, no architecture change"),
            Some("failing test committed as evidence; do not mutate production code to green"),
        ));
        assert_eq!(validate_story_intent(&view), Ok(()));
    }

    #[test]
    fn placement_prefers_inline_mod_when_neighbors_have_cfg_test() {
        let ws = tmp_workspace("inline");
        let src = ws.join("forge").join("src").join("engine");
        std::fs::create_dir_all(&src).expect("fixture");
        std::fs::write(
            src.join("scope.rs"),
            "pub fn rank() {}\n#[cfg(test)]\nmod tests { use super::*; #[test] fn ranks() {} }\n",
        )
        .expect("fixture");
        let placement =
            inspect_neighboring_tests(Some("forge/src/engine/scope.rs"), &ws);
        assert_eq!(
            placement,
            TestPlacement::CrateLocalMod {
                crate_path: "forge".to_string(),
                module_file: "forge/src/engine/scope.rs".to_string(),
            }
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn placement_prefers_integration_file_when_no_inline_tests() {
        let ws = tmp_workspace("integration");
        let src = ws.join("db").join("src");
        let tests = ws.join("db").join("tests");
        std::fs::create_dir_all(&src).expect("fixture");
        std::fs::create_dir_all(&tests).expect("fixture");
        std::fs::write(src.join("pool.rs"), "pub fn open() {}\n").expect("fixture");
        std::fs::write(
            tests.join("pool_connect_dev.rs"),
            "// requires DATABASE_URL_DEV\n#[test] fn connects() {}\n",
        )
        .expect("fixture");
        let placement = inspect_neighboring_tests(Some("db/src/pool.rs"), &ws);
        assert_eq!(
            placement,
            TestPlacement::IntegrationFile {
                crate_path: "db".to_string(),
                test_file: "db/tests/pool_connect_dev.rs".to_string(),
            }
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn placement_falls_back_to_contract_suite_without_target() {
        let ws = tmp_workspace("suite");
        std::fs::create_dir_all(&ws).expect("fixture");
        assert_eq!(
            inspect_neighboring_tests(None, &ws),
            TestPlacement::ForgeContractSuite {
                suite_file: "tests/tests/tst_worker_contract.rs".to_string(),
            }
        );
        assert_eq!(
            inspect_neighboring_tests(Some("docs/agent/notes.md"), &ws),
            TestPlacement::ForgeContractSuite {
                suite_file: "tests/tests/tst_worker_contract.rs".to_string(),
            }
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn placement_strips_legacy_rust_prefix() {
        let ws = tmp_workspace("prefix");
        let src = ws.join("forge").join("src");
        std::fs::create_dir_all(&src).expect("fixture");
        std::fs::write(src.join("scope.rs"), "pub fn x() {}\n").expect("fixture");
        // No inline tests and no forge/tests dir: colocated default.
        let placement =
            inspect_neighboring_tests(Some("rust/forge/src/scope.rs"), &ws);
        assert_eq!(
            placement,
            TestPlacement::CrateLocalMod {
                crate_path: "forge".to_string(),
                module_file: "forge/src/scope.rs".to_string(),
            }
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn idiom_captures_harness_markers() {
        let ws = tmp_workspace("idiom");
        let src = ws.join("forge").join("src");
        std::fs::create_dir_all(&src).expect("fixture");
        std::fs::write(
            src.join("scope.rs"),
            "pub fn x() {}\n#[cfg(test)]\nmod tests {\nuse super::*;\n#[test]\nfn ranks() {}\n#[tokio::test]\nasync fn async_ranks() {}\n}\n",
        )
        .expect("fixture");
        let idiom = read_neighbor_idiom("forge/src/scope.rs", &ws);
        assert_eq!(idiom.files_read, 1);
        assert!(idiom.uses_super_star);
        assert!(idiom.has_cfg_test_mod);
        assert!(idiom.uses_plain_test);
        assert!(idiom.uses_tokio_test);
        let _ = std::fs::remove_dir_all(&ws);
    }
}
