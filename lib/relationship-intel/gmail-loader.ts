// ---------------------------------------------------------------------------
// REL-INTEL — bounded Gmail census load orchestrator.
//
// Composes the pure parser with the ODS seam and the reconciliation engine:
//   artifact CSV -> parse -> intake batch -> neutral evidence -> reconcile.
//
// This is a BOUNDED batch, never a full-mailbox census. Coverage bounds and
// incompleteness live in the batch/coverage metadata. No message bodies,
// snippets, or attachments are ever accepted or stored.
// ---------------------------------------------------------------------------

import type { QueryExecutor } from '../../db/query-executor'
import {
  createIntakeBatch,
  recordReconcileDecision,
  upsertRelationshipEvidence,
} from '../../db/relationship-evidence'
import type { ReviewState } from './contracts'
import { createDbPersonLookup } from './db-lookup'
import {
  parseGmailCensus,
  type GmailBatchAccounting,
} from './gmail-census'
import { fingerprint } from './normalize'
import { reconcileEvidence } from './reconcile'

const GMAIL_SOURCE = 'gmail_contacts'

export interface GmailLoadOptions {
  csv: string
  sourceAccount: string
  externalBatchId: string
  schemaVersion?: number
  exportedAt?: string | null
  /** Run the deterministic reconciliation pass after loading. */
  reconcile?: boolean
}

export interface GmailLoadReport {
  batchId: string
  balance: GmailBatchAccounting
  reconciliation: Record<ReviewState, number>
  canonicalLinked: number
}

export async function loadGmailCensus(
  opts: GmailLoadOptions,
  execute?: QueryExecutor,
): Promise<GmailLoadReport> {
  const balance = parseGmailCensus(opts.csv)
  const checksum = fingerprint(opts.csv)

  const batchId = await createIntakeBatch(
    {
      source: GMAIL_SOURCE,
      sourceAccount: opts.sourceAccount,
      externalBatchId: opts.externalBatchId,
      schemaVersion: opts.schemaVersion ?? 1,
      exportedAt: opts.exportedAt ?? null,
      fileSha256: checksum,
      // balance: declared = valid + error; valid = new + replay + changed.
      inputCount: balance.declared,
      validCount: balance.accepted,
      newProfileCount: balance.accepted,
      replayCount: 0,
      changedRevisionCount: 0,
      errorCount: balance.rejected + balance.quarantined,
    },
    execute,
  )

  const lookup = createDbPersonLookup(execute)
  const reconciliation: Record<ReviewState, number> = {
    unresolved: 0,
    exact_linked: 0,
    review_required: 0,
    ambiguous: 0,
    unmatched: 0,
    rejected: 0,
    non_person: 0,
    deferred: 0,
  }
  let canonicalLinked = 0

  for (const row of balance.rows) {
    const id = await upsertRelationshipEvidence(
      row.evidence,
      row.fingerprint,
      batchId,
      execute,
    )
    if (!opts.reconcile) continue
    const decision = await reconcileEvidence(row.evidence, lookup)
    await recordReconcileDecision(id, decision, execute)
    reconciliation[decision.reviewState] += 1
    if (decision.reviewState === 'exact_linked' && decision.canonicalPersonId) {
      canonicalLinked += 1
    }
  }

  return { batchId, balance, reconciliation, canonicalLinked }
}
