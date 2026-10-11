//! Cheap, scope-aware diagnostics for Smith's own work, before candidate freeze. They are Smith's feedback, never QA evidence:
//! a passing diagnostic says nothing about acceptance, and nothing here marks a QA result.
//!
//! THE SCOPE RULE. A change confined to one crate gets a targeted `cargo check` of that crate. A change to a manifest or a
//! workspace-level file gets a workspace check, because its effect is not confined. A change to no Rust source (docs, data)
//! gets no compile at all. Tests are not run here: running the affected tests is QA's job, against the frozen candidate.
//!
//! This module plans the commands and interprets their outcome. Running them is the caller's job, through the existing
//! execution path, so the same permission rules apply.

use serde::{Deserialize, Serialize};

/// How far a change reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticScope {
    /// Nothing that compiles changed.
    None,
    /// The change is confined to these crates.
    Crates,
    /// The change touches the workspace itself (a root manifest, a lockfile, a toolchain file).
    Workspace,
}

/// The planned diagnostics for one set of changed paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticPlan {
    pub scope: DiagnosticScope,
    /// Ordered commands to run, each a plain argument list.
    pub commands: Vec<Vec<String>>,
}

/// A crate in the workspace: its name and the directory its sources live under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceCrate {
    pub name: String,
    pub dir: String,
}

/// Workspace-level files whose change is never confined to one crate.
const WORKSPACE_FILES: [&str; 4] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain",
    "rust-toolchain.toml",
];

/// Plan the diagnostics for `changed` paths (repository-relative), given the workspace `crates`. Deterministic: crates are
/// reported in the order the workspace lists them, and an unknown path is not guessed into a crate.
pub fn plan(changed: &[&str], crates: &[WorkspaceCrate]) -> DiagnosticPlan {
    let rust_changed: Vec<&str> = changed
        .iter()
        .copied()
        .filter(|path| {
            path.ends_with(".rs")
                || path.ends_with("Cargo.toml")
                || WORKSPACE_FILES.contains(&file_name(path))
        })
        .collect();
    if rust_changed.is_empty() {
        return DiagnosticPlan {
            scope: DiagnosticScope::None,
            commands: vec![],
        };
    }
    let touches_workspace = rust_changed.iter().any(|path| {
        WORKSPACE_FILES.contains(&file_name(path)) && !path.contains('/')
            || path.ends_with("Cargo.lock")
    });
    if touches_workspace {
        return DiagnosticPlan {
            scope: DiagnosticScope::Workspace,
            commands: vec![vec!["cargo".into(), "check".into(), "--workspace".into()]],
        };
    }
    let mut named: Vec<&str> = Vec::new();
    for krate in crates {
        let prefix = format!("{}/", krate.dir.trim_end_matches('/'));
        if rust_changed.iter().any(|path| path.starts_with(&prefix)) {
            named.push(&krate.name);
        }
    }
    if named.is_empty() {
        // Rust changed, but outside any known crate: the only honest scope is the whole workspace.
        return DiagnosticPlan {
            scope: DiagnosticScope::Workspace,
            commands: vec![vec!["cargo".into(), "check".into(), "--workspace".into()]],
        };
    }
    DiagnosticPlan {
        scope: DiagnosticScope::Crates,
        commands: named
            .into_iter()
            .map(|name| {
                vec![
                    "cargo".into(),
                    "check".into(),
                    "-p".into(),
                    name.to_string(),
                ]
            })
            .collect(),
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The result of running the planned commands. Measured; it carries no QA verdict and never marks one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticOutcome {
    pub commands_run: usize,
    pub failures: usize,
    /// The first failing command, so the failure is named rather than summarized away.
    pub first_failure: Option<String>,
}

impl DiagnosticOutcome {
    pub fn passed(&self) -> bool {
        self.failures == 0
    }
}

/// Fold per-command results (command text, passed) into an outcome. Every command is counted; the first failure is kept.
pub fn interpret(results: &[(String, bool)]) -> DiagnosticOutcome {
    let failures: Vec<&String> = results
        .iter()
        .filter(|(_, passed)| !passed)
        .map(|(command, _)| command)
        .collect();
    DiagnosticOutcome {
        commands_run: results.len(),
        failures: failures.len(),
        first_failure: failures.first().map(|command| (*command).clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crates() -> Vec<WorkspaceCrate> {
        vec![
            WorkspaceCrate {
                name: "forge".into(),
                dir: "forge".into(),
            },
            WorkspaceCrate {
                name: "model".into(),
                dir: "middle/model".into(),
            },
        ]
    }

    #[test]
    fn a_change_to_docs_alone_needs_no_compile() {
        let plan = plan(&["docs/agent/notes.md"], &crates());
        assert_eq!(plan.scope, DiagnosticScope::None);
        assert!(plan.commands.is_empty());
    }

    #[test]
    fn a_change_inside_one_crate_checks_that_crate_only() {
        let plan = plan(&["forge/src/engine/planning.rs"], &crates());
        assert_eq!(plan.scope, DiagnosticScope::Crates);
        assert_eq!(
            plan.commands,
            vec![vec![
                "cargo".to_string(),
                "check".into(),
                "-p".into(),
                "forge".into()
            ]]
        );
    }

    #[test]
    fn changes_in_two_crates_check_each_in_workspace_order() {
        let plan = plan(&["middle/model/src/a.rs", "forge/src/b.rs"], &crates());
        let names: Vec<&str> = plan.commands.iter().map(|c| c[3].as_str()).collect();
        assert_eq!(names, vec!["forge", "model"]);
    }

    #[test]
    fn a_root_manifest_change_is_workspace_scope() {
        let plan = plan(&["Cargo.toml"], &crates());
        assert_eq!(plan.scope, DiagnosticScope::Workspace);
        assert_eq!(plan.commands[0][1], "check");
        assert_eq!(plan.commands[0][2], "--workspace");
    }

    #[test]
    fn a_lockfile_change_is_workspace_scope() {
        assert_eq!(
            plan(&["Cargo.lock"], &crates()).scope,
            DiagnosticScope::Workspace
        );
    }

    #[test]
    fn rust_outside_any_known_crate_is_never_guessed_into_one() {
        let plan = plan(&["scripts/tool.rs"], &crates());
        assert_eq!(plan.scope, DiagnosticScope::Workspace);
    }

    #[test]
    fn diagnostics_never_plan_a_test_run() {
        let plan = plan(&["forge/src/x.rs"], &crates());
        assert!(plan.commands.iter().flatten().all(|arg| arg != "test"));
    }

    #[test]
    fn the_first_failure_is_named_and_every_command_is_counted() {
        let outcome = interpret(&[
            ("cargo check -p forge".into(), true),
            ("cargo check -p model".into(), false),
            ("cargo check -p db".into(), false),
        ]);
        assert_eq!(outcome.commands_run, 3);
        assert_eq!(outcome.failures, 2);
        assert_eq!(
            outcome.first_failure.as_deref(),
            Some("cargo check -p model")
        );
        assert!(!outcome.passed());
    }
}
