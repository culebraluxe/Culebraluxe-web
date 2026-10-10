//! Rust Forge learn loop.
//!
//! Ports the unattended learn pass that used to live in agent-runtime/learn-loop.ts.
//! It observes recent code changes + stale claims, files at most one learn story,
//! dedupes against open learn work, and never fixes/merges/promotes its own finding.

use crate::engine::vendor_session::with_shared;
use db::ForgeControlDao;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

mod discovery;
mod js_rules;
mod rust_rules;

const MAX_FILES: usize = 40;
const MAX_SCAN_BYTES: usize = 4 * 1024 * 1024;
const WINDOW_HOURS: u64 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Severity {
    P0,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScanStatus {
    Complete,
    Partial,
    Unavailable,
    Failed,
}

#[derive(Debug, Clone)]
pub(crate) struct LearnPassReport {
    pub(crate) status: ScanStatus,
    pub(crate) source_revision: Option<String>,
    pub(crate) scanned_files: usize,
    pub(crate) deferred_files: usize,
    pub(crate) deferred_findings: usize,
    pub(crate) filed_story: Option<String>,
    pub(crate) error: Option<String>,
}

impl LearnPassReport {
    fn complete(source_revision: String) -> Self {
        Self {
            status: ScanStatus::Complete,
            source_revision: Some(source_revision),
            scanned_files: 0,
            deferred_files: 0,
            deferred_findings: 0,
            filed_story: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Candidate {
    key: String,
    severity: Severity,
    evidence: Vec<String>,
    hit_count: usize,
    observations: Vec<RuleObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RuleObservation {
    rule_id: String,
    rule_version: u32,
    key: String,
    source_revision: String,
    path: String,
    start_line: usize,
    end_line: usize,
    normalized_context: String,
    rationale: String,
    severity: String,
    confidence: String,
    limitations: String,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn anchor_path(root: &Path) -> PathBuf {
    root.join(".forge-context").join("learn-last-run.json")
}

fn read_anchor_secs(root: &Path) -> Option<u64> {
    let raw = fs::read_to_string(anchor_path(root)).ok()?;
    let key = "\"unix\":";
    let pos = raw.find(key)? + key.len();
    raw[pos..]
        .trim_start()
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

fn write_anchor(root: &Path, key: Option<&str>) -> Result<(), String> {
    let path = anchor_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let safe = key.unwrap_or("").replace('\\', "\\\\").replace('"', "\\\"");
    fs::write(
        path,
        format!("{{\"unix\":{},\"lastKey\":\"{}\"}}\n", now_secs(), safe),
    )
    .map_err(|e| e.to_string())
}

fn code_path(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let good = path.ends_with(".ts")
        || path.ends_with(".tsx")
        || path.ends_with(".js")
        || path.ends_with(".mjs")
        || path.ends_with(".rs");
    good && !path.starts_with("docs/")
        && !file_name.starts_with("test_")
        && !file_name.ends_with("_test.rs")
        && !file_name.ends_with("_tests.rs")
        && !path.starts_with("node_modules/")
        && !path.starts_with(".next/")
        && !path.starts_with(".vercel/")
        && !path.starts_with(".forge/")
        && !path.starts_with("testv2/")
        && !path.starts_with("tests/")
        && !path.contains("/tests/")
        && !path.starts_with("target/")
        && !path.contains("/target/")
        && !path.starts_with("build/")
        && !path.contains("/build/")
        && !path.starts_with("dist/")
        && !path.contains("/dist/")
        && !path.starts_with("out/")
        && !path.contains("/out/")
        && !path.starts_with("vendor/")
        && !path.contains("/vendor/")
        && !path.starts_with("generated/")
        && !path.contains("/generated/")
        && !path.contains(".test.")
        && !path.contains(".spec.")
        && path != "agent-runtime/silent-failure-patterns.ts"
}

fn scan_observations(
    files: &[(String, String)],
    revision: &str,
) -> (Vec<RuleObservation>, Vec<rust_rules::ParseIssue>) {
    let mut observations = js_rules::scan(files, revision);
    let (rust, issues) = rust_rules::scan_rust_files(files, revision);
    observations.extend(rust);
    observations.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.start_line.cmp(&b.start_line))
            .then(a.rule_id.cmp(&b.rule_id))
    });
    (observations, issues)
}

fn observations_to_candidates(observations: Vec<RuleObservation>) -> Vec<Candidate> {
    let mut candidates = BTreeMap::<String, Candidate>::new();
    for observation in observations {
        let pattern = format!("{}:v{}", observation.rule_id, observation.rule_version);
        let key = format!("{pattern}:{}", observation.path);
        let candidate = candidates.entry(key.clone()).or_insert_with(|| Candidate {
            key,
            severity: Severity::Normal,
            evidence: Vec::new(),
            hit_count: 0,
            observations: Vec::new(),
        });
        candidate.hit_count += 1;
        if candidate.evidence.len() < 5 {
            candidate.evidence.push(format!(
                "{}:{}-{}",
                observation.path, observation.start_line, observation.end_line
            ));
        }
        if candidate.observations.len() < 5 {
            candidate.observations.push(observation);
        }
    }
    candidates.into_values().collect()
}

fn stale_candidate(minutes: i64) -> Result<Option<Candidate>, String> {
    with_shared(|db, rt| {
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(async {
            let rows = dao
                .stale_learn_claims(minutes)
                .await
                .map_err(|error| error.to_string())?;
            if rows.is_empty() {
                return Ok(None);
            }
            let mut evidence = Vec::new();
            for row in &rows {
                if evidence.len() < 5 {
                    evidence.push(format!("agent_work_item:{}", row.id));
                }
            }
            Ok(Some(Candidate {
                key: "stale-claim".into(),
                severity: Severity::P0,
                evidence,
                hit_count: rows.len(),
                observations: Vec::new(),
            }))
        })
    })?
}

fn instructions(candidate: &Candidate) -> String {
    let observations = serde_json::to_string(&candidate.observations)
        .expect("RuleObservation contains only serializable fields");
    format!(
        "Filed by the Rust learn loop: pattern {} ({} hit(s)). Lead and Architect decide SMITH or HOLD; the loop does not choose the fix. Assay does not ship code. Never auto-merge, never auto-promote a decision. Evidence: {}. Source observations (review candidates, not confirmed defects): {}",
        candidate.key,
        candidate.hit_count,
        candidate.evidence.join(", "),
        observations
    )
}

fn file_candidate(
    repository_key: &str,
    revision: &str,
    candidate: &Candidate,
) -> Result<String, String> {
    let notes = instructions(candidate);
    with_shared(|db, rt| {
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(async {
            dao.file_learn_finding(
                repository_key,
                &candidate.key,
                revision,
                &format!("learn: {}", candidate.key),
                if candidate.severity == Severity::P0 {
                    "High"
                } else {
                    "Medium"
                },
                &notes,
                &format!("Verify and resolve {}", candidate.key),
                candidate.severity == Severity::P0,
            )
            .await
            .map_err(|error| error.to_string())
        })
    })?
}

fn scan_pinned_chunk(
    root: &Path,
    revision: &str,
    pending_files: &[String],
) -> Result<(usize, Vec<RuleObservation>, Vec<rust_rules::ParseIssue>), String> {
    let mut files = Vec::new();
    let mut bytes = 0usize;
    for path in pending_files.iter().take(MAX_FILES) {
        let source = discovery::read_pinned_source(root, revision, path)?;
        if !files.is_empty() && bytes.saturating_add(source.len()) > MAX_SCAN_BYTES {
            break;
        }
        bytes += source.len();
        files.push((path.clone(), source));
        if bytes >= MAX_SCAN_BYTES {
            break;
        }
    }
    let processed = files.len();
    let (observations, parse_issues) = scan_observations(&files, revision);
    Ok((processed, observations, parse_issues))
}

fn candidate_group_key(observation: &RuleObservation) -> String {
    format!(
        "{}:v{}:{}",
        observation.rule_id, observation.rule_version, observation.path
    )
}

fn rust_rule_candidate(candidate: &Candidate) -> bool {
    candidate.key.starts_with("RUST-")
}

fn rust_rule_observation(observation: &RuleObservation) -> bool {
    observation.rule_id.starts_with("RUST-")
}

fn partition_rule_observations(
    mut actionable: Vec<RuleObservation>,
    mut deferred: Vec<RuleObservation>,
    allow_rust_rules: bool,
) -> (Vec<RuleObservation>, Vec<RuleObservation>) {
    if allow_rust_rules {
        actionable.append(&mut deferred);
        return (actionable, Vec::new());
    }
    let mut retained = Vec::with_capacity(actionable.len());
    for observation in actionable {
        if rust_rule_observation(&observation) {
            deferred.push(observation);
        } else {
            retained.push(observation);
        }
    }
    (retained, deferred)
}

fn rust_rules_enabled() -> bool {
    std::env::var("FORGE_LEARN_RUST_RULES_ENABLED")
        .ok()
        .is_some_and(|value| value.trim() == "1")
}

fn save_progress(
    repository_key: &str,
    revision: &str,
    expected_files: &[String],
    remaining_files: &[String],
    expected_observations: &serde_json::Value,
    observations: &serde_json::Value,
    expected_deferred_observations: &serde_json::Value,
    deferred_observations: &serde_json::Value,
) -> Result<(), String> {
    let saved = with_shared(|db, rt| {
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(dao.save_learn_scan_progress(
            repository_key,
            revision,
            expected_files,
            remaining_files,
            expected_observations,
            observations,
            expected_deferred_observations,
            deferred_observations,
        ))
        .map_err(|error| error.to_string())
    })??;
    if !saved {
        return Err("learning scan changed concurrently; retry on the next worker pass".into());
    }
    Ok(())
}

fn open_pattern_keys() -> Result<BTreeSet<String>, String> {
    with_shared(|db, rt| {
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(async {
            dao.open_learn_pattern_keys()
                .await
                .map(|rows| rows.into_iter().collect())
                .map_err(|error| error.to_string())
        })
    })?
}

pub(crate) fn run_learn_pass(root: &Path, stale_after_minutes: i64) -> LearnPassReport {
    match run_learn_pass_inner(root, stale_after_minutes) {
        Ok(report) => report,
        Err(error) => LearnPassReport {
            status: if error.contains("origin remote") {
                ScanStatus::Unavailable
            } else {
                ScanStatus::Failed
            },
            source_revision: None,
            scanned_files: 0,
            deferred_files: 0,
            deferred_findings: 0,
            filed_story: None,
            error: Some(error),
        },
    }
}

fn run_learn_pass_inner(root: &Path, stale_after_minutes: i64) -> Result<LearnPassReport, String> {
    let now = now_secs();
    let floor = now.saturating_sub(WINDOW_HOURS * 3600);
    let revision = discovery::source_revision(root)?;
    let repository_key = discovery::repository_key(root)?;
    let state = with_shared(|db, rt| {
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(dao.learn_scan_state(&repository_key))
            .map_err(|error| error.to_string())
    })??;
    let allow_rust_rules = rust_rules_enabled();
    let state = match state {
        Some(state) if state.active_revision.is_some() => state,
        prior => {
            let cursor = prior
                .as_ref()
                .and_then(|state| state.cursor_revision.as_deref());
            let deferred = prior.as_ref().is_some_and(|state| {
                state
                    .deferred_observations
                    .as_array()
                    .is_some_and(|rows| !rows.is_empty())
            });
            if cursor == Some(revision.as_str()) && !(allow_rust_rules && deferred) {
                return Ok(LearnPassReport::complete(revision));
            }
            let paths = discovery::changed_paths(root, cursor, &revision, floor)?
                .into_iter()
                .filter(|path| code_path(path))
                .collect::<Vec<_>>();
            let started = with_shared(|db, rt| {
                let dao = ForgeControlDao::new(db.clone());
                rt.block_on(dao.begin_learn_scan(&repository_key, cursor, &revision, &paths))
                    .map_err(|error| error.to_string())
            })??;
            if started.active_revision.is_none()
                && started.cursor_revision.as_deref() == Some(revision.as_str())
                && !(allow_rust_rules
                    && started
                        .deferred_observations
                        .as_array()
                        .is_some_and(|rows| !rows.is_empty()))
            {
                return Ok(LearnPassReport::complete(revision));
            }
            started
        }
    };
    let revision = state
        .active_revision
        .clone()
        .unwrap_or_else(|| revision.clone());
    let pending_files = state.pending_files;
    let old_value = state.pending_observations;
    let old_deferred_value = state.deferred_observations;
    let active_observations: Vec<RuleObservation> = serde_json::from_value(old_value.clone())
        .map_err(|error| format!("durable learning observations are invalid: {error}"))?;
    let deferred_observations: Vec<RuleObservation> =
        serde_json::from_value(old_deferred_value.clone()).map_err(|error| {
            format!("durable deferred learning observations are invalid: {error}")
        })?;
    let (processed, newly_found, parse_issues) =
        scan_pinned_chunk(root, &revision, &pending_files)?;
    if !parse_issues.is_empty() {
        return Err(format!(
            "Rust learning scan incomplete; parser failed for {} file(s): {}",
            parse_issues.len(),
            parse_issues
                .iter()
                .map(|issue| format!("{}: {}", issue.path, issue.message))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let remaining_files = pending_files
        .iter()
        .skip(processed)
        .cloned()
        .collect::<Vec<_>>();
    let active_observations = active_observations.into_iter().chain(newly_found).collect();
    let (mut observations, mut deferred_observations) =
        partition_rule_observations(active_observations, deferred_observations, allow_rust_rules);
    let mut new_value = serde_json::to_value(&observations)
        .map_err(|error| format!("could not persist learning observations: {error}"))?;
    let mut deferred_value = serde_json::to_value(&deferred_observations)
        .map_err(|error| format!("could not persist deferred learning observations: {error}"))?;
    if processed > 0 || new_value != old_value || deferred_value != old_deferred_value {
        save_progress(
            &repository_key,
            &revision,
            &pending_files,
            &remaining_files,
            &old_value,
            &new_value,
            &old_deferred_value,
            &deferred_value,
        )?;
    }

    let open = open_pattern_keys()?;
    let all_observations = observations
        .iter()
        .chain(deferred_observations.iter())
        .cloned()
        .collect();
    let open_source_groups = observations_to_candidates(all_observations)
        .into_iter()
        .filter(|candidate| {
            open.contains(&format!("{repository_key}:{}", candidate.key))
                || open.contains(&candidate.key)
        })
        .filter_map(|candidate| candidate.observations.first().map(candidate_group_key))
        .collect::<BTreeSet<_>>();
    if !open_source_groups.is_empty() {
        observations
            .retain(|observation| !open_source_groups.contains(&candidate_group_key(observation)));
        deferred_observations
            .retain(|observation| !open_source_groups.contains(&candidate_group_key(observation)));
        let pruned_value = serde_json::to_value(&observations)
            .map_err(|error| format!("could not prune open findings: {error}"))?;
        let pruned_deferred_value = serde_json::to_value(&deferred_observations)
            .map_err(|error| format!("could not prune deferred findings: {error}"))?;
        save_progress(
            &repository_key,
            &revision,
            &remaining_files,
            &remaining_files,
            &new_value,
            &pruned_value,
            &deferred_value,
            &pruned_deferred_value,
        )?;
        new_value = pruned_value;
        deferred_value = pruned_deferred_value;
    }
    let mut candidates = observations_to_candidates(observations.clone());
    if let Some(stale) = stale_candidate(stale_after_minutes)? {
        candidates.push(stale);
    }
    candidates.retain(|candidate| {
        !open.contains(&format!("{repository_key}:{}", candidate.key))
            && !open.contains(&candidate.key)
            && (allow_rust_rules || !rust_rule_candidate(candidate))
    });
    candidates.sort_by(|a, b| match (&a.severity, &b.severity) {
        (Severity::P0, Severity::Normal) => std::cmp::Ordering::Less,
        (Severity::Normal, Severity::P0) => std::cmp::Ordering::Greater,
        _ => b
            .hit_count
            .cmp(&a.hit_count)
            .then_with(|| a.key.cmp(&b.key)),
    });
    let filed = if let Some(candidate) = candidates.first() {
        let finding_revision = candidate
            .observations
            .last()
            .map(|observation| observation.source_revision.as_str())
            .unwrap_or(&revision);
        let story_id = file_candidate(&repository_key, finding_revision, candidate)?;
        if candidate.key != "stale-claim" {
            let group = candidate
                .observations
                .first()
                .map(candidate_group_key)
                .ok_or_else(|| "source candidate has no rule observation".to_string())?;
            let retained = observations
                .iter()
                .filter(|observation| candidate_group_key(observation) != group)
                .cloned()
                .collect::<Vec<_>>();
            let retained = serde_json::to_value(retained)
                .map_err(|error| format!("could not update finding backlog: {error}"))?;
            save_progress(
                &repository_key,
                &revision,
                &remaining_files,
                &remaining_files,
                &new_value,
                &retained,
                &deferred_value,
                &deferred_value,
            )?;
            observations = serde_json::from_value(retained)
                .map_err(|error| format!("could not reload finding backlog: {error}"))?;
        }
        Some(story_id)
    } else {
        None
    };
    let scan_complete = filed.is_none() && remaining_files.is_empty() && observations.is_empty();
    if scan_complete {
        let complete = with_shared(|db, rt| {
            let dao = ForgeControlDao::new(db.clone());
            rt.block_on(dao.complete_learn_scan(&repository_key, &revision, &serde_json::json!([])))
                .map_err(|error| error.to_string())
        })??;
        if !complete {
            return Err(
                "learning scan completion changed concurrently; retry on the next worker pass"
                    .into(),
            );
        }
        // This file is a local convenience cache; a cache write failure cannot undo or fail the DB cursor commit.
        let _ = write_anchor(root, None);
    }
    let deferred_findings = observations_to_candidates(observations).len()
        + observations_to_candidates(deferred_observations).len();
    Ok(LearnPassReport {
        status: if scan_complete {
            ScanStatus::Complete
        } else {
            ScanStatus::Partial
        },
        source_revision: Some(revision),
        scanned_files: processed,
        deferred_files: remaining_files.len(),
        deferred_findings,
        filed_story: filed,
        error: None,
    })
}

/// Read-only Rust/JavaScript rule report. It does not query the control plane, create findings, or update the cursor.
pub fn run_learn_dry_run(root: &Path) -> Result<String, String> {
    let now = now_secs();
    let floor = now.saturating_sub(WINDOW_HOURS * 3600);
    let since = read_anchor_secs(root).unwrap_or(floor).max(floor);
    let revision = discovery::source_revision(root)?;
    let paths = discovery::changed_paths(root, None, &revision, since)?
        .into_iter()
        .filter(|path| code_path(path))
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut bytes = 0usize;
    for path in paths.iter().take(MAX_FILES) {
        let source = discovery::read_pinned_source(root, &revision, path)?;
        if !files.is_empty() && bytes.saturating_add(source.len()) > MAX_SCAN_BYTES {
            break;
        }
        bytes += source.len();
        files.push((path.clone(), source));
    }
    let (observations, parse_issues) = scan_observations(&files, &revision);
    let deferred_files = paths.len().saturating_sub(files.len());
    serde_json::to_string_pretty(&serde_json::json!({
        "dryRun": true,
        "sourceRevision": revision,
        "filesScanned": files.len(),
        "totalChangedFiles": paths.len(),
        "bytesScanned": bytes,
        "deferredFiles": deferred_files,
        "scanStatus": if parse_issues.is_empty() && deferred_files == 0 { "complete" } else { "partial" },
        "observations": observations,
        "parseIssues": parse_issues,
        "filingPerformed": false,
        "cursorAdvanced": false,
    }))
    .map_err(|error| format!("could not render learn report: {error}"))
}

#[cfg(test)]
mod tests {
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
}
