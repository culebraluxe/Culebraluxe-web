//! Host XML: Forge definition constants and the host-specific attributes read from the raw tree.
//!
//! The generic XML 1.0 → ProcessGraph parser lives in `workflow-core::xml`; this module keeps the
//! Forge definition (`FORGE_SDLC-v6.xml`), the RE definition until W-1 step 5 moves it to re-host,
//! and the `service=` / human-gate semantics that only Forge uses.

use std::collections::{BTreeMap, BTreeSet};

pub use workflow::xml::{
    definition_from_xml, parse_process_definition_xml, ParsedDefinition, XmlError,
};
use workflow::xml::{parse_element, XmlElement, XmlNode};

fn req(el: &XmlElement, k: &str) -> Result<String, XmlError> {
    el.attrs
        .get(k)
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| XmlError(format!("missing {k} on <{}>", el.name)))
}

/// Which Forge service owns each executable task-node, exactly as the XML declares it (`service="forge.smith"`).
///
/// Kept beside the graph rather than inside the generic `NodeDefinition`: the binding is Forge's, and the
/// workflow engine (which also runs `RE_supermodel`) has no use for it. A `service` attribute anywhere but on a
/// `task-node`, or an empty one, is a definition error rather than a binding nobody can resolve.
pub fn service_bindings_from_xml(source: &str) -> Result<BTreeMap<String, String>, XmlError> {
    let root = parse_element(source)?;
    let mut out = BTreeMap::new();
    for ch in &root.children {
        let XmlNode::Element(el) = ch else { continue };
        let Some(service) = el.attrs.get("service") else {
            continue;
        };
        let id = req(el, "id")?;
        if el.name != "task-node" {
            return Err(XmlError(format!(
                "<{}> '{id}' declares service=\"{service}\"; only a task-node is executed by a service",
                el.name
            )));
        }
        let service = service.trim();
        if service.is_empty() {
            return Err(XmlError(format!(
                "task-node '{id}' declares an empty service"
            )));
        }
        out.insert(id, service.to_string());
    }
    Ok(out)
}

/// The task-nodes the XML binds NO service to: the human gates a person decides.
///
/// The definition states this itself, in its header: human tasks carry no service and are never made into agent
/// jobs. `service_bindings_from_xml` is the same fact read the other way round (which node a service DOES own), and
/// the two are one parse of one file, so a node cannot be one and not the other.
pub fn human_task_nodes_from_xml(source: &str) -> Result<BTreeSet<String>, XmlError> {
    let root = parse_element(source)?;
    let mut out = BTreeSet::new();
    for ch in &root.children {
        let XmlNode::Element(el) = ch else { continue };
        if el.name != "task-node" || el.attrs.contains_key("service") {
            continue;
        }
        out.insert(req(el, "id")?);
    }
    Ok(out)
}

pub const FORGE_SDLC_V6_XML: &str = include_str!("../../definitions/FORGE_SDLC-v6.xml");

pub const RE_SUPERMODEL_KEY: &str = "RE_supermodel";
pub const RE_SUPERMODEL_VERSION: i32 = 1;
pub const RE_SUPERMODEL_V1_XML: &str = include_str!("../../definitions/RE_supermodel-v1.xml");

pub fn parse_re_supermodel() -> Result<workflow::ProcessDefinition, XmlError> {
    definition_from_xml(RE_SUPERMODEL_V1_XML)
}

#[cfg(test)]
mod re_xml_tests {
    use super::*;

    #[test]
    fn re_supermodel_parses() {
        let def = parse_re_supermodel().expect("RE_supermodel-v1.xml must parse");
        assert_eq!(def.key, RE_SUPERMODEL_KEY);
        assert_eq!(def.version, RE_SUPERMODEL_VERSION);
        assert!(def
            .definition
            .nodes
            .contains_key(&def.definition.start_node_id));
        assert!(
            def.definition.nodes.len() >= 50,
            "got {}",
            def.definition.nodes.len()
        );
    }
}
