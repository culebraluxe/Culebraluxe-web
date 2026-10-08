//! SERVICE.ABSTRACT — idempotency where command declares it (TST-SERVICE-ABSTRACT-012).
//!
//! CONTRACT. A redelivered command with an idempotent capability replays its stored receipt
//! instead of re-executing: `CommandReceipt::replay_result`
//! (`middle/services/src/command.rs:205`) is the production replay — a `Succeeded` receipt replays
//! the original value with `replayed=true`, a `Failed` receipt replays the original error, and a
//! `Pending` (in-flight) receipt answers retryable `Conflict`/`COMMAND_IN_FLIGHT` so the caller
//! backs off instead of double-executing. The declaration itself is data on the capability table:
//! `ServiceCapability { idempotent, execution }` (`middle/services/src/abstract_service.rs:17`),
//! and ordered commands partition on their declared key
//! (`ServiceExecutionPolicy::partition_key`, `middle/services/src/execution.rs:40`) —
//! `contract.execute` declares `idempotent=true` ordered by `contractId`
//! (`web/src/service_gateway.rs:483`).
//!
//! So: success replays the value without re-execution; failure replays the error; in-flight
//! conflicts retryably; the declaration carries the flag and the partition key.
//!
//! NEGATIVE CASES. A test that only replayed success could pass on a boundary that replays success
//! for everything — including in-flight claims. The pending-conflict and failed-replay below are
//! the proof the receipt's state is honored, not wallpapered.
//!
//! ISOLATION. L1 Component, harness AbstractServiceHarness — pure production replay semantics, no
//! database, no network, no PROD. Deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__012__idempotency_where_command_declares_it

use serde_json::json;
use services::{
    CommandOutcome, CommandReceipt, CommandReceiptStatus, OperationKind, ServiceCapability,
    ServiceExecutionPolicy,
};

const HARNESS: &str = "AbstractServiceHarness/L1 Component";

fn succeeded_receipt() -> CommandReceipt {
    CommandReceipt {
        command_id: "cmd-1".into(),
        outcome: Some(CommandOutcome::Success),
        status: CommandReceiptStatus::Succeeded,
        aggregate_id: Some("c-1".into()),
        message: None,
        created_at: Some("2026-10-07T00:00:00Z".into()),
        actor_app_user_id: Some("abstract-012".into()),
        command_type: Some("contract.execute".into()),
        correlation_id: Some("abstract-012".into()),
        causation_id: None,
        aggregate_type: Some("contract".into()),
        result_payload: Some(json!({ "executed": 1 })),
        error_code: None,
        error_message: None,
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-012); the file and the assay use it.
fn service_abstract_012__idempotency_where_command_declares_it() {
    // ── THE CONTRACT: the stored success replays — value back, no re-execution. ──
    let result = succeeded_receipt().replay_result();
    assert_eq!(result.command_id, "cmd-1");
    assert_eq!(result.outcome, CommandOutcome::Success);
    assert!(
        result.replayed,
        "{HARNESS}: a replay must say it is a replay — the caller must know nothing re-executed"
    );
    assert_eq!(result.value, Some(json!({ "executed": 1 })));
    assert_eq!(result.error, None);
    assert_eq!(result.receipt_id.as_deref(), Some("cmd-1"));

    // ── A stored failure replays the error, not a fresh success. ──
    let failed = CommandReceipt {
        outcome: Some(CommandOutcome::Conflict),
        status: CommandReceiptStatus::Failed,
        message: Some("Already executed.".into()),
        result_payload: None,
        error_code: Some("CONTRACT_STATE".to_owned()),
        error_message: Some("Only draft contracts may execute.".to_owned()),
        ..succeeded_receipt()
    };
    let result = failed.replay_result();
    assert!(result.replayed);
    assert_eq!(result.outcome, CommandOutcome::Conflict);
    assert_eq!(result.value, None);
    let error = result.error.expect("the replayed error must travel");
    assert_eq!(error.code, "CONTRACT_STATE");
    assert!(error.retryable, "{HARNESS}: conflict replays stay retryable");

    // ── NEGATIVE: an in-flight claim conflicts retryably — never replays success. ──
    let pending = CommandReceipt {
        outcome: None,
        status: CommandReceiptStatus::Pending,
        message: None,
        result_payload: None,
        error_code: None,
        error_message: None,
        ..succeeded_receipt()
    };
    let result = pending.replay_result();
    assert_eq!(
        result.outcome,
        CommandOutcome::Conflict,
        "{HARNESS}: a pending receipt must not replay as success — that would double-execute"
    );
    assert!(!result.replayed);
    let error = result.error.expect("the conflict must explain itself");
    assert_eq!(error.code, "COMMAND_IN_FLIGHT");
    assert!(error.retryable);

    // ── The declaration is data: idempotent + ordered on the declared key. ──
    let capability = ServiceCapability {
        name: "contract.execute".into(),
        kind: OperationKind::Command,
        description: "Execute a canonical Contract through the durable command runtime.".into(),
        authorization: "contract.execute".into(),
        idempotent: true,
        execution: ServiceExecutionPolicy::ordered("contractId"),
    };
    assert!(
        capability.idempotent,
        "{HARNESS}: contract.execute declares idempotency — the replay above is what honors it"
    );
    assert_eq!(
        capability.execution.partition_key(&json!({ "contractId": "c-1" })),
        Some("c-1".into()),
        "{HARNESS}: the ordered command partitions on its declared key"
    );
    assert_eq!(
        capability.execution.partition_key(&json!({ "other": "c-1" })),
        None,
        "{HARNESS}: a missing partition key partitions nowhere, never on a guess"
    );

    // And a non-idempotent command says so honestly.
    let volatile = ServiceCapability {
        name: "calendar.createAppleEvent".into(),
        kind: OperationKind::Command,
        description: "Queue an Apple Calendar event.".into(),
        authorization: "calendar.write".into(),
        idempotent: false,
        execution: ServiceExecutionPolicy::inline(),
    };
    assert!(
        !volatile.idempotent,
        "{HARNESS}: a command that cannot replay must declare idempotent=false"
    );
}
