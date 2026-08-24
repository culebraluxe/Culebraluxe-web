import type { QueryExecutor } from './query-executor'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// CRM-27 — Agreement Execution marker + neutral evidence reader.
//
// The marker makes AGREEMENT_FULLY_EXECUTED exactly-once per immutable issued
// document version (unique(document_id, issued_version) + INSERT ... ON
// CONFLICT DO NOTHING). The evidence reader gathers NEUTRAL role-level
// completion evidence from completed signature requests — never provider
// status strings.
// ---------------------------------------------------------------------------

export type AgreementExecutionRow = {
  document_id: string
  issued_version: number
  event_id: string
  emitted_at: string
}

/**
 * Record that an immutable issued document version was judged fully executed.
 * Returns `recorded: true` ONLY when THIS call inserted the marker (i.e. the
 * caller should emit AGREEMENT_FULLY_EXECUTED); `false` on a duplicate replay
 * (already recorded) — the database unique constraint is the backstop.
 */
export async function claimAgreementExecution(
  input: {
    documentId: string
    issuedVersion: number
    eventId: string
    emittedAt: Date
  },
  run: TxRunner = neonTx,
): Promise<{ recorded: boolean }> {
  return run(async (tx) => {
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
  })
}

/**
 * Neutral execution evidence for a document: the distinct execution_roles among
 * COMPLETED signature requests for that document. Only role-labelled requests
 * contribute; requests with a NULL execution_role carry no role evidence.
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
