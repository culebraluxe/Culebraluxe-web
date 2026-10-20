//! RE_supermodel definition: the app's canonical XML and parser wrapper over `workflow-core::xml`.
//!
//! The generic XML 1.0 → ProcessGraph parser belongs to `workflow-core`; this crate owns the app
//! definition and the historical `parse_re_supermodel` seam.

pub use workflow_core::xml::{
    definition_from_xml, parse_process_definition_xml, ParsedDefinition, XmlError,
};
use workflow_core::ProcessDefinition;

pub const RE_SUPERMODEL_KEY: &str = "RE_supermodel";
pub const RE_SUPERMODEL_VERSION: i32 = 1;
pub const RE_SUPERMODEL_V1_XML: &str = include_str!("../definitions/RE_supermodel-v1.xml");

pub fn parse_re_supermodel() -> Result<ProcessDefinition, XmlError> {
    definition_from_xml(RE_SUPERMODEL_V1_XML)
}
