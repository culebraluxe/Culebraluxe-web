-- CulebraLuxe Portal
-- CRM-16: record the Documents / Signatures / Transaction Packet outcome on
-- the Story Board (notes-only; no status/completion change)
-- Migration: 055_storyboard_crm16.sql
--
-- CRM-16 ("Documents / Signatures / Transaction Packet") is the parent story;
-- DOC-01 through DOC-05 carry the rollup weight (seed migration 022 stores the
-- parent with rollup = false). All five children were delivered end-to-end in
-- earlier commits, and the parent plus each child are recorded here so the
-- authoritative Story Board reflects the delivered reality:
--
--   - DOC-01 Canonical Transaction Document Model        (commit 33ad8b8, migration 027)
--   - DOC-02 Transaction Packet                          (commit b0fdaec)
--   - DOC-03 Signature Provider Seam                     (commit c4d3939, migration 036)
--   - DOC-04 BoldSign Integration                        (commit e8616e5, migration 037)
--   - DOC-05 Signed Document Reconciliation              (commit a49fca9)
--
-- CRM-21 (closing-document readiness) then consumes the packet plus the
-- signed lineage; workflow_engine stayed generic and untouched throughout.
--
-- Verification (SCOPED policy): the targeted seam suite ran green in a clean
-- HEAD worktree — transaction-document, transaction-packet,
-- closing-document-readiness, signature-seam, boldsign-adapter and
-- signature-reconciliation 92/92, plus the adjacent command-inventory
-- (signature command registration) 10/10; `tsc --noEmit` clean (exit 0).
-- Note: the current working tree additionally carries unrelated in-flight
-- AUTH-05 receipt-actor edits that leave the four receipt-touching seam test
-- fakes mid-update (the fake SELECT matcher is not yet aligned to the new
-- actor_app_user_id column); those tests fail only on that fake matcher, not
-- on story logic — the repair belongs to AUTH-05, not this story. Full
-- regression not run per runtime policy.
--
-- The existing rows have ONLY their notes reconciled — status/completion are
-- never overwritten (the human-owned board stays authoritative for execution
-- control, per the Story Execution Contract). Applied to the DEV branch;
-- promotion to production happens only through an explicit production-release
-- task.

begin;

update storyboard_story
set notes = 'Parent story (rollup = false). All five children delivered end-to-end and recorded: DOC-01 canonical transaction document model (migration 027 + db/transaction-document.ts), DOC-02 transaction packet (workflow_app/transaction-packet.ts, pure derived projection, no packet table), DOC-03 provider-neutral SignatureProvider seam (migration 036 + db/signature-request.ts + lib/signature), DOC-04 BoldSign integration behind that seam (migration 037 + db/bold-sign-request.ts + lib/signature/boldsign), DOC-05 signed-document reconciliation (db/signature-reconciliation.ts + lib/signature/reconciliation.ts). CRM-21 consumes the packet plus signed lineage for closing-document readiness. workflow_engine stayed generic and untouched. Verified (SCOPED policy) in a clean HEAD worktree: the six targeted seam suites 92/92 (transaction-document, transaction-packet, closing-document-readiness, signature-seam, boldsign-adapter, signature-reconciliation) plus adjacent command-inventory 10/10; tsc clean. Status/completion are the human board decision.',
    updated_at = now()
where id = 'CRM-16';

update storyboard_story
set notes = 'Canonical transaction document model delivered (commit 33ad8b8; migration 027; db/transaction-document.ts). transaction_document is the DEAL-scoped canonical record: document_type/state/source are checked structural categories and document_type_label carries the SME long tail (mirrors deal_participant.role_label). File bytes live in the generic media asset store (media_type document): media.id is the draft/current bytes, signed_media_id + signed_at are the signed artifact, set together by CHECK and always a NEW media row — the draft bytes are never mutated. Source idempotency via a partial unique index on (deal_id, source_system, source_external_id) where source_external_id is not null. State transitions reuse the claim-first command-receipt pattern (migration 018): the same commandId executes its effect at most once and losers observe the stored winner result. No legacy property-scoped document migration, no workflow_engine involvement. Targeted tests: workflow_app/tests/transaction-document.test.ts green.',
    updated_at = now()
where id = 'DOC-01';

update storyboard_story
set notes = 'Transaction packet delivered (commit b0fdaec; workflow_app/transaction-packet.ts). A pure DERIVED projection with no packet table: given deal.stage plus the deal/workflow facts it determines which transaction document types are required, compares against the canonical transaction_document rows, and reports per deal present / missing / unresolved. Signature state is irrelevant to presence (any non-terminal document state counts as present), the packet never writes (no auto-creation), workflow_engine is untouched, and every jurisdiction knob is an input fact so the rule catalog stays jurisdiction-neutral. An unresolved gating fact is surfaced as unresolved — never coerced to a boolean and never fabricated into a required document. Long-tail requirements (CRIM clearance, HOA clearance, survey, closing statement, registry follow-up) reuse the other/closing categories plus curated labels matched exactly. Targeted tests: workflow_app/tests/transaction-packet.test.ts green; consumed by closing-document readiness (CRM-21).',
    updated_at = now()
where id = 'DOC-02';

update storyboard_story
set notes = 'Signature Provider Seam delivered (commit c4d3939; migration 036; db/signature-request.ts + lib/signature). signature_request is the canonical PROVIDER-NEUTRAL record of a signing request against a transaction document: the neutral status model requested -> sent -> viewed -> signed -> completed with declined/voided/expired/error sinks; provider ids and provider state never reach this table (a DOC-04 provider table lives behind the seam). lib/signature owns the SignatureProvider interface (send/status/cancel/verifyWebhook), the provider registry, the fake provider, the neutral status mapping, the application router, and the signature.request.send / signature.request.status commands in the canonical command registry. Domain services never call the provider — provider observations arrive as neutral statuses mapped at the seam. Claim-first receipt idempotency exactly like DOC-01. Targeted tests: workflow_app/tests/signature-seam.test.ts green.',
    updated_at = now()
where id = 'DOC-03';

update storyboard_story
set notes = 'BoldSign Integration delivered (commit e8616e5; migration 037; db/bold-sign-request.ts + lib/signature/boldsign). bold_sign_request holds EVERYTHING BoldSign-specific behind the DOC-03 seam: the envelope id, provider document/file ids, the last RAW BoldSign status, and an observable last_error with retryable/non-retryable classification; provider ids never cross to signature_request or transaction_document. Send idempotency: one row per canonical request plus a partial unique index on envelope_id, so a provider envelope is never persisted twice. bold_sign_webhook_event is the webhook replay dedupe (unique provider_event_id) AND the durable enqueue record for the DOC-05 async reconciler; a replayed webhook inserts nothing (ON CONFLICT DO NOTHING). lib/signature/boldsign (adapter/client/config/errors/events/webhook) composes provider calls with these writes; raw statuses map to the neutral model only at the DOC-03 seam (lib/signature/status-mapping.ts). Targeted tests: workflow_app/tests/boldsign-adapter.test.ts green.',
    updated_at = now()
where id = 'DOC-04';

update storyboard_story
set notes = 'Signed Document Reconciliation delivered (commit a49fca9; db/signature-reconciliation.ts + lib/signature/reconciliation.ts). A neutral signature.request.completed event reconciles into canonical transaction_document: the signed artifact is downloaded ONCE via the DOC-04 adapter (an external side effect that runs OUTSIDE the transaction, before the claim) into a NEW media row, then the document moves sent -> signed through the DOC-01 transition with signed_media_id/signed_at set; the draft media row is never mutated. Idempotency: a claim-first receipt keyed by the neutral event id (signature.reconcile:<eventId>) makes a replay return replayed:true with no re-download, no duplicate media and no double transition; provider-webhook dedupe happens at DOC-04 before any neutral event exists; the already-signed guard replays; a partial failure rolls back and the retry reconciles exactly once. The domain service uses DOC-01 transitions and never writes provider/signature state to canonical tables. Targeted tests: workflow_app/tests/signature-reconciliation.test.ts green.',
    updated_at = now()
where id = 'DOC-05';

commit;
