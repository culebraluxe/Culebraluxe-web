//! Which Forge service owns a READY task — read from the workflow definition, not rediscovered in Rust.
//!
//! `FORGE_SDLC-v6.xml` already owns routing, sequencing and role responsibility. Each executable task-node now
//! also names its service (`service="forge.smith"`), using the same keys the `ForgeServiceRegistry` registers
//! (`forge.scout`, `forge.architect`, `forge.lead`, `forge.smith`, `forge.inspector`, `forge.assay`,
//! `forge.devops`). The READY→job bridge can then say `job.service_key = task.service_key()` instead of running a
//! node-id match of its own.
//!
//! WHY AN EXPLICIT ATTRIBUTE. `responsibility` cannot carry it: `qa_review` and `qa_verify` are both
//! `responsibility="qa"`, but one is Inspector (semantic, may use AI) and the other is Assay (deterministic). And
//! `form-key` names a form, not a service (`fast_repair_smith` uses `forge.repair_smith`, `lead_pre` uses
//! `forge.lead_pre`). One explicit attribute is clearer than deriving the service from either.
//!
//! HUMAN TASKS HAVE NO SERVICE. `hold`, `repair_requirements` and the legacy `fast_confirmation` are decisions a
//! person makes; they carry no `service`, and `service_key()` is `None` for them — the bridge must fail closed.
//!
//! STAGED DEPRECATION. `role_mapping::forge_role_node_plan` still maps node ids to lanes, and
//! `AbstractForgeService::supports_node` still reads it. The test `the_xml_and_the_rust_lane_mapping_agree` holds
//! the two in lockstep until the bridge and the services read this binding instead; only then can the Rust match
//! be retired.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::xml::{service_bindings_from_xml, FORGE_SDLC_V6_XML};

/// `node_id → service key` for every executable task-node in `FORGE_SDLC-v6.xml`.
pub fn forge_service_bindings() -> &'static BTreeMap<String, String> {
    static BINDINGS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    BINDINGS.get_or_init(|| {
        service_bindings_from_xml(FORGE_SDLC_V6_XML)
            .expect("FORGE_SDLC-v6.xml is the definition and its service bindings must parse")
    })
}

/// The service the XML binds to `node_id`, or `None` for a human task-node or a node the definition lacks.
pub fn service_for_node(node_id: &str) -> Option<&'static str> {
    forge_service_bindings().get(node_id).map(String::as_str)
}

impl ActiveForgeRoleTask {
    /// The canonical service that owns this task, as the workflow definition declares it.
    ///
    /// `None` means no agent service owns it (a human task, or a task with no node) and the caller must refuse
    /// to make it a job rather than guess.
    pub fn service_key(&self) -> Option<&'static str> {
        self.node_id.as_deref().and_then(service_for_node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
    use crate::engine::facts::ForgeGateEvidence;
    use crate::engine::role_mapping::{forge_role_node_plan, LaneId};
    use crate::engine::xml::definition_from_xml;
    use crate::roles::architect::{ArchitectService, ARCHITECT_SERVICE_ID};
    use crate::roles::dev_ops::{DevOpsService, DEVOPS_SERVICE_ID};
    use crate::roles::inspector::{InspectorService, INSPECTOR_SERVICE_ID};
    use crate::roles::lead::{LeadService, LEAD_SERVICE_ID};
    use crate::roles::qa::{AssayService, ASSAY_SERVICE_ID};
    use crate::roles::registry::ForgeServiceRegistry;
    use crate::roles::scout::{ScoutService, SCOUT_SERVICE_ID};
    use crate::roles::smith::{SmithService, SMITH_SERVICE_ID};
    use std::collections::{BTreeSet, VecDeque};
    use workflow::{ProcessGraph, TaskStatus};

    fn task(node_id: Option<&str>) -> ActiveForgeRoleTask {
        ActiveForgeRoleTask {
            task_id: "t-1".into(),
            process_instance_id: "p-1".into(),
            story_id: "TST-BINDING-001".into(),
            token_id: None,
            node_id: node_id.map(str::to_string),
            status: TaskStatus::Ready,
            assignee: None,
            candidates: vec![],
        }
    }

    fn graph() -> ProcessGraph {
        definition_from_xml(FORGE_SDLC_V6_XML)
            .expect("the definition parses")
            .definition
    }

    fn lane_service_id(lane: LaneId) -> &'static str {
        match lane {
            LaneId::Scout => SCOUT_SERVICE_ID,
            LaneId::Architect => ARCHITECT_SERVICE_ID,
            LaneId::Lead => LEAD_SERVICE_ID,
            LaneId::Smith => SMITH_SERVICE_ID,
            LaneId::Inspector => INSPECTOR_SERVICE_ID,
            LaneId::Assay => ASSAY_SERVICE_ID,
            LaneId::DevOps => DEVOPS_SERVICE_ID,
        }
    }

    /// Nodes reachable from `from`, never stepping onto a node in `avoid`.
    fn reachable(graph: &ProcessGraph, from: &str, avoid: &[&str]) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([from.to_string()]);
        while let Some(id) = queue.pop_front() {
            if avoid.contains(&id.as_str()) || !seen.insert(id.clone()) {
                continue;
            }
            if let Some(node) = graph.nodes.get(&id) {
                for t in node.transitions.as_deref().unwrap_or(&[]) {
                    queue.push_back(t.to.clone());
                }
            }
        }
        seen
    }

    #[test]
    fn service_identity_is_read_from_the_workflow_definition() {
        let parsed = service_bindings_from_xml(FORGE_SDLC_V6_XML).expect("bindings parse");
        assert_eq!(forge_service_bindings(), &parsed);
        let graph = graph();
        for (node, service) in &parsed {
            let def = graph
                .nodes
                .get(node)
                .unwrap_or_else(|| panic!("{node} is bound but missing from the graph"));
            assert_eq!(
                def.node_type, "task",
                "{node} is bound but is not a task-node"
            );
            assert!(service.starts_with("forge."), "{node} → {service}");
        }
    }

    #[test]
    fn every_lane_binds_to_its_canonical_service() {
        let cases = [
            ("fast_smith", SMITH_SERVICE_ID),
            ("fast_repair_smith", SMITH_SERVICE_ID),
            ("smith", SMITH_SERVICE_ID),
            ("smith_split_work", SMITH_SERVICE_ID),
            ("repair_smith", SMITH_SERVICE_ID),
            ("fast_qa_verify", ASSAY_SERVICE_ID),
            ("qa_verify", ASSAY_SERVICE_ID),
            ("qa_review", INSPECTOR_SERVICE_ID),
            ("architect", ARCHITECT_SERVICE_ID),
            ("research_architect", ARCHITECT_SERVICE_ID),
            ("repair_architect", ARCHITECT_SERVICE_ID),
            ("lead_pre", LEAD_SERVICE_ID),
            ("lead_solo_implement", LEAD_SERVICE_ID),
            ("lead_post", LEAD_SERVICE_ID),
            ("failure_classifier", LEAD_SERVICE_ID),
            ("research_scout", SCOUT_SERVICE_ID),
            ("feature_scout", SCOUT_SERVICE_ID),
            ("diagnose_scout", SCOUT_SERVICE_ID),
            ("repair_scout", SCOUT_SERVICE_ID),
            ("deploy", DEVOPS_SERVICE_ID),
            ("production_smoke", DEVOPS_SERVICE_ID),
            ("repair_devops", DEVOPS_SERVICE_ID),
        ];
        for (node, expected) in cases {
            assert_eq!(
                task(Some(node)).service_key(),
                Some(expected),
                "{node} must bind to {expected}"
            );
        }
        assert_eq!(
            forge_service_bindings().len(),
            cases.len(),
            "no unlisted binding"
        );
    }

    /// Inspector and Assay are different services that share `responsibility="qa"`. The binding must not
    /// collapse them back into one generic QA.
    #[test]
    fn inspector_and_assay_stay_distinct() {
        assert_eq!(service_for_node("qa_review"), Some(INSPECTOR_SERVICE_ID));
        assert_eq!(service_for_node("qa_verify"), Some(ASSAY_SERVICE_ID));
        assert_eq!(service_for_node("fast_qa_verify"), Some(ASSAY_SERVICE_ID));
        let graph = graph();
        assert_eq!(
            graph.nodes["qa_review"].responsibility.as_deref(),
            Some("qa")
        );
        assert_eq!(
            graph.nodes["qa_verify"].responsibility.as_deref(),
            Some("qa")
        );
    }

    #[test]
    fn a_human_or_unknown_task_has_no_service_and_fails_closed() {
        for human in ["hold", "repair_requirements", "fast_confirmation"] {
            assert_eq!(
                task(Some(human)).service_key(),
                None,
                "{human} is a human task"
            );
        }
        assert_eq!(task(Some("not_a_node")).service_key(), None);
        assert_eq!(task(None).service_key(), None);
        assert!(
            service_bindings_from_xml(
                r#"<process-definition key="k" version="1" name="n"><decision id="d" service="forge.smith"/></process-definition>"#
            )
            .is_err(),
            "a service on a non-task node is a definition error"
        );
        assert!(
            service_bindings_from_xml(
                r#"<process-definition key="k" version="1" name="n"><task-node id="t" service=" "/></process-definition>"#
            )
            .is_err(),
            "an empty service is a definition error"
        );
    }

    /// The drift guard for the staged deprecation: until the bridge and `supports_node` read the XML binding,
    /// the Rust lane mapping and the XML must say the same thing for every node, in both directions.
    #[test]
    fn the_xml_and_the_rust_lane_mapping_agree() {
        let bindings = forge_service_bindings();
        for (node, service) in bindings {
            let plan = forge_role_node_plan(node)
                .unwrap_or_else(|e| panic!("XML binds {node} but Rust cannot map it: {e}"));
            assert_eq!(lane_service_id(plan.lane), service, "{node}");
        }
        for (id, def) in &graph().nodes {
            if def.node_type == "task" && forge_role_node_plan(id).is_ok() {
                assert!(
                    bindings.contains_key(id),
                    "Rust maps {id} but the XML binds no service"
                );
            }
        }
    }

    struct NoopRunner;
    impl ForgeRoleRunner for NoopRunner {
        fn run(&self, _: &str, _: &ActiveForgeRoleTask) -> workflow::Result<ForgeRoleOutcome> {
            Ok(ForgeRoleOutcome {
                transition_name: Some("complete".into()),
                evidence: ForgeGateEvidence::default(),
            })
        }
    }

    /// Every key the XML names resolves to a registered service, and that service accepts the node — so
    /// `job.service_key = task.service_key()` is a key the registry can execute.
    #[test]
    fn every_bound_key_resolves_through_the_registry() {
        let runner = NoopRunner;
        let scout = ScoutService::new(&runner);
        let architect = ArchitectService::new(&runner);
        let lead = LeadService::new(&runner);
        let smith = SmithService::new(&runner);
        let inspector = InspectorService::new(&runner);
        let assay = AssayService::new(&runner);
        let devops = DevOpsService::new(&runner);
        let mut registry = ForgeServiceRegistry::new();
        for service in [
            &scout as &dyn crate::roles::AbstractForgeService,
            &architect,
            &lead,
            &smith,
            &inspector,
            &assay,
            &devops,
        ] {
            registry.register(service).expect("register");
        }
        for (node, key) in forge_service_bindings() {
            let service = registry
                .resolve(key)
                .unwrap_or_else(|e| panic!("{node} → {key}: {e}"));
            assert!(
                service.supports_node(node),
                "{key} refuses its own node {node}"
            );
        }
    }

    /// The attribute is metadata. Stripping every `service="…"` from the XML must leave the routing graph
    /// byte-for-byte identical, so adding the binding changed no route.
    #[test]
    fn the_binding_changes_no_route() {
        let mut stripped = String::new();
        let mut rest = FORGE_SDLC_V6_XML;
        while let Some(at) = rest.find(" service=\"") {
            stripped.push_str(&rest[..at]);
            let after = &rest[at + " service=\"".len()..];
            let close = after.find('"').expect("closing quote");
            rest = &after[close + 1..];
        }
        stripped.push_str(rest);
        assert!(!stripped.contains("service=\""));
        let before = definition_from_xml(&stripped).expect("stripped XML parses");
        assert_eq!(
            format!("{:?}", before.definition),
            format!("{:?}", graph()),
            "the service attribute must not alter any node or transition"
        );
    }

    #[test]
    fn the_fast_route_is_unchanged() {
        let graph = graph();
        let to = |node: &str, transition: &str| -> String {
            graph.nodes[node]
                .transitions
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .find(|t| t.name == transition)
                .unwrap_or_else(|| panic!("{node} has no transition {transition}"))
                .to
                .clone()
        };
        assert_eq!(to("classify_work", "fast"), "fast_lane_entry");
        assert_eq!(to("fast_lane_entry", "fast"), "fast_smith");
        assert_eq!(to("fast_smith", "complete"), "fast_qa_verify");
        assert_eq!(to("fast_qa_verify", "complete"), "fast_qa_result");
        assert_eq!(to("fast_qa_result", "pass"), "fast_publish");
        assert_eq!(to("fast_qa_result", "fail"), "fast_qa_route");
        assert_eq!(to("fast_qa_route", "smith"), "fast_repair_smith");
        assert_eq!(to("fast_repair_smith", "complete"), "fast_qa_verify");
        assert_eq!(to("fast_publish", "complete"), "fast_publish_result");
        let fast = reachable(&graph, "fast_lane_entry", &["hold"]);
        for absent in ["architect", "lead_pre", "lead_post", "qa_review", "deploy"] {
            assert!(!fast.contains(absent), "FAST must not reach {absent}");
        }
    }

    /// A FAST story resumed from HOLD into Lead may pass through lead_post, but it must rejoin the same
    /// deterministic Assay path as the native FAST lane. The legacy manual-review node remains defined only so an
    /// already-running workflow instance parked there can still be resolved; new routing has no incoming edge to it.
    #[test]
    fn a_lead_resumed_fast_story_rejoins_deterministic_qa_not_manual_review() {
        let graph = graph();
        let to = |node: &str, transition: &str| -> String {
            graph.nodes[node]
                .transitions
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .find(|t| t.name == transition)
                .unwrap_or_else(|| panic!("{node} has no transition {transition}"))
                .to
                .clone()
        };

        assert_eq!(to("fast_gate", "fast"), "fast_resume_eligibility");
        assert_eq!(
            to("fast_resume_eligibility", "verify"),
            "fast_qa_verify"
        );
        assert_eq!(to("fast_resume_eligibility", "hold"), "hold");

        let from_lead_post = reachable(&graph, "lead_post", &[]);
        assert!(
            from_lead_post.contains("fast_qa_verify"),
            "FAST resumed through Lead must reach deterministic verification"
        );
        assert!(
            !from_lead_post.contains("fast_confirmation"),
            "new routing must never send resumed FAST work to manual confirmation"
        );
        assert!(
            !reachable(&graph, "start", &[]).contains("fast_confirmation"),
            "no newly started v6 workflow may reach the legacy review"
        );
        assert!(reachable(&graph, "hold", &[]).contains("lead_post"));
        assert_eq!(
            service_for_node("fast_confirmation"),
            None,
            "legacy compatibility node remains human-owned for existing instances"
        );
    }
}
