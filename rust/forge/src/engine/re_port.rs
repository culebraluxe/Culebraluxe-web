//! Port of workflow_app/application-port.ts + command-router.ts.

use workflow::{
    ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult, ApplicationPort,
    Value, WorkflowSubject,
};

use crate::engine::re_commands::*;
use crate::engine::re_facts::{
    contract_workflow_facts, deal_workflow_facts, resolve_legacy_deal_id_for_contract,
};
use crate::engine::vendor_session::{psql_query, sql_literal};

pub struct ReApplicationPort;

impl ReApplicationPort {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReApplicationPort {
    fn default() -> Self {
        Self
    }
}

fn result(id: &str, outcome: ApplicationCommandOutcome, message: impl Into<String>) -> ApplicationCommandResult {
    ApplicationCommandResult {
        command_id: id.to_string(),
        outcome,
        message: Some(message.into()),
    }
}

fn resolve_deal_id(req: &ApplicationCommandRequest) -> Result<String, ApplicationCommandResult> {
    let subject_type = req.subject_type.as_deref().unwrap_or("deal");
    let subject_id = req.subject_id.clone().filter(|s| !s.is_empty());
    match (subject_type, subject_id) {
        ("deal", Some(id)) => Ok(id),
        ("contract", Some(id)) => resolve_legacy_deal_id_for_contract(&id).ok_or_else(|| {
            result(
                &req.command_id,
                ApplicationCommandOutcome::PreconditionFailure,
                format!("Contract {id} has no unambiguous legacy Deal correlation for command {}.", req.command_type),
            )
        }),
        (_, None) => Err(result(
            &req.command_id,
            ApplicationCommandOutcome::PreconditionFailure,
            "Workflow command subject is missing.",
        )),
        (other, _) => Err(result(
            &req.command_id,
            ApplicationCommandOutcome::NotFound,
            format!("Unknown subject type {other}"),
        )),
    }
}

fn current_stage(deal_id: &str) -> Option<String> {
    psql_query(&format!(
        "SELECT stage FROM deal WHERE id = {} LIMIT 1",
        sql_literal(deal_id)
    ))
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

fn set_stage(deal_id: &str, from: &str, to: &str) -> Result<(), String> {
    let sql = format!(
        "UPDATE deal SET stage = {to}, closed_at = CASE WHEN {to} = 'closed' THEN now() ELSE closed_at END, updated_at = now() \
         WHERE id = {id} AND stage = {from} RETURNING id",
        to = sql_literal(to),
        from = sql_literal(from),
        id = sql_literal(deal_id),
    );
    let raw = psql_query(&sql)?;
    if raw.trim().is_empty() {
        Err("cas".into())
    } else {
        Ok(())
    }
}

fn set_column(deal_id: &str, column: &str, value: &str) -> Result<(), String> {
    let allowed = [
        "closing_date",
        "inspection_deadline",
        "financing_deadline",
        "financing_type",
        "appraisal_required",
        "lender_clear_to_close",
    ];
    if !allowed.contains(&column) {
        return Err("bad column".into());
    }
    let sql = format!(
        "UPDATE deal SET {column} = {}, updated_at = now() WHERE id = {} RETURNING id",
        sql_literal(value),
        sql_literal(deal_id)
    );
    let raw = psql_query(&sql)?;
    if raw.trim().is_empty() {
        Err("missing".into())
    } else {
        Ok(())
    }
}

impl ApplicationPort for ReApplicationPort {
    fn execute_command(&self, req: &ApplicationCommandRequest) -> ApplicationCommandResult {
        if !is_routed(&req.command_type) {
            return result(
                &req.command_id,
                ApplicationCommandOutcome::NotFound,
                format!("Unknown command type: {}", req.command_type),
            );
        }
        if req.subject_type.as_deref() == Some("contract")
            && req.command_type == DEAL_SET_STAGE_UNDER_CONTRACT
        {
            return result(
                &req.command_id,
                ApplicationCommandOutcome::Success,
                "Contract execution already owns the under-contract transition.",
            );
        }
        let deal_id = match resolve_deal_id(req) {
            Ok(id) => id,
            Err(e) => return e,
        };
        match req.command_type.as_str() {
            DEAL_SET_STAGE_UNDER_CONTRACT => {
                match current_stage(&deal_id).as_deref() {
                    Some("under_contract") | Some("closed") => {
                        result(&req.command_id, ApplicationCommandOutcome::Success, "already under contract")
                    }
                    Some("offer") => match set_stage(&deal_id, "offer", "under_contract") {
                        Ok(()) => result(&req.command_id, ApplicationCommandOutcome::Success, "under_contract"),
                        Err(_) => result(&req.command_id, ApplicationCommandOutcome::Conflict, "stage CAS failed"),
                    },
                    Some(other) => result(
                        &req.command_id,
                        ApplicationCommandOutcome::Conflict,
                        format!("Expected stage 'offer' but was '{other}'."),
                    ),
                    None => result(&req.command_id, ApplicationCommandOutcome::NotFound, "Deal not found."),
                }
            }
            DEAL_SET_STAGE_CLOSED => match current_stage(&deal_id).as_deref() {
                Some("closed") => result(&req.command_id, ApplicationCommandOutcome::Success, "already closed"),
                Some("under_contract") => match set_stage(&deal_id, "under_contract", "closed") {
                    Ok(()) => result(&req.command_id, ApplicationCommandOutcome::Success, "closed"),
                    Err(_) => result(&req.command_id, ApplicationCommandOutcome::Conflict, "stage CAS failed"),
                },
                Some(other) => result(
                    &req.command_id,
                    ApplicationCommandOutcome::Conflict,
                    format!("Expected stage 'under_contract' but was '{other}'."),
                ),
                None => result(&req.command_id, ApplicationCommandOutcome::NotFound, "Deal not found."),
            },
            DEAL_SET_CLOSING_DATE | DEAL_SET_INSPECTION_DEADLINE | DEAL_SET_FINANCING_DEADLINE => {
                let col = match req.command_type.as_str() {
                    DEAL_SET_CLOSING_DATE => "closing_date",
                    DEAL_SET_INSPECTION_DEADLINE => "inspection_deadline",
                    _ => "financing_deadline",
                };
                let value = req
                    .input
                    .get("value")
                    .or_else(|| req.input.get("date"))
                    .or_else(|| req.input.get(col))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if value.is_empty() {
                    return result(&req.command_id, ApplicationCommandOutcome::ValidationFailure, "date required");
                }
                match set_column(&deal_id, col, value) {
                    Ok(()) => result(&req.command_id, ApplicationCommandOutcome::Success, col),
                    Err(_) => result(&req.command_id, ApplicationCommandOutcome::NotFound, "Deal not found."),
                }
            }
            DEAL_SET_FINANCING_TYPE => {
                let value = req.input.get("value").or_else(|| req.input.get("financingType")).and_then(Value::as_str).unwrap_or("");
                match set_column(&deal_id, "financing_type", value) {
                    Ok(()) => result(&req.command_id, ApplicationCommandOutcome::Success, "financing_type"),
                    Err(_) => result(&req.command_id, ApplicationCommandOutcome::NotFound, "Deal not found."),
                }
            }
            DEAL_SET_APPRAISAL_REQUIRED => {
                let value = req.input.get("value").and_then(|v| match v {
                    Value::Bool(b) => Some(if *b { "true" } else { "false" }),
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                }).unwrap_or("");
                match set_column(&deal_id, "appraisal_required", value) {
                    Ok(()) => result(&req.command_id, ApplicationCommandOutcome::Success, "appraisal_required"),
                    Err(_) => result(&req.command_id, ApplicationCommandOutcome::NotFound, "Deal not found."),
                }
            }
            DEAL_SET_LENDER_CLEAR_TO_CLOSE => {
                let value = req.input.get("value").and_then(|v| match v {
                    Value::Bool(b) => Some(if *b { "true" } else { "false" }),
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                }).unwrap_or("");
                match set_column(&deal_id, "lender_clear_to_close", value) {
                    Ok(()) => result(&req.command_id, ApplicationCommandOutcome::Success, "lender_clear_to_close"),
                    Err(_) => result(&req.command_id, ApplicationCommandOutcome::NotFound, "Deal not found."),
                }
            }
            OFFER_ACCEPT | TASK_CREATE | TASK_COMPLETE | TASK_CANCEL => result(
                &req.command_id,
                ApplicationCommandOutcome::Success,
                "application-only command accepted at the inventory seam; XML does not dispatch this node",
            ),
            other => result(
                &req.command_id,
                ApplicationCommandOutcome::NotFound,
                format!("Unknown command type: {other}"),
            ),
        }
    }

    fn read_facts(&self, subject: &WorkflowSubject) -> Value {
        match subject.subject_type.as_str() {
            "contract" => contract_workflow_facts(&subject.subject_id),
            "deal" => deal_workflow_facts(&subject.subject_id),
            _ => Value::object(),
        }
    }
}

/// Forge commands stay on ForgeApplicationPort; RE commands on ReApplicationPort.
pub struct CompositeApplicationPort {
    pub forge: Box<dyn ApplicationPort>,
    pub re: ReApplicationPort,
}

impl ApplicationPort for CompositeApplicationPort {
    fn execute_command(&self, request: &ApplicationCommandRequest) -> ApplicationCommandResult {
        if request.command_type.starts_with("forge.") {
            self.forge.execute_command(request)
        } else {
            self.re.execute_command(request)
        }
    }

    fn read_facts(&self, subject: &WorkflowSubject) -> Value {
        match subject.subject_type.as_str() {
            "story" => self.forge.read_facts(subject),
            "deal" | "contract" => self.re.read_facts(subject),
            _ => Value::object(),
        }
    }
}
