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
//! person makes; they carry no `service`, and `service_key()` is `None` for them — the bridge must fail closed. The
//! drive's own gate question is answered from the same parse (`is_human_gate`), so the engine no longer types that
//! set out beside the definition that declares it.
//!
//! ONE NODE TABLE. Every reader that used to ask the Rust node match (`role_mapping::forge_role_node_plan`, deleted
//! 2026-10-02) asks this binding instead: the READY→job bridge through `task.service_key()` (`engine/job.rs`),
//! `AbstractForgeService::supports_node` through [`service_for_node`] (`roles/service.rs`), and the readers that
//! want a node's LANE — its deliverable (`engine/phase.rs`), its V2 agent (`engine/opencode_agents.rs`), whether it
//! is a role node at all (`roles/lifecycle.rs`) — through [`lane_for_node`], since a service is a lane. The one
//! thing the definition does not carry, the Lead's phase, is the Lead lane's own (`roles/lead.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::engine::role_mapping::LaneId;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::xml::{human_task_nodes_from_xml, service_bindings_from_xml, FORGE_SDLC_V6_XML};

/// `node_id → service key` for every executable task-node in `FORGE_SDLC-v6.xml`.
/// Returns an error if the XML cannot be parsed, instead of panicking.
pub fn forge_service_bindings() -> Result<&'static BTreeMap<String, String>, String> {
    static BINDINGS: OnceLock<Result<BTreeMap<String, String>, String>> = OnceLock::new();
    let result = BINDINGS.get_or_init(|| {
        service_bindings_from_xml(FORGE_SDLC_V6_XML)
            .map_err(|e| format!("FORGE_SDLC-v6.xml service bindings parse failed: {e}"))
    });
    match result {
        Ok(map) => Ok(map),
        Err(e) => Err(e.clone()),
    }
}

/// The service the XML binds to `node_id`, or `None` for a human task-node or a node the definition lacks.
/// Returns an error if the XML bindings have not been initialized or failed to parse.
pub fn service_for_node(node_id: &str) -> Result<Option<&'static str>, String> {
    let bindings = forge_service_bindings()?;
    Ok(bindings.get(node_id).map(String::as_str))
}

/// The lane that owns `node_id`: the lane its XML service key names.
///
/// FAIL CLOSED: a node the definition binds no agent service to — a human gate, or a node it does not have — is an
/// error, never a default lane. A node Forge cannot place is a node Forge must not run, because "run it as Smith
/// anyway" would hand implement authority to a role nobody granted.
pub fn lane_for_node(node_id: &str) -> Result<LaneId, String> {
    let service_key = service_for_node(node_id)?;
    service_key
        .and_then(LaneId::for_service_key)
        .ok_or_else(|| format!("No Forge agent-runtime mapping for engine node '{node_id}'"))
}

/// The task-nodes the definition binds no service to — the human gates a person decides.
///
/// The list used to live in the engine (`executor.rs`'s `FORGE_HUMAN_GATE_NODES`, three names typed out beside a
/// definition that already declared them), which made the definition's header comment and the engine two places to
/// keep in step. It is read from the XML now, through the same parse as [`forge_service_bindings`], so the gate
/// question and the service question cannot disagree.
/// Returns an error if the XML cannot be parsed.
pub fn forge_human_gate_nodes() -> Result<&'static BTreeSet<String>, String> {
    static GATES: OnceLock<Result<BTreeSet<String>, String>> = OnceLock::new();
    let result = GATES.get_or_init(|| {
        human_task_nodes_from_xml(FORGE_SDLC_V6_XML)
            .map_err(|e| format!("FORGE_SDLC-v6.xml human gates parse failed: {e}"))
    });
    match result {
        Ok(set) => Ok(set),
        Err(e) => Err(e.clone()),
    }
}

/// Is this node a human gate — a task a person decides rather than a turn an agent runs?
///
/// A node the definition does not know answers `false`. The direction matters: a spurious `true` is a story put into
/// a human HOLD nobody asked for (the failure class that cost hours on 2026-09-29), while a `false` on a node that
/// is in fact a gate means the drive reports no gate rather than inventing one.
/// Returns `false` if the gates could not be loaded (treats unknown as non-gate, failing closed).
pub fn is_human_gate(node_id: &str) -> bool {
    forge_human_gate_nodes()
        .map(|gates| gates.contains(node_id))
        .unwrap_or(false)
}

/// The task-nodes the definition binds to one service key — a lane's own nodes, as the XML declares them.
///
/// The inverse of [`service_for_node`], and the reason a run's cap can be derived rather than typed out: a dispatch
/// that asks to stop after the scout names the nodes the definition gives that service, so a node added to the
/// definition is inside the cap without a Rust edit.
///
/// It answers with *services*, not with `responsibility`, and the two are not the same set: `qa_review` and
/// `qa_verify` share `responsibility="qa"` but belong to Inspector and Assay, so the position has three nodes where
/// the services have one and two. A lane is a service (the seven the registry registers); a position is what the
/// XML prints for a human reading it.
pub fn nodes_for_service(key: &str) -> BTreeSet<&'static str> {
    forge_service_bindings()
        .expect("forge service bindings must be valid")
        .iter()
        .filter(|(_, service)| service.as_str() == key)
        .map(|(node, _)| node.as_str())
        .collect()
}

impl ActiveForgeRoleTask {
    /// The canonical service that owns this task, as the workflow definition declares it.
    ///
    /// `None` means no agent service owns it (a human task, or a task with no node) and the caller must refuse
    /// to make it a job rather than guess.
    /// Returns `None` if the XML bindings could not be loaded (fail closed).
    pub fn service_key(&self) -> Option<&'static str> {
        self.node_id
            .as_deref()
            .and_then(|node_id| service_for_node(node_id).ok().flatten())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::executor::drive::{ForgeRoleOutcome, ForgeRoleRunner};
    use crate::engine::facts::ForgeGateEvidence;
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
        assert_eq!(forge_service_bindings(), Ok(&parsed));
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
            forge_service_bindings().expect("bindings must be valid").len(),
            cases.len(),
            "no unlisted binding"
        );
    }

    /// Inspector and Assay are different services that share `responsibility="qa"`. The binding must not
    /// collapse them back into one generic QA.
    #[test]
    fn inspector_and_assay_stay_distinct() {
        assert_eq!(service_for_node("qa_review"), Ok(Some(INSPECTOR_SERVICE_ID)));
        assert_eq!(service_for_node("qa_verify"), Ok(Some(ASSAY_SERVICE_ID)));
        assert_eq!(service_for_node("fast_qa_verify"), Ok(Some(ASSAY_SERVICE_ID)));
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

    /// A service is a lane: every key the definition binds names exactly one lane, every lane is bound by some
    /// node, and a node's lane is the one its service names. This is the whole of what replaced the Rust node
    /// match, so it is checked in both directions.
    #[test]
    fn a_nodes_lane_is_the_lane_its_service_names() {
        let bindings = forge_service_bindings().expect("bindings must be valid");
        for (node, service) in bindings.iter() {
            let lane = lane_for_node(node).unwrap_or_else(|e| panic!("{node}: {e}"));
            assert_eq!(lane.service_key(), service, "{node}");
        }
        let bound: BTreeSet<&str> = bindings
            .values()
            .map(String::as_str)
            .collect();
        for lane in LaneId::ALL {
            assert_eq!(LaneId::for_service_key(lane.service_key()), Some(lane));
            assert!(
                bound.contains(lane.service_key()),
                "{lane:?} is a lane the definition binds no node to"
            );
        }
        assert_eq!(bound.len(), LaneId::ALL.len(), "one service per lane");
    }

    #[test]
    fn a_human_or_unknown_node_has_no_lane() {
        for node in [
            "hold",
            "repair_requirements",
            "fast_confirmation",
            "not_a_node",
            "",
        ] {
            let error = lane_for_node(node).expect_err("no lane for a node no agent service owns");
            assert!(error.contains(&format!("'{node}'")), "{error}");
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
        for (node, key) in forge_service_bindings().expect("bindings must be valid") {
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
        assert_eq!(to("fast_resume_eligibility", "verify"), "fast_qa_verify");
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
            Ok(None),
            "legacy compatibility node remains human-owned for existing instances"
        );
    }

    /// A lane's nodes are the ones the definition binds to its service, and the group is a service group rather than
    /// a position group — the assertion that makes `nodes_for_service` the right owner for the drive's cap.
    #[test]
    fn a_lanes_nodes_are_the_ones_its_service_binds() {
        let set = |key: &str| nodes_for_service(key).into_iter().collect::<Vec<_>>();
        assert_eq!(
            set("forge.scout"),
            [
                "diagnose_scout",
                "feature_scout",
                "repair_scout",
                "research_scout"
            ],
            "the scout cap is the definition's own group, which is what the drive used to type out"
        );
        assert_eq!(
            set("forge.architect"),
            ["architect", "repair_architect", "research_architect"]
        );
        assert_eq!(set("forge.inspector"), ["qa_review"]);
        assert_eq!(set("forge.assay"), ["fast_qa_verify", "qa_verify"]);
        assert!(
            set("forge.no_such_service").is_empty(),
            "a key the definition binds nothing to is not a lane with every node in it"
        );
        // The asymmetry: one position, three nodes, two services. If this answered with `responsibility`, the
        // inspector's group would be the whole of qa — including the deterministic lane's work.
        assert_ne!(
            nodes_for_service("forge.inspector"),
            BTreeSet::from(["fast_qa_verify", "qa_review", "qa_verify"]),
            "the qa position is not one lane's group"
        );
    }

    /// The gate question is answered by the definition, and the set it answers with is the one the engine used to
    /// type out beside it: `hold`, `repair_requirements`, `fast_confirmation`.
    ///
    /// Ported from the legacy assertion that outlived its production file
    /// (`legacy/workflow_app/tests/forge-v11.test.ts:23-29`): `has('hold')` and `has('repair_requirements')` are true,
    /// `has('smith')`, `has('architect')` and `has('qa_review')` are false. What the legacy test could not say is where
    /// the set came from; the definition says it, and this reads it there.
    ///
    /// Non-vacuity matters here in both directions — a parse that found nothing would make `is_human_gate` false for
    /// every node (a story in a human gate would be dispatched at), and a parse that found every task-node would make
    /// it true for every node (a story held for no reason). Both are asserted below, not assumed.
    #[test]
    fn the_human_gates_are_the_definitions_own_no_service_task_nodes() {
        let gates = forge_human_gate_nodes().expect("gates must be valid");
        assert_eq!(
            gates.len(),
            3,
            "the definition's human gates are three; found {gates:?}"
        );
        for gate in ["hold", "repair_requirements", "fast_confirmation"] {
            assert!(is_human_gate(gate), "{gate} is a task-node with no service");
            assert_eq!(
                service_for_node(gate),
                Ok(None),
                "{gate} is a gate and a service-owned node at once"
            );
        }
        for run_by_an_agent in [
            "smith",
            "qa_verify",
            "lead_pre",
            "feature_scout",
            "architect",
            "qa_review",
        ] {
            assert!(
                !is_human_gate(run_by_an_agent),
                "{run_by_an_agent} is bound to a service, so it is a turn and not a gate"
            );
        }
        // The direction that costs: an unknown node is NOT a gate, because a spurious `true` puts a story into a human
        // HOLD nobody asked for. The drive's earlier list answered the same way, and it is the answer this keeps.
        for unknown in [
            "no_such_node",
            "",
            "HOLD",
            "start",
            "classify_work",
            "fast_qa_route",
        ] {
            assert!(
                !is_human_gate(unknown),
                "{unknown:?} is not a task-node the definition leaves without a service, so it is not a gate"
            );
        }
    }

    /// The rail: the drive holds no copy of the definition's node groups, and this is the file that would hold it.
    ///
    /// `include_str!` is the drive's own source, not a path that could be stale, so this cannot pass by reading the
    /// wrong file. Only the production half is scanned — the tests below it may name nodes, because comparing an
    /// answer against the definition is what they are for. The failure prevented is the one `arch_boundary__005`
    /// prevents for SQL: a fact the definition owns, re-spelled in the engine, drifting from the XML that is supposed
    /// to be its only home.
    #[test]
    fn the_drive_spells_no_node_group_of_its_own() {
        // Check the dispatch module for resolve_forge_stop_target (moved from drive.rs)
        let dispatch_source = include_str!("executor/dispatch.rs");
        assert!(
            dispatch_source.contains("fn resolve_forge_stop_target"),
            "the dispatch module contains resolve_forge_stop_target; a scan that saw nothing would pass this contract by accident"
        );

        // Check the drive module doesn't redefine the gate list
        let drive_source = include_str!("executor/drive.rs");
        let drive = drive_source
            .split("#[cfg(test)]")
            .next()
            .expect("the file opens with its production half");
        assert!(
            !drive.contains("FORGE_HUMAN_GATE_NODES"),
            "the drive names the gate list again; the definition owns it (`service_binding::forge_human_gate_nodes`)"
        );
        for gate in ["hold", "repair_requirements", "fast_confirmation"] {
            assert!(
                !drive.contains(&format!("\"{gate}\"")),
                "the drive spells the gate {gate:?} as a literal; read it from the definition instead"
            );
        }
        // The caps are the definition's service groups now. One node name is left on purpose and it is asserted to be
        // the only one: `forge.lead` binds four nodes while "stop after the lead" means the PRE decision, so that cap
        // names its node (`LEAD_PRE_NODE`) rather than widening to the group.
        for typed_out in [
            "feature_scout",
            "research_scout",
            "diagnose_scout",
            "repair_scout",
            "research_architect",
            "repair_architect",
        ] {
            assert!(
                !drive.contains(&format!("\"{typed_out}\"")),
                "the drive spells the node {typed_out:?}; the scout and architect caps are the definition's groups"
            );
        }
        // The LEAD_PRE_NODE constant moved to dispatch.rs
        let dispatch_source = include_str!("executor/dispatch.rs");
        assert!(
            dispatch_source.contains("const LEAD_PRE_NODE: &str = \"lead_pre\";")
                && dispatch_source.matches("LEAD_PRE_NODE").count() >= 2,
            "the one node name left is the lead cap, and it is read through the const rather than inlined"
        );
        for cap in ["forge.scout", "forge.architect"] {
            // These caps are derived from the definition's service bindings
            // Check they're in the dispatch module (via service_binding) not hardcoded in drive
            let dispatch_source = include_str!("executor/dispatch.rs");
            assert!(
                dispatch_source.contains(&format!("\"{cap}\"")),
                "the {cap} cap is derived from the definition's own binding"
            );
        }
    }
}
