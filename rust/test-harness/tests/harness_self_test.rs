//! The harness's own contract, exercised from outside the crate.
//!
//! These are the self-tests the foundation story requires: they prove the deterministic clock and id helpers are
//! actually deterministic, and they prove the PRODUCTION database guard refuses execution *before* any connection is
//! attempted. Everything here runs in `cargo test -p test-harness` with no database, no network and no environment.

use test_harness::database::{
    guard_target, resolve_test_target, unique_namespace, HarnessDbError, TestDatabase,
};
use test_harness::{DeterministicIds, FixtureFactory, TestClock, TestLevel};
use db::DbTarget;

#[test]
fn the_clock_is_deterministic_and_moves_only_when_asked() {
    // Two clocks at the same instant produce the same sequence, and neither moves on its own.
    let left = TestClock::at_unix_millis(1_700_000_000_000);
    let right = TestClock::at_unix_millis(1_700_000_000_000);
    assert_eq!(left.now_millis(), right.now_millis());
    assert_eq!(left.now_millis(), left.now_millis());

    let mut left_values = Vec::new();
    let mut right_values = Vec::new();
    for _ in 0..5 {
        left_values.push(left.advance_millis(250));
        right_values.push(right.advance_millis(250));
    }
    assert_eq!(left_values, right_values, "the same moves yield the same instants");
}

#[test]
fn the_id_stream_is_deterministic() {
    let left = DeterministicIds::new(1234);
    let right = DeterministicIds::new(1234);
    for _ in 0..16 {
        assert_eq!(left.next_uuid_string(), right.next_uuid_string());
        assert_eq!(left.next_u64(), right.next_u64());
        assert_eq!(left.id("row"), right.id("row"));
    }

    // A different seed diverges, so the determinism is a seed, not a constant.
    let other = DeterministicIds::new(1235);
    assert_ne!(DeterministicIds::new(1).next_uuid_string(), other.next_uuid_string());
}

#[test]
fn fixtures_are_deterministic_for_a_seed() {
    let left = FixtureFactory::new(9);
    let right = FixtureFactory::new(9);
    assert_eq!(left.namespace(), right.namespace());
    assert_eq!(left.uuid(), right.uuid());
    assert_eq!(left.email("guest"), right.email("guest"));
    assert_eq!(left.now_millis(), right.now_millis());
}

#[test]
fn the_production_database_guard_refuses_execution() {
    // 1. The pure guard: PROD is refused, DEV is allowed.
    let refusal = guard_target(DbTarget::Prod).expect_err("PROD must be refused");
    assert!(matches!(refusal, HarnessDbError::ProductionRefused(_)));

    // 2. The declaration the guard sits on: a production environment is refused, whichever variable says so.
    assert!(resolve_test_target(Some("production"), Some("dev")).is_err());
    assert!(resolve_test_target(None, Some("production")).is_err());
    assert!(resolve_test_target(None, Some("prod")).is_err());
    assert!(resolve_test_target(None, None).is_err(), "silence is refused, not defaulted");

    // 3. And a dev/test declaration resolves to DEV, so the guard is not simply refusing everything.
    assert_eq!(resolve_test_target(None, Some("test")).unwrap(), DbTarget::Dev);
    assert_eq!(
        resolve_test_target(Some("preview"), None).unwrap(),
        DbTarget::Dev
    );
}

/// The guard's real entry point, not just its pure resolver.
///
/// `the_production_database_guard_refuses_execution` proves the *decision* (`guard_target`/`resolve_test_target`).
/// This proves the *execution*: `TestDatabase::connect_declared`, the async constructor every database-backed test
/// funnels through, returns the refusal instead of opening a pool. It runs with no database and no network, so it
/// holds even where no DEV database is reachable — which is exactly why the refusal can be proven in `cargo test`.
#[tokio::test]
async fn the_database_constructor_refuses_to_execute_against_production() {
    // A production declaration is refused before any socket is opened.
    assert!(
        matches!(
            TestDatabase::connect_declared(Some("production"), Some("dev")).await,
            Err(HarnessDbError::ProductionRefused(_))
        ),
        "a production declaration must resolve to the PRODUCTION refusal before connecting"
    );
    assert!(matches!(
        TestDatabase::connect_declared(None, Some("prod")).await,
        Err(HarnessDbError::ProductionRefused(_))
    ));

    // Silence is refused as undeclared, not defaulted to a database.
    assert!(matches!(
        TestDatabase::connect_declared(None, None).await,
        Err(HarnessDbError::Undeclared(_))
    ));
}

#[test]
fn the_database_helpers_own_cleanup_with_a_unique_namespace_per_instance() {
    // Cleanup is ownership: a `TestDatabase` drops the isolated schema named by its namespace with `CASCADE`.
    // Two instances must never share that name, or one test's cleanup would delete another's objects. `cargo test`
    // runs one binary's tests on parallel threads, so this is per-instance, not per-process.
    let first = unique_namespace();
    let second = unique_namespace();
    assert_ne!(first, second, "each test database owns its own schema");
    assert!(first.starts_with("tsth-"), "the namespace is a safe schema prefix");
}

#[test]
fn the_harness_supports_all_five_levels() {
    assert_eq!(TestLevel::ALL.len(), 5);
    assert_eq!(TestLevel::L0Pure.code(), "L0");
    assert_eq!(TestLevel::L4Adversarial.code(), "L4");
    assert!(TestLevel::L2Persistence.requires_database());
    assert!(!TestLevel::L0Pure.requires_database());
}

/// The one-way rule, made executable: `test-harness` may depend on production crates, and no production
/// crate may depend on `test-harness`.
///
/// This is what keeps the harness a harness. If a production crate linked it, the harness would become
/// part of the system under test and a contract test could pass because the harness — not the code —
/// implemented the behaviour. The rule was documented in `lib.rs` but enforced by nothing; this test
/// walks every manifest under `rust/` and refuses a `test-harness` entry in a production dependency table.
///
/// A `[dev-dependencies]` entry is allowed (that is how a downstream test crate consumes the harness), and
/// the workspace root's `[workspace]` members list is not a dependency, so neither is flagged.
#[test]
fn no_production_crate_depends_on_the_harness() {
    let rust_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the harness lives under rust/")
        .to_path_buf();

    let mut manifests: Vec<std::path::PathBuf> = Vec::new();
    collect_manifests(&rust_root, &mut manifests);

    let mut offenders = Vec::new();
    for path in manifests {
        // The harness itself owns the rule; its own manifest is not a production crate.
        if path.ends_with("test-harness/Cargo.toml") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
        if let Some(table) = production_dependency_naming_harness(&text) {
            offenders.push(format!("{} ({table})", path.display()));
        }
    }

    assert!(
        offenders.is_empty(),
        "production crates must not depend on test-harness, but found:\n{}",
        offenders.join("\n")
    );
}

/// Every `Cargo.toml` under `rust/`, skipping build output and the non-workspace experiments crate.
fn collect_manifests(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if matches!(name.as_ref(), "target" | "node_modules" | ".git") {
                continue;
            }
            collect_manifests(&path, out);
        } else if name == "Cargo.toml" {
            out.push(path);
        }
    }
}

/// Return the production dependency table that names `test-harness`, if one does.
///
/// A production dependency table is `[dependencies]`, `[build-dependencies]`, a target-qualified form of
/// either, or their dotted equivalents. `[dev-dependencies]` and `[workspace.dependencies]` are exempt,
/// because a downstream test crate legitimately consumes the harness and the workspace table only declares.
fn production_dependency_naming_harness(manifest: &str) -> Option<String> {
    let is_production_table = |table: &str| {
        let segments: Vec<&str> = table.split('.').collect();
        let names_harness = |segment: &str| segment.trim_matches('"') == "test-harness";
        let is_dependency = segments
            .iter()
            .any(|segment| *segment == "dependencies" || *segment == "build-dependencies")
            || segments.iter().any(|segment| names_harness(segment));
        let exempt = segments
            .iter()
            .any(|segment| segment.contains("dev") || *segment == "workspace");
        is_dependency && !exempt
    };

    let mut current: Option<String> = None;
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') && line.ends_with(']') {
            let table = line.trim_matches(|c| c == '[' || c == ']').trim().to_owned();
            if table.contains("test-harness") && is_production_table(&table) {
                return Some(table);
            }
            current = Some(table);
            continue;
        }
        if line.contains("test-harness") {
            if let Some(table) = &current {
                if is_production_table(table) {
                    return Some(table.clone());
                }
            }
        }
    }
    None
}

#[test]
fn the_one_way_guard_refuses_a_production_dependency() {
    // The two shapes a production dependency takes, both of which must be refused.
    assert_eq!(
        production_dependency_naming_harness("[dependencies]\ntest-harness = { path = \"../test-harness\" }\n"),
        Some("dependencies".to_owned())
    );
    assert_eq!(
        production_dependency_naming_harness("[dependencies.test-harness]\npath = \"../test-harness\"\n"),
        Some("dependencies.test-harness".to_owned())
    );
    assert_eq!(
        production_dependency_naming_harness("[build-dependencies]\ntest-harness = \"0.1\"\n"),
        Some("build-dependencies".to_owned())
    );
    // A target-qualified production table is still a production table.
    assert_eq!(
        production_dependency_naming_harness(
            "[target.'cfg(unix)'.dependencies]\ntest-harness = { path = \"../test-harness\" }\n"
        ),
        Some("target.'cfg(unix)'.dependencies".to_owned())
    );
}

#[test]
fn the_one_way_guard_allows_dev_dependencies_and_the_members_list() {
    // A downstream test crate consumes the harness through `[dev-dependencies]`.
    assert_eq!(
        production_dependency_naming_harness(
            "[dependencies]\nserde = \"1\"\n\n[dev-dependencies]\ntest-harness = { path = \"../test-harness\" }\n"
        ),
        None
    );
    // The workspace table only declares; it is not a dependency of any production crate.
    assert_eq!(
        production_dependency_naming_harness(
            "[workspace.dependencies]\ntest-harness = { path = \"test-harness\" }\n"
        ),
        None
    );
    // The workspace members list names the crate without depending on it.
    assert_eq!(
        production_dependency_naming_harness(
            "[workspace]\nmembers = [\n    \"core/domain\",\n    \"test-harness\",\n]\n"
        ),
        None
    );
}
