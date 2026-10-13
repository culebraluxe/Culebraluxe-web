//! Cheap, scope-aware diagnostics for Smith's own work, before candidate freeze. They are Smith's feedback, never QA evidence:
//! a passing diagnostic says nothing about acceptance, and nothing here marks a QA result.
//!
//! THE SCOPE RULE. A change confined to one crate gets a targeted `cargo check` of that crate. A change to a manifest or a
//! workspace-level file gets a workspace check, because its effect is not confined. A change to no Rust source (docs, data)
//! gets no compile at all. Tests are not run here: running the affected tests is QA's job, against the frozen candidate.
//!
//! This module plans the commands and interprets their outcome; running them is the caller's job, through the existing
//! execution path, so the same permission rules apply. The caller is Smith's own pre-freeze judgement
//! ([`crate::roles::smith::judge_delivered_candidate_in_surface`]), which asks these questions of every candidate
//! before that candidate can be delivered.

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

/// The command that asks cargo for the workspace's own crate list — cargo's reading of the manifests, so the crates
/// named here are exactly the crates `cargo check -p` will resolve, and a crate the candidate itself added is in it.
///
/// `--no-deps` because a dependency's package is not a crate of this workspace and must never be planned into a
/// scoped check; `--manifest-path` because the caller runs it in a worktree rather than a checkout.
pub fn workspace_crates_command(manifest_path: &str) -> Vec<String> {
    vec![
        "cargo".into(),
        "metadata".into(),
        "--no-deps".into(),
        "--format-version".into(),
        "1".into(),
        "--manifest-path".into(),
        manifest_path.into(),
    ]
}

/// Read cargo's own answer (`workspace_crates_command`'s stdout) into the workspace's crates, each `dir` relative to
/// the workspace root so it compares against a repository-relative changed path.
///
/// `None` when the document is not that document: an unreadable workspace is NOT an empty one, because the two mean
/// opposite things to a caller — one says "cannot plan", the other says "planned, nothing to run" — and a caller that
/// conflated them would refuse a candidate over a workspace it merely failed to read.
///
/// Membership comes from `workspace_members` when cargo states it, so a package the document carries for some other
/// reason is never mistaken for a crate of this workspace. A crate whose manifest IS the workspace root keeps the
/// empty directory, which matches no changed path and is therefore never guessed into one (the same rule
/// [`plan`] applies to a path outside every known crate).
pub fn workspace_crates_from_metadata(json: &str) -> Option<Vec<WorkspaceCrate>> {
    let document: serde_json::Value = serde_json::from_str(json).ok()?;
    let root = document.get("workspace_root")?.as_str()?;
    let prefix = format!("{}/", root.trim_end_matches('/'));
    let members = document
        .get("workspace_members")
        .and_then(|value| value.as_array())
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str())
                .collect::<std::collections::HashSet<&str>>()
        });
    let crates: Vec<WorkspaceCrate> = document
        .get("packages")?
        .as_array()?
        .iter()
        .filter(
            |package| match (&members, package.get("id").and_then(|id| id.as_str())) {
                // Membership is stated: a package outside it is not a crate of this workspace. A package that states no
                // id cannot be *proved* to be a member, and is left out rather than guessed in.
                (Some(members), Some(id)) => members.contains(id),
                (Some(_), None) => false,
                // Membership is not stated at all (`--no-deps` lists the members, and that is the only shape this reads).
                (None, _) => true,
            },
        )
        .filter_map(|package| {
            let name = package.get("name")?.as_str()?.to_string();
            let manifest = package.get("manifest_path")?.as_str()?;
            let dir = manifest
                .strip_prefix(&prefix)
                .unwrap_or(manifest)
                .trim_end_matches("Cargo.toml")
                .trim_end_matches('/')
                .to_string();
            Some(WorkspaceCrate { name, dir })
        })
        .collect();
    (!crates.is_empty()).then_some(crates)
}

/// One planned command as the single string it is reported, compared and run as.
///
/// ONE OWNER of the text form, because a caller that runs the commands and a gate that compares them to the plan
/// must agree character for character — two `join(" ")`s in two files is how a plan and its results drift apart
/// into a permanent "the results do not match the planned set".
pub fn command_text(args: &[String]) -> String {
    args.join(" ")
}

/// Workspace-level files whose change is never confined to one crate.
const WORKSPACE_FILES: [&str; 4] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain",
    "rust-toolchain.toml",
];

/// One changed path that does not have to compile.
fn compilable(path: &str) -> bool {
    path.ends_with(".rs")
        || path.ends_with("Cargo.toml")
        || WORKSPACE_FILES.contains(&file_name(path))
}

/// Whether any of `changed` has to compile.
///
/// The same rule [`plan`] plans by, exposed so a caller can decide whether to ask cargo for the workspace's crates at
/// all: a change that compiles nothing (docs, data, an image) owes no diagnostic, and reading a manifest to discover
/// that would be a cargo invocation spent to learn there is nothing to do.
pub fn any_path_compiles(changed: &[&str]) -> bool {
    changed.iter().copied().any(compilable)
}

/// Plan the diagnostics for `changed` paths (repository-relative), given the workspace `crates`. Deterministic: crates are
/// reported in the order the workspace lists them, and an unknown path is not guessed into a crate.
pub fn plan(changed: &[&str], crates: &[WorkspaceCrate]) -> DiagnosticPlan {
    let rust_changed: Vec<&str> = changed
        .iter()
        .copied()
        .filter(|path| compilable(path))
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
    /// Every command result, in execution order, so the caller can prove the planned set was completed.
    pub results: Vec<DiagnosticCommandResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCommandResult {
    pub command: String,
    pub passed: bool,
}

impl DiagnosticOutcome {
    pub fn passed(&self) -> bool {
        self.failures == 0 && self.results.iter().all(|result| result.passed)
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
        results: results
            .iter()
            .map(|(command, passed)| DiagnosticCommandResult {
                command: command.clone(),
                passed: *passed,
            })
            .collect(),
    }
}

/// A read-only candidate-freeze readiness answer. It never changes workflow state or produces QA evidence; the
/// authorized XML workflow remains responsible for deciding what happens after this check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreFreezeReadiness {
    pub ready: bool,
    pub reason: Option<String>,
    pub expected_commands: usize,
    pub completed_commands: usize,
    pub failed_commands: usize,
}

/// Check whether the planned diagnostics have a complete, successful result before a caller freezes a candidate.
/// This is the pure gate logic. Its production caller is Smith's pre-freeze judgement, where a `ready: false` refuses
/// the candidate in the lane's own refusal channel — cleared, named, and carried into the next attempt's repair
/// directive by the lifecycle.
pub fn pre_freeze_readiness(
    plan: &DiagnosticPlan,
    outcome: Option<&DiagnosticOutcome>,
) -> PreFreezeReadiness {
    let expected: Vec<String> = plan
        .commands
        .iter()
        .map(|args| command_text(args))
        .collect();
    let Some(outcome) = outcome else {
        let ready = expected.is_empty();
        return PreFreezeReadiness {
            ready,
            reason: (!ready).then(|| "planned diagnostics have not run".to_string()),
            expected_commands: expected.len(),
            completed_commands: 0,
            failed_commands: 0,
        };
    };

    let completed: Vec<String> = outcome
        .results
        .iter()
        .map(|result| result.command.clone())
        .collect();
    let exact_plan_ran = completed == expected && outcome.commands_run == expected.len();
    let result_failures = outcome
        .results
        .iter()
        .filter(|result| !result.passed)
        .count();
    let failed_commands = outcome.failures.max(result_failures);
    let failure_summary_matches = outcome.failures == result_failures
        && outcome.first_failure.as_deref()
            == outcome
                .results
                .iter()
                .find(|result| !result.passed)
                .map(|result| result.command.as_str());
    let reason = if !exact_plan_ran {
        Some("diagnostic results do not match the complete planned command set".to_string())
    } else if failed_commands > 0 {
        Some(format!(
            "diagnostics failed{}",
            outcome
                .first_failure
                .as_deref()
                .map(|command| format!(": {command}"))
                .unwrap_or_default()
        ))
    } else if !failure_summary_matches {
        Some("diagnostic summary does not match its command results".to_string())
    } else {
        None
    };
    PreFreezeReadiness {
        ready: reason.is_none(),
        reason,
        expected_commands: expected.len(),
        completed_commands: outcome.commands_run,
        failed_commands,
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
        // Pre-freeze diagnostics are developer feedback, never QA evidence: the planned commands may only check.
        // Every scope is covered (nothing to do, one crate, two crates, workspace, unknown Rust), so a future edit
        // that slips `cargo test` in here fails loudly instead of quietly running the suite before a freeze.
        for changed in [
            &["docs/agent/notes.md"][..],
            &["forge/src/x.rs"][..],
            &["middle/model/src/a.rs", "forge/src/b.rs"][..],
            &["Cargo.toml"][..],
            &["Cargo.lock"][..],
            &["scripts/tool.rs"][..],
        ] {
            let planned = plan(changed, &crates());
            assert!(
                planned.commands.iter().flatten().all(|arg| arg != "test"),
                "a diagnostic plan for {changed:?} named a test run"
            );
        }
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
        assert_eq!(outcome.results.len(), 3);
        assert!(!outcome.results[1].passed);
        assert_eq!(
            outcome.first_failure.as_deref(),
            Some("cargo check -p model")
        );
        assert!(!outcome.passed());
    }

    #[test]
    fn freeze_readiness_requires_every_planned_diagnostic_to_pass() {
        let plan = plan(&["forge/src/x.rs"], &crates());
        let missing = pre_freeze_readiness(&plan, None);
        assert!(!missing.ready);
        assert_eq!(
            missing.reason.as_deref(),
            Some("planned diagnostics have not run")
        );

        let incomplete = interpret(&[]);
        let blocked = pre_freeze_readiness(&plan, Some(&incomplete));
        assert!(!blocked.ready);
        assert!(blocked
            .reason
            .as_deref()
            .unwrap()
            .contains("complete planned"));

        let failed = interpret(&[("cargo check -p forge".into(), false)]);
        let blocked = pre_freeze_readiness(&plan, Some(&failed));
        assert!(!blocked.ready);
        assert!(blocked
            .reason
            .as_deref()
            .unwrap()
            .contains("cargo check -p forge"));

        let passed = interpret(&[("cargo check -p forge".into(), true)]);
        let ready = pre_freeze_readiness(&plan, Some(&passed));
        assert!(ready.ready);
        assert_eq!(ready.expected_commands, 1);
        assert_eq!(ready.completed_commands, 1);

        let substituted = interpret(&[("cargo check --workspace".into(), true)]);
        assert!(!pre_freeze_readiness(&plan, Some(&substituted)).ready);
    }

    #[test]
    fn a_non_rust_change_needs_no_diagnostic_run_before_freeze() {
        let plan = plan(&["docs/notes.md"], &crates());
        let readiness = pre_freeze_readiness(&plan, None);
        assert!(readiness.ready);
        assert_eq!(readiness.expected_commands, 0);
    }

    /// A workspace as cargo states it, in the two shapes the reader must respect: the absolute root to relativise
    /// against, and the member ids that say which packages are crates of THIS workspace.
    fn metadata(root: &str, members: &[&str], packages: &[(&str, &str, &str)]) -> String {
        let packages: Vec<serde_json::Value> = packages
            .iter()
            .map(|(id, name, manifest)| {
                serde_json::json!({ "id": id, "name": name, "manifest_path": manifest })
            })
            .collect();
        serde_json::json!({
            "workspace_root": root,
            "workspace_members": members,
            "packages": packages,
        })
        .to_string()
    }

    #[test]
    fn cargos_own_answer_becomes_the_workspaces_crates_with_root_relative_directories() {
        let json = metadata(
            "/repo",
            &[
                "path+file:///repo/app#0.1.0",
                "path+file:///repo/middle/lib#0.1.0",
            ],
            &[
                ("path+file:///repo/app#0.1.0", "app", "/repo/app/Cargo.toml"),
                (
                    "path+file:///repo/middle/lib#0.1.0",
                    "lib",
                    "/repo/middle/lib/Cargo.toml",
                ),
            ],
        );
        assert_eq!(
            workspace_crates_from_metadata(&json),
            Some(vec![
                WorkspaceCrate {
                    name: "app".into(),
                    dir: "app".into(),
                },
                WorkspaceCrate {
                    name: "lib".into(),
                    dir: "middle/lib".into(),
                },
            ]),
            "the directory is what a changed path is matched against, so it must be repository-relative"
        );
    }

    #[test]
    fn a_package_that_is_not_a_member_is_never_a_crate_to_check() {
        let json = metadata(
            "/repo",
            &["path+file:///repo/app#0.1.0"],
            &[
                ("path+file:///repo/app#0.1.0", "app", "/repo/app/Cargo.toml"),
                (
                    "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0",
                    "serde",
                    "/home/.cargo/registry/src/serde-1.0.0/Cargo.toml",
                ),
            ],
        );
        assert_eq!(
            workspace_crates_from_metadata(&json),
            Some(vec![WorkspaceCrate {
                name: "app".into(),
                dir: "app".into(),
            }]),
            "a dependency's package is not a crate of this workspace and must not be named in a plan"
        );
    }

    #[test]
    fn a_crate_at_the_workspace_root_matches_no_changed_path_rather_than_every_one() {
        let json = metadata(
            "/repo",
            &["path+file:///repo#0.1.0"],
            &[("path+file:///repo#0.1.0", "root", "/repo/Cargo.toml")],
        );
        let crates = workspace_crates_from_metadata(&json).expect("one crate");
        assert_eq!(crates[0].dir, "");
        // The whole point of the empty directory: it is not a prefix of anything, so a change anywhere in the tree is
        // planned as an unconfined one instead of being charged to whatever crate happens to sit at the root.
        assert_eq!(
            plan(&["src/main.rs"], &crates).scope,
            DiagnosticScope::Workspace
        );
    }

    #[test]
    fn a_manifest_that_is_not_cargos_answer_is_not_an_empty_workspace() {
        let empty = metadata("/repo", &[], &[]);
        for not_a_document in [
            "",
            "not json",
            "{}",
            // A document with no crates is `None`, never `Some(vec![])`: "cannot plan" and "nothing to run" are
            // opposite answers to a caller deciding whether to refuse a candidate.
            empty.as_str(),
            // No `workspace_root`: the directories could not be relativised, so every answer would be a guess.
            r#"{"packages":[{"id":"a","name":"app","manifest_path":"/repo/app/Cargo.toml"}]}"#,
        ] {
            assert_eq!(
                workspace_crates_from_metadata(not_a_document),
                None,
                "{not_a_document:?} must not read as a workspace with no crates"
            );
        }
    }

    #[test]
    fn the_crate_list_command_asks_cargo_for_the_workspace_and_checks_nothing() {
        let command = workspace_crates_command("/worktree/Cargo.toml");
        assert_eq!(command[0], "cargo");
        assert_eq!(command[1], "metadata");
        assert!(
            command.contains(&"--no-deps".to_string()),
            "a dependency's packages are not this workspace's crates: {command:?}"
        );
        assert_eq!(
            command.last().map(String::as_str),
            Some("/worktree/Cargo.toml"),
            "the worktree being read is named, never inferred from a working directory: {command:?}"
        );
        assert!(
            !command.iter().any(|arg| arg == "check" || arg == "test"),
            "reading the crate list is not a diagnostic: {command:?}"
        );
    }

    #[test]
    fn the_gate_and_its_caller_agree_on_a_commands_text() {
        // The caller runs `command_text` of each planned command; the gate compares exactly that to the plan. If the
        // two ever disagreed, a complete and successful run of the plan would read as an incomplete one.
        let plan = plan(&["forge/src/x.rs"], &crates());
        let results: Vec<(String, bool)> = plan
            .commands
            .iter()
            .map(|args| (command_text(args), true))
            .collect();
        let readiness = pre_freeze_readiness(&plan, Some(&interpret(&results)));
        assert!(readiness.ready, "{:?}", readiness.reason);
        assert_eq!(readiness.completed_commands, plan.commands.len());
    }
}
