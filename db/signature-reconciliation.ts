import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import type { AuditTrailDownload, SignedArtifactDownload } from '../lib/signature/contracts'
import type { QueryExecutor } from './query-executor'
import type { TxRunner } from './tx'
import { getSignatureRequest } from './signature-request'
import {
  getTransactionDocument,
  transitionTransactionDocumentState,
} from './transaction-document'
import {
  claimReceipt,
  finalizeReceipt,
  readFinalReceipt,
  replayOutcome,
} from './workflow-command-receipt'

// ---------------------------------------------------------------------------
// DOC-05 — Signed Document Reconciliation.
//
// A completed signature request (neutral signature.request.completed event)
// is reconciled into the CANONICAL transaction_document state: a NEW media row
// holding the signed artifact bytes (downloaded ONCE via the DOC-04 adapter) is
// appended — the draft/original media row is never mutated — and the document
// is moved to 'signed' through DOC-01's transition (state sent -> signed) with
// signed_media_id / signed_at set (updated_at = now() is the version bump).
//
// Boundaries (architect brief):
//   - This is a DOMAIN service (db/*). It uses DOC-01 transitions, NEVER writes
//     provider state, and touches provider tables ONLY through the injected
//     download (which the application layer implements via DOC-04). The
//     provider call never happens inside a transaction (external side effect —
//     it runs BEFORE the claim transaction; the same "after commit" discipline
//     as lib/signature/application.ts).
//   - transaction_document gains ONLY signed_media_id / signed_at (already its
//     DOC-01 lineage columns). No provider/signature state is ever written
//     there; the canonical signature_request is only READ.
//   - Provider-webhook dedupe happens at DOC-04 (unique provider event id in
//     bold_sign_webhook_event) BEFORE any neutral event exists; this service
//     subscribes to NEUTRAL events only.
//
// Idempotency (the story's model):
//   1. CLAIM: a claim-first command receipt keyed by the NEUTRAL EVENT ID
//      (`signature.reconcile:<eventId>`) means a replayed event reads the
//      winner's stored receipt and returns replayed:true — no re-download, no
//      re-append, no double transition.
//   2. RESOLVE: the transaction_document is loaded via
//      signature_request.transaction_document_id (never from the event
//      payload), and 'completed' is treated as TERMINAL regardless of
//      intermediate statuses (out-of-order tolerant).
//   3. ALREADY-SIGNED GUARD: if signed_media_id is already set (a duplicate
//      completion event with a different event id, or a partially-observed
//      prior reconciliation), the event is treated as replayed — no duplicate
//      signed media row, no double transition. THIS is the idempotent-retry
//      detection the partial-failure risk requires.
//   4. ATOMIC APPEND: the media insert + the sent->signed transition + the
//      receipt finalize commit in ONE transaction. A failure between the media
//      create and the transition rolls the whole transaction back (no stray
//      media row), so a retry of the same event re-appends cleanly exactly
//      once. The download failure path (before the transaction) keeps the
//      document in its current state ('sent') for a later retry.
// ---------------------------------------------------------------------------

export const SIGNATURE_RECONCILE_COMMAND_PREFIX = 'signature.reconcile:'

/**
 * Document states from which a completed signature request may be recorded.
 * draft/ready/sent are the full pre-signed chain: a completion that arrives
 * before the document-level send was recorded is still reconciled (out-of-order
 * tolerance) by advancing the document through the legal chain to 'sent' first,
 * then 'sent' -> 'signed'. voided/superseded are NOT signable.
 */
export const SIGNABLE_DOCUMENT_STATES = ['draft', 'ready', 'sent'] as const

export type ReconcileCompletedSignatureRequestInput = {
  /** The NEUTRAL event id (DomainEvent.eventId) — the command receipt key. */
  eventId: string
  /** The neutral signature request the completed event concerns. */
  signatureRequestId: string
  /** Event time (informational; the authoritative signed_at is application now). */
  occurredAt?: string | null
}

export type ReconcileCompletedSignatureRequestDeps = {
  /** The transaction runner (Neon interactive tx in production). */
  run: TxRunner
  /** Executor for pre-claim reads (default: lazy Neon client). */
  execute?: QueryExecutor
  /**
   * One-time signed-artifact download via DOC-04. Injected — this domain
   * service never touches provider tables or the provider directly.
   */
  downloadSignedArtifact: (signatureRequestId: string) => Promise<SignedArtifactDownload>
  /** Completed-envelope audit trail downloaded beside the signed PDF. */
  downloadAuditTrail?: (signatureRequestId: string) => Promise<AuditTrailDownload>
  /** Application clock (injectable for deterministic tests). */
  now?: () => Date
}

export type SignatureReconciliationValue = {
  /** True when the completion was a no-op (receipt replay or already signed). */
  replayed: boolean
  /** The transaction document the signed outcome was recorded on. */
  documentId: string
  /** The NEW signed media row id (absent on replay/no-op). */
  mediaId?: string | null
  auditMediaId?: string | null
  /** signed_at recorded by the sent -> signed transition. */
  signedAt?: string | null
  /** The neutral signature request that completed. */
  signatureRequestId: string
}

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/**
 * The legal path from a signable document state to 'sent' (the pre-signed
 * chain), per DOC-01's transition model:
 *   draft -> [draft->ready, ready->sent]; ready -> [ready->sent]; sent -> [].
 * Every step is a receipt-backed compare-and-set transition.
 */
function pathToSent(state: string): Array<[string, string]> {
  if (state === 'sent') return []
  if (state === 'ready') return [['ready', 'sent']]
  if (state === 'draft') {
    return [
      ['draft', 'ready'],
      ['ready', 'sent'],
    ]
  }
  throw new Error(`No legal path to 'sent' from document state '${state}'.`)
}

/**
 * Append the signed artifact as a NEW media row (media_type 'document'). The
 * draft/original media row is never touched — signed-artifact lineage is
 * append-only (DOC-01).
 */
async function insertSignedMediaRow(
  download: SignedArtifactDownload,
  tx: QueryExecutor,
): Promise<string> {
  const rows = await tx`
    insert into media (file_data, filename, mime_type, file_size, media_type)
    values (
      ${download.bytes}, ${download.filename}, ${download.mimeType},
      ${download.bytes.length}, 'document'
    )
    returning id
  `
  const row = rows[0] as { id?: unknown } | undefined
  if (!row?.id) {
    throw new Error('Signed-artifact media insert returned no row.')
  }
  return String(row.id)
}

/**
 * Reconcile a NEUTRAL signature.request.completed event into canonical
 * transaction_document state. Idempotent at two levels:
 *   1. claim-first receipt keyed by the neutral event id (replays are no-ops);
 *   2. the already-signed guard (signed_media_id set) treats a duplicate
 *      completion as replayed — never a second signed media row.
 *
 * Returns a CommandResult with value:
 *   { replayed: false, documentId, mediaId, signedAt }   — appended + signed
 *   { replayed: true, documentId, mediaId?, signedAt? }  — replay / no-op
 */
export async function reconcileCompletedSignatureRequest(
  input: ReconcileCompletedSignatureRequestInput,
  deps: ReconcileCompletedSignatureRequestDeps,
): Promise<CommandResult> {
  const commandId = `${SIGNATURE_RECONCILE_COMMAND_PREFIX}${input.eventId}`
  const read = deps.execute ?? (await executor())

  // ---- Pre-claim fast paths (avoid the one-time download on replays) -------
  // A TERMINAL receipt for this event id means this exact neutral event was
  // already reconciled (or definitively rejected): replay it. A 'pending'
  // receipt is an in-flight claim by a concurrent attempt — proceed; the
  // claim transaction decides the winner.
  const existingReceipt = await readFinalReceipt(read, commandId)
  if (existingReceipt && existingReceipt.outcome !== 'pending') {
    const replay = replayOutcome(existingReceipt)
    return {
      commandId,
      outcome: replay.outcome,
      emittedEvents: [],
      aggregateId: existingReceipt.aggregateId,
      message: replay.message,
      replayed: true,
      value: {
        replayed: true,
        documentId: existingReceipt.aggregateId ?? input.signatureRequestId,
        signatureRequestId: input.signatureRequestId,
      },
    }
  }

  // Resolve the document BEFORE downloading: when it is already signed (a
  // duplicate completion event with a different event id) or unresolvable,
  // the one-time download is skipped entirely.
  let needsDownload = true
  const request = await getSignatureRequest(input.signatureRequestId, read)
  if (!request) {
    needsDownload = false
  } else {
    const doc = await getTransactionDocument(request.transactionDocumentId, read)
    if (doc?.signedMediaId != null) needsDownload = false
  }

  // ---- One-time download via DOC-04 (external; BEFORE the transaction, so
  // no DB transaction is held open across the provider call). A download
  // failure THROWS: nothing is mutated (the document keeps its current
  // 'sent' state) and the event is retried later. ----
  const [download, auditDownload] = needsDownload
    ? await Promise.all([
        deps.downloadSignedArtifact(input.signatureRequestId),
        deps.downloadAuditTrail?.(input.signatureRequestId) ?? Promise.resolve(null),
      ])
    : [null, null]

  // ---- Authoritative claim transaction: claim + resolve + append + advance
  // + sent->signed + finalize, committed as ONE atomic unit. ----
  return deps.run(async (tx) => {
    const claimed = await claimReceipt(tx, commandId)
    if (!claimed) {
      const stored = await readFinalReceipt(tx, commandId)
      const replay = replayOutcome(stored)
      return {
        commandId,
        outcome: replay.outcome,
        emittedEvents: [],
        aggregateId: stored?.aggregateId ?? null,
        message: replay.message,
        replayed: true,
        value: {
          replayed: true,
          documentId: stored?.aggregateId ?? input.signatureRequestId,
          signatureRequestId: input.signatureRequestId,
        },
      }
    }

    let outcome: CommandOutcome = 'success'
    let aggregateId: string | null = input.signatureRequestId
    let message: string | null = null
    let value: SignatureReconciliationValue | undefined

    // RESOLVE — the transaction_document is loaded via the canonical request
    // row (never from the event payload). 'completed' is terminal regardless
    // of the request's intermediate status history.
    const requestRow = await getSignatureRequest(input.signatureRequestId, tx)
    if (!requestRow) {
      outcome = 'not_found'
      aggregateId = null
      message = 'Signature request not found.'
    } else {
      const doc = await getTransactionDocument(requestRow.transactionDocumentId, tx)
      if (!doc) {
        outcome = 'not_found'
        aggregateId = null
        message = 'Transaction document not found.'
      } else if (doc.signedMediaId != null) {
        // Already reconciled by an earlier completion (duplicate webhook /
        // partial retry): replayed — no append, no transition.
        aggregateId = doc.id
        value = {
          replayed: true,
          documentId: doc.id,
          mediaId: doc.signedMediaId,
          signedAt: doc.signedAt,
          signatureRequestId: input.signatureRequestId,
        }
        message = 'Document already signed; completion treated as replayed.'
      } else if (!(SIGNABLE_DOCUMENT_STATES as readonly string[]).includes(doc.state)) {
        outcome = 'validation_failure'
        aggregateId = doc.id
        message = `Transaction document cannot be signed from state '${doc.state}'.`
      } else {
        // APPEND — the signed bytes become a NEW media row; the draft row is
        // untouched. `download` is non-null here: the pre-claim resolution
        // only skipped it when the document was already signed (handled
        // above) or unresolvable.
        if (!download) {
          outcome = 'conflict'
          message = 'Signed-artifact download was skipped but the document still requires it; retry the event.'
        } else {
          const signedAt = (deps.now?.() ?? new Date()).toISOString()
          const mediaId = await insertSignedMediaRow(download, tx)
          const auditMediaId = auditDownload
            ? await insertSignedMediaRow(auditDownload, tx)
            : null
          aggregateId = doc.id

          if (auditMediaId) {
            await tx`
              update transaction_document
              set signed_audit_media_id = ${auditMediaId}
              where id = ${doc.id}
            `
          }

          // Out-of-order tolerance: a completion that arrives while the
          // document is still draft/ready is recorded by advancing through the
          // legal chain to 'sent' first (each step receipt-backed), then the
          // DOC-01 sent -> signed transition with the signed lineage.
          const txRun: TxRunner = (cb) => cb(tx)
          const advance = pathToSent(doc.state)
          let failed: CommandResult | null = null
          for (const [from, to] of advance) {
            const step = await transitionTransactionDocumentState(
              doc.id,
              { commandId: `${commandId}:doc:${from}->${to}`, to },
              txRun,
            )
            if (step.outcome !== 'success') {
              failed = step
              break
            }
          }
          if (!failed) {
            const signed = await transitionTransactionDocumentState(
              doc.id,
              {
                commandId: `${commandId}:doc:signed`,
                to: 'signed',
                signedMediaId: mediaId,
                signedAt,
              },
              txRun,
            )
            if (signed.outcome !== 'success') failed = signed
          }
          if (failed) {
            // The media row was appended in THIS transaction; a failed
            // transition must never commit a stray signed media row. Throw so
            // the whole transaction rolls back (media + claim + advance steps)
            // and a retry of the same event re-appends cleanly — the
            // already-signed guard and the receipt make it exactly-once.
            throw new Error(
              `Signed-artifact reconciliation transition failed (${failed.outcome}: ${failed.message ?? 'unknown'}).`,
            )
          }
          value = {
            replayed: false,
            documentId: doc.id,
            mediaId,
            auditMediaId,
            signedAt,
            signatureRequestId: input.signatureRequestId,
          }
        }
      }
    }

    await finalizeReceipt(tx, commandId, outcome, aggregateId, message)
    return {
      commandId,
      outcome,
      emittedEvents: [],
      aggregateId,
      message,
      replayed: false,
      value,
    }
  })
}
