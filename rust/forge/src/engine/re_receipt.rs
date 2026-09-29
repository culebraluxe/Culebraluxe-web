//! workflow_command_receipt claim-first through the canonical Forge repository boundary.

use crate::engine::vendor_session::with_shared;
use db::{ForgeEngineDao, WorkflowReceiptClaim};

pub struct Receipt {
    pub outcome: String,
    pub message: Option<String>,
}

pub fn claim_receipt(command_id: &str, actor: Option<&str>) -> Result<Option<Receipt>, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.claim_workflow_receipt(command_id, actor)
                .await
                .map(|claim| match claim {
                    // This caller only replays a decided command: "I own it now" and "another process owns
                    // it" both mean the same thing here — run it (`WorkflowReceiptClaim` is the ledger's
                    // distinction, not this one's).
                    WorkflowReceiptClaim::Acquired | WorkflowReceiptClaim::HeldByAnother => None,
                    WorkflowReceiptClaim::AlreadyFinal(row) => Some(Receipt {
                        outcome: row.outcome,
                        message: row.message.filter(|value| !value.is_empty()),
                    }),
                })
                .map_err(|error| error.to_string())
        })
    })?
}

pub fn finalize_receipt(
    command_id: &str,
    outcome: &str,
    aggregate_id: Option<&str>,
    message: Option<&str>,
) -> Result<(), String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.finalize_workflow_receipt(command_id, outcome, aggregate_id, message)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}
