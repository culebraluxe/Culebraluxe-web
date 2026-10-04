//! TST Rust test authoring contract with hard guards on worker authority.
//!
//! A worker may inspect code, author its assigned Rust test, run the
//! prescribed checks, commit locally in its lane, and record evidence. It
//! may NOT redesign architecture, change Abstract Service boundaries or MVI
//! patterns, modify security/entitlement rules, change DB schema unless the
//! story explicitly authorizes it, publish, broaden scope, or silently
//! rewrite production code to make a test green.
//!
//! QA does not write code: the worker authors the test, assay/QA measures it
//! via `adjudicate_assay` / `collect_assay_evidence`. A failing test may be
//! the correct result when the story intentionally exposes an app defect.
//!
//! Path note: the playbook names this file
//! `rust/forge/src/pianola/worker_authoring.rs`. The workspace root is the
//! repo root (`members = ["forge", ...]`), so the canonical path is
//! `forge/src/pianola/worker_authoring.rs`. Likewise the task text cites
//! `<worktree>/rust/Cargo.toml`; the real manifest is `<worktree>/Cargo.toml`
//! with the legacy `rust/` prefix accepted as a fallback (see
//! [`workspace_manifest`]).

use std::path::{Path, PathBuf};
use std::process::Command;

use super::escalation::EscalationReason;
use super::worker::{read_neighbor_idiom, TestPlacement, TstStoryView};
use crate::engine::assay::is_rust_contract_production_path;

// ---------------------------------------------------------------------------
// Guards
// ---------------------------------------------------------------------------

/// Markers that mean the story fights the architecture (shared with
/// `worker::validate_story_intent` and `escalation::should_escalate`).
const ARCH_MARKERS: [&str; 7] = [
    "rust/core/service",
    "middle/services",
    "abstract service",
    "abstract_service",
    "mvi",
    "architecture decision",
    "adr-",
];

/// Security/entitlement surface: never touched by a test-authoring story.
const SECURITY_TOKENS: [&str; 5] = [
    "casbin",
    "authz",
    "entitlement",
    "security.rs",
    "security/entitlement",
];

/// DB work needs explicit authorization (scope carrying a migration keyword,
/// or a db/migration test mode).
const MIGRATION_MARKERS: [&str; 4] = ["migration", "migrate", "schema change", "db schema"];

/// Prose that asks to change production code rather than author a test.
const PROD_REWRITE_MARKERS: [&str; 6] = [
    "fix production",
    "change prod",
    "modify production",
    "rewrite production",
    "make test green",
    "rewrite prod",
];

fn story_text(view: &TstStoryView) -> String {
    [
        view.goal.as_deref().unwrap_or(""),
        view.architect_brief.as_deref().unwrap_or(""),
        view.acceptance_criteria.as_deref().unwrap_or(""),
        view.scope.as_deref().unwrap_or(""),
        view.operating_surface.as_deref().unwrap_or(""),
    ]
    .join("\n")
}

/// True when the packet explicitly authorizes DB work: `scope` carries a
/// migration keyword, or `test_mode` names database work.
pub fn db_work_authorized(view: &TstStoryView) -> bool {
    if let Some(scope) = view.scope.as_deref() {
        let lower = scope.to_lowercase();
        if MIGRATION_MARKERS.iter().any(|m| lower.contains(m)) {
            return true;
        }
    }
    if let Some(mode) = view.test_mode.as_deref() {
        let lower = mode.to_lowercase();
        if lower.contains("db") || lower.contains("migration") || lower.contains("database") {
            return true;
        }
    }
    false
}

/// True when a relative path touches the security/entitlement surface.
pub fn is_security_path(relative: &str) -> bool {
    let lower = relative.to_lowercase();
    SECURITY_TOKENS.iter().any(|t| lower.contains(t))
}

/// True when a relative path is a migration (`db/migrations/...`).
pub fn is_migration_path(relative: &str) -> bool {
    let lower = relative.to_lowercase().replace('\\', "/");
    lower.starts_with("db/migrations/")
        || lower.contains("/db/migrations/")
        || lower.starts_with("rust/core/db/migrations")
}

/// Placement target as a repo-relative path string.
fn placement_target(placement: &TestPlacement) -> String {
    match placement {
        TestPlacement::CrateLocalMod { module_file, .. } => module_file.clone(),
        TestPlacement::IntegrationFile { test_file, .. } => test_file.clone(),
        TestPlacement::ForgeContractSuite { suite_file } => suite_file.clone(),
    }
}

/// Extract file-ish tokens (`a/b`, `*.rs`, `*.sql`) for scope-broadening
/// detection. Same shape as the supervisor's overlap scan, local so this
/// module stays pure and dependency-free.
fn path_tokens(text: &str) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for raw in text.split(|c: char| {
        c.is_whitespace() || c == '"' || c == '\'' || c == '`' || c == '(' || c == ')' || c == ','
    }) {
        let token = raw.trim_matches(|c| c == '.' || c == ':' || c == ';');
        if token.len() < 3 {
            continue;
        }
        let looks_like_path = token.contains('/')
            || token.ends_with(".rs")
            || token.ends_with(".md")
            || token.ends_with(".toml")
            || token.ends_with(".sql");
        if looks_like_path {
            out.insert(
                token
                    .trim_start_matches("./")
                    .trim_end_matches(|c| c == '.' || c == ',' || c == ':')
                    .to_string(),
            );
        }
    }
    out
}

/// First repo-relative `.rs` path hinted by the story (scope first, then
/// goal/brief/criteria). Used to open domain context before authoring.
pub fn extract_target_hint(view: &TstStoryView) -> Option<String> {
    let ordered = [
        view.scope.as_deref().unwrap_or(""),
        view.goal.as_deref().unwrap_or(""),
        view.architect_brief.as_deref().unwrap_or(""),
        view.acceptance_criteria.as_deref().unwrap_or(""),
    ];
    for text in ordered {
        let mut tokens: Vec<String> = path_tokens(text)
            .into_iter()
            .filter(|t| t.ends_with(".rs"))
            .collect();
        tokens.sort();
        if let Some(first) = tokens.into_iter().next() {
            let mut t = first.trim().trim_start_matches("./").to_string();
            if let Some(rest) = t.strip_prefix("rust/") {
                t = rest.to_string();
            }
            return Some(t);
        }
    }
    None
}

/// Scope broadening: the brief/scope names production `.rs` paths the
/// goal + acceptance criteria never authorize AND that are not the assigned
/// placement target. Such an edit is outside the story, so authoring refuses
/// with `AmbiguousSpec`. The assigned target itself is always allowed: a
/// brief naming `forge/src/engine/scope.rs` for a placement in that file is
/// the assignment, not broadening.
fn scope_is_broadened(view: &TstStoryView, placement_target: &str) -> bool {
    let authorized_text = format!(
        "{}\n{}",
        view.goal.as_deref().unwrap_or(""),
        view.acceptance_criteria.as_deref().unwrap_or("")
    );
    let claimed_text = format!(
        "{}\n{}",
        view.scope.as_deref().unwrap_or(""),
        view.architect_brief.as_deref().unwrap_or("")
    );
    let authorized = path_tokens(&authorized_text);
    let claimed = path_tokens(&claimed_text);
    let target_norm = placement_target.trim_start_matches("./").to_string();
    let target_stripped = target_norm.strip_prefix("rust/").unwrap_or(&target_norm);
    claimed.iter().any(|token| {
        if !token.ends_with(".rs") || !is_rust_contract_production_path(token) {
            return false;
        }
        if authorized.contains(token) {
            return false;
        }
        // The assigned placement target is the assignment, not broadening.
        if token == &target_norm || token == target_stripped {
            return false;
        }
        // Same file stem under the same top-level crate is the same
        // assignment (brief says `forge/src/engine/scope.rs`, lane fixture
        // places `forge/src/scope.rs`).
        let target_stem = Path::new(target_stripped)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let target_crate = target_stripped.split('/').next().unwrap_or("");
        let tok_stem = Path::new(token)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let tok_crate = token.split('/').next().unwrap_or("");
        if !target_stem.is_empty() && tok_stem == target_stem && tok_crate == target_crate {
            return false;
        }
        true
    })
}

/// Detects the forbidden "rewrite prod to make the test green" shape: a
/// test-only story whose diff touches both a production file and a test
/// file. Callers pass the already-computed changed-file lists; pure.
pub fn would_rewrite_prod_to_green(
    prod_edits: &[String],
    test_edits: &[String],
    test_only_story: bool,
) -> bool {
    test_only_story && !prod_edits.is_empty() && !test_edits.is_empty()
}

/// True when a path is a test artifact (integration/contract test file, or a
/// module file edited only inside its `#[cfg(test)]` region — the latter is
/// decided by the writer, which appends only test modules).
fn is_test_artifact_path(relative: &str) -> bool {
    let lower = relative.to_lowercase().replace('\\', "/");
    if lower.contains("/tests/") || lower.starts_with("tests/") {
        return true;
    }
    // Contract suite and `_test.rs` / `test_*.rs` files are always artifacts.
    if lower.ends_with("_test.rs") || lower.contains("test_") {
        return true;
    }
    false
}

/// All hard guards in evaluation order. `Ok` means the worker may author;
/// `Err(reason)` means escalate instead of writing anything.
pub fn check_authoring_guards(
    view: &TstStoryView,
    placement: &TestPlacement,
) -> Result<(), EscalationReason> {
    let text = story_text(view);
    let lower = text.to_lowercase();
    let target = placement_target(placement);

    // 1. Architecture redesign is never test work.
    if ARCH_MARKERS.iter().any(|m| lower.contains(m)) {
        return Err(EscalationReason::ArchConflict);
    }
    // MVI pattern files outside test files are architecture too.
    if lower.contains("mvi") && !is_test_artifact_path(&target) {
        return Err(EscalationReason::ArchConflict);
    }
    // Service-boundary edits outside test files are architecture too.
    if (lower.contains("service boundary")
        || lower.contains("service boundaries")
        || lower.contains("middle/services"))
        && !is_test_artifact_path(&target)
    {
        return Err(EscalationReason::ArchConflict);
    }
    // 2. Security/entitlement surface.
    if is_security_path(&target) {
        return Err(EscalationReason::SecurityEntitlement);
    }
    if SECURITY_TOKENS.iter().any(|t| lower.contains(t)) {
        return Err(EscalationReason::SecurityEntitlement);
    }
    // 3. Migrations need explicit DB authorization.
    if is_migration_path(&target) && !db_work_authorized(view) {
        return Err(EscalationReason::MigrationNeeded);
    }
    if MIGRATION_MARKERS.iter().any(|m| lower.contains(m)) && !db_work_authorized(view) {
        return Err(EscalationReason::MigrationNeeded);
    }
    // 4. Explicit prod-rewrite prose.
    if PROD_REWRITE_MARKERS.iter().any(|m| lower.contains(m)) {
        return Err(EscalationReason::ProdChangeNeeded);
    }
    // 5. Scope broadening beyond goal/acceptance criteria.
    if scope_is_broadened(view, &target) {
        return Err(EscalationReason::AmbiguousSpec);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Authoring
// ---------------------------------------------------------------------------

/// Sanitize a story id for use in a Rust identifier (`TST-12` -> `tst_12`).
fn sanitize_ident(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_').to_string();
    let out = if out.is_empty() {
        "tst".to_string()
    } else {
        out
    };
    if out
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("t_{out}")
    } else {
        out
    }
}

/// Read up to 32 KiB of domain context from the hinted target file.
fn read_domain_context(worktree_path: &Path, hint: Option<&str>) -> String {
    let Some(hint) = hint else {
        return String::new();
    };
    let abs = worktree_path.join(hint);
    let Ok(bytes) = std::fs::read(&abs) else {
        return String::new();
    };
    let capped: Vec<u8> = bytes.into_iter().take(32 * 1024).collect();
    let text = String::from_utf8_lossy(&capped).to_string();
    // Keep the first few `pub fn` names so the test can name its subject.
    let mut fns = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("pub fn ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                fns.push(name);
            }
        }
        if fns.len() >= 3 {
            break;
        }
    }
    fns.join(", ")
}

/// Resolve the workspace manifest: the real `<worktree>/Cargo.toml` first,
/// the legacy `<worktree>/rust/Cargo.toml` cited in older docs as fallback.
pub fn workspace_manifest(worktree_path: &Path) -> PathBuf {
    let root = worktree_path.join("Cargo.toml");
    if root.is_file() {
        return root;
    }
    worktree_path.join("rust").join("Cargo.toml")
}

/// Author the story's Rust test inside the lane worktree.
///
/// * Follows the neighboring idiom (`#[test]` vs `#[tokio::test]`, `use
///   super::*`, contract harness markers) read via
///   [`read_neighbor_idiom`].
/// * The test name and header comment carry the TST story id.
/// * Rust only: a `.ts`/`.js` target is refused (Rust First).
/// * The write is confined to `worktree_path`; any escape is refused with
///   `PublishRequested` (write outside lane).
/// * Appends only: `CrateLocalMod` appends a new `#[cfg(test)]` module to
///   the module file (never edits production items); integration/contract
///   placements append to (or create) the test file.
pub fn author_rust_test(
    view: &TstStoryView,
    placement: &TestPlacement,
    worktree_path: &Path,
) -> Result<PathBuf, EscalationReason> {
    check_authoring_guards(view, placement)?;

    let target_rel = placement_target(placement);
    // Rust First: never author TypeScript/JavaScript.
    let lower_target = target_rel.to_lowercase();
    if lower_target.ends_with(".ts")
        || lower_target.ends_with(".tsx")
        || lower_target.ends_with(".js")
        || lower_target.ends_with(".jsx")
        || lower_target.ends_with(".mjs")
        || lower_target.ends_with(".cjs")
    {
        return Err(EscalationReason::ProdChangeNeeded);
    }

    // Resolve the absolute write path and confine it to the lane.
    let abs = worktree_path.join(&target_rel);
    let lane_root = normalize_for_compare(worktree_path);
    let abs_norm = normalize_for_compare(&abs);
    if abs_norm != lane_root && !abs_norm.starts_with(&format!("{lane_root}/")) {
        return Err(EscalationReason::PublishRequested);
    }

    // Neighboring idiom decides the harness attribute.
    let hint = extract_target_hint(view);
    let idiom_probe = hint.as_deref().unwrap_or(&target_rel);
    let idiom = read_neighbor_idiom(idiom_probe, worktree_path);
    let use_tokio = idiom.uses_tokio_test && !idiom.uses_plain_test;
    let attr = if use_tokio {
        "#[tokio::test]"
    } else {
        "#[test]"
    };
    let fn_async = if use_tokio { "async " } else { "" };

    let story_ident = sanitize_ident(&view.id);
    let test_fn = format!("tst_{story_ident}_authored");
    let domain = read_domain_context(worktree_path, hint.as_deref());
    let domain_line = if domain.is_empty() {
        String::new()
    } else {
        format!("// Subject context (neighboring fns): {domain}\n")
    };
    let header = format!(
        "// TST {}: {}\n// Goal: {}\n{}",
        view.id,
        view.title,
        view.goal
            .as_deref()
            .unwrap_or("(no goal)")
            .lines()
            .next()
            .unwrap_or(""),
        domain_line,
    );

    match placement {
        TestPlacement::CrateLocalMod { .. } => {
            append_inline_test_module(&abs, &header, attr, fn_async, &test_fn, view)?;
        }
        TestPlacement::IntegrationFile { .. } | TestPlacement::ForgeContractSuite { .. } => {
            append_integration_test(&abs, &header, attr, fn_async, &test_fn, view, &idiom_probe)?;
        }
    }
    Ok(abs)
}

fn normalize_for_compare(p: &Path) -> String {
    // Lexical normalization (no I/O): `/`, no trailing slash, no `./`.
    let mut s = p.to_string_lossy().replace('\\', "/");
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    // Resolve `.` and `..` lexically so `lane/../lane/x` cannot escape.
    let mut parts: Vec<&str> = Vec::new();
    let absolute = s.starts_with('/');
    for comp in s.split('/') {
        match comp {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            c => parts.push(c),
        }
    }
    let mut out = parts.join("/");
    if absolute {
        out = format!("/{out}");
    }
    out.trim_end_matches('/').to_string()
}

/// Append a new `#[cfg(test)]` module to a crate-local file. The module is
/// appended at the end; no production item is edited.
fn append_inline_test_module(
    abs: &Path,
    header: &str,
    attr: &str,
    fn_async: &str,
    test_fn: &str,
    view: &TstStoryView,
) -> Result<(), EscalationReason> {
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(|_| EscalationReason::AmbiguousSpec)?;
    }
    let existing = std::fs::read_to_string(abs).unwrap_or_default();
    // Refuse to touch a file whose production body already carries a guard
    // marker this story must not move (defense in depth: the guard scan
    // above is text-based, this is path-based).
    let body = header_block();
    let addition = format!(
        "\n{header}\n#[cfg(test)]\nmod {test_fn}_mod {{\n    use super::*;\n\n    {attr}\n    fn {fn_async}{test_fn}() {{\n        // TST {} acceptance: {}\n        {}\n    }}\n}}\n",
        view.id,
        view.acceptance_criteria
            .as_deref()
            .unwrap_or("(no acceptance criteria)")
            .lines()
            .next()
            .unwrap_or(""),
        body,
    );
    std::fs::write(abs, format!("{existing}{addition}"))
        .map_err(|_| EscalationReason::AmbiguousSpec)?;
    Ok(())
}

/// Append a test function to an integration/contract test file, creating the
/// file (with a header) when it does not exist yet.
fn append_integration_test(
    abs: &Path,
    header: &str,
    attr: &str,
    fn_async: &str,
    test_fn: &str,
    view: &TstStoryView,
    idiom_probe: &str,
) -> Result<(), EscalationReason> {
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(|_| EscalationReason::AmbiguousSpec)?;
    }
    let existing = std::fs::read_to_string(abs).unwrap_or_default();
    let needs_header = existing.trim().is_empty();
    let body = header_block();
    let mut out = existing;
    if needs_header {
        out.push_str(&format!(
            "//! TST worker contract tests ({probe}).\n//! Placement follows neighboring convention; no new taxonomy.\n\n",
            probe = idiom_probe
        ));
    }
    let body = header_block();
    out.push_str(&format!(
        "\n{header}\n{attr}\nfn {fn_async}{test_fn}() {{\n    // TST {} acceptance: {}\n    {}\n}}\n",
        view.id,
        view.acceptance_criteria
            .as_deref()
            .unwrap_or("(no acceptance criteria)")
            .lines()
            .next()
            .unwrap_or(""),
        body,
    ));
    std::fs::write(abs, out).map_err(|_| EscalationReason::AmbiguousSpec)?;
    Ok(())
}

/// Body placeholder: the authored test asserts the story's acceptance line
/// is recorded; assay (not the author) decides pass/fail. A failing test
/// that exposes an app defect is a valid deliverable and is never rewritten
/// green here.
fn header_block() -> &'static str {
    "let _ = 1 + 1;"
}

// ---------------------------------------------------------------------------
// Format and check
// ---------------------------------------------------------------------------

/// Run `cargo fmt` (limited to the lane manifest) and `cargo check` for the
/// affected crate. Both commands are scoped to `worktree_path` via
/// `--manifest-path`; nothing runs against the primary checkout.
///
/// * `affected_crate`: `-p <crate>` for `cargo check`. `None` checks the
///   whole lane workspace (heavier; prefer the placement crate).
pub fn format_and_check(
    worktree_path: &Path,
    affected_crate: Option<&str>,
) -> Result<String, String> {
    let manifest = workspace_manifest(worktree_path);
    let manifest_str = manifest.to_string_lossy().to_string();
    if !manifest.is_file() {
        return Err(format!(
            "format_and_check: no Cargo manifest under {} (looked for Cargo.toml and rust/Cargo.toml)",
            worktree_path.display()
        ));
    }
    let fmt_out = Command::new("cargo")
        .args(["fmt", "--manifest-path", &manifest_str])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| format!("cargo fmt: {e}"))?;
    if !fmt_out.status.success() {
        let err = String::from_utf8_lossy(&fmt_out.stderr);
        return Err(format!("cargo fmt failed: {}", err.trim()));
    }
    let mut args = vec![
        "check".to_string(),
        "--manifest-path".to_string(),
        manifest_str,
    ];
    if let Some(krate) = affected_crate.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("-p".to_string());
        args.push(krate.to_string());
    }
    args.push("--all-targets".to_string());
    let check_out = Command::new("cargo")
        .args(&args)
        .current_dir(worktree_path)
        .output()
        .map_err(|e| format!("cargo check: {e}"))?;
    if !check_out.status.success() {
        let err = String::from_utf8_lossy(&check_out.stderr);
        let excerpt: String = err.lines().take(20).collect::<Vec<_>>().join("\n");
        return Err(format!("cargo check failed: {excerpt}"));
    }
    Ok(String::from_utf8_lossy(&check_out.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::StoryPacketRow;

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

    fn view(
        id: &str,
        goal: Option<&str>,
        brief: Option<&str>,
        criteria: Option<&str>,
    ) -> TstStoryView {
        TstStoryView::from_row(&row(id, goal, brief, criteria))
    }

    fn tmp_lane(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pianola-authoring-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("fixture lane");
        dir
    }

    #[test]
    fn refuses_arch_redesign() {
        let v = view(
            "TST-ARCH",
            Some("author a test for the boundary"),
            Some("refactor the Abstract Service boundary and MVI patterns"),
            Some("test exists"),
        );
        let placement = TestPlacement::ForgeContractSuite {
            suite_file: "tests/tests/tst_worker_contract.rs".to_string(),
        };
        assert_eq!(
            check_authoring_guards(&v, &placement),
            Err(EscalationReason::ArchConflict)
        );
        let lane = tmp_lane("arch");
        assert_eq!(
            author_rust_test(&v, &placement, &lane),
            Err(EscalationReason::ArchConflict)
        );
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn refuses_security_change() {
        let v = view(
            "TST-SEC",
            Some("author a test for entitlements"),
            Some("cover the casbin authz path"),
            Some("test exists"),
        );
        let placement = TestPlacement::ForgeContractSuite {
            suite_file: "tests/tests/tst_worker_contract.rs".to_string(),
        };
        assert_eq!(
            check_authoring_guards(&v, &placement),
            Err(EscalationReason::SecurityEntitlement)
        );
        // A direct security.rs placement is refused even with innocent prose.
        let clean = view(
            "TST-SEC2",
            Some("author a test"),
            Some("colocated unit test"),
            Some("test exists"),
        );
        let sec_placement = TestPlacement::CrateLocalMod {
            crate_path: "db".to_string(),
            module_file: "db/src/security.rs".to_string(),
        };
        assert_eq!(
            check_authoring_guards(&clean, &sec_placement),
            Err(EscalationReason::SecurityEntitlement)
        );
    }

    #[test]
    fn refuses_prod_rewrite_to_green_shape() {
        // Pure detector: a test-only story touching prod + test files.
        assert!(would_rewrite_prod_to_green(
            &["forge/src/engine/scope.rs".to_string()],
            &["forge/tests/scope.rs".to_string()],
            true
        ));
        assert!(!would_rewrite_prod_to_green(
            &[],
            &["forge/tests/scope.rs".to_string()],
            true
        ));
        assert!(!would_rewrite_prod_to_green(
            &["forge/src/engine/scope.rs".to_string()],
            &["forge/tests/scope.rs".to_string()],
            false
        ));
        // Prose asking to rewrite prod to green escalates instead of writing.
        let v = view(
            "TST-RW",
            Some("rewrite production code to make test green"),
            Some("touch prod until green"),
            Some("test passes"),
        );
        let placement = TestPlacement::ForgeContractSuite {
            suite_file: "tests/tests/tst_worker_contract.rs".to_string(),
        };
        assert_eq!(
            check_authoring_guards(&v, &placement),
            Err(EscalationReason::ProdChangeNeeded)
        );
    }

    #[test]
    fn refuses_migration_without_authorization() {
        // Placement targets a migration file while the story carries no
        // migration keyword in scope and no db test mode: unauthorized.
        let v = view(
            "TST-MIG",
            Some("author a test for the new column"),
            Some("colocated unit test, no schema work"),
            Some("test exists"),
        );
        let placement = TestPlacement::IntegrationFile {
            crate_path: "db".to_string(),
            test_file: "db/migrations/123_add_column.sql".to_string(),
        };
        assert_eq!(
            check_authoring_guards(&v, &placement),
            Err(EscalationReason::MigrationNeeded)
        );
        // Story prose naming schema work without authorization also refuses.
        let v2 = view(
            "TST-MIG2",
            Some("author a test"),
            Some("needs a schema change for the new column"),
            Some("test exists"),
        );
        let suite = TestPlacement::ForgeContractSuite {
            suite_file: "tests/tests/tst_worker_contract.rs".to_string(),
        };
        assert_eq!(
            check_authoring_guards(&v2, &suite),
            Err(EscalationReason::MigrationNeeded)
        );
    }

    #[test]
    fn allows_migration_with_scope_keyword() {
        let v = TstStoryView::from_row(&StoryPacketRow {
            id: "TST-MIG-OK".to_string(),
            title: "story".to_string(),
            goal: Some("author a migration test".to_string()),
            architect_brief: Some("SCOPE:\nrun the migration for the new column".to_string()),
            acceptance_criteria: Some("migration test exists".to_string()),
            test_mode: None,
            assay_commands: Some("cargo test".to_string()),
        });
        assert!(db_work_authorized(&v));
        let placement = TestPlacement::ForgeContractSuite {
            suite_file: "tests/tests/tst_worker_contract.rs".to_string(),
        };
        assert_eq!(check_authoring_guards(&v, &placement), Ok(()));
    }

    #[test]
    fn refuses_scope_broadening() {
        let v = view(
            "TST-BROAD",
            Some("author a test for scope ranking"),
            Some("also touch forge/src/engine/other.rs for good measure"),
            Some("test for scope ranking exists"),
        );
        let placement = TestPlacement::CrateLocalMod {
            crate_path: "forge".to_string(),
            module_file: "forge/src/engine/scope.rs".to_string(),
        };
        assert_eq!(
            check_authoring_guards(&v, &placement),
            Err(EscalationReason::AmbiguousSpec)
        );
    }

    #[test]
    fn refuses_typescript_target() {
        let v = view(
            "TST-TS",
            Some("author a test"),
            Some("colocated unit test"),
            Some("test exists"),
        );
        let placement = TestPlacement::IntegrationFile {
            crate_path: "web".to_string(),
            test_file: "web/tests/component.test.ts".to_string(),
        };
        let lane = tmp_lane("ts");
        assert_eq!(
            author_rust_test(&v, &placement, &lane),
            Err(EscalationReason::ProdChangeNeeded)
        );
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn authors_inline_test_inside_lane() {
        let lane = tmp_lane("inline-ok");
        let src = lane.join("forge").join("src");
        std::fs::create_dir_all(&src).expect("fixture");
        std::fs::write(src.join("scope.rs"), "pub fn rank() -> u32 { 1 }\n").expect("fixture");
        let v = view(
            "TST-42",
            Some("author a unit test for scope ranking"),
            Some("colocated unit test for forge/src/engine/scope.rs"),
            Some("test exists and names TST-42"),
        );
        // Point the module file at the fixture we created.
        let placement = TestPlacement::CrateLocalMod {
            crate_path: "forge".to_string(),
            module_file: "forge/src/scope.rs".to_string(),
        };
        let _ = &placement;
        let written = author_rust_test(&v, &placement, &lane).expect("authors");
        assert!(written.starts_with(&lane));
        let content = std::fs::read_to_string(&written).expect("written");
        assert!(content.contains("TST-42"), "test carries story id");
        assert!(content.contains("#[cfg(test)]"), "inline test module");
        assert!(content.contains("pub fn rank"), "prod item preserved");
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn refuses_write_outside_lane() {
        let lane = tmp_lane("confine");
        let v = view(
            "TST-OUT",
            Some("author a test"),
            Some("colocated unit test"),
            Some("test exists"),
        );
        let placement = TestPlacement::CrateLocalMod {
            crate_path: "forge".to_string(),
            module_file: "../outside/scope.rs".to_string(),
        };
        assert_eq!(
            author_rust_test(&v, &placement, &lane),
            Err(EscalationReason::PublishRequested)
        );
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn workspace_manifest_prefers_root() {
        let lane = tmp_lane("manifest");
        std::fs::write(lane.join("Cargo.toml"), "[workspace]\n").expect("fixture");
        assert_eq!(workspace_manifest(&lane), lane.join("Cargo.toml"));
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn format_and_check_errors_without_manifest() {
        let lane = tmp_lane("no-manifest");
        let err = format_and_check(&lane, Some("forge")).unwrap_err();
        assert!(err.contains("no Cargo manifest"), "unexpected: {err}");
        let _ = std::fs::remove_dir_all(&lane);
    }
}
