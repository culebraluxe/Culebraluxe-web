import type { CommandDispatcher } from '../commands/contracts'
import type { QueryExecutor } from '../../db/query-executor'
import { evaluateAgreementViaCommand } from './re-drive'

// ---------------------------------------------------------------------------
// CRM-27 — Durable agreement-execution recovery pass (BLOCKER 3).
//
// A real durable reconciliation runtime that can recover a fully-executed
// agreement WITHOUT another provider webhook arriving. It discovers eligible
// completed signature evidence that has NOT yet produced an agreement_execution
// marker and re-drives each through the canonical `agreement.execution.claim`
// command.
//
//   discover:  transaction_document with a completed signature request, a valid
//              issued_version, and NO agreement_execution marker yet;
//   dispatch:  evaluateAgreementViaCommand (deterministic evidence-fingerprint
//              command id + canonical atomic marker/receipt/outbox commit);
//   idempotent: the marker unique(document_id, issued_version) + ON CONFLICT DO
//              NOTHING backstop, plus the fingerprint-keyed command id, make
//              overlapping/re-run passes a no-op (no duplicate business event);
//   survives process/webhook failure: each document re-drive is its own command
//              transaction (the dispatcher), so a failure for one document never
//              rolls back the others, and failures are returned for retry/operator
//              review (never silently swallowed).
//
// No second queue or event store is created — this reuses the canonical command
// + outbox seam. The caller decides the schedule (a cron/worker entry point);
// `runAgreementExecutionRecovery` is the durable, callable, testable pass.
// ---------------------------------------------------------------------------

export type AgreementRecoverySummary = {
  /** Documents whose execution was (re)evaluated this pass. */
  evaluated: number
  /** Documents that became fully executed this pass (marker + event emitted). */
  emitted: number
  /** Documents evaluated but not yet fully executed (or already recorded). */
  notYetExecuted: number
  /** Documents whose re-drive failed — surfaced for retry / operator review. */
  failed: Array<{ documentId: string; outcome?: string; error: string }>
}

export type AgreementRecoveryDeps = {
  dispatcher: CommandDispatcher
  /** Executor for discovery + evidence fingerprint reads. */
  execute?: QueryExecutor
}

/** Discover + re-drive one durable recovery pass. */
export async function runAgreementExecutionRecovery(
  deps: AgreementRecoveryDeps,
): Promise<AgreementRecoverySummary> {
  const q = deps.execute ?? (await defaultExecutor())
  const rows = await q`
    select distinct td.id
    from transaction_document td
    join signature_request sr on sr.transaction_document_id = td.id
    where sr.status = 'completed'
      and td.issued_version is not null
      and td.issued_version >= 1
      and not exists (
        select 1 from agreement_execution ae
        where ae.document_id = td.id and ae.issued_version = td.issued_version
      )
  `

  const summary: AgreementRecoverySummary = {
    evaluated: 0,
    emitted: 0,
    notYetExecuted: 0,
    failed: [],
  }

  for (const row of rows) {
    const documentId = String(row.id)
    summary.evaluated++
    try {
      const completion = await evaluateAgreementViaCommand(
        { dispatcher: deps.dispatcher, execute: q },
        documentId,
        `recovery:${documentId}`,
      )
      if (completion.outcome !== 'success') {
        summary.failed.push({
          documentId,
          outcome: completion.outcome,
          error: completion.error ?? completion.outcome,
        })
      } else if (completion.shouldEmit) {
        summary.emitted++
      } else {
        summary.notYetExecuted++
      }
    } catch (error) {
      // Infrastructure failure — throw propagates out of the command transaction;
      // record it so the operator/retry pass can re-attempt this document.
      summary.failed.push({
        documentId,
        error: String((error as Error)?.message ?? error),
      })
    }
  }

  return summary
}

/** Default executor: the shared application DB handle. */
async function defaultExecutor(): Promise<QueryExecutor> {
  const client = await import('../../db/client')
  return client.sql as unknown as QueryExecutor
}
