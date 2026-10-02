//! Forge-internal service boundary.
//!
//! The application has `AbstractService`; Forge has the same pattern at the role layer.
//! Workflow decides which logical role is ready. JobService makes that work durable/reliable,
//! and the XML service binding selects the concrete service. The concrete Forge service owns
//! how that role behaves.
//!
//! `ForgeServiceRouter` is the strangler adapter from the established
//! `ProductionRoleRunner` seam into concrete `AbstractForgeService` implementations.

use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::role_mapping::LaneId;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::service_binding::service_for_node;
use crate::roles::architect::ArchitectService;
use crate::roles::dev_ops::DevOpsService;
use crate::roles::inspector::InspectorService;
use crate::roles::lead::LeadService;
use crate::roles::lifecycle::{run_lane_turn, ForgeRoleHooks, NoRoleHooks};
use crate::roles::qa::AssayService;
use crate::roles::registry::ForgeServiceRegistry;
use crate::roles::scout::ScoutService;
use crate::roles::smith::SmithService;
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

    /// The runner this service's lane turns run through.
    ///
    /// The service does NOT own the ports a turn runs under — the harness, the evidence it starts from, the
    /// writer its records go through, the dispatch knobs. Its host does. Naming the runner is what lets a
    /// lane inherit the shared lifecycle and still read the turn through the very envelope it was run with.
    fn runner(&self) -> &dyn ForgeRoleRunner;

    /// The lane's own reading of its turn — the only part of the turn this boundary cannot supply.
    ///
    /// The default claims nothing, which is the right answer for a lane whose intelligence is downstream of
    /// its turn rather than about it (a publisher, a reviewer, a diagnoser writing its own marker).
    fn hooks(&self) -> &dyn ForgeRoleHooks {
        &NoRoleHooks
    }

    /// The shared execution lifecycle, INHERITED rather than copied.
    ///
    /// This is the whole point of the boundary. Every Forge lane runs the same sequence — the execution-target
    /// guard, the bounded attempt loop with its self-heal directive, the spend meter, the bench-intent cap,
    /// the deliverable gate that ends in a hold — and a lane supplies only its own reading. A service that
    /// re-implemented this method would be a service free to drift from the other six, which is exactly what
    /// this default exists to prevent: the sequence is `roles::lifecycle`'s, and the lane's answer to
    /// `hooks()` is the only thing a service adds to it.
    fn execute(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        self.assert_supports_node(node_id)?;
        run_lane_turn(self.runner(), node_id, task, self.hooks())
    }

    fn supports_node(&self, node_id: &str) -> bool {
        service_for_node(node_id)
            .map(|service_key| service_key == self.descriptor().service_id)
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

/// Compatibility adapter from the established `ForgeRoleRunner` seam into Forge role services.
///
/// Every canonical Forge lane is registered explicitly. An unmapped node is a configuration
/// error, never permission to bypass the service boundary through a generic fallback.
pub struct ForgeServiceRouter<'a> {
    services: Vec<&'a dyn AbstractForgeService>,
}

impl<'a> ForgeServiceRouter<'a> {
    pub fn new() -> Self {
        Self {
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
        let service = self.service_for(node_id).ok_or_else(|| {
            WorkflowError::generic(format!(
                "no registered Forge service owns workflow node {node_id:?}"
            ))
        })?;
        service.execute(node_id, task)
    }
}

/// Every canonical Forge lane, composed over the runner whose ports its turns run under.
///
/// ONE place lists the lanes, so the CLI's composition and the compatibility runner cannot disagree about
/// which service owns what. The list is not role policy: it names services, and each service answers for
/// itself which nodes are its own from the canonical XML service binding applied by `supports_node`.
pub struct ForgeLaneServices<'a> {
    scout: ScoutService<'a>,
    architect: ArchitectService<'a>,
    lead: LeadService<'a>,
    smith: SmithService<'a>,
    inspector: InspectorService<'a>,
    assay: AssayService<'a>,
    devops: DevOpsService<'a>,
}

impl<'a> ForgeLaneServices<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self {
            scout: ScoutService::new(runner),
            architect: ArchitectService::new(runner),
            lead: LeadService::new(runner),
            smith: SmithService::new(runner),
            inspector: InspectorService::new(runner),
            assay: AssayService::new(runner),
            devops: DevOpsService::new(runner),
        }
    }

    /// Registry view of the same seven concrete services.
    ///
    /// JobService resolves the XML-owned service key through this map. Keeping
    /// this beside `router()` means the canonical lane composition is still
    /// declared exactly once.
    pub fn registry(&self) -> Result<ForgeServiceRegistry<'_>> {
        let mut registry = ForgeServiceRegistry::new();
        for service in [
            &self.scout as &dyn AbstractForgeService,
            &self.architect,
            &self.lead,
            &self.smith,
            &self.inspector,
            &self.assay,
            &self.devops,
        ] {
            registry.register(service)?;
        }
        Ok(registry)
    }

    /// The router over these lanes. Constructed per call rather than stored: it borrows the services, and a
    /// turn is short enough that the borrow never has to outlive it.
    pub fn router(&self) -> ForgeServiceRouter<'_> {
        ForgeServiceRouter::new()
            .with_service(&self.scout)
            .with_service(&self.architect)
            .with_service(&self.lead)
            .with_service(&self.smith)
            .with_service(&self.inspector)
            .with_service(&self.assay)
            .with_service(&self.devops)
    }
}

/// The routing itself, so a holder of these lanes is itself a `ForgeRoleRunner`.
///
/// This is what lets `ProductionRoleRunner` stay a compatibility adapter: nothing in it dispatches by node,
/// because the dispatch is here and the ownership is the services'.
impl ForgeRoleRunner for ForgeLaneServices<'_> {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        self.router().run(node_id, task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::facts::ForgeGateEvidence;
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
            assert!(
                !service.supports_node(node),
                "{node} must not leak into {label}"
            );
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

        let router = ForgeServiceRouter::new()
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
    }

    #[test]
    fn router_fails_closed_for_unmapped_future_nodes() {
        let router = ForgeServiceRouter::new();
        let error = match router.run("future_unmapped_role", &task("future_unmapped_role")) {
            Ok(_) => panic!("an unmapped workflow node must never bypass the service boundary"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("no registered Forge service owns workflow node"));
    }

    /// Inspector is NOT Assay, and the boundary is where that is enforced: the review lane claims no
    /// measurement and no candidate, and the measurement lane claims no review. A lane that acquired the
    /// other's nodes would silently replace a review with a measurement.
    #[test]
    fn inspector_is_not_assay_and_neither_takes_the_others_nodes() {
        let runner = RecordingRunner::new("lane");
        let inspector = InspectorService::new(&runner);
        let assay = AssayService::new(&runner);

        assert!(inspector.supports_node("qa_review"));
        assert!(!inspector.supports_node("qa_verify"));
        assert!(assay.supports_node("qa_verify"));
        assert!(!assay.supports_node("qa_review"));

        assert!(
            !inspector.hooks().adopts_candidate_sha("qa_review"),
            "a review is not a deliverable to adopt"
        );
        assert!(
            !assay.hooks().adopts_candidate_sha("qa_verify"),
            "a measurement is not a candidate either"
        );
    }

    /// Publication belongs to one lane. Without DevOps registered, the publish nodes have no owner at all —
    /// they fall through rather than being adjudicated or measured by a lane that does not publish — and with
    /// it, they are DevOps's.
    #[test]
    fn devops_alone_owns_the_publish_nodes() {
        let publish_nodes = ["deploy", "repair_devops", "production_smoke"];

        let runner = RecordingRunner::new("lane");
        let scout = ScoutService::new(&runner);
        let architect = ArchitectService::new(&runner);
        let lead = LeadService::new(&runner);
        let smith = SmithService::new(&runner);
        let inspector = InspectorService::new(&runner);
        let assay = AssayService::new(&runner);
        let without_devops = ForgeServiceRouter::new()
            .with_service(&scout)
            .with_service(&architect)
            .with_service(&lead)
            .with_service(&smith)
            .with_service(&inspector)
            .with_service(&assay);

        for node in publish_nodes {
            assert!(
                without_devops.run(node, &task(node)).is_err(),
                "no lane but DevOps may take {node}"
            );
        }

        let devops = DevOpsService::new(&runner);
        let with_devops = ForgeServiceRouter::new().with_service(&devops);
        for node in publish_nodes {
            let out = with_devops.run(node, &task(node)).expect("devops route");
            assert_eq!(out.evidence.work_type.as_deref(), Some("lane"), "{node}");
        }
    }

    /// Code delivery is ONE behavior with ONE owner. Smith's family and the Lead's solo implement deliver the
    /// same act, so the Lead INHERITS Smith's reading instead of re-implementing the capture — and neither
    /// lane claims a node that is not a delivery.
    #[test]
    fn code_delivery_is_shared_and_claims_only_delivery_nodes() {
        let runner = RecordingRunner::new("lane");
        let smith = SmithService::new(&runner);
        let lead = LeadService::new(&runner);

        assert!(smith.hooks().adopts_candidate_sha("smith"));
        assert!(smith.hooks().adopts_candidate_sha("smith_split_work"));
        assert!(
            lead.hooks().adopts_candidate_sha("lead_solo_implement"),
            "when the Lead decides SOLO it delivers code, and that reading is Smith's"
        );
        assert!(!lead.hooks().adopts_candidate_sha("lead_pre"));
        assert!(!lead.hooks().adopts_candidate_sha("lead_post"));
        assert!(
            !smith.hooks().adopts_candidate_sha("qa_verify"),
            "a measurement is not a delivery"
        );
    }

    /// THE PROPERTY THE WHOLE EXTRACTION RESTS ON, checked where it can be checked: no concrete service
    /// implements `execute`. Every lane inherits the shared lifecycle, so a lane cannot silently acquire its
    /// own copy of the turn sequence — the only thing a service file is allowed to add is its own reading.
    #[test]
    fn no_concrete_service_copies_the_lifecycle() {
        let lanes = [
            ("scout", include_str!("scout.rs")),
            ("architect", include_str!("architect.rs")),
            ("lead", include_str!("lead.rs")),
            ("smith", include_str!("smith.rs")),
            ("inspector", include_str!("inspector.rs")),
            ("assay", include_str!("qa.rs")),
            ("devops", include_str!("dev_ops.rs")),
        ];

        for (lane, source) in lanes {
            let code = source
                .split("#[cfg(test)]")
                .next()
                .expect("each service has code before its tests");
            assert!(
                !code.contains("fn execute("),
                "{lane} copied the lifecycle instead of inheriting it"
            );
            assert!(
                code.contains("fn runner(&self)"),
                "{lane} must name the runner its turns run through"
            );
        }
    }
}
