//! FORGE.RELEASE — release receipt replay (TST-FORGE-RELEASE-008).
//!
//! Contract: a release command that is asked twice must be answered from the receipt the first run wrote, and
//! the answer must never be a success nobody measured. `Pending` is the CLAIM SENTINEL, not a terminal
//! outcome, so a receipt that is absent or in flight replays as a retryable conflict — a second process that
//! read `pending` as `success` would report a release that may never have happened.
//!
//! Two production seams carry the fact, and they may not disagree:
//!
//!   1. **the replay rule** — `replay_outcome` (`forge/src/engine/receipt.rs:43-56`). Missing receipt ⇒
//!        `Conflict`; `Pending` receipt ⇒ `Conflict`; any final receipt ⇒ ITS OWN outcome, replayed verbatim
//!        with its message. `ReceiptOutcome::parse` / `as_str` (`receipt.rs:14-41`) are the round trip that
//!        makes a stored row readable back, and `parse` is deliberately fail-closed: anything it does not name
//!        is a `Conflict`, never a success.
//!   2. **the completion unit** — `apply_completion_unit` (`forge/src/engine/completion.rs:180-198`) claims the
//!        receipt BEFORE the writes and finalizes it AFTER, so the evidence merge and the repair/replan
//!        counters run exactly once even across processes. This is the property a replay is protecting: the
//!        durable ledger exists because the engine is a CHILD PROCESS PER DISPATCH, so an in-memory guard
//!        cannot hold "exactly once" (`forge/src/engine/db_ledger.rs:1-13`).
//!
//! The negative cases are the point, and they are the two that actually bite in production: a receipt that is
//! still in flight, and a receipt whose stored outcome string is one the parser does not name. Both must
//! replay as a retryable conflict. Add a claim that has already been taken (the loser applies nothing) and a
//! second full replay of a finalized unit (also nothing, and no counter moved).
//!
//! Deterministic and isolated: no database, no network, no external provider, no environment mutation. The
//! ledger is the production `MemoryLedger`, which is the same trait `DbCompletionLedger` implements.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__008__release_receipt_replay

use forge::engine::completion::{
    apply_completion_unit, CompletionLedger, CompletionRecord, MemoryLedger,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::receipt::{replay_outcome, CommandReceipt, ReceiptOutcome};
use forge::engine::runtime::completion_receipt_id;

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-008";
/// The task whose completion receipt the replay is about. The receipt id is derived from it, so the id under
/// test is the one production derives rather than one written here.
const TASK_ID: &str = "task-release-008-publish";

/// A stored receipt row, as the `workflow_command_receipt` table hands it back.
fn receipt(outcome: ReceiptOutcome, message: &str) -> CommandReceipt {
    CommandReceipt {
        command_id: completion_receipt_id(TASK_ID),
        outcome,
        aggregate_id: Some(STORY_ID.into()),
        message: Some(message.into()),
    }
}

/// The completion unit the release ledger applies when a release task completes: the publish facts it merged.
/// The node decides which budget the unit spends; the task id decides which receipt it claims.
fn completion_record(task_id: &str, node_id: &str) -> CompletionRecord {
    CompletionRecord {
        task_id: task_id.into(),
        process_instance_id: "instance-release-008".into(),
        story_id: STORY_ID.into(),
        node_id: Some(node_id.into()),
        evidence: ForgeGateEvidence {
            publish_succeeded: Some(true),
            published_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
            ..ForgeGateEvidence::default()
        },
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-008); the file and the assay use it.
fn forge_release_008__release_receipt_replay() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. THE RECEIPT ID IS DERIVED, NOT INVENTED. The replay is keyed to the id production derives for a
    //    release task, so a test that made up its own id would be proving nothing about the real key.
    // -----------------------------------------------------------------------------------------------------------
    let receipt_id = completion_receipt_id(TASK_ID);
    assert_eq!(
        receipt_id,
        format!("forge.completion:{TASK_ID}"),
        "{HARNESS}: the completion receipt id is the one production derives for this task"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — A FINAL RECEIPT REPLAYS AS ITS OWN OUTCOME. The release task completed, the receipt
    //    says `success`, and a second dispatch asking the same question is answered from that receipt — with
    //    its message, not with a fresh verdict.
    // -----------------------------------------------------------------------------------------------------------
    let success = receipt(ReceiptOutcome::Success, "published 0123456789ab");
    let replayed = replay_outcome(Some(&success));
    assert_eq!(
        replayed.outcome,
        ReceiptOutcome::Success,
        "{HARNESS}: a finalized success receipt replays as success"
    );
    assert_eq!(
        replayed.message.as_deref(),
        Some("published 0123456789ab"),
        "{HARNESS}: and it replays the receipt's own message, so the reader is told what happened"
    );

    // Every final outcome replays ITSELF — a refusal is never softened into a success by a replay.
    for outcome in [
        ReceiptOutcome::ValidationFailure,
        ReceiptOutcome::NotFound,
        ReceiptOutcome::Conflict,
        ReceiptOutcome::Unauthorized,
        ReceiptOutcome::PreconditionFailure,
    ] {
        let stored = receipt(outcome.clone(), "the first run's reason");
        let again = replay_outcome(Some(&stored));
        assert_eq!(
            again.outcome, outcome,
            "{HARNESS}: {outcome:?} replays as itself, not as a success"
        );
        assert_eq!(
            again.message.as_deref(),
            Some("the first run's reason"),
            "{HARNESS}: and its message comes from the receipt, not from this call"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — AN IN-FLIGHT RECEIPT IS A RETRYABLE CONFLICT, NEVER A SUCCESS. `pending` is the claim
    //    sentinel: another process is mid-flight on this command, so the honest answer is "not yet", and the
    //    caller retries. Reading it as a terminal outcome would report a release that may never happen.
    // -----------------------------------------------------------------------------------------------------------
    let in_flight = receipt(ReceiptOutcome::Pending, "claimed");
    let held = replay_outcome(Some(&in_flight));
    assert_eq!(
        held.outcome,
        ReceiptOutcome::Conflict,
        "{HARNESS}: a pending claim is not a terminal outcome and replays as a conflict"
    );
    assert!(
        !matches!(held.outcome, ReceiptOutcome::Success),
        "{HARNESS}: and it is never a success"
    );
    assert_eq!(
        held.message.as_deref(),
        Some("Command claim is in-flight (pending receipt); retry later."),
        "{HARNESS}: the refusal says the claim is in flight and to retry later"
    );
    // A pending receipt carrying a SUCCESSFUL message is still in flight. The message is a note; the outcome
    // is the answer, and a boundary that read the note would call an unfinished release finished.
    let in_flight_but_optimistic = CommandReceipt {
        command_id: receipt_id.clone(),
        outcome: ReceiptOutcome::Pending,
        aggregate_id: Some(STORY_ID.into()),
        message: Some("published 0123456789ab".into()),
    };
    assert_eq!(
        replay_outcome(Some(&in_flight_but_optimistic)).outcome,
        ReceiptOutcome::Conflict,
        "{HARNESS}: an optimistic message on a pending receipt does not make it a success"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — A MISSING RECEIPT IS ALSO A RETRYABLE CONFLICT, never a success. This is the crash window
    //    the receipt exists to describe: the transition is durable and the evidence is not.
    // -----------------------------------------------------------------------------------------------------------
    let missing = replay_outcome(None);
    assert_eq!(
        missing.outcome,
        ReceiptOutcome::Conflict,
        "{HARNESS}: a command with no receipt replays as a conflict"
    );
    assert!(
        missing
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("in-flight"),
        "{HARNESS}: and the message names the window it is in"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. FAIL CLOSED — AN OUTCOME STRING THE PARSER DOES NOT NAME IS A CONFLICT. A row written by an older
    //    build, or corrupted, must not be read as permission; `parse` refuses to guess.
    // -----------------------------------------------------------------------------------------------------------
    for stored in [
        "",
        "SUCCESS",
        "succeeded",
        "done",
        "ok",
        "true",
        "COMPLETED",
    ] {
        let parsed = ReceiptOutcome::parse(stored);
        assert_eq!(
            parsed,
            ReceiptOutcome::Conflict,
            "{HARNESS}: {stored:?} is not an outcome this engine names, so it reads as a conflict"
        );
        assert_ne!(
            parsed.as_str(),
            "success",
            "{HARNESS}: and it must never decode to success"
        );
    }
    // Every named outcome survives the round trip — which is what makes the conflict above the only refusal.
    for outcome in [
        ReceiptOutcome::Success,
        ReceiptOutcome::ValidationFailure,
        ReceiptOutcome::NotFound,
        ReceiptOutcome::Conflict,
        ReceiptOutcome::Unauthorized,
        ReceiptOutcome::PreconditionFailure,
        ReceiptOutcome::Pending,
    ] {
        assert_eq!(
            ReceiptOutcome::parse(outcome.as_str()),
            outcome,
            "{HARNESS}: {} must survive the store/read round trip",
            outcome.as_str()
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE COMPLETION UNIT RUNS EXACTLY ONCE. The durable ledger claims the receipt BEFORE the writes, so a
    //    replay is answered by "someone already applied this unit" rather than by applying it a second time.
    //    The counters are the witness: a second application would double the repair budget.
    // -----------------------------------------------------------------------------------------------------------
    // The unit used here is a repair turn, because the repair counter is the one a double application would
    // be visible in — a release node spends no budget, so a replay of one would look identical either way.
    const REPAIR_TASK: &str = "task-release-008-repair-smith";
    let ledger = MemoryLedger::new();
    assert!(
        apply_completion_unit(&ledger, completion_record(REPAIR_TASK, "repair_smith"))
            .expect("the first application is owned by this caller"),
        "{HARNESS}: the first caller owns the unit"
    );
    assert!(
        ledger
            .has_final(&completion_receipt_id(REPAIR_TASK))
            .expect("the ledger answers"),
        "{HARNESS}: and the receipt is finalized once it is applied"
    );
    assert_eq!(
        ledger.repairs(STORY_ID),
        1,
        "{HARNESS}: exactly one repair attempt was spent"
    );
    let merged = ledger
        .evidence_for(STORY_ID)
        .expect("the unit merged its evidence");
    assert_eq!(
        merged.published_sha.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567"),
        "{HARNESS}: and the release facts the unit carried were merged"
    );

    // THE REPLAY. A second dispatch of the same task finds the receipt finalized and applies nothing.
    assert!(
        !apply_completion_unit(&ledger, completion_record(REPAIR_TASK, "repair_smith"))
            .expect("a replay is answered, not failed"),
        "{HARNESS}: a replayed unit is NOT applied a second time"
    );
    assert_eq!(
        ledger.repairs(STORY_ID),
        1,
        "{HARNESS}: the repair budget is still exactly one — a replay spends nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. THE COUNTERS ARE PER NODE, AND ONLY WHERE THE DEFINITION SPENDS A BUDGET. A release node applies the
    //    unit without spending a repair budget; `repair_smith` and `repair_architect` each spend their own, and
    //    the FAST repair lane shares the ONE repair budget with the full one — they are the same door.
    // -----------------------------------------------------------------------------------------------------------
    let per_node = MemoryLedger::new();
    for (index, node) in [
        "publish_candidate",
        "deploy",
        "production_smoke",
        "complete",
    ]
    .iter()
    .enumerate()
    {
        assert!(
            apply_completion_unit(
                &per_node,
                completion_record(&format!("task-release-node-{index}"), node)
            )
            .expect("each release node applies its unit"),
            "{HARNESS}: {node} applies its completion unit"
        );
    }
    assert_eq!(
        per_node.repairs(STORY_ID),
        0,
        "{HARNESS}: a release node spends no repair budget — only the repair lanes do"
    );
    assert!(
        per_node.evidence_for(STORY_ID).is_some(),
        "{HARNESS}: but it still merged its evidence"
    );

    let repair = MemoryLedger::new();
    assert!(
        apply_completion_unit(
            &repair,
            completion_record("task-repair-smith", "repair_smith")
        )
        .expect("applied"),
        "{HARNESS}: a repair Smith turn applies its unit and spends the repair budget"
    );
    assert!(
        apply_completion_unit(
            &repair,
            completion_record("task-fast-repair-smith", "fast_repair_smith")
        )
        .expect("a different task is a different unit"),
        "{HARNESS}: the FAST repair lane is its own task, so it is its own unit"
    );
    assert_eq!(
        repair.repairs(STORY_ID),
        2,
        "{HARNESS}: and both repair lanes spend the SAME repair budget, because they are the same door"
    );
    let replan = MemoryLedger::new();
    assert!(
        apply_completion_unit(
            &replan,
            completion_record("task-repair-architect", "repair_architect")
        )
        .expect("applied"),
        "{HARNESS}: a repair Architect turn applies its unit"
    );
    assert_eq!(
        replan.replans(STORY_ID),
        1,
        "{HARNESS}: and spends the REPLAN budget, which is not the repair budget"
    );
    assert_eq!(
        replan.repairs(STORY_ID),
        0,
        "{HARNESS}: a replan spends no repair budget"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. THE LEDGER'S CLAIM SET IS WHAT MAKES THE REPLAY ANSWERABLE. `claim` is the first thing the unit does
    //    and `finalize` the last, so a crash between them leaves a claimed-but-unfinal receipt — which is
    //    exactly the in-flight case section 2 refuses.
    // -----------------------------------------------------------------------------------------------------------
    let crashed = MemoryLedger::new();
    assert!(crashed.claim(&receipt_id).expect("claimed"));
    assert!(
        !crashed.has_final(&receipt_id).expect("answered"),
        "{HARNESS}: a claimed receipt is not a finalized one — the crash window is real"
    );
    // And a claim already held is not taken twice.
    assert!(
        !crashed
            .claim(&receipt_id)
            .expect("a second claim is answered, not failed"),
        "{HARNESS}: a receipt already claimed is not claimed again"
    );
    assert!(
        crashed.finalize(&receipt_id).is_ok(),
        "{HARNESS}: and the holder can still finalize it"
    );
    assert!(
        crashed.has_final(&receipt_id).expect("answered"),
        "{HARNESS}: which turns the in-flight receipt into a replayable one"
    );

    // Once finalized, the replay rule reads the stored outcome rather than the in-flight conflict — the same
    // receipt id moving from "not yet" to "here is what happened", never from "not yet" to "success by
    // default".
    let before = replay_outcome(Some(&CommandReceipt {
        command_id: receipt_id.clone(),
        outcome: ReceiptOutcome::Pending,
        aggregate_id: None,
        message: None,
    }));
    assert_eq!(
        before.outcome,
        ReceiptOutcome::Conflict,
        "{HARNESS}: while the claim is in flight"
    );
    let after = replay_outcome(Some(&CommandReceipt {
        command_id: receipt_id.clone(),
        outcome: ReceiptOutcome::Success,
        aggregate_id: Some(STORY_ID.into()),
        message: Some("published".into()),
    }));
    assert_eq!(
        after.outcome,
        ReceiptOutcome::Success,
        "{HARNESS}: once the receipt is finalized"
    );

    // A failure to ASK is an `Err`, never a `false`: reporting a lost connection as "someone else applied this
    // unit" would silently drop the evidence merge and the counters. So the retryable answer has one name, and
    // a caller cannot mistake it for a verdict.
    assert_eq!(
        ReceiptOutcome::Conflict.as_str(),
        "conflict",
        "{HARNESS}: the retryable answer has one name"
    );
}
