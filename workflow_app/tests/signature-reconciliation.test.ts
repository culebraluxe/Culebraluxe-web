import { test } from 'node:test'
import assert from 'node:assert/strict'

// ---------------------------------------------------------------------------
// DOC-05 — Signed Document Reconciliation tests.
//
// Proves the canonical reconciliation service (db/signature-reconciliation.ts)
// + the neutral completed-event subscriber (lib/signature/reconciliation.ts)
// + the router wiring (lib/signature/application.ts) against an in-memory fake
// covering workflow_command_receipt + transaction_document + signature_request
// + media. The fake runner implements ROLLBACK so the partial-failure path
// (media appended, transition failed) is exercised honestly.
//
// Acceptance criteria covered:
//   (1) a completed event appends a NEW signed media row and sets
//       signed_media_id/signed_at with the sent -> signed transition;
//   (2) the original/draft media row is preserved byte-for-byte;
//   (3) a replayed completed event is a no-op (replayed:true, no duplicate
//       signed media, no double transition);
//   (4) webhook duplication (same provider event id twice) reconciles exactly
//       once — at the router (webhook dedupe) AND at the reconciler (already-
//       signed guard for a second neutral event);
//   (5) out-of-order completion (document still draft/ready) resolves;
//   (6) partial failure (media created, transition failed) recovers
//       idempotently — the failed transaction rolls back, the retry appends
//       exactly once;
//   (7) no provider/signature state lands on transaction_document;
//   (8) scoped verification (this file + directly adjacent boldsign adapter
//       download test only).
//
// Scoped per the runtime test policy: this file only — no full regression, no
// persistence harness.
// ---------------------------------------------------------------------------

import { createCommandRegistry } from '../../lib/commands/register'
import { CommandDispatcherImpl } from '../../lib/commands/dispatcher'
import { PostgresCommandReceiptRepository } from '../../db/command-receipt-repository'
import { SignatureApplication } from '../../lib/signature/application'
import { FakeSignatureProvider } from '../../lib/signature/fake-provider'
import { SignatureReconciliationHandler } from '../../lib/signature/reconciliation'
import {
  reconcileCompletedSignatureRequest,
  SIGNATURE_RECONCILE_COMMAND_PREFIX,
  type ReconcileCompletedSignatureRequestDeps,
} from '../../db/signature-reconciliation'
import type {
  SignatureRecipient,
  SignedArtifactDownload,
} from '../../lib/signature/contracts'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'
import type { DomainEvent } from '../../lib/workflow/contracts'

type Row = Record<string, any>
const FIXED_NOW = () => new Date('2026-08-22T00:00:00.000Z')

const CANONICAL_DOC_KEYS = [
  'created_at', 'deal_id', 'document_type', 'document_type_label', 'id',
  'media_id', 'party_person_id', 'prepared_by_user_id', 'signed_at',
  'signed_media_id', 'source', 'source_external_id', 'source_system',
  'state', 'supersedes_document_id', 'title', 'updated_at',
]

const CANONICAL_REQUEST_KEYS = [
  'created_at', 'created_by_user_id', 'id', 'message', 'status',
  'transaction_document_id', 'updated_at',
]

function copyBytes(value: unknown): unknown {
  if (value instanceof Uint8Array) return new Uint8Array(value)
  return value
}

const ACTIVE_REQUEST_STATUSES = ['requested', 'sent', 'viewed', 'signed']
function isActive(status: string): boolean {
  return ACTIVE_REQUEST_STATUSES.includes(status)
}

// ---------------------------------------------------------------------------
// FakeDb — in-memory canonical seam + media store, WITH rollback.
// ---------------------------------------------------------------------------

class FakeDb {
  receipts: Row[] = []
  documents: Row[] = []
  requests: Row[] = []
  media: Row[] = []
  seq = 0
  now = '2026-08-22T00:00:00.000Z'
  failNextDocumentTransition = false

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  /** TxRunner with snapshot/rollback: a thrown transaction restores all
   *  arrays, so partial writes (e.g. a media insert before a failing
   *  transition) never persist — exactly like a real rolled-back tx. */
  runner: TxRunner = async (cb) => {
    const snap = {
      receipts: this.receipts.map((r) => ({ ...r })),
      documents: this.documents.map((r) => ({ ...r })),
      requests: this.requests.map((r) => ({ ...r })),
      media: this.media.map((r) => ({ ...r, file_data: copyBytes(r.file_data) })),
    }
    try {
      return await cb(this.tx)
    } catch (err) {
      this.receipts = snap.receipts
      this.documents = snap.documents
      this.requests = snap.requests
      this.media = snap.media
      throw err
    }
  }

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''),
        '',
      ),
    )
    const p = params as any[]

    // ---- workflow_command_receipt (claim-first) ----
    if (t.includes('insert into workflow_command_receipt') && t.includes('on conflict')) {
      if (this.receipts.some((r) => r.command_id === p[0])) return Promise.resolve([])
      this.receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null })
      return Promise.resolve([{ command_id: p[0] }])
    }
    if (t.includes('update workflow_command_receipt set outcome =')) {
      const r = this.receipts.find((x) => x.command_id === p[3])
      if (r) {
        r.outcome = p[0]
        r.aggregate_id = p[1]
        r.message = p[2]
      }
      return Promise.resolve([])
    }
    if (
      t.includes('select command_id, outcome, aggregate_id, message from workflow_command_receipt') &&
      t.includes('where command_id')
    ) {
      const r = this.receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(
        r
          ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message }]
          : [],
      )
    }

    // ---- transaction_document ----
    // transition CAS: [to, to, signedMediaId, to, signedAt, id, from]
    if (t.includes('update transaction_document set state =')) {
      if (this.failNextDocumentTransition) {
        this.failNextDocumentTransition = false
        throw new Error('simulated transition failure after media append')
      }
      const r = this.documents.find((d) => d.id === p[5] && d.state === p[6])
      if (!r) return Promise.resolve([])
      r.state = p[0]
      if (p[0] === 'signed') {
        r.signed_media_id = p[2]
        r.signed_at = p[4]
      }
      r.updated_at = this.now
      return Promise.resolve([{ id: r.id }])
    }
    if (t.includes('from transaction_document') && t.includes('where id =')) {
      return Promise.resolve(this.documents.filter((d) => d.id === p[0]))
    }

    // ---- signature_request (send insert + status transition for the e2e
    // router path; reconciliation only reads) ----
    if (t.includes('insert into signature_request')) {
      const dup = this.requests.find(
        (r) => r.transaction_document_id === p[0] && isActive(r.status),
      )
      if (dup) return Promise.resolve([]) // one active request per document
      this.seq += 1
      const row = {
        id: `sig-${this.seq}`,
        transaction_document_id: p[0],
        status: 'requested',
        message: p[1],
        created_by_user_id: p[2],
        created_at: this.now,
        updated_at: this.now,
      }
      this.requests.push(row)
      return Promise.resolve([row])
    }
    if (t.includes('update signature_request set status =')) {
      const r = this.requests.find((x) => x.id === p[1] && x.status === p[2])
      if (!r) return Promise.resolve([])
      r.status = p[0]
      r.updated_at = this.now
      return Promise.resolve([{ ...r }])
    }
    if (t.includes('from signature_request') && t.includes('where id =')) {
      return Promise.resolve(this.requests.filter((r) => r.id === p[0]))
    }

    // ---- media (signed-artifact append; media_type 'document' is a literal
    // in the INSERT, not a parameter) ----
    if (t.includes('insert into media')) {
      this.seq += 1
      const row = {
        id: `media-${this.seq}`,
        file_data: p[0],
        filename: p[1],
        mime_type: p[2],
        file_size: p[3],
        media_type: 'document',
        created_at: this.now,
        updated_at: this.now,
      }
      this.media.push(row)
      return Promise.resolve([{ id: row.id }])
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function seedDocument(db: FakeDb, overrides: Row = {}): string {
  const id = overrides.id ?? 'doc-1'
  db.documents.push({
    id,
    deal_id: 'deal-1',
    document_type: 'agreement',
    document_type_label: null,
    title: 'Purchase Agreement',
    state: 'sent',
    source: 'generated',
    source_system: null,
    source_external_id: null,
    prepared_by_user_id: null,
    party_person_id: null,
    media_id: 'media-1',
    signed_media_id: null,
    signed_at: null,
    supersedes_document_id: null,
    created_at: db.now,
    updated_at: db.now,
    ...overrides,
  })
  return id
}

function seedRequest(db: FakeDb, overrides: Row = {}): string {
  const id = overrides.id ?? 'sig-1'
  db.requests.push({
    id,
    transaction_document_id: 'doc-1',
    status: 'completed',
    message: null,
    created_by_user_id: null,
    created_at: db.now,
    updated_at: db.now,
    ...overrides,
  })
  return id
}

function seedDraftMedia(
  db: FakeDb,
  id = 'media-1',
  bytes: Uint8Array = new Uint8Array(Buffer.from('ORIGINAL-DRAFT-PDF-BYTES', 'utf8')),
): Row {
  const row = {
    id,
    file_data: bytes,
    filename: 'purchase-agreement.pdf',
    mime_type: 'application/pdf',
    file_size: bytes.length,
    media_type: 'document',
    created_at: db.now,
    updated_at: db.now,
  }
  db.media.push(row)
  // Reserve the media id sequence so the FIRST appended signed media row is
  // media-2 (distinct from the seeded draft row).
  db.seq = Math.max(db.seq, 1)
  return row
}

function signedDownload(requestId: string): SignedArtifactDownload {
  return {
    bytes: new Uint8Array(Buffer.from(`%PDF-SIGNED-${requestId}`, 'utf8')),
    filename: `${requestId}-signed.pdf`,
    mimeType: 'application/pdf',
  }
}

function makeDeps(db: FakeDb, overrides: Partial<ReconcileCompletedSignatureRequestDeps> = {}) {
  const downloads: string[] = []
  const deps: ReconcileCompletedSignatureRequestDeps = {
    run: db.runner,
    execute: db.tx,
    downloadSignedArtifact: async (id) => {
      downloads.push(id)
      return signedDownload(id)
    },
    now: FIXED_NOW,
    ...overrides,
  }
  return { deps, downloads }
}

function reconcile(
  db: FakeDb,
  eventId: string,
  signatureRequestId = 'sig-1',
  overrides: Partial<ReconcileCompletedSignatureRequestDeps> = {},
) {
  const { deps, downloads } = makeDeps(db, overrides)
  return {
    downloads,
    run: () =>
      reconcileCompletedSignatureRequest(
        { eventId, signatureRequestId, occurredAt: '2026-08-22T00:00:00.000Z' },
        deps,
      ),
  }
}

function makeDispatcher(db: FakeDb) {
  return new CommandDispatcherImpl({
    registry: createCommandRegistry(),
    receipts: new PostgresCommandReceiptRepository(),
    run: db.runner,
    now: FIXED_NOW,
  })
}

function makeApp(db: FakeDb, provider: FakeSignatureProvider) {
  const reconciler = new SignatureReconciliationHandler({
    provider,
    run: db.runner,
    execute: db.tx,
    now: FIXED_NOW,
  })
  return new SignatureApplication({
    dispatcher: makeDispatcher(db),
    provider,
    reconciler,
    now: FIXED_NOW,
  })
}

const RECIPIENTS: SignatureRecipient[] = [
  { role: 'signer', name: 'Buyer One', email: 'buyer1@example.com', order: 1 },
]

function assertMediaBytes(row: Row, expected: Uint8Array): void {
  assert.ok(row.file_data instanceof Uint8Array)
  assert.equal(Buffer.from(row.file_data).equals(Buffer.from(expected)), true)
}

// ---------------------------------------------------------------------------
// Acceptance (1) + (2): a completed event appends a NEW signed media row,
// sets signed_media_id/signed_at via sent -> signed, and preserves the
// original/draft media row byte-for-byte.
// ---------------------------------------------------------------------------

test('completed event appends a NEW signed media row and signs the document via sent -> signed', async () => {
  const db = new FakeDb()
  const docId = seedDocument(db)
  const requestId = seedRequest(db)
  const original = seedDraftMedia(db)

  const { run, downloads } = reconcile(db, 'evt-completed-1')
  const result = await run()
  assert.equal(result.outcome, 'success')
  assert.equal(result.replayed, false)
  const value = result.value as any
  assert.equal(value.replayed, false)
  assert.equal(value.documentId, docId)
  assert.equal(value.signatureRequestId, requestId)
  assert.ok(value.mediaId)
  assert.equal(value.signedAt, '2026-08-22T00:00:00.000Z')

  // Exactly ONE signed artifact was downloaded (downloaded once).
  assert.deepEqual(downloads, ['sig-1'])

  // The document landed in 'signed' with the signed lineage via the DOC-01
  // transition; the draft media_id is untouched.
  const doc = db.documents.find((d) => d.id === docId)!
  assert.equal(doc.state, 'signed')
  assert.equal(doc.signed_media_id, value.mediaId)
  assert.equal(doc.signed_at, '2026-08-22T00:00:00.000Z')
  assert.equal(doc.media_id, 'media-1', 'the draft/original media row reference is preserved')

  // A NEW media row exists holding the signed bytes; the original row is
  // preserved byte-for-byte (append-only lineage).
  assert.equal(db.media.length, 2, 'exactly one signed media row was appended')
  const signed = db.media.find((m) => m.id === value.mediaId)!
  assert.equal(signed.media_type, 'document')
  assert.equal(signed.filename, 'sig-1-signed.pdf')
  assert.equal(signed.mime_type, 'application/pdf')
  assertMediaBytes(signed, signedDownload('sig-1').bytes)
  assertMediaBytes(original, new Uint8Array(Buffer.from('ORIGINAL-DRAFT-PDF-BYTES', 'utf8')))

  // The signed media row is distinct from the draft row.
  assert.notEqual(signed.id, original.id)

  // The reconcile receipt is finalized as the winner.
  const receipt = db.receipts.find((r) => r.command_id === `${SIGNATURE_RECONCILE_COMMAND_PREFIX}evt-completed-1`)!
  assert.equal(receipt.outcome, 'success')
  assert.equal(receipt.aggregate_id, docId)
})

// ---------------------------------------------------------------------------
// Acceptance (7): no provider/signature state lands on transaction_document;
// the canonical request is untouched.
// ---------------------------------------------------------------------------

test('reconciliation never writes provider/signature state to transaction_document or the request', async () => {
  const db = new FakeDb()
  const docId = seedDocument(db)
  const requestId = seedRequest(db)
  seedDraftMedia(db)

  const { run } = reconcile(db, 'evt-completed-1')
  const result = await run()
  assert.equal(result.outcome, 'success')

  // transaction_document gained ONLY signed_media_id / signed_at (its DOC-01
  // lineage columns) — no provider ids/status, no signature_request state.
  const doc = db.documents.find((d) => d.id === docId)!
  assert.deepEqual(Object.keys(doc).sort(), CANONICAL_DOC_KEYS, 'document carries ONLY canonical columns')
  assert.equal(doc.signed_media_id != null, true)
  assert.equal(doc.signed_at != null, true)

  // The canonical signature_request is read-only for reconciliation.
  const request = db.requests.find((r) => r.id === requestId)!
  assert.equal(request.status, 'completed')
  assert.deepEqual(Object.keys(request).sort(), CANONICAL_REQUEST_KEYS, 'request carries ONLY canonical columns')
})

// ---------------------------------------------------------------------------
// Acceptance (3): a replayed completed event (same neutral event id) is a
// no-op — no re-download, no duplicate signed media, no double transition.
// ---------------------------------------------------------------------------

test('a replayed completed event is a no-op: replayed:true, no duplicate media, no double transition', async () => {
  const db = new FakeDb()
  const docId = seedDocument(db)
  seedRequest(db)
  seedDraftMedia(db)

  const { run, downloads } = reconcile(db, 'evt-completed-1')
  const first = await run()
  assert.equal(first.outcome, 'success')
  assert.equal(first.replayed, false)

  const signedAtAfterFirst = db.documents.find((d) => d.id === docId)!.signed_at

  // Replay the SAME neutral event id.
  const replay = await run()
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true, 'the receipt replay is flagged replayed')
  assert.equal((replay.value as any).replayed, true)

  assert.deepEqual(downloads, ['sig-1'], 'a replay does NOT re-download the artifact')
  assert.equal(db.media.length, 2, 'no duplicate signed media row')
  assert.equal(db.documents.find((d) => d.id === docId)!.state, 'signed')
  assert.equal(
    db.documents.find((d) => d.id === docId)!.signed_at,
    signedAtAfterFirst,
    'no double transition (signed_at unchanged)',
  )
  assert.equal(db.receipts.filter((r) => r.command_id === `${SIGNATURE_RECONCILE_COMMAND_PREFIX}evt-completed-1`).length, 1)
})

// ---------------------------------------------------------------------------
// Acceptance (4): webhook duplication. Same provider event id is deduped at
// DOC-04 BEFORE any neutral event, so the router reconciles exactly once; a
// SECOND neutral event for the same completion is caught by the already-signed
// guard (never a second signed media row).
// ---------------------------------------------------------------------------

test('a second completed event for the same completion is replayed by the already-signed guard — no duplicate media', async () => {
  const db = new FakeDb()
  const docId = seedDocument(db)
  seedRequest(db)
  seedDraftMedia(db)

  const first = reconcile(db, 'evt-completed-1')
  const r1 = await first.run()
  assert.equal(r1.outcome, 'success')
  const mediaIdAfterFirst = (r1.value as any).mediaId

  // A DIFFERENT neutral event id (e.g. a second provider webhook that also
  // reports completion, or a retried reconciliation) for the same request.
  const second = reconcile(db, 'evt-completed-2')
  const r2 = await second.run()
  assert.equal(r2.outcome, 'success')
  assert.equal((r2.value as any).replayed, true, 'already-signed completion is treated as replayed')
  assert.equal(r2.replayed, false, 'the second event claimed its own receipt (not a receipt replay)')

  assert.deepEqual(second.downloads, [], 'the already-signed guard skips the one-time download')
  assert.equal(db.media.length, 2, 'no duplicate signed media row')
  const doc = db.documents.find((d) => d.id === docId)!
  assert.equal(doc.signed_media_id, mediaIdAfterFirst, 'no double transition')

  // The second event still finalized its own receipt (idempotent consumption).
  assert.equal(db.receipts.some((r) => r.command_id === `${SIGNATURE_RECONCILE_COMMAND_PREFIX}evt-completed-2`), true)
})

// ---------------------------------------------------------------------------
// Acceptance (5): out-of-order completion — the document is still draft/ready
// when the completed event arrives; it is advanced through the legal chain to
// sent and then signed.
// ---------------------------------------------------------------------------

test('out-of-order completion: a draft document is reconciled through draft -> ready -> sent -> signed', async () => {
  const db = new FakeDb()
  const docId = seedDocument(db, { state: 'draft' })
  seedRequest(db)
  seedDraftMedia(db)

  const { run } = reconcile(db, 'evt-completed-1')
  const result = await run()
  assert.equal(result.outcome, 'success')
  assert.equal((result.value as any).replayed, false)

  const doc = db.documents.find((d) => d.id === docId)!
  assert.equal(doc.state, 'signed', 'completed is terminal regardless of intermediate document state')
  assert.equal(doc.signed_media_id, 'media-2')
  assert.equal(doc.media_id, 'media-1', 'the draft media row is still referenced')

  // Every advance step is its own receipt-backed transition.
  const receiptIds = db.receipts.map((r) => r.command_id)
  const prefix = `${SIGNATURE_RECONCILE_COMMAND_PREFIX}evt-completed-1`
  assert.ok(receiptIds.includes(`${prefix}:doc:draft->ready`), 'draft -> ready transition receipt exists')
  assert.ok(receiptIds.includes(`${prefix}:doc:ready->sent`), 'ready -> sent transition receipt exists')
  assert.ok(receiptIds.includes(`${prefix}:doc:signed`), 'sent -> signed transition receipt exists')
})

test('out-of-order completion: a ready document is reconciled through ready -> sent -> signed', async () => {
  const db = new FakeDb()
  const docId = seedDocument(db, { state: 'ready' })
  seedRequest(db)
  seedDraftMedia(db)

  const { run } = reconcile(db, 'evt-completed-1')
  const result = await run()
  assert.equal(result.outcome, 'success')
  assert.equal(db.documents.find((d) => d.id === docId)!.state, 'signed')
  assert.equal(db.media.length, 2)
})

// ---------------------------------------------------------------------------
// Acceptance (6): partial failure between media create and transition recovers
// idempotently — the failed transaction rolls back, the retry appends exactly
// once.
// ---------------------------------------------------------------------------

test('partial failure (media appended, transition failed) rolls back; the retry reconciles exactly once', async () => {
  const db = new FakeDb()
  const docId = seedDocument(db)
  seedRequest(db)
  seedDraftMedia(db)

  // First attempt: the sent -> signed transition fails AFTER the media row was
  // appended inside the transaction.
  db.failNextDocumentTransition = true
  const attempt = reconcile(db, 'evt-completed-1')
  await assert.rejects(() => attempt.run(), /simulated transition failure/)

  // The whole transaction rolled back: no stray media row, no signed state,
  // no receipt — the document is untouched and remains 'sent'.
  assert.equal(db.media.length, 1, 'the appended media row rolled back (no stray signed media)')
  assert.equal(db.receipts.length, 0, 'the claim rolled back')
  const doc = db.documents.find((d) => d.id === docId)!
  assert.equal(doc.state, 'sent')
  assert.equal(doc.signed_media_id, null)

  // Retry the SAME event id: it claims cleanly and appends exactly once.
  const retry = reconcile(db, 'evt-completed-1')
  const result = await retry.run()
  assert.equal(result.outcome, 'success')
  assert.equal(result.replayed, false)
  assert.equal(db.documents.find((d) => d.id === docId)!.state, 'signed')
  assert.equal(db.media.length, 2, 'exactly ONE signed media row after the retry')
  assert.equal(db.receipts.filter((r) => r.command_id === `${SIGNATURE_RECONCILE_COMMAND_PREFIX}evt-completed-1`).length, 1)
})

// ---------------------------------------------------------------------------
// Risk: signed-artifact download failure keeps state sent; the retry succeeds.
// ---------------------------------------------------------------------------

test('download failure keeps the document in sent (no mutation); the retry reconciles', async () => {
  const db = new FakeDb()
  const docId = seedDocument(db)
  seedRequest(db)
  seedDraftMedia(db)

  const { run } = reconcile(db, 'evt-completed-1', 'sig-1', {
    downloadSignedArtifact: async () => {
      throw new Error('BoldSign download timed out')
    },
  })
  await assert.rejects(() => run(), /BoldSign download timed out/)

  // Nothing mutated: document stays 'sent', no media row, no receipt claim.
  assert.equal(db.documents.find((d) => d.id === docId)!.state, 'sent')
  assert.equal(db.documents.find((d) => d.id === docId)!.signed_media_id, null)
  assert.equal(db.media.length, 1)
  assert.equal(db.receipts.length, 0)

  // Retry with the same event id succeeds.
  const retry = reconcile(db, 'evt-completed-1')
  const result = await retry.run()
  assert.equal(result.outcome, 'success')
  assert.equal(db.documents.find((d) => d.id === docId)!.state, 'signed')
  assert.equal(db.media.length, 2)
})

// ---------------------------------------------------------------------------
// Resolution guards: unknown request / unknown document / non-signable state.
// ---------------------------------------------------------------------------

test('reconciliation resolves the document via signature_request.transaction_document_id and rejects unknowns', async () => {
  const db = new FakeDb()
  seedDocument(db)
  seedRequest(db)
  seedDraftMedia(db)

  // Unknown signature request.
  const missingRequest = reconcile(db, 'evt-1', 'sig-unknown')
  const r1 = await missingRequest.run()
  assert.equal(r1.outcome, 'not_found')
  assert.match(r1.message ?? '', /Signature request not found/)
  assert.equal(db.media.length, 1, 'no append for an unresolvable request')

  // Request pointing at an unknown document.
  const db2 = new FakeDb()
  seedRequest(db2, { id: 'sig-2', transaction_document_id: 'doc-ghost' })
  const r2 = await reconcile(db2, 'evt-2', 'sig-2').run()
  assert.equal(r2.outcome, 'not_found')
  assert.match(r2.message ?? '', /Transaction document not found/)

  // A voided document is NOT signable — no mutation.
  const db3 = new FakeDb()
  seedDocument(db3, { state: 'voided' })
  seedRequest(db3)
  seedDraftMedia(db3)
  const r3 = await reconcile(db3, 'evt-3').run()
  assert.equal(r3.outcome, 'validation_failure')
  assert.match(r3.message ?? '', /cannot be signed from state 'voided'/)
  assert.equal(db3.documents[0].state, 'voided')
  assert.equal(db3.media.length, 1)
})

// ---------------------------------------------------------------------------
// The neutral subscriber (lib/signature/reconciliation.ts): only
// SIGNATURE_REQUEST_COMPLETED is reconciled; provider download is wired via
// the DOC-04 adapter behind the seam.
// ---------------------------------------------------------------------------

test('the neutral subscriber ignores non-completed events and rejects malformed payloads', async () => {
  const db = new FakeDb()
  seedDocument(db)
  seedRequest(db)
  seedDraftMedia(db)
  const provider = new FakeSignatureProvider()
  const handler = new SignatureReconciliationHandler({
    provider,
    run: db.runner,
    execute: db.tx,
    now: FIXED_NOW,
  })
  // Register the provider-side record so the one-time download resolves.
  await provider.send({
    signatureRequestId: 'sig-1',
    transactionDocumentId: 'doc-1',
    recipients: RECIPIENTS,
    message: null,
  })

  const base: DomainEvent = {
    eventId: 'evt-1',
    eventType: 'SIGNATURE_REQUEST_COMPLETED',
    occurredAt: '2026-08-22T00:00:00.000Z',
    actorAppUserId: null,
    aggregateType: 'signature_request',
    aggregateId: 'sig-1',
    correlationId: null,
    causationId: 'st-1',
    payload: { signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', status: 'completed' },
  }

  // A SENT event is explicitly not reconciled.
  const sent = await handler.onCompletedEvent({ ...base, eventId: 'evt-sent', eventType: 'SIGNATURE_REQUEST_SENT' })
  assert.equal(sent.outcome, 'precondition_failure')
  assert.match(sent.message ?? '', /only to SIGNATURE_REQUEST_COMPLETED/)
  assert.equal(db.media.length, 1, 'non-completed events never append')

  // A completed event missing its payload key is rejected.
  const malformed = await handler.onCompletedEvent({ ...base, payload: {} })
  assert.equal(malformed.outcome, 'validation_failure')
  assert.match(malformed.message ?? '', /missing signatureRequestId/)

  // A valid completed event reconciles through the provider download.
  const ok = await handler.onCompletedEvent(base)
  assert.equal(ok.outcome, 'success')
  assert.equal((ok.value as any).replayed, false)
  assert.equal(db.media.length, 2)
  assert.equal(db.documents[0].state, 'signed')
})

// ---------------------------------------------------------------------------
// End-to-end (router): the webhook path normalizes at the seam, the neutral
// completed event is subscribed to AFTER commit, and webhook duplication
// (same provider event id twice) reconciles exactly once.
// ---------------------------------------------------------------------------

test('e2e: a completed webhook reconciles the signed artifact; re-delivery (same provider event id) is a no-op', async () => {
  const db = new FakeDb()
  seedDocument(db, { state: 'sent' })
  seedDraftMedia(db)
  const provider = new FakeSignatureProvider()
  const app = makeApp(db, provider)

  const sent = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS }, { correlationId: 'wf-1' })
  assert.equal(sent.outcome, 'success')
  const request = (sent.value as { signatureRequest: SignatureRequest }).signatureRequest

  // Provider system advances; webhooks deliver viewed -> signed -> completed.
  let completedOutcome: Awaited<ReturnType<typeof app.handleWebhook>> | null = null
  for (const status of ['fake_viewed', 'fake_signed', 'fake_completed']) {
    const payload = { signatureRequestId: request.id, providerStatus: status }
    const outcome = await app.handleWebhook(payload, provider.signWebhookPayload(payload), { correlationId: 'wf-1' })
    assert.equal(outcome.result.outcome, 'success')
    if (status === 'fake_completed') completedOutcome = outcome
  }

  // The completed webhook subscribed to the NEUTRAL event AFTER commit and
  // reconciled the signed artifact.
  assert.equal(completedOutcome?.reconciliation?.outcome, 'success')
  const mediaId = (completedOutcome!.reconciliation!.value as any).mediaId
  assert.ok(mediaId && mediaId !== 'media-1', 'a NEW signed media row was appended')
  assert.equal(db.documents[0].state, 'signed')
  assert.equal(db.media.length, 2, 'exactly one signed media row appended')
  assert.equal(db.documents[0].signed_media_id, mediaId)

  // Webhook duplication: the SAME provider event (same completed webhook) is
  // re-delivered. The status command is a no-op (no transition -> no neutral
  // event), so the reconciler is never re-invoked.
  const payload = { signatureRequestId: request.id, providerStatus: 'fake_completed' }
  const redelivery = await app.handleWebhook(payload, provider.signWebhookPayload(payload), { correlationId: 'wf-1' })
  assert.equal(redelivery.result.outcome, 'success')
  assert.equal(redelivery.result.emittedEvents.length, 0, 'no duplicate neutral event on re-delivery')
  assert.equal(redelivery.reconciliation, null, 'no reconciliation when no completion occurred')
  assert.equal(db.media.length, 2, 'no duplicate signed media')
  assert.equal(db.documents[0].state, 'signed')
  assert.equal(db.documents[0].signed_media_id, mediaId, 'no double transition')

  // The signed artifact came from the provider download (fake provider), and
  // provider ids never appear in canonical rows.
  assert.ok(!JSON.stringify(db.documents).includes('env-') && !JSON.stringify(db.requests).includes('env-'))
})

test('e2e: handleWebhook surfaces the reconciliation outcome for the completed event', async () => {
  const db = new FakeDb()
  seedDocument(db, { state: 'sent' })
  seedDraftMedia(db)
  const provider = new FakeSignatureProvider()
  const app = makeApp(db, provider)

  const sent = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS }, { correlationId: 'wf-2' })
  const request = (sent.value as { signatureRequest: SignatureRequest }).signatureRequest
  for (const status of ['fake_viewed', 'fake_signed']) {
    const payload = { signatureRequestId: request.id, providerStatus: status }
    await app.handleWebhook(payload, provider.signWebhookPayload(payload), { correlationId: 'wf-2' })
  }

  const completed = { signatureRequestId: request.id, providerStatus: 'fake_completed' }
  const outcome = await app.handleWebhook(completed, provider.signWebhookPayload(completed), { correlationId: 'wf-2' })
  assert.equal(outcome.result.outcome, 'success')
  assert.equal(outcome.reconciliation?.outcome, 'success')
  assert.equal((outcome.reconciliation?.value as any).replayed, false)
  assert.equal((outcome.reconciliation?.value as any).documentId, 'doc-1')
  assert.equal(db.documents[0].state, 'signed')
})
