// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
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

fn view(id: &str, goal: Option<&str>, brief: Option<&str>, criteria: Option<&str>) -> TstStoryView {
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
