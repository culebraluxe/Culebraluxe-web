//! FORGE-FIX-007 — Per-worker CARGO_TARGET_DIR + assay env isolation.
//!
//! Contract: Two concurrent assays from different worktrees build in different target dirs
//! (assert via env capture or dir creation). Assay env contains no provider keys /
//! DATABASE_URL_* (toolchain vars intact). Workspace-scoped assay failure in crate X does not
//! fail a story whose target set excludes X (or is recorded as build-fail per FORGE-FIX-003,
//! not test-fail).
//!
//! Level: L0 Unit (no database, no vendor process).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_build__001__per_worker_target_dir -- --nocapture

use forge::engine::opencode::{derive_cargo_target_dir, sanitize_assay_env, OpenCodeHarness};
use forge::engine::packet::{ExecutionWorkspace, StoryPacket};
use forge::engine::runner::RoleHarness;
use std::collections::HashMap;
use std::env;
use tempfile::TempDir;

const HARNESS: &str = "OpenCodeHarness/L0 Unit";

#[test]
fn per_worker_target_dirs_are_distinct() {
    // Create two temp directories simulating different worktrees
    let worktree1 = TempDir::new().expect("create worktree1");
    let worktree2 = TempDir::new().expect("create worktree2");

    let path1 = worktree1.path().to_path_buf();
    let path2 = worktree2.path().to_path_buf();

    // Derive target dirs for both worktrees
    let target_dir1 = derive_cargo_target_dir(&path1);
    let target_dir2 = derive_cargo_target_dir(&path2);

    // Target dirs must be distinct
    assert_ne!(
        target_dir1, target_dir2,
        "{HARNESS}: different worktrees must have different CARGO_TARGET_DIR"
    );

    // Both should be under /Users/Shared/dev/build/rust-<worktree-name>
    let target_dir1_str = target_dir1.to_string_lossy();
    let target_dir2_str = target_dir2.to_string_lossy();
    assert!(
        target_dir1_str.starts_with("/Users/Shared/dev/build/rust-"),
        "{HARNESS}: target_dir1 must be under shared build root: {target_dir1_str}"
    );
    assert!(
        target_dir2_str.starts_with("/Users/Shared/dev/build/rust-"),
        "{HARNESS}: target_dir2 must be under shared build root: {target_dir2_str}"
    );

    // The worktree names should be embedded in the target dir paths
    let name1 = path1
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let name2 = path2
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    assert!(
        target_dir1_str.contains(name1),
        "{HARNESS}: target_dir1 must contain worktree name: {target_dir1_str}"
    );
    assert!(
        target_dir2_str.contains(name2),
        "{HARNESS}: target_dir2 must contain worktree name: {target_dir2_str}"
    );
}

#[test]
fn run_command_sets_cargo_target_dir_and_sanitizes_env() {
    // Create a temp directory for the worktree
    let worktree = TempDir::new().expect("create worktree");
    let worktree_path = worktree.path().to_path_buf();

    // Set up some secret environment variables that should be filtered
    env::set_var("DATABASE_URL_PROD", "postgres://prod-secret");
    env::set_var("NEON_API_KEY", "neon-secret");
    env::set_var("APP_ENV", "production");
    env::set_var("TEST_TOOLCHAIN_VAR", "should-be-preserved");

    // Create a harness with execution_workspace pointing to the worktree
    let harness = OpenCodeHarness {
        cli_bin: "echo".into(), // dummy
        workspace: worktree_path.clone(),
        model: "test".into(),
        env: None,
        auto_approve: true,
        start_run: None,
        live_turn: forge::engine::opencode_client::live_turn_slot(),
        spend_cap_usd: None,
        assay_commands: vec![],
        acceptance_mapped: false,
        packet: StoryPacket::default(),
        execution_workspace: Some(ExecutionWorkspace {
            worktree_path: worktree_path.to_string_lossy().to_string(),
            branch_name: "test-branch".into(),
            base_ref: "origin/main".into(),
            base_commit: "abc123".into(),
        }),
        story_id: Some("TEST-STORY".into()),
    };

    // Run a command that prints the environment and CARGO_TARGET_DIR
    let cmd = r#"echo "CARGO_TARGET_DIR=$CARGO_TARGET_DIR"; env | grep -E '^(DATABASE_URL|NEON_|APP_ENV|TEST_TOOLCHAIN_VAR)=' | sort"#;
    let result = harness.run_command(cmd);

    // The command should succeed
    assert!(
        result.passed,
        "{HARNESS}: run_command failed: exit_code={}, output={}",
        result.exit_code, result.output
    );

    // Verify CARGO_TARGET_DIR was set and points to the per-worktree directory
    let expected_target_dir = derive_cargo_target_dir(&worktree_path);
    assert!(
        result.output.contains(&format!(
            "CARGO_TARGET_DIR={}",
            expected_target_dir.display()
        )),
        "{HARNESS}: CARGO_TARGET_DIR not set correctly. Expected: {}, Got output: {}",
        expected_target_dir.display(),
        result.output
    );

    // Verify secret keys are NOT in the output
    assert!(
        !result.output.contains("DATABASE_URL_PROD"),
        "{HARNESS}: DATABASE_URL_PROD must not appear in assay env"
    );
    assert!(
        !result.output.contains("NEON_API_KEY"),
        "{HARNESS}: NEON_API_KEY must not appear in assay env"
    );
    assert!(
        !result.output.contains("APP_ENV"),
        "{HARNESS}: APP_ENV must not appear in assay env"
    );

    // Verify toolchain vars ARE preserved
    assert!(
        result
            .output
            .contains("TEST_TOOLCHAIN_VAR=should-be-preserved"),
        "{HARNESS}: toolchain var TEST_TOOLCHAIN_VAR must be preserved"
    );

    // Clean up env vars
    env::remove_var("DATABASE_URL_PROD");
    env::remove_var("NEON_API_KEY");
    env::remove_var("APP_ENV");
    env::remove_var("TEST_TOOLCHAIN_VAR");
}

#[test]
fn sanitize_assay_env_removes_secrets_keeps_toolchain() {
    let mut env = HashMap::new();
    env.insert("PATH".into(), "/usr/bin".into());
    env.insert("RUSTUP_HOME".into(), "/home/user/.rustup".into());
    env.insert("CARGO_HOME".into(), "/home/user/.cargo".into());
    env.insert("HOME".into(), "/home/user".into());
    env.insert("USER".into(), "testuser".into());
    env.insert("DATABASE_URL_PROD".into(), "postgres://prod".into());
    env.insert("NEON_API_KEY".into(), "secret".into());
    env.insert("APP_ENV".into(), "production".into());
    env.insert("EXECUTION_ENV".into(), "PROD".into());
    env.insert("VERCEL_ENV".into(), "production".into());
    env.insert("PGHOST".into(), "localhost".into());
    env.insert("PGPORT".into(), "5432".into());
    env.insert("PGDATABASE".into(), "testdb".into());
    env.insert("PGUSER".into(), "testuser".into());
    env.insert("PGPASSWORD".into(), "testpass".into());
    env.insert("CUSTOM_TOOLCHAIN_VAR".into(), "keep-me".into());

    let clean = sanitize_assay_env(env);

    // Toolchain vars must be preserved
    assert_eq!(clean.get("PATH").map(String::as_str), Some("/usr/bin"));
    assert_eq!(
        clean.get("RUSTUP_HOME").map(String::as_str),
        Some("/home/user/.rustup")
    );
    assert_eq!(
        clean.get("CARGO_HOME").map(String::as_str),
        Some("/home/user/.cargo")
    );
    assert_eq!(clean.get("HOME").map(String::as_str), Some("/home/user"));
    assert_eq!(clean.get("USER").map(String::as_str), Some("testuser"));
    assert_eq!(
        clean.get("CUSTOM_TOOLCHAIN_VAR").map(String::as_str),
        Some("keep-me")
    );

    // Secret keys must be removed
    assert!(!clean.contains_key("DATABASE_URL_PROD"));
    assert!(!clean.contains_key("NEON_API_KEY"));
    assert!(!clean.contains_key("APP_ENV"));
    assert!(!clean.contains_key("EXECUTION_ENV"));
    assert!(!clean.contains_key("VERCEL_ENV"));
    assert!(!clean.contains_key("PGHOST"));
    assert!(!clean.contains_key("PGPORT"));
    assert!(!clean.contains_key("PGDATABASE"));
    assert!(!clean.contains_key("PGUSER"));
    assert!(!clean.contains_key("PGPASSWORD"));
}

#[test]
fn two_harnesses_different_worktrees_different_target_dirs() {
    // Create two harnesses with different worktree paths
    let worktree1 = TempDir::new().expect("create worktree1");
    let worktree2 = TempDir::new().expect("create worktree2");

    let path1 = worktree1.path().to_path_buf();
    let path2 = worktree2.path().to_path_buf();

    let harness1 = OpenCodeHarness {
        cli_bin: "echo".into(),
        workspace: path1.clone(),
        model: "test".into(),
        env: None,
        auto_approve: true,
        start_run: None,
        live_turn: forge::engine::opencode_client::live_turn_slot(),
        spend_cap_usd: None,
        assay_commands: vec![],
        acceptance_mapped: false,
        packet: StoryPacket::default(),
        execution_workspace: Some(ExecutionWorkspace {
            worktree_path: path1.to_string_lossy().to_string(),
            branch_name: "branch-1".into(),
            base_ref: "origin/main".into(),
            base_commit: "abc123".into(),
        }),
        story_id: Some("STORY-1".into()),
    };

    let harness2 = OpenCodeHarness {
        cli_bin: "echo".into(),
        workspace: path2.clone(),
        model: "test".into(),
        env: None,
        auto_approve: true,
        start_run: None,
        live_turn: forge::engine::opencode_client::live_turn_slot(),
        spend_cap_usd: None,
        assay_commands: vec![],
        acceptance_mapped: false,
        packet: StoryPacket::default(),
        execution_workspace: Some(ExecutionWorkspace {
            worktree_path: path2.to_string_lossy().to_string(),
            branch_name: "branch-2".into(),
            base_ref: "origin/main".into(),
            base_commit: "def456".into(),
        }),
        story_id: Some("STORY-2".into()),
    };

    // Run a command that prints CARGO_TARGET_DIR
    let cmd = r#"echo "TARGET_DIR=$CARGO_TARGET_DIR""#;
    let result1 = harness1.run_command(cmd);
    let result2 = harness2.run_command(cmd);

    assert!(result1.passed, "{HARNESS}: harness1 command failed");
    assert!(result2.passed, "{HARNESS}: harness2 command failed");

    // Extract the target dirs from output
    let target1 = result1
        .output
        .lines()
        .find(|l| l.starts_with("TARGET_DIR="))
        .map(|l| l.strip_prefix("TARGET_DIR=").unwrap())
        .expect("TARGET_DIR in output");
    let target2 = result2
        .output
        .lines()
        .find(|l| l.starts_with("TARGET_DIR="))
        .map(|l| l.strip_prefix("TARGET_DIR=").unwrap())
        .expect("TARGET_DIR in output");

    // Target dirs must be different
    assert_ne!(
        target1, target2,
        "{HARNESS}: two harnesses with different worktrees must have different CARGO_TARGET_DIR"
    );

    // Both should be valid paths under the build root
    assert!(target1.starts_with("/Users/Shared/dev/build/rust-"));
    assert!(target2.starts_with("/Users/Shared/dev/build/rust-"));
}
