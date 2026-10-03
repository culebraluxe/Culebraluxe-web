//! Port of workflow_app/definitions/deploy.ts. Same process_definitions table.

use crate::engine::vendor_session::with_shared;
use crate::engine::version_policy::{classify_deploy, DeployDecision};
use crate::engine::xml::definition_from_xml;
use db::ForgeEngineDao;
use workflow::json_codec::{graph_from_json, graph_to_json};

#[derive(Debug, Clone)]
pub struct DeployDefinitionInput {
    pub key: String,
    pub version: i32,
    pub name: String,
    pub description: Option<String>,
    pub graph_json: String,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeployDefinitionResult {
    pub id: String,
    pub created: bool,
}

pub fn upsert_process_definition(
    input: DeployDefinitionInput,
) -> Result<DeployDefinitionResult, String> {
    let incoming = graph_from_json(&input.graph_json).map_err(|error| error.to_string())?;
    let incoming_json: serde_json::Value =
        serde_json::from_str(&graph_to_json(&incoming)).map_err(|error| error.to_string())?;

    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            if let Some(existing) = dao
                .process_definition(&input.key, input.version)
                .await
                .map_err(|error| error.to_string())?
            {
                let previous = graph_from_json(&existing.definition.to_string()).ok();
                let used = dao
                    .process_definition_use_count(&existing.id)
                    .await
                    .map_err(|error| error.to_string())?;
                if let DeployDecision::Reject { message } =
                    classify_deploy(true, used as i32, previous.as_ref(), &incoming)
                {
                    return Err(format!(
                        "{message} (definition '{}' v{})",
                        input.key, input.version
                    ));
                }
                dao.update_process_definition(
                    &existing.id,
                    &input.name,
                    input.description.as_deref(),
                    &incoming_json,
                )
                .await
                .map_err(|error| error.to_string())?;
                return Ok(DeployDefinitionResult {
                    id: existing.id,
                    created: false,
                });
            }

            let id = dao
                .insert_process_definition(
                    &input.key,
                    input.version,
                    &input.name,
                    input.description.as_deref(),
                    &incoming_json,
                    input.created_by.as_deref(),
                )
                .await
                .map_err(|error| error.to_string())?;
            Ok(DeployDefinitionResult { id, created: true })
        })
    })?
}

pub fn deploy_xml(xml: &str, created_by: Option<&str>) -> Result<DeployDefinitionResult, String> {
    let parsed = definition_from_xml(xml).map_err(|e| e.to_string())?;
    upsert_process_definition(DeployDefinitionInput {
        key: parsed.key,
        version: parsed.version,
        name: parsed.name,
        description: parsed.description,
        graph_json: graph_to_json(&parsed.definition),
        created_by: created_by.map(|s| s.to_string()),
    })
}
