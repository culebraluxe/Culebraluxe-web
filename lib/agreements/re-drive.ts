// ---------------------------------------------------------------------------
// CRM-27 — Durable agreement-execution re-drive (Phase 1F).
//
// The reconciliation handler (DOC-05) previously evaluated agreement completion
// as a fragile post-commit callback that could be lost on a transient failure.
// This adapter re-drives that evaluation through the canonical
// `agreement.execution.claim` command, which atomically commits the command
// receipt + agreement_execution marker + AGREEMENT_FULLY_EXECUTED outbox row in
// ONE transaction and is idempotent (dispatcher receipt replay + unique marker).
//
// Using the neutral completion event id as the commandId makes re-evaluating the
// SAME completed event a no-op replay; a later completion for the same document
// is a NEW commandId whose marker already exists, so it emits nothing. Completed
// eligible documents can therefore be reevaluated idempotently by any
// reconciliation/poller pass without waiting for another provider webhook.
// ---------------------------------------------------------------------------

import { createHash } from 'node:crypto'
import { AGREEMENT_EXECUTION_CLAIM } from '../commands/command-types'
import type { CommandDispatcher } from '../commands/contracts'
import { getCompletedExecutionRoles } from '../../db/agreement-execution'
import type { QueryExecutor } from '../../db/query-executor'
import type { AgreementCompletionResult } from './completion'

// ---------------------------------------------------------------------------
// CRM-27 — Durable agreement-execution re-drive (Phase 1F).
//
// Re-drives agreement-completion evaluation through the canonical
// `agreement.execution.claim` command, which atomically commits the command
// receipt + agreement_execution marker + AGREEMENT_FULLY_EXECUTED outbox row in
// ONE transaction and is idempotent.
//
// EVALUATION-ATTEMPT IDENTITY (CRM-27 BLOCKER 2): the source-event id is NOT the
// command id. The command id is DETERMINISTIC on the document + a fingerprint of
// the CURRENT completed execution evidence. This separates source-event
// idempotency from evaluation-attempt identity:
//   - a duplicate/replayed completed event re-drives the SAME evidence -> the
//     SAME command id -> a committed-receipt replay (no re-run, no duplicate);
//   - a LATER evidence change (a new role/slot completes) -> a NEW fingerprint
//     -> a NEW command id -> the dispatcher RE-EVALUATES (not a replay);
//   - once agreement_execution exists, the unique(document_id, issued_version)
//     marker + INSERT ... ON CONFLICT DO NOTHING backstop means every later
//     evaluation is a no-op (no second business event).
//
// ERROR PROPAGATION: the adapter inspects CommandResult.outcome and never
// fabricates a "not fully executed" result for conflicts, infrastructure
// failures, validation failures or not-found outcomes — those are surfaced
// truthfully so the recovery runtime can observe and retry them.
// ---------------------------------------------------------------------------

export type AgreementReDriveDeps = {
  /** The canonical command seam (all command effects commit through it). */
  dispatcher: CommandDispatcher
  /** Executor for the evidence fingerprint read (default: lazy Neon client). */
  execute?: QueryExecutor
}

/**
 * Deterministic command id for an agreement-execution evaluation attempt:
 * `agreement.execution.claim:{documentId}:{evidenceFingerprint}`. When the
 * completed evidence changes, the fingerprint changes, so the attempt identity
 * changes and the dispatcher re-evaluates instead of replaying an old receipt.
 */
export function agreementExecutionClaimCommandId(
  documentId: string,
  evidence: readonly string[],
): string {
  const fingerprint = createHash('sha256')
    .update([...evidence].sort().join('\n'))
    .digest('hex')
    .slice(0, 16)
  return `${AGREEMENT_EXECUTION_CLAIM}:${documentId}:${fingerprint}`
}

/** Re-drive agreement-completion evaluation through the durable command path. */
export async function evaluateAgreementViaCommand(
  deps: AgreementReDriveDeps,
  transactionDocumentId: string,
  eventId: string,
): Promise<AgreementCompletionResult> {
  const q = deps.execute ?? (await defaultExecutor())
  const evidence = await getCompletedExecutionRoles(transactionDocumentId, q)
  const commandId = agreementExecutionClaimCommandId(transactionDocumentId, evidence)

  const result = await deps.dispatcher.execute({
    commandId,
    commandType: AGREEMENT_EXECUTION_CLAIM,
    actorAppUserId: null,
    aggregateType: 'transaction_document',
    aggregateId: transactionDocumentId,
    correlationId: null,
    causationId: eventId,
    requestedAt: new Date().toISOString(),
    input: { transactionDocumentId },
  })

  if (result.outcome !== 'success') {
    // Truthful propagation — never "not fully executed". The recovery runtime
    // observes the real outcome (not_found / validation_failure /
    // precondition_failure / conflict) and can retry or escalate it.
    return {
      outcome: result.outcome,
      error:
        result.message ??
        `Agreement execution re-drive failed (${result.outcome}).`,
      verdict: { fullyExecuted: false, missingRoles: [], reason: 'missing_required_roles' },
      shouldEmit: false,
      document: null,
      templateId: null,
      dealId: null,
      eventId: null,
    }
  }

  const completion = (result.value as { completion?: AgreementCompletionResult } | undefined)
    ?.completion
  if (!completion) {
    // A committed success replay returns no `completion` value: the business fact
    // was already emitted (or the evidence is unchanged and no-op). Report a
    // truthful success no-op — not a fabricated "not fully executed".
    return {
      outcome: 'success',
      error: null,
      verdict: { fullyExecuted: false, missingRoles: [], reason: 'missing_required_roles' },
      shouldEmit: false,
      document: null,
      templateId: null,
      dealId: null,
      eventId: null,
    }
  }
  return completion
}

/** Default executor: the shared application DB handle. */
async function defaultExecutor(): Promise<QueryExecutor> {
  const client = await import('../../db/client')
  return client.sql as unknown as QueryExecutor
}
