//! Port of workflow_app/application-port.ts + command-router.ts.

use workflow_core::{
    ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, Value, WorkflowSubject,
};

use crate::commands::*;
use crate::db::with_shared;
use crate::facts::{
    contract_workflow_facts, deal_workflow_facts, resolve_legacy_deal_id_for_contract,
};
use crate::re_receipt::{claim_receipt, finalize_receipt, Receipt, ReceiptClaim};
use crate::receipt::{replay_outcome, CommandReceipt, ReceiptOutcome};
use db::ForgeEngineDao;

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

/// Replay a stored receipt through the ONE parser (`receipt.rs`, which mirrors `db/workflow-command-receipt.ts`).
///
/// This used to be a hand-rolled six-arm match with `_ => ApplicationCommandOutcome::Success` at the end, so a
/// stored `unauthorized` — or an outcome string this build has never heard of, or a corrupt row — replayed as a
/// successful command and could advance a deal. `replay_outcome` also refuses to call a stored `pending` (the
/// claim sentinel) a terminal outcome.
fn replay_stored(command_id: &str, stored: &Receipt) -> ApplicationCommandResult {
    let decision = replay_outcome(Some(&CommandReceipt {
        command_id: command_id.to_string(),
        outcome: ReceiptOutcome::parse(&stored.outcome),
        aggregate_id: None,
        message: stored.message.clone(),
    }));
    result(
        command_id,
        application_outcome(decision.outcome),
        decision.message.unwrap_or_else(|| "replayed".into()),
    )
}

fn application_outcome(outcome: ReceiptOutcome) -> ApplicationCommandOutcome {
    match outcome {
        ReceiptOutcome::Success => ApplicationCommandOutcome::Success,
        ReceiptOutcome::ValidationFailure => ApplicationCommandOutcome::ValidationFailure,
        ReceiptOutcome::NotFound => ApplicationCommandOutcome::NotFound,
        ReceiptOutcome::Conflict => ApplicationCommandOutcome::Conflict,
        ReceiptOutcome::Unauthorized => ApplicationCommandOutcome::Unauthorized,
        ReceiptOutcome::PreconditionFailure => ApplicationCommandOutcome::PreconditionFailure,
        // A `pending` row is the claim sentinel, never a terminal engine outcome, and `replay_outcome` has
        // already turned it into a conflict before it can reach here.
        ReceiptOutcome::Pending => ApplicationCommandOutcome::Conflict,
    }
}

fn result(
    id: &str,
    outcome: ApplicationCommandOutcome,
    message: impl Into<String>,
) -> ApplicationCommandResult {
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
                format!(
                    "Contract {id} has no unambiguous legacy Deal correlation for command {}.",
                    req.command_type
                ),
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
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.current_deal_stage(deal_id)
                .await
                .map_err(|error| error.to_string())
        })
    })
    .ok()
    .and_then(Result::ok)
    .flatten()
}

fn set_stage(deal_id: &str, from: &str, to: &str) -> Result<(), String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.compare_and_set_deal_stage(deal_id, from, to)
                .await
                .map_err(|error| error.to_string())
                .and_then(|changed| if changed { Ok(()) } else { Err("cas".into()) })
        })
    })?
}

fn set_column(deal_id: &str, column: &str, value: &str) -> Result<(), String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.set_deal_field(deal_id, column, value)
                .await
                .map_err(|error| error.to_string())
                .and_then(|changed| {
                    if changed {
                        Ok(())
                    } else {
                        Err("missing".into())
                    }
                })
        })
    })?
}

/// What a receipt-write failure adds to the command result (WO-120): the domain write already
/// happened, so the outcome stands — but the caller must see that the outcome was NOT recorded,
/// or a replay will re-execute a command Forge believes it already ran.
pub(crate) fn receipt_write_failure_message(
    message: Option<&str>,
    command_id: &str,
    error: &str,
) -> String {
    match message {
        Some(message) => {
            format!("{message} [receipt write failed for command {command_id}: {error}]")
        }
        None => format!("[receipt write failed for command {command_id}: {error}]"),
    }
}

impl ApplicationPort for ReApplicationPort {
    fn execute_command(&self, req: &ApplicationCommandRequest) -> ApplicationCommandResult {
        let out = self.execute_command_inner(req);
        if out.message.as_deref() != Some("replayed") {
            if let Err(error) = finalize_receipt(
                &req.command_id,
                out.outcome.as_str(),
                req.subject_id.as_deref(),
                out.message.as_deref(),
            ) {
                eprintln!(
                    "re-port: receipt finalize failed for command {}: {error}",
                    req.command_id
                );
                return ApplicationCommandResult {
                    command_id: req.command_id.clone(),
                    outcome: out.outcome,
                    message: Some(receipt_write_failure_message(
                        out.message.as_deref(),
                        &req.command_id,
                        &error,
                    )),
                };
            }
        }
        out
    }

    fn read_facts(&self, subject: &WorkflowSubject) -> Value {
        match subject.subject_type.as_str() {
            "contract" => contract_workflow_facts(&subject.subject_id),
            "deal" => deal_workflow_facts(&subject.subject_id),
            _ => Value::object(),
        }
    }
}

impl ReApplicationPort {
    fn execute_command_inner(&self, req: &ApplicationCommandRequest) -> ApplicationCommandResult {
        if !is_routed(&req.command_type) {
            return result(
                &req.command_id,
                ApplicationCommandOutcome::NotFound,
                format!("Unknown command type: {}", req.command_type),
            );
        }
        // One claim, three answers, and none of them is "run it anyway":
        //   Final    → replay the decided outcome (normalized through `receipt.rs`, never a `_ => Success`);
        //   Held     → another owner's LIVE claim: answer "in flight" instead of applying the command twice;
        //   Acquired → the claim is ours, so the command runs;
        //   Err      → the claim could not be taken, so idempotency is unknown and the command does NOT run.
        // `HeldByAnother` used to be folded into `Acquired`'s arm ("both mean run it"), which made this port
        // concurrent rather than idempotent, and `Err(_) => {}` ran the command after a failed claim.
        match claim_receipt(&req.command_id, None) {
            Ok(ReceiptClaim::Final(existing)) => {
                return replay_stored(&req.command_id, &existing);
            }
            Ok(ReceiptClaim::HeldByAnother) => {
                return result(
                    &req.command_id,
                    ApplicationCommandOutcome::Conflict,
                    "Command is held by another owner; it is not applied twice.",
                );
            }
            Ok(ReceiptClaim::Acquired) => {}
            Err(error) => {
                return result(
                    &req.command_id,
                    ApplicationCommandOutcome::Conflict,
                    format!("Command receipt claim failed, so the command was not run: {error}"),
                );
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(outcome: &str) -> Receipt {
        Receipt {
            outcome: outcome.into(),
            message: None,
        }
    }

    #[test]
    fn a_decided_success_still_replays_as_success() {
        assert_eq!(
            replay_stored("cmd-1", &stored("success")).outcome,
            ApplicationCommandOutcome::Success
        );
    }

    #[test]
    fn an_unauthorized_receipt_does_not_replay_as_success() {
        // The hand-rolled match had no `unauthorized` arm, so this replayed as Success and advanced the deal.
        assert_eq!(
            replay_stored("cmd-1", &stored("unauthorized")).outcome,
            ApplicationCommandOutcome::Unauthorized
        );
    }

    #[test]
    fn an_outcome_this_build_does_not_know_is_not_a_success() {
        assert_eq!(
            replay_stored("cmd-1", &stored("teleported")).outcome,
            ApplicationCommandOutcome::Conflict
        );
    }

    #[test]
    fn a_pending_receipt_replays_as_an_in_flight_conflict() {
        assert_eq!(
            replay_stored("cmd-1", &stored("pending")).outcome,
            ApplicationCommandOutcome::Conflict
        );
    }

    #[test]
    fn settlement_required_is_not_a_replayable_engine_outcome() {
        // A stored `settlement_required` is not one of the receipt's terminal outcomes either: it must not
        // replay as a success, and it is not silently dropped.
        assert_eq!(
            replay_stored("cmd-1", &stored("settlement_required")).outcome,
            ApplicationCommandOutcome::Conflict
        );
    }

    /// WO-120: a receipt-write failure is reported on the result it failed to record — the domain
    /// write already happened, so the outcome stands, but the caller must see the receipt is missing.
    #[test]
    fn receipt_write_failure_is_reported_on_the_result() {
        let noted = receipt_write_failure_message(Some("under_contract"), "cmd-1", "db down");
        for part in ["under_contract", "cmd-1", "db down"] {
            assert!(noted.contains(part), "the note must carry {part}: {noted}");
        }
        let bare = receipt_write_failure_message(None, "cmd-1", "db down");
        assert!(bare.contains("cmd-1"), "{bare}");
        assert!(bare.contains("db down"), "{bare}");
    }
}
