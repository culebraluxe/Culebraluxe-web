//! ARCH-SEAM-003 — the story queue and the role-job queue stay two queues.
//!
//! CONTRACT. Two queues, two owners, one meeting point:
//!
//! ```text
//! storyboard_story Ready  → agent_work_item             (story level: dispatch, claim, begin, settle — PostgreSQL)
//! Workflow role task Ready → jobs.type = 'forge.role'    (role level: lease, heartbeat, retry, recovery — JobService)
//! ```
//!
//! They meet in exactly one place, the lane's composition root (`forge/src/bin/forge.rs`): a claimed work item opens a
//! run, the run drives role jobs, the work item is settled. Nothing else may use one as a replacement for the other.
//! `forge_job__013` pins the narrow half (`job.rs` vs the worker). This rail pins the whole boundary:
//!
//!   * EXECUTABLE: a whole FEATURE generation runs its role jobs with NO story-queue row in existence, and every role
//!     job's envelope is exactly the Workflow identity — it carries no work-item identity to collapse onto;
//!   * the story-queue side — the db DAOs, the stored routines (migrations 262–267) and the dispatch trigger (025, 146),
//!     the worker — never creates, reads or claims a `jobs` row;
//!   * the role-job side — the driver, the job layer, the registry, the roles and the Workflow kernel — never names
//!     `agent_work_item` or a story-queue verb;
//!   * the composition root is the only production file that holds both.
//!
//! Level: L1 executable; L0 structural.

#[path = "support/forge_arch_chain.rs"]
mod arch;

use std::collections::BTreeSet;

use arch::*;
use forge::engine::job::{WorkflowJobService, FORGE_ROLE_JOB_TYPE};
use test_harness::source;

/// The keys a `forge.role` envelope carries: the Workflow task's identity and its XML service key. Nothing else.
const ROLE_JOB_ENVELOPE: [&str; 6] = [
    "nodeId",
    "processInstanceId",
    "serviceKey",
    "storyId",
    "taskId",
    "tokenId",
];

#[test]
fn a_generation_runs_its_role_jobs_with_no_story_queue_row_and_carries_no_work_item_identity() {
    let runners = Runners::new(feature_script());
    let services = Services::new(&runners);
    let registry = services.registry();
    // The in-memory runtime HAS no `agent_work_item`: if the role-job path needed the story queue, this would not run.
    let (fixture, memory) = fixture();
    let jobs = WorkflowJobService::new(fixture.rt.engine());
    drive(&fixture, &jobs, &registry, 40)
        .expect("the role-job queue drives a generation on its own");

    let turns = runners.turns();
    assert_eq!(turns.len(), 6);
    for turn in &turns {
        let job = job(&memory, &turn.task_id).expect("a role job per turn");
        assert_eq!(job.job_type, FORGE_ROLE_JOB_TYPE);
        let keys: BTreeSet<String> = job
            .payload
            .as_object()
            .expect("the envelope is an object")
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            keys,
            ROLE_JOB_ENVELOPE
                .iter()
                .map(|key| key.to_string())
                .collect(),
            "{}: the role job is the Workflow task's identity, not a work item's",
            turn.node
        );
    }
}

/// The story-queue side, and what it may never touch.
const STORY_QUEUE_RUST: [&str; 5] = [
    "core/db/src/forge_engine.rs",
    "core/db/src/forge_control.rs",
    "core/db/src/forge_reset.rs",
    "forge/src/engine/agent_work.rs",
    "forge/src/engine/worker.rs",
];

/// The story queue's own SQL: the dispatch trigger and the stored routines.
const STORY_QUEUE_SQL: [&str; 8] = [
    "db/migrations/025_agent_work_queue.sql",
    "db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql",
    "db/migrations/262_forge_agent_work_claim.sql",
    "db/migrations/263_forge_agent_work_settlement.sql",
    "db/migrations/264_forge_agent_work_begin.sql",
    "db/migrations/265_forge_dispatch_reconcile.sql",
    "db/migrations/266_forge_stale_recovery.sql",
    "db/migrations/267_forge_tool_artifact_write.sql",
];

const ROLE_JOB_VOCABULARY: [&str; 9] = [
    "forge.role",
    "FORGE_ROLE_JOB_TYPE",
    "WorkflowJobService",
    "JobService",
    "into jobs",
    "update jobs",
    "from jobs",
    "claim_jobs_by_type",
    "create_job",
];

#[test]
fn the_story_queue_never_creates_reads_or_claims_a_role_job() {
    let mut hits = Vec::new();
    for file in STORY_QUEUE_RUST {
        let code = production_code(&rust_root().join(file));
        assert!(!code.trim().is_empty(), "{file} must be read");
        for needle in ROLE_JOB_VOCABULARY {
            if code.contains(needle) {
                hits.push(format!("{file}: `{needle}`"));
            }
        }
    }
    for file in STORY_QUEUE_SQL {
        let sql: String = source::read(&source::repo_root().join(file))
            .lines()
            .map(|line| line.split("--").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();
        assert!(!sql.trim().is_empty(), "{file} must be read");
        for needle in ROLE_JOB_VOCABULARY {
            if sql.contains(&needle.to_lowercase()) {
                hits.push(format!("{file}: `{needle}`"));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "the story queue reaches the role-job queue — one layer standing in for the other:\n{}",
        hits.join("\n")
    );
}

const STORY_QUEUE_VOCABULARY: [&str; 8] = [
    "agent_work_item",
    "agent_work::",
    "AgentWorkOutcome",
    "claim_next_agent_work",
    "claim_specific_agent_work",
    "begin_agent_work_run",
    "finish_agent_work_run",
    "ForgeAgentWorkRow",
];

#[test]
fn the_role_job_stack_never_touches_the_story_queue() {
    let mut tree: Vec<(String, String)> = [
        "forge/src/engine/executor.rs",
        "forge/src/engine/job.rs",
        "forge/src/engine/runtime.rs",
    ]
    .iter()
    .map(|file| (file.to_string(), production_code(&rust_root().join(file))))
    .collect();
    tree.extend(production_tree("forge/src/roles"));
    tree.extend(production_tree("core/workflow/src"));
    let hits = naming(&tree, &STORY_QUEUE_VOCABULARY);
    assert!(
        hits.is_empty(),
        "the role-job stack reaches the story queue — a role turn must not claim, begin or settle a story:\n{}",
        hits.join("\n")
    );
}

#[test]
fn the_composition_root_is_the_one_place_both_queues_meet() {
    let mut both = Vec::new();
    for tree in ["forge/src", "cli/src", "server/src"] {
        for (path, code) in production_tree(tree) {
            let story = STORY_QUEUE_VOCABULARY
                .iter()
                .any(|needle| code.contains(needle));
            let role = [
                "WorkflowJobService",
                "drive_forge_story_with_jobs",
                "FORGE_ROLE_JOB_TYPE",
            ]
            .iter()
            .any(|needle| code.contains(needle));
            if story && role {
                both.push(path);
            }
        }
    }
    assert_eq!(
        both,
        vec!["rust/forge/src/bin/forge.rs".to_string()],
        "a second production file holds both queues — the place a re-collapse would start"
    );
}
