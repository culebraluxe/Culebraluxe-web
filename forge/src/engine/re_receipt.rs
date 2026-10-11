//! workflow_command_receipt claim-first through the canonical Forge repository boundary.

use crate::engine::vendor_session::with_shared;
use db::{ForgeEngineDao, WorkflowReceiptClaim};

pub struct Receipt {
    pub outcome: String,
    pub message: Option<String>,
}

/// What taking the claim said, with `Acquired` and `HeldByAnother` kept APART.
///
/// They used to be folded into one `None` ("run it"), on the reasoning that the ledger's distinction is not
/// this caller's. It is this caller's: the `pending` row that makes a claim `HeldByAnother` exists precisely
/// to stop a second process from applying the same command, so a caller that runs the command in both cases
/// is not idempotent — it is concurrent. A deal command applied twice is a deal advanced twice.
pub enum ReceiptClaim {
    /// The claim is this caller's (fresh, or stale and legitimately re-taken). Run the command.
    Acquired,
    /// Another owner holds a live claim. Do not run; the caller answers "in flight".
    HeldByAnother,
    /// Already decided. Replay this outcome; do not run the command again.
    Final(Receipt),
}

pub fn claim_receipt(command_id: &str, actor: Option<&str>) -> Result<ReceiptClaim, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.claim_workflow_receipt(command_id, actor)
                .await
                .map(|claim| match claim {
                    WorkflowReceiptClaim::Acquired => ReceiptClaim::Acquired,
                    WorkflowReceiptClaim::HeldByAnother => ReceiptClaim::HeldByAnother,
                    WorkflowReceiptClaim::AlreadyFinal(row) => ReceiptClaim::Final(Receipt {
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
