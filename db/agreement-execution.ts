import type { QueryExecutor } from './query-executor'

// ---------------------------------------------------------------------------
// CRM-27 — Agreement Execution marker + neutral evidence reader.
//
// The marker makes AGREEMENT_FULLY_EXECUTED exactly-once per immutable issued
// document version (unique(document_id, issued_version) + INSERT ... ON
// CONFLICT DO NOTHING). The evidence reader gathers NEUTRAL role-level
// completion evidence from completed signature requests — never provider
// status strings.
//
// DURABILITY REPAIR (CRM-27): `claimAgreementExecution` writes the marker
// through the SUPPLIED interactive transaction (`tx`), so the marker commits
// atomically with the canonical command receipt and the AGREEMENT_FULLY_EXECUTED
// outbox row. It never opens a nested independent `neonTx` — a rolled-back
// command transaction leaves neither marker nor event.
// ---------------------------------------------------------------------------

export type AgreementExecutionRow = {
  document_id: string
  issued_version: number
  event_id: string
  emitted_at: string
}

/**
 * Record that an immutable issued document version was judged fully executed.
 * Runs in the CALLER's interactive transaction (`tx`) — the same one that owns
 * the command receipt and the outbox append — so a single commit covers all of
 * them and a rollback leaves none. Returns `recorded: true` ONLY when THIS call
 * inserted the marker (i.e. the caller should emit AGREEMENT_FULLY_EXECUTED);
 * `false` on a duplicate replay (already recorded) — the database unique
 * constraint is the exactly-once backstop.
 */
export async function claimAgreementExecution(
  tx: QueryExecutor,
  input: {
    documentId: string
    issuedVersion: number
    eventId: string
    emittedAt: Date
  },
): Promise<{ recorded: boolean }> {
  const rows = await tx`
    insert into agreement_execution (document_id, issued_version, event_id, emitted_at)
    values (
      ${input.documentId}, ${input.issuedVersion}, ${input.eventId},
      ${input.emittedAt.toISOString()}
    )
    on conflict (document_id, issued_version) do nothing
    returning id
  `
  return { recorded: rows.length > 0 }
}

/**
 * Neutral execution evidence for a document: the distinct execution_roles among
 * COMPLETED signature requests for that document. Only role-labelled requests
 * contribute; requests with a NULL execution_role carry no role evidence.
 *
 * HONEST LIMITATION — CRM-27 participant cardinality (architect stop condition
 * #1 / continuity #5): this reader returns ROLE NAMES, so it cannot distinguish
 * multiple people sharing one role (two Buyers collapse to a single 'BUYER')
 * and cannot prove WHICH actual participant completed a role. It also keys on
 * transaction_document_id alone — not (document, issued_version) — because
 * signature_request carries no issued_version. The smallest provider-neutral
 * schema seam to close both gaps is an issued_version + recipient-identity key
 * on signature_request populated at send (the forms participant collection
 * document_form_participant already provides the required participant set).
 * That seam is deliberately NOT invented here to keep this repair bounded;
 * until it exists, evidence is role-scoped and the predicate is evaluated as
 * such.
 */
export async function getCompletedExecutionRoles(
  documentId: string,
  execute: QueryExecutor,
): Promise<string[]> {
  const rows = await execute`
    select distinct execution_role
    from signature_request
    where transaction_document_id = ${documentId}
      and status = 'completed'
      and execution_role is not null
  `
  return rows
    .map((row) => (typeof row.execution_role === 'string' ? row.execution_role : ''))
    .filter((role) => role.length > 0)
}
