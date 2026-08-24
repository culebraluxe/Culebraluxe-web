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

import { AGREEMENT_EXECUTION_CLAIM } from '../commands/command-types'
import type { CommandDispatcher } from '../commands/contracts'
import type { AgreementCompletionResult } from './completion'

export type AgreementReDriveDeps = {
  /** The canonical command seam (all command effects commit through it). */
  dispatcher: CommandDispatcher
}

const NOT_FULLY_EXECUTED: AgreementCompletionResult = {
  verdict: { fullyExecuted: false, missingRoles: [], reason: 'missing_required_roles' },
  shouldEmit: false,
  document: null,
  templateId: null,
  dealId: null,
}

/** Re-drive agreement-completion evaluation through the durable command path. */
export async function evaluateAgreementViaCommand(
  deps: AgreementReDriveDeps,
  transactionDocumentId: string,
  eventId: string,
): Promise<AgreementCompletionResult> {
  const result = await deps.dispatcher.execute({
    commandId: eventId,
    commandType: AGREEMENT_EXECUTION_CLAIM,
    actorAppUserId: null,
    aggregateType: 'transaction_document',
    aggregateId: transactionDocumentId,
    correlationId: null,
    causationId: eventId,
    requestedAt: new Date().toISOString(),
    input: { transactionDocumentId },
  })
  const completion = (result.value as { completion?: AgreementCompletionResult } | undefined)
    ?.completion
  return completion ?? NOT_FULLY_EXECUTED
}
