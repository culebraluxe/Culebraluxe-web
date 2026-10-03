//! FORGE.JOB — the job layer stays generic, and stays apart from the story-level queue.
//!
//! Contract 11 (registry is lookup only), structural half: the PRODUCTION code of the JobService module
//! (`forge/src/engine/job.rs`, everything above its `#[cfg(test)]`) and of the registry
//! (`forge/src/roles/registry.rs`) names no role, no Forge node, no lane enum and no node→lane map. Service
//! identity reaches a job only through `task.service_key()` (the XML binding) and leaves it only through
//! `registry.resolve(key)`. The executed half is `forge_job__011`. This is a tripwire: the day someone writes
//! `if node == "fast_smith"` in the job layer, this fails and says where.
//!
//! Contract 12 (story-level queue remains distinct), structural: two queues, two meanings, no crossing.
//!
//!   storyboard_story.status = Ready → agent_work_item (story-level dispatch, `engine::worker`/`engine::agent_work`)
//!       → starts/wakes ONE Forge story run
//!   workflow role task = Ready → jobs.type = 'forge.role' (`engine::job`) → ONE role execution inside that run
//!
//! The role-job layer must not read, write or reinterpret `agent_work_item`, and the story-level dispatcher must not
//! claim or create `forge.role` jobs. The check is on code, not comments.
//!
//! Level: L0 Static, harness `source`.

use std::path::Path;

use test_harness::source::{code_of, contains_segment, read, workspace_root};

/// The production part of a Rust source file: everything above its first `#[cfg(test)]`, comments stripped.
fn production_code(path: &Path) -> Vec<(usize, String)> {
    let text = read(path);
    text.lines()
        .take_while(|line| !line.trim_start().starts_with("#[cfg(test)]"))
        .enumerate()
        .map(|(index, line)| (index + 1, code_of(line).to_string()))
        .filter(|(_, code)| !code.trim().is_empty())
        .collect()
}

fn offending(path: &Path, needles: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for (line, code) in production_code(path) {
        let lower = code.to_ascii_lowercase();
        for needle in needles {
            if contains_segment(&lower, needle) {
                out.push(format!(
                    "{}:{line}: `{needle}` in `{}`",
                    path.display(),
                    code.trim()
                ));
            }
        }
    }
    out
}

/// Role, lane and node vocabulary that must never decide anything inside the generic job layer.
const ROLE_VOCABULARY: &[&str] = &[
    "smith",
    "architect",
    "scout",
    "lead",
    "inspector",
    "assay",
    "devops",
    "dev_ops",
    "fast",
    "qa_verify",
    "qa_review",
    "laneid",
    "forge_role_node_plan",
    "role_mapping",
];

#[test]
fn the_job_service_and_registry_name_no_role_lane_or_node() {
    let root = workspace_root();
    let mut hits = Vec::new();
    for file in ["forge/src/engine/job.rs", "forge/src/roles/registry.rs"] {
        let path = root.join(file);
        assert!(
            !production_code(&path).is_empty(),
            "{file} must be read, not skipped"
        );
        hits.extend(offending(&path, ROLE_VOCABULARY));
    }
    assert!(
        hits.is_empty(),
        "role-specific dispatch crept into the generic job layer:\n{}",
        hits.join("\n")
    );
}

/// The scanner must be able to fail: the node→lane map it guards against is full of the vocabulary, so a scan of it
/// that found nothing would mean the clean result above is vacuous.
#[test]
fn the_scan_detects_role_vocabulary_where_it_does_exist() {
    let mapping = workspace_root().join("forge/src/engine/role_mapping.rs");
    let hits = offending(&mapping, ROLE_VOCABULARY);
    for word in ["smith", "architect", "scout", "assay", "laneid"] {
        assert!(
            hits.iter().any(|hit| hit.contains(&format!("`{word}`"))),
            "the scan missed `{word}` in role_mapping.rs; hits: {hits:?}"
        );
    }
}

#[test]
fn the_job_service_reaches_a_service_only_through_the_binding_and_the_registry() {
    let path = workspace_root().join("forge/src/engine/job.rs");
    let code: String = production_code(&path)
        .into_iter()
        .map(|(_, code)| code)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains("service_key()"),
        "the bridge reads the XML binding"
    );
    assert!(
        code.contains("registry.resolve("),
        "the worker resolves by key"
    );
    assert!(
        !code.contains("resolve_node("),
        "resolving by node id re-derives what the XML already declared"
    );
}

#[test]
fn role_jobs_and_story_level_agent_work_never_cross() {
    let root = workspace_root();
    let job = root.join("forge/src/engine/job.rs");
    let agent_side = [
        "forge/src/engine/worker.rs",
        "forge/src/engine/agent_work.rs",
    ];

    let job_hits = offending(
        &job,
        &["agent_work", "agent_work_item", "agentwork", "storyboard"],
    );
    assert!(
        job_hits.is_empty(),
        "the role-job layer must not touch the story-level queue:\n{}",
        job_hits.join("\n")
    );

    let mut agent_hits = Vec::new();
    for file in agent_side {
        let path = root.join(file);
        assert!(
            !production_code(&path).is_empty(),
            "{file} must be read, not skipped"
        );
        for (line, code) in production_code(&path) {
            for needle in [
                "FORGE_ROLE_JOB_TYPE",
                "\"forge.role\"",
                "WorkflowJobService",
                "engine::job",
            ] {
                if code.contains(needle) {
                    agent_hits.push(format!("{file}:{line}: `{needle}`"));
                }
            }
        }
    }
    assert!(
        agent_hits.is_empty(),
        "the story-level dispatcher must not create or claim role jobs:\n{}",
        agent_hits.join("\n")
    );
}
