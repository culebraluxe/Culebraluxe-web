//! workflow_command_receipt claim-first. Same table as db/workflow-command-receipt.ts.

use crate::engine::vendor_session::{psql_query, sql_literal};

pub struct Receipt {
    pub outcome: String,
    pub message: Option<String>,
}

pub fn claim_receipt(command_id: &str, actor: Option<&str>) -> Result<Option<Receipt>, String> {
    let actor_sql = actor.map(sql_literal).unwrap_or_else(|| "NULL".into());
    let inserted = psql_query(&format!(
        "INSERT INTO workflow_command_receipt (command_id, outcome, aggregate_id, message, actor_app_user_id) \
         VALUES ({}, 'pending', NULL, NULL, {actor_sql}) \
         ON CONFLICT (command_id) DO NOTHING RETURNING command_id",
        sql_literal(command_id)
    ))?;
    if !inserted.trim().is_empty() {
        return Ok(None);
    }
    let row = psql_query(&format!(
        "SELECT outcome, COALESCE(message,'') FROM workflow_command_receipt WHERE command_id = {} LIMIT 1",
        sql_literal(command_id)
    ))?;
    if row.trim().is_empty() {
        return Ok(None);
    }
    let mut parts = row.splitn(2, '|');
    let outcome = parts.next().unwrap_or("").trim().to_string();
    let message = parts
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if outcome == "pending" {
        Ok(None)
    } else {
        Ok(Some(Receipt { outcome, message }))
    }
}

pub fn finalize_receipt(
    command_id: &str,
    outcome: &str,
    aggregate_id: Option<&str>,
    message: Option<&str>,
) -> Result<(), String> {
    let agg = aggregate_id
        .map(sql_literal)
        .unwrap_or_else(|| "NULL".into());
    let msg = message.map(sql_literal).unwrap_or_else(|| "NULL".into());
    psql_query(&format!(
        "UPDATE workflow_command_receipt SET outcome = {}, aggregate_id = {agg}, message = {msg} WHERE command_id = {}",
        sql_literal(outcome),
        sql_literal(command_id)
    )).map(|_| ())
}
