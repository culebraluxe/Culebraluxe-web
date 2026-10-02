//! Forge READY-task to job bridge.
//!
//! This module is the C-architecture seam:
//! Workflow exposes READY role work, the bridge creates a standard job request,
//! JobService executes it, and the registry resolves the concrete role service.
//!
//! None of these types may contain Scout/Smith/QA/etc. behavior. Role semantics
//! belong behind `AbstractForgeService`.

use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::registry::ForgeServiceRegistry;
use workflow::{Result, TaskStatus, WorkflowError};

#[derive(Debug, Clone)]
pub struct ForgeJobRequest {
    pub service_key: String,
    pub node_id: String,
    pub task: ActiveForgeRoleTask,
}

/// The only Forge-specific translation between workflow state and job execution.
///
/// The bridge validates that the workflow task is READY, asks the registry which
/// service owns its node, and emits a standard job request. It does not know what
/// any role actually does.
pub struct ForgeJobBridge<'registry, 'services> {
    registry: &'registry ForgeServiceRegistry<'services>,
}

impl<'registry, 'services> ForgeJobBridge<'registry, 'services> {
    pub fn new(registry: &'registry ForgeServiceRegistry<'services>) -> Self {
        Self { registry }
    }

    pub fn job_for_ready_task(&self, task: &ActiveForgeRoleTask) -> Result<ForgeJobRequest> {
        if !matches!(task.status, TaskStatus::Ready) {
            return Err(WorkflowError::generic(format!(
                "workflow task {} is {:?}, not Ready; refusing job creation",
                task.task_id, task.status
            )));
        }

        let node_id = task
            .node_id
            .as_deref()
            .filter(|node| !node.trim().is_empty())
            .ok_or_else(|| {
                WorkflowError::generic(format!(
                    "workflow task {} has no Forge node id",
                    task.task_id
                ))
            })?;

        let service = self.registry.resolve_node(node_id)?;

        Ok(ForgeJobRequest {
            service_key: service.descriptor().service_id.to_string(),
            node_id: node_id.to_string(),
            task: task.clone(),
        })
    }
}

/// Generic execution mechanism from the Forge control plane's point of view.
///
/// A durable implementation will own persistence/claim/lease/heartbeat/retry/
/// timeout/cancellation/concurrency/recovery. It must not contain role behavior.
pub trait JobService: Send + Sync {
    fn run(
        &self,
        request: &ForgeJobRequest,
        registry: &ForgeServiceRegistry<'_>,
    ) -> Result<ForgeRoleOutcome>;
}

/// Compatibility implementation for the strangler migration.
///
/// This intentionally provides no durability; it executes immediately through
/// the registry so the READY -> job -> service architecture can be established
/// before the existing story-level queue is migrated to role-level jobs.
pub struct InlineJobService;

impl JobService for InlineJobService {
    fn run(
        &self,
        request: &ForgeJobRequest,
        registry: &ForgeServiceRegistry<'_>,
    ) -> Result<ForgeRoleOutcome> {
        let service = registry.resolve(&request.service_key)?;
        service.execute(&request.node_id, &request.task)
    }
}

/// Adapter that lets the existing workflow driver keep its proven `ForgeRoleRunner`
/// port while execution crosses the new C-architecture seam.
pub struct ForgeJobRunner<'registry, 'services, 'jobs> {
    bridge: ForgeJobBridge<'registry, 'services>,
    registry: &'registry ForgeServiceRegistry<'services>,
    jobs: &'jobs dyn JobService,
}

impl<'registry, 'services, 'jobs> ForgeJobRunner<'registry, 'services, 'jobs> {
    pub fn new(
        registry: &'registry ForgeServiceRegistry<'services>,
        jobs: &'jobs dyn JobService,
    ) -> Self {
        Self {
            bridge: ForgeJobBridge::new(registry),
            registry,
            jobs,
        }
    }
}

impl ForgeRoleRunner for ForgeJobRunner<'_, '_, '_> {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        let request = self.bridge.job_for_ready_task(task)?;

        if request.node_id != node_id {
            return Err(WorkflowError::generic(format!(
                "workflow runner node {node_id:?} disagrees with READY task node {:?}",
                request.node_id
            )));
        }

        self.jobs.run(&request, self.registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::facts::ForgeGateEvidence;
    use crate::roles::architect::ArchitectService;
    use crate::roles::dev_ops::DevOpsService;
    use crate::roles::inspector::InspectorService;
    use crate::roles::lead::LeadService;
    use crate::roles::qa::AssayService;
    use crate::roles::scout::ScoutService;
    use crate::roles::smith::SmithService;
    use std::sync::Mutex;

    struct RecordingRunner {
        calls: Mutex<Vec<String>>,
    }

    impl RecordingRunner {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("calls lock").clone()
        }
    }

    impl ForgeRoleRunner for RecordingRunner {
        fn run(&self, node_id: &str, _task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(node_id.to_string());
            Ok(ForgeRoleOutcome {
                transition_name: Some("complete".into()),
                evidence: ForgeGateEvidence::default(),
            })
        }
    }

    fn task(node_id: &str, status: TaskStatus) -> ActiveForgeRoleTask {
        ActiveForgeRoleTask {
            task_id: format!("task-{node_id}"),
            process_instance_id: "proc-1".into(),
            story_id: "ENG-FORGE-JOB-BRIDGE-01".into(),
            token_id: None,
            node_id: Some(node_id.into()),
            status,
            assignee: None,
            candidates: vec![],
        }
    }

    fn registry<'a>(runner: &'a RecordingRunner) -> (
        ForgeServiceRegistry<'a>,
        ScoutService<'a>,
        ArchitectService<'a>,
        LeadService<'a>,
        SmithService<'a>,
        InspectorService<'a>,
        AssayService<'a>,
        DevOpsService<'a>,
    ) {
        let scout = ScoutService::new(runner);
        let architect = ArchitectService::new(runner);
        let lead = LeadService::new(runner);
        let smith = SmithService::new(runner);
        let inspector = InspectorService::new(runner);
        let assay = AssayService::new(runner);
        let devops = DevOpsService::new(runner);

        // This helper cannot return a registry borrowing the local service values,
        // so registration is intentionally done in each test where those owners live.
        (
            ForgeServiceRegistry::new(),
            scout,
            architect,
            lead,
            smith,
            inspector,
            assay,
            devops,
        )
    }

    #[test]
    fn bridge_maps_every_current_lane_to_its_stable_service_key() {
        let runner = RecordingRunner::new();
        let (
            mut registry,
            scout,
            architect,
            lead,
            smith,
            inspector,
            assay,
            devops,
        ) = registry(&runner);

        for service in [
            &scout as &dyn crate::roles::AbstractForgeService,
            &architect,
            &lead,
            &smith,
            &inspector,
            &assay,
            &devops,
        ] {
            registry.register(service).expect("register service");
        }

        let bridge = ForgeJobBridge::new(&registry);
        let cases = [
            ("feature_scout", "forge.scout"),
            ("architect", "forge.architect"),
            ("lead_pre", "forge.lead"),
            ("smith", "forge.smith"),
            ("qa_review", "forge.inspector"),
            ("qa_verify", "forge.assay"),
            ("deploy", "forge.devops"),
        ];

        for (node, expected_service_key) in cases {
            let job = bridge
                .job_for_ready_task(&task(node, TaskStatus::Ready))
                .expect("READY task becomes a job");
            assert_eq!(job.service_key, expected_service_key);
        }
    }

    #[test]
    fn bridge_refuses_a_workflow_task_that_is_not_ready() {
        let runner = RecordingRunner::new();
        let (mut registry, _scout, _architect, _lead, smith, _inspector, _assay, _devops) =
            registry(&runner);
        registry.register(&smith).expect("register Smith");

        let error = ForgeJobBridge::new(&registry)
            .job_for_ready_task(&task("smith", TaskStatus::Claimed))
            .expect_err("only READY workflow work may become a job");

        assert!(error.to_string().contains("not Ready"));
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn inline_job_service_is_only_a_generic_registry_delegate() {
        let runner = RecordingRunner::new();
        let (mut registry, _scout, _architect, _lead, smith, _inspector, _assay, _devops) =
            registry(&runner);
        registry.register(&smith).expect("register Smith");

        let jobs = InlineJobService;
        let job_runner = ForgeJobRunner::new(&registry, &jobs);
        job_runner
            .run("smith", &task("smith", TaskStatus::Ready))
            .expect("Smith job executes");

        assert_eq!(runner.calls(), vec!["smith".to_string()]);
    }
}
