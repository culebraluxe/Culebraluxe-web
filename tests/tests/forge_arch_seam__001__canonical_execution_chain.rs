//! ARCH-SEAM-001 — the canonical execution chain, and no other.
//!
//! CONTRACT. A Forge role turn is reached one way:
//!
//! ```text
//! Workflow READY role task → ForgeJobBridge → JobService → ForgeServiceRegistry
//!   → AbstractForgeService (shared lifecycle) → concrete role service → RoleHooks → OpenCodeHarness
//! ```
//!
//! EXECUTABLE HALF, through the production durable driver on the in-memory store:
//!   * every role turn of a FEATURE generation was a durable `forge.role` job keyed by its Workflow task, carried the
//!     XML service key, and is Completed — one job per turn, none left open;
//!   * the driver's `runner` field is not a second path: a runner handed to a durable drive is never called;
//!   * the registry is not bypassed: a process that did not register the service a READY task needs pays for no turn
//!     and writes no job — the bridge refuses before enqueue.
//!
//! STRUCTURAL HALF, over production code only:
//!   * the composition root wires the chain in its order (harness → runner → lane services → registry → JobService →
//!     durable driver), and no production code calls the non-durable `drive_forge_story`;
//!   * JobService and the registry name no role, no node and no vendor;
//!   * roles own no durable-job semantics;
//!   * the Workflow crate launches no OpenCode, and the OpenCode harness owns no Workflow routing;
//!   * the layers that move work between roles (driver, runtime, job layer, registry, shared lifecycle, worker,
//!     composition root) name no Workflow node beyond three pinned literals — a node literal there is how a Rust
//!     `if architect { call lead }` path would come back around the Workflow.
//!
//! Level: L1 executable + L0 structural.

#[path = "support/forge_arch_chain.rs"]
mod arch;

use std::collections::BTreeSet;

use arch::*;
use forge::engine::executor::{
    drive_forge_story_with_jobs, DriveForgeStoryOptions, DurableForgeExecution, ForgeRoleOutcome,
    ForgeRoleRunner,
};
use forge::engine::job::{WorkflowJobService, FORGE_ROLE_JOB_TYPE};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::service_binding::forge_service_bindings;
use forge::roles::ForgeServiceRegistry;
use workflow::{JobStatus, Value};

#[test]
fn every_role_turn_is_one_durable_job_keyed_by_its_workflow_task() {
    let runners = Runners::new(feature_script());
    let services = Services::new(&runners);
    let registry = services.registry();
    let (fixture, memory) = fixture();
    let jobs = WorkflowJobService::new(fixture.rt.engine());

    let out = drive(&fixture, &jobs, &registry, 40).expect("the FEATURE generation drives");
    let turns = runners.turns();
    assert_eq!(
        turns.len(),
        6,
        "a FEATURE generation is six role turns: {out:?}"
    );
    for turn in &turns {
        let job = job(&memory, &turn.task_id)
            .unwrap_or_else(|| panic!("{} ran with no durable job for its task", turn.node));
        assert_eq!(job.job_type, FORGE_ROLE_JOB_TYPE, "{}", turn.node);
        assert!(
            job.status.is_settled(),
            "{}: a finished generation leaves its role job settled, found {:?}",
            turn.node,
            job.status
        );
        assert_eq!(
            job.payload.get("serviceKey").and_then(Value::as_str),
            Some(turn.service),
            "{}: the service that ran is the key the job carried",
            turn.node
        );
        assert_eq!(
            job.payload.get("taskId").and_then(Value::as_str),
            Some(turn.task_id.as_str())
        );
    }
    let task_ids: std::collections::BTreeSet<&str> =
        turns.iter().map(|turn| turn.task_id.as_str()).collect();
    assert_eq!(
        task_ids.len(),
        turns.len(),
        "one Workflow task, one turn, one job"
    );
    assert!(
        open_jobs(&memory, &out.instance_id).is_empty(),
        "a finished generation leaves no open role job"
    );
}

/// Every executed role turn leaves a `Completed` durable receipt, the generation's last one included.
///
/// Kept because a RED was once reported here in error (2026-10-03): the fixture's generation was not reaching
/// `complete` but a publish step with no release executor, and that TERMINATION cancelled the in-flight job. A
/// generation that genuinely completes settles every receipt `Completed`; `cancelled by` names the kernel's own reason
/// if that ever stops being true.
#[test]
fn every_executed_role_turn_leaves_a_completed_receipt() {
    let runners = Runners::new(feature_script());
    let services = Services::new(&runners);
    let registry = services.registry();
    let (fixture, memory) = fixture();
    let jobs = WorkflowJobService::new(fixture.rt.engine());
    let out = drive(&fixture, &jobs, &registry, 40).expect("the FEATURE generation drives");
    // Who cancelled a role job, and why — the kernel's own event, so a RED names its site.
    let cancellations: Vec<String> = memory
        .with_tx(|tx| tx.history(&out.instance_id, 512))
        .expect("history")
        .into_iter()
        .filter(|event| event.event_type == "job.cancelled")
        .map(|event| format!("{} {:?}", event.actor, event.data))
        .collect();
    let receipts: Vec<(String, JobStatus)> = runners
        .turns()
        .iter()
        .map(|turn| {
            let job = job(&memory, &turn.task_id).expect("a job per turn");
            (turn.node.clone(), job.status)
        })
        .collect();
    for (node, status) in &receipts {
        assert_eq!(
            *status,
            JobStatus::Completed,
            "{node} ran and succeeded; its durable receipt must say so: {receipts:?}; cancelled by: \
             {cancellations:?}"
        );
    }
}

/// A runner that must never be reached.
struct Forbidden;
impl ForgeRoleRunner for Forbidden {
    fn run(&self, node: &str, _: &ActiveForgeRoleTask) -> workflow::Result<ForgeRoleOutcome> {
        panic!("the durable driver called the options runner for {node}: a second execution path")
    }
}

#[test]
fn the_drivers_runner_field_is_not_a_second_path() {
    let runners = Runners::new(feature_script());
    let services = Services::new(&runners);
    let registry = services.registry();
    let (fixture, _memory) = fixture();
    let jobs = WorkflowJobService::new(fixture.rt.engine());
    let forbidden = Forbidden;

    drive_forge_story_with_jobs(
        &fixture.rt,
        STORY,
        DriveForgeStoryOptions {
            work_type: "FEATURE",
            evidence: feature_evidence(),
            runner: Some(&forbidden),
            max_steps: 40,
            worker_id: WORKER,
            within_story_concurrency: 1,
            stop_after: None,
            turn_cap: 32,
        },
        DurableForgeExecution {
            jobs: &jobs,
            registry: &registry,
        },
    )
    .expect("the generation drives through the registry");
    assert_eq!(
        runners.turns().len(),
        6,
        "every turn went through a registered service"
    );
}

#[test]
fn a_service_the_process_did_not_register_costs_no_turn_and_writes_no_job() {
    let runners = Runners::new(feature_script());
    let services = Services::new(&runners);
    // Every lane except the Architect, which is where a FEATURE story wakes.
    let mut registry = ForgeServiceRegistry::new();
    for service in services.all() {
        if service.descriptor().service_id != "forge.architect" {
            registry.register(service).expect("register");
        }
    }
    let (fixture, memory) = fixture();
    let jobs = WorkflowJobService::new(fixture.rt.engine());
    let wake = fixture
        .rt
        .wake_story(STORY, "FEATURE", feature_evidence())
        .expect("wake");
    let task = wake.open.expect("the Architect task is open");

    let refused = drive(&fixture, &jobs, &registry, 40);
    assert!(
        refused.is_err(),
        "the bridge refuses a key nobody registered"
    );
    assert!(runners.turns().is_empty(), "no turn was paid for");
    assert!(
        job(&memory, &task.task_id).is_none(),
        "no durable job was written for the refused task"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Structural half.
// ---------------------------------------------------------------------------------------------------------------

/// The composition root's wiring, in order. Each is the production call that adds one link of the chain.
const COMPOSITION_ORDER: [&str; 6] = [
    "create_harness(",
    "ProductionRoleRunner::new",
    "ForgeLaneServices::new",
    ".registry()",
    "WorkflowJobService::new",
    "drive_forge_story_with_jobs(",
];

#[test]
fn the_composition_root_wires_the_chain_in_order_and_nothing_drives_around_it() {
    let root = production_code(&workspace_root().join("forge/src/bin/forge.rs"));
    let mut last = 0;
    for link in COMPOSITION_ORDER {
        let at = root[last..]
            .find(link)
            .map(|offset| last + offset)
            .unwrap_or_else(|| {
                panic!("the composition root no longer wires `{link}` after the previous link")
            });
        last = at + link.len();
    }

    // The non-durable `drive_forge_story` calls `runner.run` directly. It may exist for fixtures; nothing in
    // production may call it.
    let mut callers = Vec::new();
    for tree in ["forge/src", "cli/src", "web/src"] {
        for (path, code) in production_tree(tree) {
            if path.ends_with("engine/executor/drive.rs") {
                continue;
            }
            if code.contains("drive_forge_story(") {
                callers.push(path);
            }
        }
    }
    assert!(
        callers.is_empty(),
        "production calls the non-durable driver, which reaches a runner without JobService or the registry: \
         {callers:?}"
    );
}

#[test]
fn the_job_layer_and_registry_know_no_role_no_node_and_no_vendor() {
    let files = ["forge/src/engine/job.rs", "forge/src/roles/registry.rs"];
    let mut hits = Vec::new();
    for file in files {
        let code = production_code(&workspace_root().join(file));
        assert!(!code.trim().is_empty(), "{file} must be read");
        for needle in [
            "forge.scout",
            "forge.architect",
            "forge.lead",
            "forge.smith",
            "forge.inspector",
            "forge.assay",
            "forge.devops",
            "opencode",
            "OpenCode",
            "RoleHarness",
            "ProductionRoleRunner",
        ] {
            if code.contains(needle) {
                hits.push(format!("{file}: `{needle}`"));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "JobService / the registry branch on a role or reach the vendor:\n{}",
        hits.join("\n")
    );
}

#[test]
fn roles_own_no_durable_job_semantics() {
    let hits = naming(
        &production_tree("forge/src/roles"),
        &[
            "JobService",
            "WorkflowJobService",
            "ForgeJobLease",
            "claim_job",
            "complete_job",
            "fail_job",
            "heartbeat_job",
            "requeue_job",
            "FORGE_ROLE_JOB_TYPE",
        ],
    );
    assert!(
        hits.is_empty(),
        "a role owns lease, retry or settlement of its durable job — JobService's work:\n{}",
        hits.join("\n")
    );
}

#[test]
fn the_workflow_launches_no_vendor_and_the_vendor_harness_routes_nothing() {
    let workflow = naming(
        &production_tree("middle/workflow/src"),
        &["opencode", "OpenCode", "RoleHarness", "forge::"],
    );
    assert!(
        workflow.is_empty(),
        "the Workflow kernel reaches the vendor or Forge:\n{}",
        workflow.join("\n")
    );

    let harness: Vec<(String, String)> = [
        "forge/src/engine/opencode.rs",
        "forge/src/engine/opencode_client.rs",
        "forge/src/engine/opencode_agents.rs",
    ]
    .iter()
    .map(|file| {
        (
            file.to_string(),
            production_code(&workspace_root().join(file)),
        )
    })
    .collect();
    let routing = naming(
        &harness,
        &[
            "complete_role_task",
            "claim_role_task",
            "WorkflowEngine",
            "ForgeRuntime",
            "ForgeJobBridge",
            "JobService",
            "ForgeServiceRegistry",
            "transition_name",
        ],
    );
    assert!(
        routing.is_empty(),
        "the OpenCode harness owns Workflow routing or durable execution:\n{}",
        routing.join("\n")
    );
}

/// The node literals the role-moving layers are known to need, and why. A new one is a role branch until shown not
/// to be, so adding to this pin is a deliberate act.
const KNOWN_NODE_LITERALS: [(&str, &str); 3] = [
    // `--stop-after architect`: the operator's cap word, resolved to the definition's own Architect nodes.
    ("forge/src/engine/executor/dispatch.rs", "architect"),
    // `--stop-after lead`: the cap stops after the Lead's PRE turn.
    ("forge/src/engine/executor/dispatch.rs", "lead_pre"),
    // The split fan-out marker for wave planning.
    ("forge/src/engine/executor/drive.rs", "smith_split_work"),
];

/// The layers that move work between roles. `executor.rs` was one file until 2026-10-04; it is now the five files
/// below, and every one of them is scanned because the layer is the executor, not a filename.
const ROLE_MOVING_LAYERS: [&str; 12] = [
    "forge/src/engine/executor/completion.rs",
    "forge/src/engine/executor/dispatch.rs",
    "forge/src/engine/executor/drive.rs",
    "forge/src/engine/executor/lane_failure.rs",
    "forge/src/engine/executor/wave.rs",
    "forge/src/engine/runtime.rs",
    "forge/src/engine/job.rs",
    "forge/src/roles/registry.rs",
    "forge/src/roles/service.rs",
    "forge/src/roles/lifecycle.rs",
    "forge/src/engine/worker.rs",
    "forge/src/bin/forge.rs",
];

#[test]
fn no_layer_that_moves_work_between_roles_names_a_next_node() {
    let bindings = forge_service_bindings().expect("the definition's service bindings parse");
    let nodes: BTreeSet<&str> = bindings.keys().map(String::as_str).collect();
    let mut found = BTreeSet::new();
    for file in ROLE_MOVING_LAYERS {
        let code = production_code(&workspace_root().join(file));
        assert!(!code.trim().is_empty(), "{file} must be read, not skipped");
        for literal in code.split('"').skip(1).step_by(2) {
            if nodes.contains(literal) {
                found.insert((file.to_string(), literal.to_string()));
            }
        }
    }
    let known: BTreeSet<(String, String)> = KNOWN_NODE_LITERALS
        .iter()
        .map(|(file, node)| (file.to_string(), node.to_string()))
        .collect();
    assert_eq!(
        found, known,
        "a layer that moves work between roles names a Workflow node. Which node runs next is the XML gate's \
         answer; a node literal here is how `if architect {{ call lead }}` comes back"
    );
}
