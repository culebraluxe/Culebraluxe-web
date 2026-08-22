import { test } from 'node:test'
import assert from 'node:assert/strict'

// ---------------------------------------------------------------------------
// DOC-03 — Signature Provider Seam tests.
//
// Proves the provider-neutral signing boundary end-to-end WITHOUT a database
// and WITHOUT a provider: an in-memory fake covering workflow_command_receipt
// + transaction_document + signature_request, the REAL canonical signature
// service + receipt repository + dispatcher + command handlers, and the fake
// SignatureProvider behind the seam. Scoped per the runtime test policy:
// this file only — no full regression, no persistence harness.
// ---------------------------------------------------------------------------

import { createCommandRegistry } from '../../lib/commands/register'
import { CommandDispatcherImpl } from '../../lib/commands/dispatcher'
import { PostgresCommandReceiptRepository } from '../../db/command-receipt-repository'
import { SIGNATURE_REQUEST_SEND, SIGNATURE_REQUEST_STATUS } from '../../lib/commands/command-types'
import { SignatureApplication } from '../../lib/signature/application'
import { FakeSignatureProvider } from '../../lib/signature/fake-provider'
import { SignatureProviderRegistry } from '../../lib/signature/provider-registry'
import {
  mapProviderStatus,
  neutralStatusForProviderEvent,
} from '../../lib/signature/status-mapping'
import type { SignatureRequest, SignatureRecipient } from '../../lib/signature/contracts'
import {
  getActiveSignatureRequestForDocument,
  getSignatureRequest,
} from '../../db/signature-request'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'
import type { CommandEnvelope } from '../../lib/workflow/contracts'

type Row = Record<string, any>
const FIXED_NOW = () => new Date('2026-08-22T00:00:00.000Z')

const ACTIVE = ['requested', 'sent', 'viewed', 'signed']

function isActive(status: string): boolean {
  return ACTIVE.includes(status)
}

// ---------------------------------------------------------------------------
// FakeDb — in-memory transaction over the three tables the seam touches.
// ---------------------------------------------------------------------------

class FakeDb {
  documents: Row[] = []
  requests: Row[] = []
  receipts: Row[] = []
  seq = 0
  now = '2026-08-22T00:00:00.000Z'

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  runner: TxRunner = async (cb) => cb(this.tx)

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

    // ---- transaction_document (existence lookup only) ----
    if (t.includes('select id from transaction_document') && t.includes('where id =')) {
      const doc = this.documents.find((d) => d.id === p[0])
      return Promise.resolve(doc ? [{ id: doc.id }] : [])
    }

    // ---- signature_request ----
    // insert ... on conflict (one active request per document) do nothing
    if (t.includes('insert into signature_request')) {
      const dup = this.requests.find(
        (r) => r.transaction_document_id === p[0] && isActive(r.status),
      )
      if (dup) return Promise.resolve([]) // on conflict ... do nothing
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
    // status transition (compare-and-set)
    if (t.includes('update signature_request set status =')) {
      const r = this.requests.find((x) => x.id === p[1] && x.status === p[2])
      if (!r) return Promise.resolve([])
      r.status = p[0]
      r.updated_at = this.now
      return Promise.resolve([{ ...r }])
    }
    if (t.includes('from signature_request')) {
      if (t.includes('where id =')) {
        return Promise.resolve(this.requests.filter((r) => r.id === p[0]))
      }
      if (t.includes('where transaction_document_id =')) {
        return Promise.resolve(
          this.requests
            .filter((r) => r.transaction_document_id === p[0])
            .sort((a, b) => (a.created_at < b.created_at ? -1 : 1)),
        )
      }
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
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

function makeApp(db: FakeDb, provider = new FakeSignatureProvider()) {
  return new SignatureApplication({
    dispatcher: makeDispatcher(db),
    provider,
    now: FIXED_NOW,
  })
}

function seedDocument(db: FakeDb, overrides: Row = {}): string {
  const id = overrides.id ?? 'doc-1'
  db.documents.push({
    id,
    deal_id: 'deal-1',
    document_type: 'agreement',
    title: 'Purchase Agreement',
    state: 'ready',
    source: 'generated',
    ...overrides,
  })
  return id
}

const RECIPIENTS: SignatureRecipient[] = [
  { role: 'signer', name: 'Buyer One', email: 'buyer1@example.com', order: 1 },
  { role: 'signer', name: 'Buyer Two', email: 'buyer2@example.com', order: 2 },
]

function sigEnvelope(overrides: Partial<CommandEnvelope>): CommandEnvelope {
  return {
    commandId: 'cmd-1',
    commandType: SIGNATURE_REQUEST_SEND,
    actorAppUserId: null,
    aggregateType: 'signature_request',
    aggregateId: null,
    correlationId: null,
    causationId: null,
    requestedAt: FIXED_NOW().toISOString(),
    input: {},
    ...overrides,
  } as CommandEnvelope
}

// ---------------------------------------------------------------------------
// 1. Neutral SignatureProvider interface + command set + neutral status model
// ---------------------------------------------------------------------------

test('seam surface: neutral commands, status model and provider interface exist', () => {
  const statuses = ['requested', 'sent', 'viewed', 'signed', 'completed', 'declined', 'voided', 'expired', 'error']
  for (const s of statuses) assert.ok(s.length > 0)
  // The neutral status model is the story's chain.
  assert.deepEqual(
    statuses,
    ['requested', 'sent', 'viewed', 'signed', 'completed', 'declined', 'voided', 'expired', 'error'],
  )
  // Command identifiers are stable and provider-neutral.
  assert.equal(SIGNATURE_REQUEST_SEND, 'signature.request.send')
  assert.equal(SIGNATURE_REQUEST_STATUS, 'signature.request.status')
  // The provider interface methods exist on the fake implementation.
  const provider = new FakeSignatureProvider()
  assert.equal(provider.name, 'fake-sign')
  for (const m of ['send', 'status', 'cancel', 'verifyWebhook']) {
    assert.equal(typeof (provider as any)[m], 'function', `${m} must exist on SignatureProvider`)
  }
  // The configured-provider registry dispatches by provider, never by a
  // provider-specific command (fail-closed until configured).
  const registry = new SignatureProviderRegistry()
  assert.equal(registry.isConfigured(), false)
  assert.throws(() => registry.get(), /No SignatureProvider is configured/)
  registry.configure(provider)
  assert.equal(registry.isConfigured(), true)
  assert.equal(registry.get(), provider)
  assert.equal(registry.name(), 'fake-sign')
})

// ---------------------------------------------------------------------------
// 2. Canonical signature_request references transaction_document, NO provider
//    fields; intermediate provider state never touches transaction_document
// ---------------------------------------------------------------------------

test('send records a canonical request referencing the document with neutral status and zero provider fields', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const dispatcher = makeDispatcher(db)

  const res = await dispatcher.execute(
    sigEnvelope({
      commandId: 'send-1',
      input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: 'Please sign' },
    }),
  )
  assert.equal(res.outcome, 'success')
  const value = res.value as { signatureRequest: SignatureRequest; existing: boolean }
  assert.equal(value.existing, false)
  assert.equal(value.signatureRequest.transactionDocumentId, 'doc-1')
  assert.equal(value.signatureRequest.status, 'requested')
  assert.equal(value.signatureRequest.message, 'Please sign')

  // The canonical row has ONLY the neutral columns — no provider ids/state.
  const stored = db.requests.find((r) => r.id === value.signatureRequest.id)!
  assert.deepEqual(
    Object.keys(stored).sort(),
    [
      'created_at', 'created_by_user_id', 'id', 'message',
      'status', 'transaction_document_id', 'updated_at',
    ].sort(),
  )

  // The referenced transaction_document row is untouched (no signature state).
  const doc = db.documents.find((d) => d.id === 'doc-1')!
  assert.equal(doc.state, 'ready')
  assert.equal(doc.signed_at, undefined)
  assert.equal(doc.signed_media_id, undefined)

  // Read seams work.
  const byId = await getSignatureRequest(value.signatureRequest.id, db.tx)
  assert.equal(byId?.id, value.signatureRequest.id)
  const active = await getActiveSignatureRequestForDocument('doc-1', db.tx)
  assert.equal(active?.id, value.signatureRequest.id)
})

// ---------------------------------------------------------------------------
// 3. send/status/cancel are idempotent (replay-safe) via receipts
// ---------------------------------------------------------------------------

test('send with the same commandId replays the stored result without a duplicate', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const dispatcher = makeDispatcher(db)

  const first = await dispatcher.execute(
    sigEnvelope({ commandId: 'send-dup', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS } }),
  )
  assert.equal(first.outcome, 'success')
  assert.equal(first.replayed, false)

  const replay = await dispatcher.execute(
    sigEnvelope({ commandId: 'send-dup', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS } }),
  )
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
  assert.equal(replay.receiptId, 'send-dup')
  assert.equal(db.requests.length, 1, 'replay must not create a second request')
  assert.equal(db.receipts.filter((r) => r.command_id === 'send-dup').length, 1)
})

test('send for a document with an existing ACTIVE request returns the existing request, never a duplicate', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const dispatcher = makeDispatcher(db)

  await dispatcher.execute(
    sigEnvelope({ commandId: 'send-a', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS } }),
  )

  // Same document, NEW commandId: the active request is returned (existing).
  const second = await dispatcher.execute(
    sigEnvelope({ commandId: 'send-b', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: 'ignored' } }),
  )
  assert.equal(second.outcome, 'success')
  assert.equal(second.replayed, false, 'a new commandId is a new request, not a replay')
  const value = second.value as { signatureRequest: SignatureRequest; existing: boolean }
  assert.equal(value.existing, true, 'the existing active request is returned')
  assert.equal(value.signatureRequest.id, db.requests[0].id)
  assert.equal(db.requests.length, 1, 'no duplicate request row')
})

test('status application is idempotent: re-applying the current status is a success no-op', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const dispatcher = makeDispatcher(db)

  await dispatcher.execute(
    sigEnvelope({ commandId: 'send-1', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS } }),
  )
  const reqId = db.requests[0].id

  const toSent = await dispatcher.execute(
    sigEnvelope({
      commandId: 'st-1',
      commandType: SIGNATURE_REQUEST_STATUS,
      aggregateId: reqId,
      input: { signatureRequestId: reqId, targetStatus: 'sent' },
    }),
  )
  assert.equal(toSent.outcome, 'success')
  assert.equal((toSent.value as { transitioned: boolean }).transitioned, true)

  const again = await dispatcher.execute(
    sigEnvelope({
      commandId: 'st-2',
      commandType: SIGNATURE_REQUEST_STATUS,
      aggregateId: reqId,
      input: { signatureRequestId: reqId, targetStatus: 'sent' },
    }),
  )
  assert.equal(again.outcome, 'success')
  assert.equal((again.value as { transitioned: boolean }).transitioned, false, 'same-status re-application is a no-op')
  assert.equal(db.requests[0].status, 'sent')
})

test('status command without a target is a read-only status application', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const dispatcher = makeDispatcher(db)

  await dispatcher.execute(
    sigEnvelope({ commandId: 'send-1', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS } }),
  )
  const reqId = db.requests[0].id

  const read = await dispatcher.execute(
    sigEnvelope({
      commandId: 'st-read',
      commandType: SIGNATURE_REQUEST_STATUS,
      aggregateId: reqId,
      input: { signatureRequestId: reqId },
    }),
  )
  assert.equal(read.outcome, 'success')
  assert.equal((read.value as { signatureRequest: SignatureRequest; transitioned: boolean }).signatureRequest.status, 'requested')
  assert.equal((read.value as { transitioned: boolean }).transitioned, false)
  assert.equal(db.requests[0].status, 'requested', 'a read never mutates')
  assert.equal(read.emittedEvents.length, 0)
})

test('cancel is idempotent and replay-safe (voided); decline lands in declined', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const dispatcher = makeDispatcher(db)

  await dispatcher.execute(
    sigEnvelope({ commandId: 'send-1', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS } }),
  )
  const reqId = db.requests[0].id

  const cancelled = await dispatcher.execute(
    sigEnvelope({
      commandId: 'cancel-1',
      commandType: 'signature.request.cancel',
      aggregateId: reqId,
      input: { signatureRequestId: reqId },
    }),
  )
  assert.equal(cancelled.outcome, 'success')
  assert.equal(db.requests[0].status, 'voided')

  // Replay the same cancel commandId: stored result, no mutation, no event.
  const replay = await dispatcher.execute(
    sigEnvelope({
      commandId: 'cancel-1',
      commandType: 'signature.request.cancel',
      aggregateId: reqId,
      input: { signatureRequestId: reqId },
    }),
  )
  assert.equal(replay.replayed, true)
  assert.equal(replay.emittedEvents.length, 0, 'no event on replay')
  assert.equal(db.requests[0].status, 'voided')

  // A NEW cancel commandId on an already-voided request is a success no-op
  // (idempotent: the request is already voided; re-application never mutates).
  const second = await dispatcher.execute(
    sigEnvelope({
      commandId: 'cancel-2',
      commandType: 'signature.request.cancel',
      aggregateId: reqId,
      input: { signatureRequestId: reqId },
    }),
  )
  assert.equal(second.outcome, 'success')
  assert.equal((second.value as { transitioned: boolean }).transitioned, false)
  assert.equal(db.requests[0].status, 'voided')
  assert.equal(second.emittedEvents.length, 0, 'no event for a no-op re-application')

  // A genuinely illegal cross-transition (declined from voided) is rejected.
  const illegal = await dispatcher.execute(
    sigEnvelope({
      commandId: 'decl-bad',
      commandType: 'signature.request.decline',
      aggregateId: reqId,
      input: { signatureRequestId: reqId },
    }),
  )
  assert.equal(illegal.outcome, 'validation_failure')
  assert.match(illegal.message ?? '', /Transition voided -> declined is not allowed/)

  // Decline on a fresh request.
  const db2 = new FakeDb()
  seedDocument(db2)
  const dispatcher2 = makeDispatcher(db2)
  await dispatcher2.execute(
    sigEnvelope({ commandId: 'send-1', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS } }),
  )
  const declined = await dispatcher2.execute(
    sigEnvelope({
      commandId: 'decl-1',
      commandType: 'signature.request.decline',
      aggregateId: db2.requests[0].id,
      input: { signatureRequestId: db2.requests[0].id },
    }),
  )
  assert.equal(declined.outcome, 'success')
  assert.equal(db2.requests[0].status, 'declined')
})

// ---------------------------------------------------------------------------
// 4. Provider status maps to neutral at the seam
// ---------------------------------------------------------------------------

test('seam status mapping: fake provider vocabulary -> neutral; unknown fails closed to error', () => {
  assert.equal(mapProviderStatus('fake-sign', 'fake_requested'), 'requested')
  assert.equal(mapProviderStatus('fake-sign', 'fake_sent'), 'sent')
  assert.equal(mapProviderStatus('fake-sign', 'fake_viewed'), 'viewed')
  assert.equal(mapProviderStatus('fake-sign', 'fake_signed'), 'signed')
  assert.equal(mapProviderStatus('fake-sign', 'fake_completed'), 'completed')
  assert.equal(mapProviderStatus('fake-sign', 'fake_declined'), 'declined')
  assert.equal(mapProviderStatus('fake-sign', 'fake_voided'), 'voided')
  assert.equal(mapProviderStatus('fake-sign', 'fake_expired'), 'expired')
  assert.equal(mapProviderStatus('fake-sign', 'fake_error'), 'error')
  // Unknown provider / unknown status never cast to a false success.
  assert.equal(mapProviderStatus('bold-sign', 'anything'), 'error')
  assert.equal(mapProviderStatus('fake-sign', 'bogus_status'), 'error')
  // Webhook event vocabulary normalizes onto the neutral status model.
  assert.equal(neutralStatusForProviderEvent('completed'), 'completed')
  assert.equal(neutralStatusForProviderEvent('declined'), 'declined')
  assert.equal(neutralStatusForProviderEvent('voided'), 'voided')
  assert.equal(neutralStatusForProviderEvent('sent'), 'sent')
})

test('provider.status() returns the status ALREADY mapped to neutral at the seam', async () => {
  const provider = new FakeSignatureProvider()
  await provider.send({
    signatureRequestId: 'sig-1',
    transactionDocumentId: 'doc-1',
    recipients: RECIPIENTS,
    message: null,
  })
  const s = await provider.status('sig-1')
  assert.equal(s.status, 'sent', 'the provider adapter maps its raw status to neutral')
  provider.setProviderStatus('sig-1', 'fake_completed')
  const after = await provider.status('sig-1')
  assert.equal(after.status, 'completed')
})

// ---------------------------------------------------------------------------
// 5. Neutral events (sent/completed/declined/voided) carry correlation/causation
// ---------------------------------------------------------------------------

test('status transitions emit neutral events with correlationId/causationId from the envelope', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const dispatcher = makeDispatcher(db)

  await dispatcher.execute(
    sigEnvelope({ commandId: 'send-1', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS } }),
  )
  const reqId = db.requests[0].id

  const sent = await dispatcher.execute(
    sigEnvelope({
      commandId: 'st-sent',
      commandType: SIGNATURE_REQUEST_STATUS,
      aggregateId: reqId,
      correlationId: 'corr-sig-1',
      causationId: 'caus-sig-1',
      input: { signatureRequestId: reqId, targetStatus: 'sent' },
    }),
  )
  assert.equal(sent.outcome, 'success')
  assert.equal(sent.emittedEvents.length, 1)
  const sentEvent = sent.emittedEvents[0]
  assert.equal(sentEvent.eventType, 'SIGNATURE_REQUEST_SENT')
  assert.equal(sentEvent.correlationId, 'corr-sig-1')
  assert.equal(sentEvent.causationId, 'st-sent', 'the command caused the fact')
  assert.equal(sentEvent.aggregateType, 'signature_request')
  assert.equal(sentEvent.aggregateId, reqId)
  assert.equal((sentEvent.payload as any).transactionDocumentId, 'doc-1')

  // Move to signed then completed: only the named neutral events are emitted.
  await dispatcher.execute(
    sigEnvelope({ commandId: 'st-viewed', commandType: SIGNATURE_REQUEST_STATUS, aggregateId: reqId, input: { signatureRequestId: reqId, targetStatus: 'viewed' } }),
  )
  await dispatcher.execute(
    sigEnvelope({ commandId: 'st-signed', commandType: SIGNATURE_REQUEST_STATUS, aggregateId: reqId, input: { signatureRequestId: reqId, targetStatus: 'signed' } }),
  )
  const completed = await dispatcher.execute(
    sigEnvelope({
      commandId: 'st-completed',
      commandType: SIGNATURE_REQUEST_STATUS,
      aggregateId: reqId,
      correlationId: 'corr-sig-2',
      input: { signatureRequestId: reqId, targetStatus: 'completed' },
    }),
  )
  assert.equal(completed.outcome, 'success')
  assert.equal(completed.emittedEvents.length, 1)
  assert.equal(completed.emittedEvents[0].eventType, 'SIGNATURE_REQUEST_COMPLETED')
  assert.equal(completed.emittedEvents[0].correlationId, 'corr-sig-2')
  assert.equal(completed.emittedEvents[0].causationId, 'st-completed')

  // completed is terminal: further transitions are rejected, and re-application
  // is a no-op WITHOUT an event.
  const illegal = await dispatcher.execute(
    sigEnvelope({ commandId: 'st-bad', commandType: SIGNATURE_REQUEST_STATUS, aggregateId: reqId, input: { signatureRequestId: reqId, targetStatus: 'declined' } }),
  )
  assert.equal(illegal.outcome, 'validation_failure')
  const noop = await dispatcher.execute(
    sigEnvelope({ commandId: 'st-again', commandType: SIGNATURE_REQUEST_STATUS, aggregateId: reqId, input: { signatureRequestId: reqId, targetStatus: 'completed' } }),
  )
  assert.equal(noop.outcome, 'success')
  assert.equal(noop.emittedEvents.length, 0, 'no event on a no-op re-application')
})

test('cancel and decline emit their neutral events (voided/declined)', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const dispatcher = makeDispatcher(db)
  await dispatcher.execute(
    sigEnvelope({ commandId: 'send-1', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS } }),
  )
  const reqId = db.requests[0].id

  const cancelled = await dispatcher.execute(
    sigEnvelope({
      commandId: 'cancel-1',
      commandType: 'signature.request.cancel',
      aggregateId: reqId,
      correlationId: 'corr-cancel',
      input: { signatureRequestId: reqId },
    }),
  )
  assert.equal(cancelled.emittedEvents.length, 1)
  assert.equal(cancelled.emittedEvents[0].eventType, 'SIGNATURE_REQUEST_VOIDED')
  assert.equal(cancelled.emittedEvents[0].correlationId, 'corr-cancel')

  const db2 = new FakeDb()
  seedDocument(db2)
  const dispatcher2 = makeDispatcher(db2)
  await dispatcher2.execute(
    sigEnvelope({ commandId: 'send-1', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS } }),
  )
  const declined = await dispatcher2.execute(
    sigEnvelope({
      commandId: 'decl-1',
      commandType: 'signature.request.decline',
      aggregateId: db2.requests[0].id,
      input: { signatureRequestId: db2.requests[0].id },
    }),
  )
  assert.equal(declined.emittedEvents.length, 1)
  assert.equal(declined.emittedEvents[0].eventType, 'SIGNATURE_REQUEST_DECLINED')
})

// ---------------------------------------------------------------------------
// 6. Application validation stays at the boundary (application owns authority)
// ---------------------------------------------------------------------------

test('send validates transport shape: unknown document, bad recipients, overlong message', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const dispatcher = makeDispatcher(db)

  const missing = await dispatcher.execute(
    sigEnvelope({ commandId: 'send-1', input: { transactionDocumentId: 'doc-nope', recipients: RECIPIENTS } }),
  )
  assert.equal(missing.outcome, 'not_found')

  const noRecipients = await dispatcher.execute(
    sigEnvelope({ commandId: 'send-2', input: { transactionDocumentId: 'doc-1', recipients: [] } }),
  )
  assert.equal(noRecipients.outcome, 'validation_failure')
  assert.match(noRecipients.message ?? '', /At least one recipient/)

  const badRole = await dispatcher.execute(
    sigEnvelope({ commandId: 'send-3', input: { transactionDocumentId: 'doc-1', recipients: [{ role: 'witness', name: 'X', email: 'x@example.com', order: 1 } as any] } }),
  )
  assert.equal(badRole.outcome, 'validation_failure')
  assert.match(badRole.message ?? '', /Invalid recipient role/)

  const longMessage = await dispatcher.execute(
    sigEnvelope({ commandId: 'send-4', input: { transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: 'x'.repeat(501) } }),
  )
  assert.equal(longMessage.outcome, 'validation_failure')
  assert.match(longMessage.message ?? '', /500/)
})

// ---------------------------------------------------------------------------
// 7. Fake provider proves the seam end-to-end (send -> provider -> status;
//    webhook normalization; DOC-05 boundary respected)
// ---------------------------------------------------------------------------

test('end-to-end: send dispatches to the provider and lands in neutral sent', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const provider = new FakeSignatureProvider()
  const app = makeApp(db, provider)

  const res = await app.send(
    {
      transactionDocumentId: 'doc-1',
      recipients: RECIPIENTS,
      message: 'Please sign the agreement',
    },
    { correlationId: 'wf-instance-1', causationId: 'wf-cmd-1' },
  )
  assert.equal(res.outcome, 'success')
  const value = res.value as { signatureRequest: SignatureRequest; transitioned: boolean }
  assert.equal(value.signatureRequest.status, 'sent', 'provider confirmation mapped to neutral sent')
  assert.equal(value.signatureRequest.message, 'Please sign the agreement')
  assert.equal(res.emittedEvents.length, 1)
  assert.equal(res.emittedEvents[0].eventType, 'SIGNATURE_REQUEST_SENT')
  assert.equal(res.emittedEvents[0].correlationId, 'wf-instance-1')

  // Only ONE canonical request exists, and only ONE provider-side record.
  assert.equal(db.requests.length, 1)
  assert.equal((provider as any).requests.size, 1)
})

test('end-to-end: a provider send failure lands as neutral error (never a false sent)', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const provider = new FakeSignatureProvider()
  provider.failNextSendWith('BoldSign timeout')
  const app = makeApp(db, provider)

  const res = await app.send(
    { transactionDocumentId: 'doc-1', recipients: RECIPIENTS },
    { correlationId: 'wf-instance-2' },
  )
  assert.equal(res.outcome, 'success')
  const value = res.value as { signatureRequest: SignatureRequest; transitioned: boolean }
  assert.equal(value.signatureRequest.status, 'error', 'delivery failure maps to neutral error')
  // error is terminal: a fresh send for the document creates a NEW request.
  const retry = await app.send(
    { transactionDocumentId: 'doc-1', recipients: RECIPIENTS },
    { correlationId: 'wf-instance-2' },
  )
  assert.equal(retry.outcome, 'success')
  const retryValue = retry.value as { signatureRequest: SignatureRequest; transitioned: boolean }
  assert.equal(retryValue.signatureRequest.status, 'sent')
  assert.equal(db.requests.length, 2, 'a new send after a terminal error is a new request')
})

test('end-to-end: duplicate send through the router never creates a second request', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const provider = new FakeSignatureProvider()
  const app = makeApp(db, provider)

  const first = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS }, { commandId: 'send-app-1' })
  assert.equal(first.outcome, 'success')
  const second = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS }, { commandId: 'send-app-2' })
  assert.equal(second.outcome, 'success')
  assert.equal(db.requests.length, 1, 'the active request is returned, never a duplicate')
})

test('end-to-end: webhook normalizes at the seam and completes the request; transaction_document stays untouched', async () => {
  const db = new FakeDb()
  seedDocument(db, { state: 'sent', signed_at: undefined, signed_media_id: undefined })
  const provider = new FakeSignatureProvider()
  const app = makeApp(db, provider)

  const sent = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS }, { correlationId: 'wf-3' })
  const reqId = (sent.value as { signatureRequest: SignatureRequest }).signatureRequest.id

  // Provider system moves state; its webhook delivers signed payloads.
  for (const status of ['fake_viewed', 'fake_signed', 'fake_completed']) {
    const payload = { signatureRequestId: reqId, providerStatus: status }
    const signature = provider.signWebhookPayload(payload)
    const outcome = await app.handleWebhook(payload, signature, { correlationId: 'wf-3' })
    assert.equal(outcome.result.outcome, 'success')
  }

  const final = db.requests.find((r) => r.id === reqId)!
  assert.equal(final.status, 'completed', 'provider webhook state normalized onto the neutral model')

  // The neutral COMPLETED event was emitted by the canonical command.
  const doc = db.documents.find((d) => d.id === 'doc-1')!
  assert.equal(doc.state, 'sent', 'transaction_document is untouched by intermediate provider state')
  assert.equal(doc.signed_at, undefined, 'signed_at is a DOC-05 reconciliation outcome, not set here')
  assert.equal(doc.signed_media_id, undefined, 'signed_media_id is a DOC-05 reconciliation outcome, not set here')

  // A terminal webhook re-delivery is a success no-op (idempotent, no event).
  const redelivery = await app.handleWebhook(
    { signatureRequestId: reqId, providerStatus: 'fake_completed' },
    provider.signWebhookPayload({ signatureRequestId: reqId, providerStatus: 'fake_completed' }),
    { correlationId: 'wf-3' },
  )
  assert.equal(redelivery.result.outcome, 'success')
  assert.equal(redelivery.result.emittedEvents.length, 0, 'no duplicate event on webhook redelivery')
  assert.equal(redelivery.event, 'completed')
})

test('webhook boundary: invalid signatures are rejected; provider payloads never reach canonical models', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const provider = new FakeSignatureProvider()
  const app = makeApp(db, provider)

  const sent = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS })
  const reqId = (sent.value as { signatureRequest: SignatureRequest }).signatureRequest.id

  await assert.rejects(
    () => app.handleWebhook(
      { signatureRequestId: reqId, providerStatus: 'fake_completed' },
      'forged-signature',
      { correlationId: 'wf-4' },
    ),
    /invalid webhook signature/,
  )
  assert.equal(db.requests[0].status, 'sent', 'a rejected webhook never mutates canonical state')

  // A webhook for an unknown provider-side request is rejected too.
  await assert.rejects(
    () => app.handleWebhook(
      { signatureRequestId: 'sig-unknown', providerStatus: 'fake_completed' },
      provider.signWebhookPayload({ signatureRequestId: 'sig-unknown', providerStatus: 'fake_completed' }),
    ),
    /unknown request/,
  )
})

test('refreshStatus polls the provider and applies the mapped neutral status', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const provider = new FakeSignatureProvider()
  const app = makeApp(db, provider)

  const sent = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS })
  const reqId = (sent.value as { signatureRequest: SignatureRequest }).signatureRequest.id

  // Provider system advances to signed; a status poll syncs the canonical record.
  provider.setProviderStatus(reqId, 'fake_signed')
  const polled = await app.refreshStatus(reqId, { correlationId: 'wf-5' })
  assert.equal(polled.outcome, 'success')
  assert.equal((polled.value as { signatureRequest: SignatureRequest }).signatureRequest.status, 'signed')
  assert.equal(polled.emittedEvents.length, 0, 'viewed/signed are not named neutral events')
})
