//! Forge-internal service boundary.
//!
//! The application has `AbstractService`; Forge has the same pattern at the role layer.
//! Workflow decides which logical role is ready. A future JobService will make that work
//! durable/reliable. The concrete Forge service owns how that role behaves.
//!
//! During the strangler migration, `ForgeServiceRouter` lets selected lanes move behind
//! `AbstractForgeService` while every other lane stays on the proven production runner.

use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::role_mapping::{forge_role_node_plan, LaneId};
use crate::engine::runtime::ActiveForgeRoleTask;
use workflow::{Result, WorkflowError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForgeServiceDescriptor {
    pub service_id: &'static str,
    pub lane: LaneId,
    pub description: &'static str,
}

/// Common contract for Forge role services.
///
/// This is intentionally Forge-internal. It is not an application `AbstractService`,
/// and Workflow must not call a concrete role service directly. Workflow exposes ready
/// logical work; Forge/JobService dispatch that work to the appropriate service.
pub trait AbstractForgeService: Send + Sync {
    fn descriptor(&self) -> ForgeServiceDescriptor;

    fn execute(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
    ) -> Result<ForgeRoleOutcome>;

    fn supports_node(&self, node_id: &str) -> bool {
        forge_role_node_plan(node_id)
            .map(|plan| plan.lane == self.descriptor().lane)
            .unwrap_or(false)
    }

    fn assert_supports_node(&self, node_id: &str) -> Result<()> {
        if self.supports_node(node_id) {
            return Ok(());
        }
        let descriptor = self.descriptor();
        Err(WorkflowError::generic(format!(
            "{} refuses Forge node {node_id:?}: expected lane {:?}",
            descriptor.service_id, descriptor.lane
        )))
    }
}

/// Migration adapter from the existing `ForgeRoleRunner` seam into role services.
///
/// A lane with a registered service goes through that service. All unregistered lanes
/// use the existing runner unchanged. This lets Smith prove the boundary without
/// forcing Scout/Architect/Lead/Assay/DEV_OPS through an unfinished abstraction.
pub struct ForgeServiceRouter<'a> {
    fallback: &'a dyn ForgeRoleRunner,
    services: Vec<&'a dyn AbstractForgeService>,
}

impl<'a> ForgeServiceRouter<'a> {
    pub fn new(fallback: &'a dyn ForgeRoleRunner) -> Self {
        Self {
            fallback,
            services: Vec::new(),
        }
    }

    pub fn with_service(mut self, service: &'a dyn AbstractForgeService) -> Self {
        self.services.push(service);
        self
    }

    fn service_for(&self, node_id: &str) -> Option<&dyn AbstractForgeService> {
        self.services
            .iter()
            .copied()
            .find(|service| service.supports_node(node_id))
    }
}

impl ForgeRoleRunner for ForgeServiceRouter<'_> {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        match self.service_for(node_id) {
            Some(service) => service.execute(node_id, task),
            None => self.fallback.run(node_id, task),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::facts::ForgeGateEvidence;
    use crate::roles::smith::SmithService;
    use std::sync::Mutex;

    struct RecordingRunner {
        tag: &'static str,
        calls: Mutex<Vec<String>>,
    }

    impl RecordingRunner {
        fn new(tag: &'static str) -> Self {
            Self {
                tag,
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
                evidence: ForgeGateEvidence {
                    work_type: Some(self.tag.into()),
                    ..Default::default()
                },
            })
        }
    }

    fn task(node_id: &str) -> ActiveForgeRoleTask {
        ActiveForgeRoleTask {
            task_id: format!("task-{node_id}"),
            process_instance_id: "proc-1".into(),
            story_id: "ENG-ABSTRACT-FORGE-SERVICE-01".into(),
            token_id: None,
            node_id: Some(node_id.into()),
            status: workflow::TaskStatus::Ready,
            assignee: None,
            candidates: vec![],
        }
    }

    #[test]
    fn smith_service_owns_exactly_the_smith_lane() {
        let runner = RecordingRunner::new("smith-service");
        let service = SmithService::new(&runner);

        for node in [
            "smith",
            "smith_split_work",
            "repair_smith",
            "fast_smith",
            "fast_repair_smith",
        ] {
            assert!(service.supports_node(node), "{node} must belong to SmithService");
        }

        for node in ["architect", "lead_pre", "lead_solo_implement", "qa_verify", "deploy"] {
            assert!(
                !service.supports_node(node),
                "{node} must not leak into SmithService"
            );
        }
    }

    #[test]
    fn smith_service_fails_closed_on_a_non_smith_node() {
        let runner = RecordingRunner::new("smith-service");
        let service = SmithService::new(&runner);
        let err = service
            .execute("architect", &task("architect"))
            .expect_err("Smith must refuse Architect work");

        assert!(err.to_string().contains("forge.smith"));
        assert!(
            runner.calls().is_empty(),
            "wrong-lane work must never reach the underlying role runner"
        );
    }

    #[test]
    fn router_strangles_only_smith_and_leaves_other_roles_on_the_existing_runner() {
        let fallback = RecordingRunner::new("legacy");
        let smith_runner = RecordingRunner::new("smith-service");
        let smith = SmithService::new(&smith_runner);
        let router = ForgeServiceRouter::new(&fallback).with_service(&smith);

        let smith_out = router.run("smith", &task("smith")).expect("smith route");
        assert_eq!(smith_out.evidence.work_type.as_deref(), Some("smith-service"));

        let architect_out = router
            .run("architect", &task("architect"))
            .expect("architect fallback");
        assert_eq!(architect_out.evidence.work_type.as_deref(), Some("legacy"));

        assert_eq!(smith_runner.calls(), vec!["smith".to_string()]);
        assert_eq!(fallback.calls(), vec!["architect".to_string()]);
    }
}
