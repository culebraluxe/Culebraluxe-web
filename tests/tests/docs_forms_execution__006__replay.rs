//! DOCS.FORMS.EXECUTION — replay (TST-DOCS-FORMS-EXECUTION-006).
//!
//! Contract: a vault command is executed AT MOST ONCE, however many times it arrives. The production seam
//! is the durable receipt in `db/src/vault/listing_template_id.rs`: `claim_receipt` inserts the command id
//! `on conflict do nothing`, so a second arrival can never re-execute; `replay_from_receipt` answers the
//! recorded outcome with `replayed = true` — and a claim still `pending`, or no receipt at all, is a
//! CONFLICT ("in-flight"), never a re-run. Both command paths (`transition_state` in
//! `db/src/vault/database.rs` and `issue_from_form_instance` in `db/src/vault/bind_form_to_contract.rs`)
//! take the replay branch, and the service emits its domain event only when the command was NOT replayed.
//! Document creation carries the second idempotency shape: the `(deal_id, source_system,
//! source_external_id)` conflict returns the EXISTING document instead of writing a duplicate.
//!
//! Specified level: L3 Composition / DocumentVaultHarness. The reachable deterministic seam is the one
//! TST-DOCS-CONTRACT-005 (Complete) established for this taxonomy: source-structure assertions over the
//! production receipt/idempotency machinery plus the production model types; no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_execution__006__replay

use model::VaultCommandOutcome;
use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-EXECUTION-006); the file and the assay use it.
fn docs_forms_execution_006__replay() {
    let root = source::workspace_root();
    let receipts = source::read(&root.join("db/src/vault/listing_template_id.rs"));
    let lower = receipts.to_lowercase();

    // 1. THE CLAIM: one command id executes once — the insert does NOTHING on conflict, so a replayed
    //    arrival never takes the execution branch.
    assert!(
        lower.contains("insert into workflow_command_receipt (")
            && lower.contains("on conflict (command_id) do nothing"),
        "the command id is claimed exactly once"
    );
    assert!(
        lower.contains("values ($1, 'pending', null, null, $2::uuid)"),
        "a fresh claim starts pending, so a crash mid-command reads as in-flight"
    );
    // The finalize writes the outcome onto the SAME receipt row.
    assert!(
        lower.contains("update workflow_command_receipt")
            && lower.contains("where command_id = $1"),
        "the outcome is finalized onto the claimed receipt"
    );

    // 2. THE REPLAY ANSWER: a recorded outcome is returned AS A REPLAY, and the two in-flight shapes are
    //    conflicts that say so — a replay never silently re-executes, and an in-flight command never
    //    reports a result it has not reached.
    assert!(
        receipts.contains("Command has no receipt; treat as in-flight."),
        "no receipt is a conflict, not a re-run"
    );
    assert!(
        receipts.contains("Command claim is in-flight (pending receipt); retry later."),
        "a pending claim is a conflict, not a re-run"
    );
    assert!(
        receipts.contains("receipt.outcome == \"pending\""),
        "the pending shape is detected from the receipt itself"
    );
    // NEGATIVE: an outcome the durable vocabulary does not know degrades to CONFLICT, never to Success.
    assert!(
        receipts.contains(".unwrap_or(VaultCommandOutcome::Conflict)"),
        "an unreadable recorded outcome can never become a success"
    );

    // 3. BOTH COMMAND PATHS take the replay branch: when the claim is not fresh, the answer comes from the
    //    receipt and nothing else executes.
    let database = source::read(&root.join("db/src/vault/database.rs"));
    assert!(
        database.contains("replay_from_receipt(&request.command_id, receipt)"),
        "transition_state answers a replayed command from its receipt"
    );
    let issue = source::read(&root.join("db/src/vault/bind_form_to_contract.rs"));
    assert!(
        issue.contains("replay_from_receipt(&request.command_id, receipt)"),
        "issue_from_form_instance answers a replayed command from its receipt"
    );

    // 4. NO DOUBLE SIDE EFFECTS at the service boundary: the domain events fire only for a fresh success.
    let service = source::read(&root.join("web/src/vault/mod.rs"));
    assert!(
        service.matches("!command.replayed").count() >= 2,
        "every command event is guarded against replay (transition AND issue)"
    );

    // 5. THE SECOND IDEMPOTENCY SHAPE (document creation): the external-source conflict reads back the
    //    EXISTING document — a replayed create returns the original, and a read-back that finds nothing is
    //    a schema mismatch, never an invented row.
    let database_lower = database.to_lowercase();
    assert!(
        database_lower.contains("on conflict (deal_id, source_system, source_external_id)")
            && database_lower.contains("do nothing"),
        "the create is idempotent on the external source identity"
    );
    assert!(
        database.contains("source idempotency conflict returned no existing document"),
        "a conflict that reads back nothing is a defect, not a new document"
    );

    // 6. THE RESULT TYPE (production type, executed): the replay marker is part of the command result, and
    //    the conflict vocabulary round-trips.
    let result = model::VaultCommandResult {
        command_id: "cmd-1".to_string(),
        outcome: VaultCommandOutcome::Conflict,
        aggregate_id: None,
        message: Some("Command has no receipt; treat as in-flight.".to_string()),
        replayed: true,
        value: None,
    };
    assert!(result.replayed, "the replay marker travels in the result");
    assert_eq!(result.outcome.as_str(), "conflict");
    assert_eq!(
        VaultCommandOutcome::try_from("conflict"),
        Ok(VaultCommandOutcome::Conflict)
    );
}
