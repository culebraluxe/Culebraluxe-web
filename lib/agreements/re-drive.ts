import { createHash } from 'node:crypto'
import { AGREEMENT_EXECUTION_CLAIM } from '../commands/command-types'
import type { CommandDispatcher } from '../commands/contracts'
import { getCompletedExecutionSlots } from '../../db/agreement-execution'
import type { QueryExecutor } from '../../db/query-executor'
import type { AgreementCompletionResult } from './completion'

export type AgreementReDriveDeps = {
  dispatcher: CommandDispatcher
  execute?: QueryExecutor
}

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

export async function evaluateAgreementViaCommand(
  deps: AgreementReDriveDeps,
  transactionDocumentId: string,
  eventId: string,
): Promise<AgreementCompletionResult> {
  const q = deps.execute ?? (await defaultExecutor())
  const evidence = await getCompletedExecutionSlots(transactionDocumentId, q)
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
    return {
      outcome: result.outcome,
      error: result.message ?? `Agreement execution re-drive failed (${result.outcome}).`,
      verdict: { fullyExecuted: false, missingRoles: [], missingSlotIds: [], reason: 'missing_required_roles' },
      shouldEmit: false,
      document: null,
      templateId: null,
      contractId: null,
      dealId: null,
      eventId: null,
    }
  }

  const completion = (result.value as { completion?: AgreementCompletionResult } | undefined)
    ?.completion
  if (!completion) {
    return {
      outcome: 'success',
      error: null,
      verdict: { fullyExecuted: false, missingRoles: [], missingSlotIds: [], reason: 'missing_required_roles' },
      shouldEmit: false,
      document: null,
      templateId: null,
      contractId: null,
      dealId: null,
      eventId: null,
    }
  }
  return completion
}

async function defaultExecutor(): Promise<QueryExecutor> {
  const client = await import('../../db/client')
  return client.sql as unknown as QueryExecutor
}
