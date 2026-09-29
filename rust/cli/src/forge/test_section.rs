//! Rust replacement for the former test-section and test-sections scripts.
//! Keeps the operator-facing pnpm commands while routing test execution to Rust crates.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use super::{repo_root, Failure};

#[derive(Clone, Copy)]
struct Section {
    name: &'static str,
    area: &'static str,
    about: &'static str,
    historical: bool,
    crates: &'static [&'static str],
}

const SECTIONS: &[Section] = &[
    Section {
        name: "forge-engine",
        area: "FORGE",
        about: "the engine: waves, dispatch, routing, contracts, gates",
        historical: false,
        crates: &["forge", "workflow", "cli"],
    },
    Section {
        name: "forge-verify",
        area: "FORGE",
        about: "verification itself: acceptance, assertions, receipts, migrations",
        historical: true,
        crates: &["cli"],
    },
    Section {
        name: "forge-runtime",
        area: "FORGE",
        about: "the agent runtime: adapters, sessions, workspaces, invokers",
        historical: true,
        crates: &["forge"],
    },
    Section {
        name: "app-crm",
        area: "APP",
        about: "the book of business: clients, deals, documents, deadlines",
        historical: true,
        crates: &["domain", "db"],
    },
    Section {
        name: "app-intake",
        area: "APP",
        about: "intake: mail, messages, Apple surfaces, calendar, conversations",
        historical: true,
        crates: &["integrations", "server"],
    },
    Section {
        name: "app-money",
        area: "APP",
        about: "money: accounting, banking, statements",
        historical: true,
        crates: &["server", "db"],
    },
    Section {
        name: "app-identity",
        area: "APP",
        about: "identity and access: auth, entitlements, people",
        historical: true,
        crates: &["server", "db", "service"],
    },
    Section {
        name: "app-portal",
        area: "APP",
        about: "the portal surface: navigation, views, boards, readiness",
        historical: false,
        crates: &["ui"],
    },
    Section {
        name: "app-core",
        area: "APP",
        about: "shared app core: commands, db seams, contracts",
        historical: false,
        crates: &["db", "domain", "service", "server", "integrations"],
    },
    Section {
        name: "harness",
        area: "HARNESS",
        about: "the guardrails themselves: manifests, packets, protections",
        historical: false,
        crates: &["cli"],
    },
];

fn contains_any(path: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| path.contains(needle))
}

fn section_for_file(path: &str) -> Option<&'static str> {
    let path = path.trim_start_matches("./").replace('\\', "/");

    if path.starts_with("scripts/") {
        return Some("harness");
    }
    if path.starts_with("rust/cli/src/forge/")
        || path.starts_with("rust/forge/")
        || path.starts_with("rust/core/workflow/")
    {
        return Some("forge-engine");
    }
    if path.starts_with("rust/ui/") {
        return Some("app-portal");
    }
    if path.starts_with("rust/core/")
        || path.starts_with("rust/server/")
        || path.starts_with("rust/integrations/")
    {
        return Some("app-core");
    }
    if path.starts_with("rust/cli/") {
        return Some("harness");
    }
    if path.starts_with("agent-runtime/") {
        return Some("forge-runtime");
    }
    if path.starts_with("legacy/workflow_app/tests/forge-") {
        return Some("forge-engine");
    }
    if contains_any(
        &path,
        &[
            "claim-clock",
            "dynamic-fork",
            "execution-graph",
            "agent-scheduler",
            "agent-work",
            "concurrency",
            "db-routing",
        ],
    ) {
        return Some("forge-engine");
    }
    if path.starts_with("legacy/workflow_app/tests/")
        && contains_any(
            &path,
            &[
                "acceptance",
                "assertion-",
                "evidence",
                "migration-",
                "release-",
                "verify-",
                "qa-",
                "receipt",
            ],
        )
    {
        return Some("forge-verify");
    }
    if contains_any(
        &path,
        &[
            "apple-",
            "applemail",
            "whatsapp",
            "calendar-",
            "conversation",
            "intake",
            "message",
            "mail",
        ],
    ) {
        return Some("app-intake");
    }
    if contains_any(
        &path,
        &["accounting", "bank-", "ofx", "pnl", "expense", "payment"],
    ) {
        return Some("app-money");
    }
    if contains_any(
        &path,
        &[
            "auth",
            "identity",
            "entitlement",
            "people",
            "dev-bypass",
            "secret-binding",
            "favorite",
        ],
    ) {
        return Some("app-identity");
    }
    if contains_any(
        &path,
        &[
            "client",
            "deal",
            "closing",
            "document",
            "deadline",
            "broker",
            "showing",
            "property",
            "signature",
            "agreement",
            "crm",
        ],
    ) {
        return Some("app-crm");
    }
    if path.starts_with("app/")
        || path.starts_with("components/")
        || contains_any(
            &path,
            &[
                "portal",
                "navigation",
                "storyboard",
                "flight-recorder",
                "readiness",
            ],
        )
    {
        return Some("app-portal");
    }
    if path.starts_with("legacy/workflow_app/tests/")
        || path.starts_with("legacy/db/")
        || path.starts_with("lib/")
    {
        return Some("app-core");
    }
    None
}

fn holds_tests(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if normalized.ends_with(".test.ts") {
        return true;
    }
    if path.extension().and_then(|value| value.to_str()) != Some("rs") {
        return false;
    }
    if normalized.contains("/tests/") || normalized.ends_with("_test.rs") {
        return true;
    }
    fs::read_to_string(path)
        .map(|content| content.contains("#[cfg(test)]"))
        .unwrap_or(false)
}

fn walk_tests(root: &Path, dir: &Path, found: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), "target" | "node_modules" | "dist") || name.starts_with('.') {
                continue;
            }
            walk_tests(root, &path, found);
        } else if holds_tests(&path) {
            if let Ok(rel) = path.strip_prefix(root) {
                found.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

fn list_test_files(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for tree in [
        "rust",
        "scripts",
        "legacy/workflow_app/tests",
        "testv2/engine_tests",
        "agent-runtime",
    ] {
        let path = root.join(tree);
        if path.exists() {
            walk_tests(root, &path, &mut found);
        }
    }
    found.sort();
    found
}

fn section_report(root: &Path) -> (BTreeMap<&'static str, Vec<String>>, Vec<String>) {
    let mut by_section = BTreeMap::new();
    for section in SECTIONS {
        by_section.insert(section.name, Vec::new());
    }
    let mut unclassified = Vec::new();
    for file in list_test_files(root) {
        match section_for_file(&file) {
            Some(section) if by_section.contains_key(section) => by_section
                .get_mut(section)
                .expect("section inserted")
                .push(file),
            _ => unclassified.push(file),
        }
    }
    (by_section, unclassified)
}

fn changed_paths(root: &Path, since: Option<&str>) -> Result<Vec<String>, Failure> {
    let output = if let Some(reference) = since {
        Command::new("git")
            .current_dir(root)
            .args(["diff", "--name-only", &format!("{reference}...HEAD")])
            .output()
    } else {
        Command::new("git")
            .current_dir(root)
            .args(["status", "--porcelain"])
            .output()
    }
    .map_err(|error| Failure::configuration(format!("cannot run git for test-section: {error}")))?;

    if !output.status.success() {
        return Err(Failure::failed("git could not determine changed paths"));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let value = if since.is_some() {
                line.trim()
            } else {
                line.get(3..).unwrap_or("").trim()
            };
            (!value.is_empty() && !value.contains(" -> ")).then(|| value.to_string())
        })
        .collect())
}

fn sections_for_paths(paths: &[String]) -> Vec<&'static str> {
    let mut touched = BTreeSet::new();
    for path in paths {
        if path.starts_with("docs/") || path.ends_with(".md") {
            continue;
        }
        if path.starts_with("legacy/workflow_app/forge/") {
            touched.insert("forge-engine");
            touched.insert("forge-verify");
            continue;
        }
        if matches!(
            path.as_str(),
            "package.json"
                | "pnpm-lock.yaml"
                | "tsconfig.json"
                | "rust/Cargo.toml"
                | "rust/Cargo.lock"
        ) || path.starts_with("eslint")
        {
            touched.insert("harness");
            touched.insert("app-core");
            continue;
        }
        if let Some(section) = section_for_file(path) {
            touched.insert(section);
        }
    }
    touched.into_iter().collect()
}

fn parse_requested(args: &[String]) -> Result<Vec<String>, Failure> {
    let mut requested = Vec::new();
    let mut index = 0usize;
    while index < args.len() {
        if args[index] == "--since" {
            if index + 1 >= args.len() {
                return Err(Failure::usage("test-section --since requires a git ref"));
            }
            index += 2;
            continue;
        }
        if !args[index].starts_with("--") {
            requested.push(args[index].clone());
        }
        index += 1;
    }
    Ok(requested)
}

fn crates_for_sections(names: &[&str]) -> BTreeSet<&'static str> {
    let mut crates = BTreeSet::new();
    for name in names {
        if let Some(section) = SECTIONS.iter().find(|section| section.name == *name) {
            crates.extend(section.crates.iter().copied());
        }
    }
    crates
}

fn run_crates(root: &Path, sections: &[&str]) -> Result<u8, Failure> {
    let crates = crates_for_sections(sections);
    if crates.is_empty() {
        println!("nothing to run.");
        return Ok(0);
    }

    println!("running Rust crates for sections: {}", sections.join(", "));
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .arg("test")
        .arg("--manifest-path")
        .arg(root.join("rust/Cargo.toml"))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    for krate in crates {
        command.arg("-p").arg(krate);
    }

    let status = command
        .status()
        .map_err(|error| Failure::configuration(format!("cannot run cargo test: {error}")))?;

    if status.success() {
        Ok(0)
    } else {
        Err(Failure::failed(format!("cargo test exited with {status}")))
    }
}

pub fn run(args: &[String]) -> Result<u8, Failure> {
    let root = repo_root();
    let (report, unclassified) = section_report(&root);

    if args.is_empty() || args.iter().any(|arg| arg == "--list") {
        let total: usize = report.values().map(Vec::len).sum();
        let mut by_area: BTreeMap<&str, usize> = BTreeMap::new();

        for section in SECTIONS {
            let count = report.get(section.name).map(Vec::len).unwrap_or(0);
            println!(
                "{:<8} {:<14} {:>4} files  {}",
                section.area, section.name, count, section.about
            );
            *by_area.entry(section.area).or_default() += count;
            if count == 0 && !section.historical {
                eprintln!(
                    "warning: {} is empty and is not marked historical",
                    section.name
                );
            }
        }

        println!(
            "\ntotal {total} test files · APP {} · FORGE {} · HARNESS {}",
            by_area.get("APP").copied().unwrap_or(0),
            by_area.get("FORGE").copied().unwrap_or(0),
            by_area.get("HARNESS").copied().unwrap_or(0)
        );

        if !unclassified.is_empty() {
            eprintln!("\n{} test file(s) unclassified:", unclassified.len());
            for file in unclassified.iter().take(10) {
                eprintln!("  ? {file}");
            }
            return Err(Failure::failed(
                "test-section taxonomy has unclassified test files",
            ));
        }

        return Ok(0);
    }

    let selected = if args.iter().any(|arg| arg == "--changed") {
        let since = args
            .iter()
            .position(|arg| arg == "--since")
            .and_then(|index| args.get(index + 1))
            .map(String::as_str);
        let changed = changed_paths(&root, since)?;
        let selected = sections_for_paths(&changed);

        println!(
            "changed since {}: {} file(s)",
            since.unwrap_or("working tree"),
            changed.len()
        );
        println!(
            "sections to run: {}",
            if selected.is_empty() {
                "(none)".into()
            } else {
                selected.join(", ")
            }
        );
        selected
    } else {
        let requested = parse_requested(args)?;
        let mut selected = BTreeSet::new();

        for want in requested {
            if let Some(section) = SECTIONS.iter().find(|section| section.name == want) {
                selected.insert(section.name);
            } else if matches!(want.as_str(), "APP" | "FORGE" | "HARNESS") {
                for section in SECTIONS.iter().filter(|section| section.area == want) {
                    selected.insert(section.name);
                }
            } else {
                return Err(Failure::usage(format!(
                    "unknown section {want:?} — try: pnpm test:sections"
                )));
            }
        }

        selected.into_iter().collect()
    };

    run_crates(&root, &selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_fence_paths_keep_their_sections() {
        assert_eq!(
            section_for_file("legacy/workflow_app/tests/claim-clock.test.ts"),
            Some("forge-engine")
        );
        assert_eq!(
            section_for_file("legacy/workflow_app/tests/qa-disposition-vocab.test.ts"),
            Some("forge-verify")
        );
        assert_eq!(
            section_for_file("agent-runtime/lead-decision.test.ts"),
            Some("forge-runtime")
        );
        assert_eq!(
            section_for_file("scripts/protected-files.test.ts"),
            Some("harness")
        );
        assert_eq!(
            section_for_file("legacy/workflow_app/tests/clients-pagination.test.ts"),
            Some("app-crm")
        );
        assert_eq!(
            section_for_file("legacy/workflow_app/tests/auth-boundary.test.ts"),
            Some("app-identity")
        );
    }

    #[test]
    fn rust_tree_is_classified_by_crate() {
        assert_eq!(
            section_for_file("rust/forge/src/scope_manifest.rs"),
            Some("forge-engine")
        );
        assert_eq!(
            section_for_file("rust/cli/src/forge/manifest.rs"),
            Some("forge-engine")
        );
        assert_eq!(
            section_for_file("rust/core/workflow/src/types.rs"),
            Some("forge-engine")
        );
        assert_eq!(
            section_for_file("rust/server/src/api/engine.rs"),
            Some("app-core")
        );
        assert_eq!(
            section_for_file("rust/ui/src/update.rs"),
            Some("app-portal")
        );
        assert_eq!(
            section_for_file("rust/cli/src/main.rs"),
            Some("harness")
        );
    }

    #[test]
    fn changed_paths_map_to_expected_sections() {
        assert_eq!(
            sections_for_paths(&["legacy/workflow_app/forge/forge-executor.ts".into()]),
            vec!["forge-engine", "forge-verify"]
        );
        assert_eq!(
            sections_for_paths(&["agent-runtime/invoker.ts".into()]),
            vec!["forge-runtime"]
        );
        assert_eq!(
            sections_for_paths(&["components/portal/x.tsx".into()]),
            vec!["app-portal"]
        );
        assert!(sections_for_paths(&["docs/agent/MEMORY.md".into()]).is_empty());
        assert_eq!(
            sections_for_paths(&["rust/forge/src/engine/runner.rs".into()]),
            vec!["forge-engine"]
        );
        assert_eq!(
            sections_for_paths(&["rust/core/db/src/pool.rs".into()]),
            vec!["app-core"]
        );
        assert_eq!(
            sections_for_paths(&["rust/Cargo.lock".into()]),
            vec!["app-core", "harness"]
        );
    }

    #[test]
    fn every_section_has_a_rust_execution_target() {
        assert!(
            SECTIONS
                .iter()
                .all(|section| !section.crates.is_empty() && section.about.len() > 20)
        );
    }
}
