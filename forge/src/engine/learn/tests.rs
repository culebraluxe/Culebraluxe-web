// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use std::{fs, process::Command};
#[test]
fn learn_sort_prefers_p0() {
    let mut v = vec![
        Candidate {
            key: "x".into(),
            severity: Severity::Normal,
            evidence: vec![],
            hit_count: 99,
            observations: vec![],
        },
        Candidate {
            key: "stale-claim".into(),
            severity: Severity::P0,
            evidence: vec![],
            hit_count: 1,
            observations: vec![],
        },
    ];
    v.sort_by(|a, b| match (&a.severity, &b.severity) {
        (Severity::P0, Severity::Normal) => std::cmp::Ordering::Less,
        (Severity::Normal, Severity::P0) => std::cmp::Ordering::Greater,
        _ => b.hit_count.cmp(&a.hit_count),
    });
    assert_eq!(v[0].key, "stale-claim");
}

#[test]
fn rust_rule_gate_only_selects_rust_candidates_for_explicit_enablement() {
    let rust = Candidate {
        key: "RUST-EMPTY-ERROR-ARM:v1:src/lib.rs".into(),
        severity: Severity::Normal,
        evidence: vec![],
        hit_count: 1,
        observations: vec![],
    };
    let js = Candidate {
        key: "JS-EMPTY-CATCH:v1:app/route.ts".into(),
        severity: Severity::Normal,
        evidence: vec![],
        hit_count: 1,
        observations: vec![],
    };
    assert!(rust_rule_candidate(&rust));
    assert!(!rust_rule_candidate(&js));
}

#[test]
fn disabled_rust_findings_move_to_backlog_and_reenable_with_original_revision() {
    let rust = RuleObservation {
        rule_id: "RUST-EMPTY-ERROR-ARM".into(),
        rule_version: 1,
        key: "rust-key".into(),
        source_revision: "revision-a".into(),
        path: "src/lib.rs".into(),
        start_line: 4,
        end_line: 5,
        normalized_context: "match result".into(),
        rationale: "empty error arm".into(),
        severity: "normal".into(),
        confidence: "high".into(),
        limitations: "review candidate".into(),
    };
    let js = RuleObservation {
        rule_id: "JS-EMPTY-CATCH".into(),
        rule_version: 1,
        key: "js-key".into(),
        source_revision: "revision-a".into(),
        path: "app/route.ts".into(),
        start_line: 8,
        end_line: 9,
        normalized_context: "catch".into(),
        rationale: "empty catch".into(),
        severity: "normal".into(),
        confidence: "high".into(),
        limitations: "review candidate".into(),
    };

    let (actionable, deferred) =
        partition_rule_observations(vec![rust.clone(), js.clone()], Vec::new(), false);
    assert_eq!(actionable, vec![js]);
    assert_eq!(deferred, vec![rust.clone()]);

    let (actionable, deferred) = partition_rule_observations(actionable, deferred, true);
    assert!(deferred.is_empty());
    assert!(actionable.contains(&rust));
    assert_eq!(
        actionable
            .iter()
            .find(|observation| observation.rule_id.starts_with("RUST-"))
            .map(|observation| observation.source_revision.as_str()),
        Some("revision-a")
    );
}

#[test]
fn source_scan_chunk_respects_file_bound_without_dropping_remaining_paths() {
    let temp = tempfile::tempdir().expect("temporary repository");
    let root = temp.path();
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.name", "Forge Test"],
        vec!["config", "user.email", "forge-test@example.invalid"],
    ] {
        let status = Command::new("git")
            .current_dir(root)
            .args(args)
            .status()
            .expect("git starts");
        assert!(status.success());
    }
    fs::write(root.join("README.md"), "base\n").expect("fixture base");
    let status = Command::new("git")
        .current_dir(root)
        .args(["add", "README.md"])
        .status()
        .expect("git starts");
    assert!(status.success());
    let status = Command::new("git")
        .current_dir(root)
        .args(["commit", "-qm", "base fixture"])
        .status()
        .expect("git starts");
    assert!(status.success());
    let base_revision = discovery::source_revision(root).expect("base revision");
    fs::create_dir_all(root.join("forge/src")).expect("source directory");
    for index in 0..45 {
        fs::write(
            root.join(format!("forge/src/file-{index:02}.rs")),
            "pub fn clean() {}\n",
        )
        .expect("fixture source");
    }
    let status = Command::new("git")
        .current_dir(root)
        .args(["add", "forge/src"])
        .status()
        .expect("git starts");
    assert!(status.success());
    let status = Command::new("git")
        .current_dir(root)
        .args(["commit", "-qm", "source fixture"])
        .status()
        .expect("git starts");
    assert!(status.success());
    let revision = discovery::source_revision(root).expect("pinned revision");
    let paths = discovery::changed_paths(root, Some(&base_revision), &revision, 0)
        .expect("changed paths")
        .into_iter()
        .filter(|path| code_path(path))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 45);
    let (processed, observations, issues) =
        scan_pinned_chunk(root, &revision, &paths).expect("bounded chunk");
    assert_eq!(processed, MAX_FILES);
    assert!(observations.is_empty());
    assert!(issues.is_empty());
    assert_eq!(paths.len() - processed, 5);
}
