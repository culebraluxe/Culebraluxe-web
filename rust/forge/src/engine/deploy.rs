//! Port of workflow_app/definitions/deploy.ts. Same process_definitions table.

use crate::engine::version_policy::{classify_deploy, DeployDecision};
use crate::engine::vendor_session::{psql_query, sql_literal};
use crate::engine::xml::definition_from_xml;
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

pub fn upsert_process_definition(input: DeployDefinitionInput) -> Result<DeployDefinitionResult, String> {
    let incoming = graph_from_json(&input.graph_json).map_err(|e| e.to_string())?;
    let key = sql_literal(&input.key);
    let existing = psql_query(&format!(
        "SELECT id::text || '|' || definition::text FROM process_definitions \
         WHERE tenant_id IS NULL AND key = {key} AND version = {} LIMIT 1",
        input.version
    ))?;
    if !existing.is_empty() {
        let mut parts = existing.splitn(2, '|');
        let id = parts.next().unwrap_or("").trim().to_string();
        let prev_json = parts.next().unwrap_or("{}");
        let previous = graph_from_json(prev_json).ok();
        let used = psql_query(&format!(
            "SELECT count(*)::int FROM process_instances WHERE definition_id = {}",
            sql_literal(&id)
        ))?;
        let cnt: i32 = used.trim().parse().unwrap_or(0);
        match classify_deploy(true, cnt, previous.as_ref(), &incoming) {
            DeployDecision::Reject { message } => {
                return Err(format!("{message} (definition '{}' v{})", input.key, input.version));
            }
            _ => {}
        }
        let name = sql_literal(&input.name);
        let desc = input
            .description
            .as_deref()
            .map(sql_literal)
            .unwrap_or_else(|| "NULL".into());
        let def = sql_literal(&graph_to_json(&incoming));
        let iid = sql_literal(&id);
        psql_query(&format!(
            "UPDATE process_definitions SET name = {name}, description = {desc}, \
             definition = {def}::jsonb, status = 'active', updated_at = now() WHERE id = {iid}::uuid"
        ))?;
        return Ok(DeployDefinitionResult { id, created: false });
    }
    let name = sql_literal(&input.name);
    let desc = input
        .description
        .as_deref()
        .map(sql_literal)
        .unwrap_or_else(|| "NULL".into());
    let def = sql_literal(&graph_to_json(&incoming));
    let by = input
        .created_by
        .as_deref()
        .map(sql_literal)
        .unwrap_or_else(|| "NULL".into());
    let id = psql_query(&format!(
        "INSERT INTO process_definitions (tenant_id, key, version, name, description, definition, status, created_by) \
         VALUES (NULL, {key}, {}, {name}, {desc}, {def}::jsonb, 'active', {by}) RETURNING id::text",
        input.version
    ))?;
    Ok(DeployDefinitionResult { id, created: true })
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
