//! Production definition is definitions/FORGE_SDLC-v6.xml.
//! forge_sdlc_graph() is a tiny fixture for unit tests only.

use std::collections::BTreeMap;
use workflow::{DecisionArm, NodeDefinition, ProcessDefinition, ProcessGraph, ProcessOutcome, TransitionDefinition, DefinitionStatus};
use crate::engine::topology::{with_position, FORGE_SDLC_KEY, FORGE_SDLC_VERSION};

fn t(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition { name: name.into(), to: to.into(), condition: None, required: None }
}
fn node(id: &str, ty: &str, transitions: Vec<TransitionDefinition>) -> NodeDefinition {
    NodeDefinition { id: id.into(), node_type: ty.into(), name: Some(id.into()), transitions: Some(transitions), ..Default::default() }
}
fn end(id: &str, outcome: ProcessOutcome) -> NodeDefinition {
    NodeDefinition { id: id.into(), node_type: "end".into(), name: Some(id.into()), outcome: Some(outcome), ..Default::default() }
}

pub fn forge_sdlc_graph() -> ProcessGraph {
    let mut nodes = BTreeMap::new();
    nodes.insert("start".into(), node("start", "start", vec![t("go", "classify_work")]));
    nodes.insert("classify_work".into(), {
        let mut n = node("classify_work", "decision", vec![
            t("feature", "execution_shape"), t("bug", "execution_shape"), t("hotfix", "execution_shape"),
            t("migration", "execution_shape"), t("research", "archive_research"), t("fast", "execution_shape"),
        ]);
        n.decisions = Some(vec![
            DecisionArm { condition: "workType == \"RESEARCH\"".into(), transition: "research".into() },
            DecisionArm { condition: "workType == \"FEATURE\"".into(), transition: "feature".into() },
            DecisionArm { condition: "workType == \"BUG\"".into(), transition: "bug".into() },
            DecisionArm { condition: "workType == \"HOTFIX\"".into(), transition: "hotfix".into() },
            DecisionArm { condition: "workType == \"MIGRATION\"".into(), transition: "migration".into() },
            DecisionArm { condition: "workType == \"FAST\"".into(), transition: "fast".into() },
        ]);
        n
    });
    nodes.insert("execution_shape".into(), with_position(node("execution_shape", "task", vec![
        t("solo", "lead_implement"), t("smith", "smith"), t("split", "split_dispatch"), t("hold", "hold"),
    ]), "lead"));
    nodes.insert("lead_implement".into(), with_position(node("lead_implement", "task", vec![t("complete", "qa_result")]), "lead"));
    nodes.insert("smith".into(), with_position(node("smith", "task", vec![t("complete", "lead_post")]), "smith"));
    nodes.insert("split_dispatch".into(), NodeDefinition {
        id: "split_dispatch".into(), node_type: "dynamic-fork".into(), name: Some("split_dispatch".into()),
        plan_variable: Some("smithPlan".into()), branch_node: Some("smith".into()), join: Some("lead_post".into()),
        transitions: Some(vec![t("join", "lead_post")]), ..Default::default()
    });
    nodes.insert("lead_post".into(), with_position(node("lead_post", "task", vec![t("complete", "qa_result")]), "lead"));
    nodes.insert("qa_result".into(), with_position(node("qa_result", "task", vec![
        t("pass", "publish_candidate"), t("fail", "repair_smith"), t("replan", "repair_architect"), t("hold", "hold"),
    ]), "qa"));
    nodes.insert("repair_smith".into(), with_position(node("repair_smith", "task", vec![t("complete", "qa_result")]), "smith"));
    nodes.insert("repair_architect".into(), with_position(node("repair_architect", "task", vec![t("complete", "execution_shape")]), "architect"));
    let mut publish = node("publish_candidate", "command", vec![t("ok", "complete")]);
    publish.command_type = Some(crate::engine::commands::PUBLISH_CANDIDATE.into());
    nodes.insert("publish_candidate".into(), with_position(publish, "dev_ops"));
    nodes.insert("hold".into(), with_position(node("hold", "task", vec![t("resume", "execution_shape"), t("cancel", "cancelled")]), "lead"));
    nodes.insert("complete".into(), end("complete", ProcessOutcome::Completed));
    nodes.insert("cancelled".into(), end("cancelled", ProcessOutcome::Cancelled));
    nodes.insert("failed".into(), end("failed", ProcessOutcome::Failed));
    nodes.insert("archive_research".into(), end("archive_research", ProcessOutcome::Completed));
    ProcessGraph { nodes, start_node_id: "start".into(), display_order: None }
}

pub fn forge_sdlc_definition() -> ProcessDefinition {
    crate::engine::xml::definition_from_xml(crate::engine::xml::FORGE_SDLC_V6_XML)
        .expect("FORGE_SDLC-v6.xml is the definition and must parse")
}

pub fn forge_sdlc_compact_definition() -> ProcessDefinition {
    ProcessDefinition {
        id: "forge-sdlc-compact".into(), tenant_id: None, key: FORGE_SDLC_KEY.into(), version: FORGE_SDLC_VERSION,
        name: "FORGE_SDLC compact fixture".into(),
        description: Some("Test fixture only. Production is the XML.".into()),
        definition: forge_sdlc_graph(), status: DefinitionStatus::Active,
    }
}
