//! Shared fixture for the FORGE ARCHITECTURE SEAM suite (`forge_arch_seam__0NN__*.rs`).
//!
//! These rails bind the layers the seam closure separated — Workflow (routing/state), JobService (lease, retry,
//! recovery), the registry and `AbstractForgeService` (shared lifecycle), the concrete role services (interpretation)
//! and the OpenCode harness (vendor mechanics) — so a later change cannot silently disconnect them.
//!
//! The composition is PRODUCTION: `ForgeRuntime::from_store` over the XML definition on the in-memory store, the
//! production `WorkflowJobService`, the production `ForgeServiceRegistry` and the seven production role services, and
//! the production durable driver `drive_forge_story_with_jobs`. Only the role TURN is scripted (no model, no
//! OpenCode, no network): `Scripted` answers each node with the evidence a healthy turn of that role would carry, and
//! records which node ran, under which Workflow task, through which service.
//!
//! The structural half reads PRODUCTION code only: comments stripped, everything from the first `#[cfg(test)]` on
//! dropped, so a test module's fixture vocabulary is never mistaken for a production dependency.
//!
//! Included with `#[path = "support/forge_arch_chain.rs"] mod arch;` — a directory under `tests/` is not a target.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use forge::engine::completion::MemoryLedger;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{
    drive_forge_story_with_jobs, DriveForgeStoryOptions, DriveForgeStoryResult,
    DurableForgeExecution, ForgeRoleOutcome, ForgeRoleRunner,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::JobService;
use forge::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use forge::engine::writer::RecordingWriter;
use forge::roles::architect::ArchitectService;
use forge::roles::dev_ops::DevOpsService;
use forge::roles::inspector::InspectorService;
use forge::roles::lead::LeadService;
use forge::roles::qa::AssayService;
use forge::roles::scout::ScoutService;
use forge::roles::smith::SmithService;
use forge::roles::{AbstractForgeService, ForgeServiceRegistry};
use test_harness::source;
use workflow::{MemoryStore, Result as WfResult, TxStore};

pub const STORY: &str = "TST-FORGE-ARCH-SEAM";
pub const WORKER: &str = "forge-arch-seam-worker";
/// The one candidate a healthy FEATURE generation produces in this fixture.
pub const CANDIDATE: &str = "c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00";

/// The evidence a FEATURE story is woken with: no scout leg, so the generation starts at the Architect.
pub fn feature_evidence() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some("FEATURE".into()),
        scout_required: Some(false),
        ..Default::default()
    }
}

/// What a healthy turn of each FEATURE node answers. Every value is a fact a real role turn would report; none of
/// them names the next node — which node runs next is the Workflow's answer, read from the XML gates.
pub fn feature_script() -> BTreeMap<&'static str, ForgeGateEvidence> {
    let mut script = BTreeMap::new();
    script.insert(
        "architect",
        ForgeGateEvidence {
            architecture_review_required: Some(false),
            ..Default::default()
        },
    );
    script.insert(
        "lead_pre",
        ForgeGateEvidence {
            lead_decision: Some("SMITH".into()),
            ..Default::default()
        },
    );
    script.insert(
        "smith",
        ForgeGateEvidence {
            candidate_sha: Some(CANDIDATE.into()),
            ..Default::default()
        },
    );
    script.insert(
        "lead_post",
        ForgeGateEvidence {
            qa_review_required: Some(true),
            ..Default::default()
        },
    );
    script.insert(
        "qa_review",
        ForgeGateEvidence {
            qa_review_passed: Some(true),
            ..Default::default()
        },
    );
    script.insert(
        "qa_verify",
        ForgeGateEvidence {
            qa_passed: Some(true),
            // `releaseDeferred` is derived from the batch deferral (facts.rs), so the generation ends QA-verified at
            // `complete` with nothing published — no release executor is needed or reached.
            deployment_deferred_to_batch: Some(1),
            ..Default::default()
        },
    );
    script
}

/// One executed role turn, as the runner behind a production service saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub service: &'static str,
    pub node: String,
    pub task_id: String,
}

/// A scripted role turn, shared by every service so the order of turns across services is one log.
///
/// It answers no envelope (`turn_ports()` is `None`), so each production service delegates its whole turn to it —
/// one call here is one paid turn. A node with no script entry is a turn the fixture did not expect, and it fails.
pub struct Scripted {
    script: BTreeMap<&'static str, ForgeGateEvidence>,
    log: Arc<Mutex<Vec<Turn>>>,
    service: &'static str,
}

impl ForgeRoleRunner for Scripted {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> WfResult<ForgeRoleOutcome> {
        self.log.lock().expect("turn log").push(Turn {
            service: self.service,
            node: node_id.to_string(),
            task_id: task.task_id.clone(),
        });
        let evidence = self.script.get(node_id).cloned().ok_or_else(|| {
            workflow::WorkflowError::generic(format!(
                "the FEATURE fixture has no turn scripted for node {node_id}"
            ))
        })?;
        Ok(ForgeRoleOutcome {
            transition_name: Some("complete".into()),
            evidence,
        })
    }
}

/// One scripted runner per production service, all writing to one turn log.
pub struct Runners {
    pub log: Arc<Mutex<Vec<Turn>>>,
    pub scout: Scripted,
    pub architect: Scripted,
    pub lead: Scripted,
    pub smith: Scripted,
    pub inspector: Scripted,
    pub assay: Scripted,
    pub devops: Scripted,
}

impl Runners {
    pub fn new(script: BTreeMap<&'static str, ForgeGateEvidence>) -> Self {
        let log = Arc::new(Mutex::new(Vec::new()));
        let make = |service: &'static str| Scripted {
            script: script.clone(),
            log: log.clone(),
            service,
        };
        Self {
            scout: make("forge.scout"),
            architect: make("forge.architect"),
            lead: make("forge.lead"),
            smith: make("forge.smith"),
            inspector: make("forge.inspector"),
            assay: make("forge.assay"),
            devops: make("forge.devops"),
            log,
        }
    }

    pub fn turns(&self) -> Vec<Turn> {
        self.log.lock().expect("turn log").clone()
    }
}

/// The seven production role services over a `Runners` set.
pub struct Services<'a> {
    pub scout: ScoutService<'a>,
    pub architect: ArchitectService<'a>,
    pub lead: LeadService<'a>,
    pub smith: SmithService<'a>,
    pub inspector: InspectorService<'a>,
    pub assay: AssayService<'a>,
    pub devops: DevOpsService<'a>,
}

impl<'a> Services<'a> {
    pub fn new(runners: &'a Runners) -> Self {
        Self {
            scout: ScoutService::new(&runners.scout),
            architect: ArchitectService::new(&runners.architect),
            lead: LeadService::new(&runners.lead),
            smith: SmithService::new(&runners.smith),
            inspector: InspectorService::new(&runners.inspector),
            assay: AssayService::new(&runners.assay),
            devops: DevOpsService::new(&runners.devops),
        }
    }

    pub fn all(&self) -> [&dyn AbstractForgeService; 7] {
        [
            &self.scout,
            &self.architect,
            &self.lead,
            &self.smith,
            &self.inspector,
            &self.assay,
            &self.devops,
        ]
    }

    /// The production registry over every service.
    pub fn registry(&self) -> ForgeServiceRegistry<'_> {
        let mut registry = ForgeServiceRegistry::new();
        for service in self.all() {
            registry
                .register(service)
                .expect("register production service");
        }
        registry
    }
}

/// The production runtime: the XML definition on the in-memory store, a canonical Story Board writer, an in-memory
/// completion ledger. `memory` is a second handle on the same store, for observation.
pub struct Fixture<S: TxStore> {
    pub rt: ForgeRuntime<S>,
    pub writer: Arc<RecordingWriter>,
}

pub fn fixture() -> (Fixture<MemoryStore>, MemoryStore) {
    let memory = MemoryStore::new();
    let writer = Arc::new(RecordingWriter::default());
    let rt = ForgeRuntime::from_store(
        memory.clone(),
        writer.clone(),
        None,
        None,
        Arc::new(MemoryLedger::new()),
        forge_sdlc_definition(),
    )
    .expect("the XML Forge definition seeds");
    (Fixture { rt, writer }, memory)
}

/// One durable FEATURE generation through the production driver.
pub fn drive<S: TxStore>(
    fixture: &Fixture<S>,
    jobs: &dyn JobService,
    registry: &ForgeServiceRegistry<'_>,
    max_steps: usize,
) -> WfResult<DriveForgeStoryResult> {
    drive_forge_story_with_jobs(
        &fixture.rt,
        STORY,
        DriveForgeStoryOptions {
            work_type: "FEATURE",
            evidence: feature_evidence(),
            // The durable branch resolves services through the registry; a runner here would be a second path.
            runner: None,
            max_steps,
            worker_id: WORKER,
            within_story_concurrency: 1,
            stop_after: None,
            turn_cap: 32,
        },
        DurableForgeExecution { jobs, registry },
    )
}

/// The durable job row `id`, through the observation handle, or `None` when no such row exists.
pub fn job(memory: &MemoryStore, id: &str) -> Option<workflow::Job> {
    memory.with_tx(|tx| tx.get_job(id)).ok()
}

/// The open (pending or locked) jobs of an instance — a finished generation leaves none.
pub fn open_jobs(memory: &MemoryStore, instance_id: &str) -> Vec<workflow::Job> {
    memory
        .with_tx(|tx| tx.open_jobs_for_instance(instance_id))
        .expect("open jobs read")
}

// ---------------------------------------------------------------------------------------------------------------
// Production-code reading for the structural half.
// ---------------------------------------------------------------------------------------------------------------

pub fn workspace_root() -> PathBuf {
    source::workspace_root()
}

/// The production code of one file: comments stripped, everything from the first `#[cfg(test)]` dropped.
pub fn production_code(path: &Path) -> String {
    let text = source::read(path);
    let production = text.split("#[cfg(test)]").next().unwrap_or("");
    production
        .lines()
        .map(source::code_of)
        .collect::<Vec<_>>()
        .join("\n")
}

/// `(relative path, production code)` for every `.rs` under `dir` (relative to the repository root).
pub fn production_tree(dir: &str) -> Vec<(String, String)> {
    let root = workspace_root().join(dir);
    assert!(root.is_dir(), "{dir} must exist to be scanned");
    let files: Vec<(String, String)> = source::sources_under(&root)
        .into_iter()
        .map(|path| (source::relative(&path), production_code(&path)))
        .collect();
    assert!(
        !files.is_empty(),
        "{dir} held no source: a scan of nothing passes"
    );
    files
}

/// The files in `tree` whose production code names any of `needles`, as `file: needle`.
pub fn naming(tree: &[(String, String)], needles: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for (path, code) in tree {
        for needle in needles {
            if code.contains(needle) {
                hits.push(format!("{path}: `{needle}`"));
            }
        }
    }
    hits
}
