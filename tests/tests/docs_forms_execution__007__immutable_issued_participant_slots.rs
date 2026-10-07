//! DOCS.FORMS.EXECUTION — immutable issued participant slots (TST-DOCS-FORMS-EXECUTION-007).
//!
//! Contract: once a form is issued, WHO signs it is frozen. The slot each participant occupies is built
//! once, by the one canonicalization boundary (`model::forms_execution::canonicalize_execution_participants`
//! / `build_issued_execution_slots`, `middle/model/src/forms_execution.rs`): deterministic order, numbered
//! per role from one as `ROLE:sequence`, duplicates of a strong identity removed within their role only —
//! so the same input can never produce two different slot maps. Issuance then freezes that map into the
//! issued document's `source_snapshot.issuedParticipants` (`db/src/vault/bind_form_to_contract.rs`), the
//! envelope is told slot ids rather than people (`execution_slot_id` in
//! `web/src/api/portal_bridge/forms_write_actions.rs`), and an issued form's context is locked
//! (`FORM_CLIENT_BIND_LOCKED`): the participants of an issued document can never be re-selected,
//! re-numbered, or re-pointed.
//!
//! Specified level: L3 Composition / DocumentVaultHarness. The reachable deterministic seam is the one
//! TST-DOCS-CONTRACT-005 (Complete) established for this taxonomy: the production canonicalization functions
//! plus source-structure assertions over the issuance/send boundaries; no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_execution__007__immutable_issued_participant_slots

use model::forms_execution::{
    build_issued_execution_slots, canonicalize_execution_participants, ExecutionParticipantInput,
    PR_PNS_REQUIRED_ROLES,
};
use test_harness::source;

fn person(
    role: &str,
    person_id: Option<&str>,
    name: &str,
    email: Option<&str>,
) -> ExecutionParticipantInput {
    ExecutionParticipantInput {
        role: role.to_string(),
        person_id: person_id.map(str::to_string),
        name: name.to_string(),
        email: email.map(str::to_string),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-EXECUTION-007); the file and the assay use it.
fn docs_forms_execution_007__immutable_issued_participant_slots() {
    let root = source::workspace_root();

    // 1. ONE SLOT MAP, BUILT ONCE: slots are `ROLE:sequence` numbered from one, and the SAME input always
    //    produces the SAME map — an issued document's participants can never drift between reads.
    let input = vec![
        person("SELLER", Some("s-2"), "Beto", None),
        person("SELLER_BROKER", Some("b-1"), "Lisa Penfield", None),
        person("SELLER", Some("s-1"), "Ana", None),
    ];
    let first = canonicalize_execution_participants(&input);
    let second = canonicalize_execution_participants(&input);
    assert_eq!(
        first, second,
        "the slot map is deterministic — it cannot drift"
    );
    assert_eq!(
        first
            .iter()
            .map(|slot| slot.slot_id.as_str())
            .collect::<Vec<_>>(),
        vec!["SELLER:1", "SELLER:2", "SELLER_BROKER:1"],
        "each role's slots number from one, in canonical order"
    );
    // The brokerage's required roles are exactly the PR-PNS set, and only they are required.
    assert_eq!(PR_PNS_REQUIRED_ROLES, ["BUYER", "SELLER", "SELLER_BROKER"]);
    let slots = build_issued_execution_slots(&[
        person("BUYER", None, "Chris", None),
        person("OTHER", None, "Notary", None),
    ]);
    assert!(slots[0].required && !slots[1].required);

    // 2. NEGATIVE (executed): a duplicated strong identity can never occupy two slots of one role — the
    //    same person is merged, so the issued map never double-signs a line.
    let merged = canonicalize_execution_participants(&[
        person("SELLER", Some("s-1"), "Ana", None),
        person("SELLER", Some("s-1"), "Ana Duplicate", None),
    ]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].slot_id, "SELLER:1");
    // …and a row with NO strong identity is kept rather than silently merged away: two unnamed rows stay
    // two slots, so the issued map never drops a signer.
    let kept = canonicalize_execution_participants(&[
        person("SELLER", None, "Ana", None),
        person("SELLER", None, "Ana", None),
    ]);
    assert_eq!(kept.len(), 2);

    // 3. THE SLOT'S DURABLE SHAPE: the slot id is the field the issued snapshot and the envelope read, in
    //    its camelCase wire form — a slot renamed anywhere breaks the evidence, and this is what catches it.
    let slot = &first[0];
    let wire = serde_json::to_value(slot).expect("the slot serializes");
    assert!(
        wire.get("slotId").is_some(),
        "the wire shape names slotId, the field the snapshot reads"
    );

    // 4. FROZEN AT ISSUANCE (source structure): the issued document's snapshot records the participants
    //    themselves, inside the one issuance transaction — the slot map is written once, with the document,
    //    and no later read recomputes it.
    let issue = source::read(&root.join("db/src/vault/bind_form_to_contract.rs"));
    assert!(
        issue.contains("\"issuedParticipants\": participants"),
        "issuance freezes the participant slots into the source snapshot"
    );
    assert!(
        issue.contains("list_signers_on(tx.connection(), &form.id).await?"),
        "the frozen map is the one resolved inside the issuance transaction"
    );
    // The snapshot is bound to the document row at insert time — the same statement, never a later patch.
    let insert = issue
        .find("insert into transaction_document (")
        .expect("the document insert");
    let snapshot_bind = issue
        .find(".bind(source_snapshot.clone())")
        .expect("the snapshot is bound");
    let mark_form = issue
        .find("set status = 'issued'")
        .expect("the form is marked issued");
    assert!(insert < snapshot_bind && snapshot_bind < mark_form);

    // 5. THE ENVELOPE IS TOLD SLOTS, NOT PEOPLE (source structure): the send-for-signature path carries
    //    each signer's immutable slot id into the recipient, so the provider's signature lands on the slot
    //    the issued snapshot already froze.
    let actions = source::read(&root.join("web/src/api/portal_bridge/forms_write_actions.rs"));
    assert!(
        actions.contains("let execution_slot_id = signer.slot_id.clone();")
            && actions.contains("execution_slot_id,"),
        "the recipient carries the immutable execution slot"
    );
    assert!(
        actions.contains(
            "let execution_role = execution_slot_id.as_ref().map(|_| signer.role.clone());"
        ),
        "the execution role travels only with a slot"
    );

    // 6. NEGATIVE (source structure): an issued form's context is LOCKED — rebinding its client is a
    //    conflict, so the participants behind the frozen slots can never be re-pointed after issuance.
    assert!(
        actions.contains("FormInstanceStatus::Issued")
            && actions.contains("FORM_CLIENT_BIND_LOCKED"),
        "an issued form refuses a context change"
    );
}
