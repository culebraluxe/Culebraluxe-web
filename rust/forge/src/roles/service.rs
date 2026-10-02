//! Forge-internal service boundary.
//!
//! The application has `AbstractService`; Forge has the same pattern at the role layer.
//! Workflow decides which logical role is ready. A future JobService will make that work
//! durable/reliable. The concrete Forge service owns how that role behaves.
//!
//! `ForgeServiceRouter` is the strangler adapter from the established
//! `ProductionRoleRunner` seam into concrete `AbstractForgeService` implementations.

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

    fn execute(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome>;

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

/// Adapter from the established `ForgeRoleRunner` seam into Forge role services.
///
/// Every canonical Forge lane is now registered through this router. The fallback remains
/// intentionally available during the strangler period for an unrecognized/future lane;
/// it does not own any currently mapped Forge role.
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
    use crate::roles::architect::ArchitectService;
    use crate::roles::dev_ops::DevOpsService;
    use crate::roles::inspector::InspectorService;
    use crate::roles::lead::LeadService;
    use crate::roles::qa::AssayService;
    use crate::roles::scout::ScoutService;
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

    fn assert_exact_lane(
        service: &dyn AbstractForgeService,
        owned: &[&str],
        rejected: &[&str],
        label: &str,
    ) {
        for node in owned {
            assert!(service.supports_node(node), "{node} must belong to {label}");
        }
        for node in rejected {
            assert!(!service.supports_node(node), "{node} must not leak into {label}");
        }
    }

    fn assert_fails_closed(
        service: &dyn AbstractForgeService,
        wrong_node: &str,
        expected_service_id: &str,
        runner: &RecordingRunner,
    ) {
        let err = match service.execute(wrong_node, &task(wrong_node)) {
            Ok(_) => panic!("{expected_service_id} must refuse {wrong_node}"),
            Err(err) => err,
        };
        assert!(err.to_string().contains(expected_service_id));
        assert!(
            runner.calls().is_empty(),
            "wrong-lane work must never reach the underlying role runner"
        );
    }

    #[test]
    fn scout_service_owns_exactly_the_scout_lane() {
        let runner = RecordingRunner::new("scout-service");
        let service = ScoutService::new(&runner);
        assert_exact_lane(
            &service,
            &[
                "research_scout",
                "feature_scout",
                "diagnose_scout",
                "repair_scout",
            ],
            &[
                "architect",
                "lead_pre",
                "smith",
                "qa_review",
                "qa_verify",
                "deploy",
            ],
            "ScoutService",
        );
    }

    #[test]
    fn architect_service_owns_exactly_the_architect_lane() {
        let runner = RecordingRunner::new("architect-service");
        let service = ArchitectService::new(&runner);
        assert_exact_lane(
            &service,
            &["research_architect", "architect", "repair_architect"],
            &[
                "research_scout",
                "lead_pre",
                "smith",
                "qa_review",
                "qa_verify",
                "deploy",
            ],
            "ArchitectService",
        );
    }

    #[test]
    fn lead_service_owns_exactly_the_lead_lane() {
        let runner = RecordingRunner::new("lead-service");
        let service = LeadService::new(&runner);
        assert_exact_lane(
            &service,
            &[
                "lead_pre",
                "lead_solo_implement",
                "lead_post",
                "failure_classifier",
            ],
            &["architect", "smith", "qa_review", "qa_verify", "deploy"],
            "LeadService",
        );
    }

    #[test]
    fn smith_service_owns_exactly_the_smith_lane() {
        let runner = RecordingRunner::new("smith-service");
        let service = SmithService::new(&runner);
        assert_exact_lane(
            &service,
            &[
                "smith",
                "smith_split_work",
                "repair_smith",
                "fast_smith",
                "fast_repair_smith",
            ],
            &["architect", "lead_pre", "qa_review", "qa_verify", "deploy"],
            "SmithService",
        );
    }

    #[test]
    fn inspector_service_owns_exactly_the_inspector_lane() {
        let runner = RecordingRunner::new("inspector-service");
        let service = InspectorService::new(&runner);
        assert_exact_lane(
            &service,
            &["qa_review"],
            &[
                "architect",
                "smith",
                "qa_verify",
                "fast_qa_verify",
                "deploy",
            ],
            "InspectorService",
        );
    }

    #[test]
    fn assay_service_owns_exactly_the_assay_lane() {
        let runner = RecordingRunner::new("assay-service");
        let service = AssayService::new(&runner);
        assert_exact_lane(
            &service,
            &["qa_verify", "fast_qa_verify"],
            &["architect", "smith", "qa_review", "deploy"],
            "AssayService",
        );
    }

    #[test]
    fn devops_service_owns_exactly_the_devops_lane() {
        let runner = RecordingRunner::new("devops-service");
        let service = DevOpsService::new(&runner);
        assert_exact_lane(
            &service,
            &["repair_devops", "deploy", "production_smoke"],
            &["architect", "smith", "qa_review", "qa_verify"],
            "DevOpsService",
        );
    }

    #[test]
    fn every_concrete_service_fails_closed_on_wrong_lane_work() {
        let scout_runner = RecordingRunner::new("scout-service");
        let architect_runner = RecordingRunner::new("architect-service");
        let lead_runner = RecordingRunner::new("lead-service");
        let smith_runner = RecordingRunner::new("smith-service");
        let inspector_runner = RecordingRunner::new("inspector-service");
        let assay_runner = RecordingRunner::new("assay-service");
        let devops_runner = RecordingRunner::new("devops-service");

        assert_fails_closed(
            &ScoutService::new(&scout_runner),
            "architect",
            "forge.scout",
            &scout_runner,
        );
        assert_fails_closed(
            &ArchitectService::new(&architect_runner),
            "smith",
            "forge.architect",
            &architect_runner,
        );
        assert_fails_closed(
            &LeadService::new(&lead_runner),
            "architect",
            "forge.lead",
            &lead_runner,
        );
        assert_fails_closed(
            &SmithService::new(&smith_runner),
            "architect",
            "forge.smith",
            &smith_runner,
        );
        assert_fails_closed(
            &InspectorService::new(&inspector_runner),
            "qa_verify",
            "forge.inspector",
            &inspector_runner,
        );
        assert_fails_closed(
            &AssayService::new(&assay_runner),
            "qa_review",
            "forge.assay",
            &assay_runner,
        );
        assert_fails_closed(
            &DevOpsService::new(&devops_runner),
            "smith",
            "forge.devops",
            &devops_runner,
        );
    }

    #[test]
    fn router_routes_every_current_forge_lane_through_its_service() {
        let fallback = RecordingRunner::new("legacy");
        let scout_runner = RecordingRunner::new("scout-service");
        let architect_runner = RecordingRunner::new("architect-service");
        let lead_runner = RecordingRunner::new("lead-service");
        let smith_runner = RecordingRunner::new("smith-service");
        let inspector_runner = RecordingRunner::new("inspector-service");
        let assay_runner = RecordingRunner::new("assay-service");
        let devops_runner = RecordingRunner::new("devops-service");

        let scout = ScoutService::new(&scout_runner);
        let architect = ArchitectService::new(&architect_runner);
        let lead = LeadService::new(&lead_runner);
        let smith = SmithService::new(&smith_runner);
        let inspector = InspectorService::new(&inspector_runner);
        let assay = AssayService::new(&assay_runner);
        let devops = DevOpsService::new(&devops_runner);

        let router = ForgeServiceRouter::new(&fallback)
            .with_service(&scout)
            .with_service(&architect)
            .with_service(&lead)
            .with_service(&smith)
            .with_service(&inspector)
            .with_service(&assay)
            .with_service(&devops);

        let cases = [
            ("feature_scout", "scout-service"),
            ("architect", "architect-service"),
            ("lead_pre", "lead-service"),
            ("smith", "smith-service"),
            ("qa_review", "inspector-service"),
            ("qa_verify", "assay-service"),
            ("deploy", "devops-service"),
        ];

        for (node, expected) in cases {
            let out = router.run(node, &task(node)).expect("service route");
            assert_eq!(out.evidence.work_type.as_deref(), Some(expected));
        }

        assert_eq!(scout_runner.calls(), vec!["feature_scout".to_string()]);
        assert_eq!(architect_runner.calls(), vec!["architect".to_string()]);
        assert_eq!(lead_runner.calls(), vec!["lead_pre".to_string()]);
        assert_eq!(smith_runner.calls(), vec!["smith".to_string()]);
        assert_eq!(inspector_runner.calls(), vec!["qa_review".to_string()]);
        assert_eq!(assay_runner.calls(), vec!["qa_verify".to_string()]);
        assert_eq!(devops_runner.calls(), vec!["deploy".to_string()]);
        assert!(
            fallback.calls().is_empty(),
            "every currently mapped Forge lane must be service-owned"
        );
    }

    #[test]
    fn router_keeps_fallback_only_for_unmapped_future_nodes() {
        let fallback = RecordingRunner::new("legacy");
        let router = ForgeServiceRouter::new(&fallback);
        let out = router
            .run("future_unmapped_role", &task("future_unmapped_role"))
            .expect("fallback route");
        assert_eq!(out.evidence.work_type.as_deref(), Some("legacy"));
        assert_eq!(fallback.calls(), vec!["future_unmapped_role".to_string()]);
    }
}
