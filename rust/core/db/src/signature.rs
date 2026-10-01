use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use domain::{
    normalize_signature_email, validate_signature_recipients, ApplySignatureStatusRequest, PrepareSignatureRequest,
    PreparedSignatureRecipient, SendSignatureRequest, SignatureArtifactDownload, SignatureCommandOutcome, SignatureCommandResult, SignatureRecipient,
    SignatureRequest, SignatureRequestResult, SignatureRequestStatus, SignatureStatusResult,
};
use serde_json::{json, Value};
use sqlx::FromRow;
use std::collections::BTreeSet;
mod database;
mod reconcile_completed;
mod signature_row;
#[allow(unused_imports)]
pub use database::*;
#[allow(unused_imports)]
pub use reconcile_completed::*;
#[allow(unused_imports)]
pub use signature_row::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_slot_parser_rejects_role_mismatch() {
        let snapshot = json!({
            "issuedParticipants": [{
                "slotId": "BUYER:1",
                "role": "SELLER",
                "email": "person@example.test",
                "required": true
            }]
        });
        assert!(parse_slots(Some(&snapshot)).is_err());
    }

    #[test]
    fn strict_slot_parser_rejects_unanchored_required_slot() {
        let snapshot = json!({
            "issuedParticipants": [{
                "slotId": "BUYER:1",
                "role": "BUYER",
                "required": true
            }]
        });
        assert!(parse_slots(Some(&snapshot)).is_err());
    }
}
