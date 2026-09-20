//! Port of workflow_app/definitions/validate-definition.ts layers 1–4.

use crate::engine::commands;
use crate::engine::xml::{definition_from_xml, XmlError};

#[derive(Debug, Clone, Default)]
pub struct DefinitionValidationReport {
    pub valid: bool,
    pub xml_errors: Vec<String>,
    pub grammar_errors: Vec<String>,
    pub graph_errors: Vec<String>,
    pub application_errors: Vec<String>,
    pub errors: Vec<String>,
}

pub fn validate_definition_xml(xml: &str) -> DefinitionValidationReport {
    let mut report = DefinitionValidationReport::default();
    let parsed = match definition_from_xml(xml) {
        Ok(d) => d,
        Err(XmlError(m)) => {
            let layer = if m.contains("missing") || m.contains("expected") || m.contains("unknown") {
                "grammar"
            } else {
                "xml"
            };
            if layer == "grammar" {
                report.grammar_errors.push(m.clone());
            } else {
                report.xml_errors.push(m.clone());
            }
            report.errors.push(format!("[{layer}] {m}"));
            return report;
        }
    };
    let graph = &parsed.definition;
    if graph.start_node_id.is_empty() || !graph.nodes.contains_key(&graph.start_node_id) {
        report.graph_errors.push("missing start node".into());
    }
    for (id, node) in &graph.nodes {
        if let Some(ts) = &node.transitions {
            for t in ts {
                if !graph.nodes.contains_key(&t.to) {
                    report.graph_errors.push(format!("{id} transitions to unknown node {}", t.to));
                }
            }
        }
        if node.node_type == "command" {
            if let Some(ct) = &node.command_type {
                if !commands::is_routed(ct) {
                    report.application_errors.push(format!(
                        "command-node {id} uses unroutable command {ct}"
                    ));
                }
            } else {
                report.application_errors.push(format!("command-node {id} has no commandType"));
            }
        }
    }
    report.errors.extend(report.graph_errors.iter().map(|m| format!("[graph] {m}")));
    report.errors.extend(report.application_errors.iter().map(|m| format!("[application] {m}")));
    report.valid = report.errors.is_empty();
    report
}

pub fn validate_forge_sdlc_v6() -> DefinitionValidationReport {
    validate_definition_xml(crate::engine::xml::FORGE_SDLC_V6_XML)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn v6_is_valid() {
        let r = validate_forge_sdlc_v6();
        assert!(r.valid, "{:?}", r.errors);
    }
}
