import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  createTransactionDocument,
  getTransactionDocument,
  listTransactionDocumentsByDeal,
  transitionTransactionDocumentState,
} from '../../db/transaction-document'
import { PortalWriteError } from '../../lib/portal-write-error'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'

// ---------------------------------------------------------------------------
// DOC-01 canonical transaction document model tests. In-memory fake covering
// transaction_document + workflow_command_receipt (the claim-first idempotency
// surface used for state transitions). No database.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

class FakeDb {
  documents: Row[] = []
  receipts: Row[] = []
  seq = 0
  now = '2026-08-21T12:00:00Z'

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

    // ---- workflow_command_receipt ----
    if (t.includes('insert into workflow_command_receipt') && t.includes('on conflict')) {
      if (this.receipts.some((r) => r.command_id === p[0])) return Promise.resolve([])
      this.receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null, actor_app_user_id: p[1] ?? null })
      return Promise.resolve([{ command_id: p[0] }])
    }
    if (t.includes('update workflow_command_receipt set outcome =')) {
      const r = this.receipts.find((x) => x.command_id === p[4])
      if (r) {
        r.outcome = p[0]
        r.aggregate_id = p[1]
        r.message = p[2]
        r.actor_app_user_id = p[3] ?? null
      }
      return Promise.resolve([])
    }
    if (
      t.includes('select command_id, outcome, aggregate_id, message') &&
      t.includes('where command_id')
    ) {
      const r = this.receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(
        r
          ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message, actor_app_user_id: r.actor_app_user_id ?? null }]
          : [],
      )
    }

    // ---- transaction_document ----
    // create: [deal, type, typeLabel, title, state, source, system, ext, user, party, media, signedMedia, signedAt, supersedes]
    if (t.includes('insert into transaction_document') && t.includes('on conflict')) {
      const dup = this.documents.find(
        (d) =>
          d.deal_id === p[0] &&
          d.source_system === p[6] &&
          d.source_external_id === p[7] &&
          p[7] !== null,
      )
      if (dup) return Promise.resolve([]) // on conflict ... do nothing
      this.seq += 1
      const row = {
        id: `doc-${this.seq}`,
        deal_id: p[0],
        document_type: p[1],
        document_type_label: p[2],
        title: p[3],
        state: p[4],
        source: p[5],
        source_system: p[6],
        source_external_id: p[7],
        prepared_by_user_id: p[8],
        party_person_id: p[9],
        media_id: p[10],
        signed_media_id: p[11],
        signed_at: p[12],
        supersedes_document_id: p[13],
        created_at: this.now,
        updated_at: this.now,
      }
      this.documents.push(row)
      return Promise.resolve([row])
    }
    // transition: [to, to, signedMediaId, to, signedAt, id, from]
    if (t.includes('update transaction_document set state =')) {
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
    if (t.includes('from transaction_document')) {
      if (t.includes('where id =')) {
        return Promise.resolve(this.documents.filter((d) => d.id === p[0]))
      }
      if (t.includes('and source_system')) {
        return Promise.resolve(
          this.documents.filter(
            (d) => d.deal_id === p[0] && d.source_system === p[1] && d.source_external_id === p[2],
          ),
        )
      }
      if (t.includes('where deal_id =')) {
        return Promise.resolve(this.documents.filter((d) => d.deal_id === p[0]))
      }
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }
}

async function seedDraft(f: FakeDb, overrides: Record<string, any> = {}): Promise<string> {
  const doc = await createTransactionDocument(
    {
      dealId: 'deal-1',
      documentType: 'agreement',
      source: 'generated',
      mediaId: 'media-1',
      title: 'Purchase Agreement',
      ...overrides,
    },
    f.tx,
  )
  return doc.id
}

// ---------------------------------------------------------------------------
// Create + validation
// ---------------------------------------------------------------------------

test('createTransactionDocument persists type/state/source/ownership and deal association', async () => {
  const f = new FakeDb()
  const id = await createTransactionDocument(
    {
      dealId: 'deal-1',
      documentType: 'disclosure',
      documentTypeLabel: 'Seller Property Disclosure',
      title: 'SPD v2',
      source: 'upload',
      sourceSystem: null,
      sourceExternalId: null,
      preparedByUserId: 'user-1',
      partyPersonId: 'person-1',
      mediaId: 'media-1',
    },
    f.tx,
  )

  assert.equal(id.dealId, 'deal-1')
  assert.equal(id.documentType, 'disclosure')
  assert.equal(id.documentTypeLabel, 'Seller Property Disclosure')
  assert.equal(id.state, 'draft')
  assert.equal(id.source, 'upload')
  assert.equal(id.preparedByUserId, 'user-1')
  assert.equal(id.partyPersonId, 'person-1')
  assert.equal(id.mediaId, 'media-1')
  assert.equal(id.signedMediaId, null)
  assert.equal(id.signedAt, null)

  const byId = await getTransactionDocument(id.id, f.tx)
  assert.equal(byId?.id, id.id)
  const byDeal = await listTransactionDocumentsByDeal('deal-1', f.tx)
  assert.equal(byDeal.length, 1)
  assert.equal(byDeal[0].id, id.id)
})

test('createTransactionDocument rejects invalid type/state/source', async () => {
  const f = new FakeDb()
  for (const bad of [
    { documentType: 'lease' },
    { state: 'archived' },
    { source: 'email' },
  ]) {
    await assert.rejects(
      createTransactionDocument(
        { dealId: 'deal-1', documentType: 'agreement', source: 'generated', ...bad },
        f.tx,
      ),
      (error: unknown) =>
        error instanceof PortalWriteError && error.code === 'validation',
    )
  }
})

test('createTransactionDocument rejects an overlong type label and a signed pair mismatch', async () => {
  const f = new FakeDb()
  await assert.rejects(
    createTransactionDocument(
      { dealId: 'deal-1', documentType: 'agreement', source: 'generated', documentTypeLabel: 'x'.repeat(121) },
      f.tx,
    ),
    (e) => e instanceof PortalWriteError && e.code === 'validation',
  )
  await assert.rejects(
    createTransactionDocument(
      { dealId: 'deal-1', documentType: 'agreement', source: 'generated', signedMediaId: 'media-9' },
      f.tx,
    ),
    (e) => e instanceof PortalWriteError && e.code === 'validation',
  )
})

test('createTransactionDocument with an external source is idempotent', async () => {
  const f = new FakeDb()
  const first = await createTransactionDocument(
    { dealId: 'deal-1', documentType: 'agreement', source: 'provider', sourceSystem: 'bold-sign', sourceExternalId: 'env-123' },
    f.tx,
  )
  const second = await createTransactionDocument(
    { dealId: 'deal-1', documentType: 'agreement', source: 'provider', sourceSystem: 'bold-sign', sourceExternalId: 'env-123' },
    f.tx,
  )
  assert.equal(second.id, first.id, 'source idempotency returns the existing row')
  assert.equal(f.documents.filter((d) => d.source_external_id === 'env-123').length, 1)
})


// ---------------------------------------------------------------------------
// Lifecycle transitions (claim-first command-receipt idempotency)
// ---------------------------------------------------------------------------

test('transition walks draft -> ready -> sent -> signed and preserves the draft media row', async () => {
  const f = new FakeDb()
  const id = await seedDraft(f)

  const ready = await transitionTransactionDocumentState(id, { commandId: 'c-ready', to: 'ready' }, f.runner)
  assert.equal(ready.outcome, 'success')
  assert.equal(ready.replayed, false)
  const sent = await transitionTransactionDocumentState(id, { commandId: 'c-sent', to: 'sent' }, f.runner)
  assert.equal(sent.outcome, 'success')
  const signed = await transitionTransactionDocumentState(
    id,
    { commandId: 'c-signed', to: 'signed', signedMediaId: 'media-2', signedAt: '2026-08-21T13:00:00Z' },
    f.runner,
  )
  assert.equal(signed.outcome, 'success')
  assert.equal(signed.aggregateId, id)

  const doc = f.documents.find((d) => d.id === id)!
  assert.equal(doc.state, 'signed')
  assert.equal(doc.media_id, 'media-1', 'draft media row is NEVER mutated')
  assert.equal(doc.signed_media_id, 'media-2', 'signed artifact is a distinct media row')
  assert.equal(doc.signed_at, '2026-08-21T13:00:00Z')
})

test('transition rejects disallowed transitions and unknown documents', async () => {
  const f = new FakeDb()
  const id = await seedDraft(f)

  const jump = await transitionTransactionDocumentState(id, { commandId: 'c-jump', to: 'signed' }, f.runner)
  assert.equal(jump.outcome, 'validation_failure')
  assert.match(jump.message ?? '', /not allowed/)
  assert.equal(f.documents.find((d) => d.id === id)!.state, 'draft', 'no mutation on rejection')

  const missing = await transitionTransactionDocumentState('doc-nope', { commandId: 'c-missing', to: 'ready' }, f.runner)
  assert.equal(missing.outcome, 'not_found')

  const invalid = await transitionTransactionDocumentState(id, { commandId: 'c-invalid', to: 'archived' }, f.runner)
  assert.equal(invalid.outcome, 'validation_failure')
})

test('signed transition requires the signed artifact lineage', async () => {
  const f = new FakeDb()
  const id = await seedDraft(f)
  await transitionTransactionDocumentState(id, { commandId: 'c1', to: 'ready' }, f.runner)
  await transitionTransactionDocumentState(id, { commandId: 'c2', to: 'sent' }, f.runner)

  const missing = await transitionTransactionDocumentState(id, { commandId: 'c3', to: 'signed' }, f.runner)
  assert.equal(missing.outcome, 'validation_failure')
  assert.match(missing.message ?? '', /signedMediaId and signedAt/i)
  assert.equal(f.documents.find((d) => d.id === id)!.state, 'sent')
})

test('a pending receipt returns conflict and does NOT mutate state', async () => {
  const f = new FakeDb()
  const id = await seedDraft(f)
  f.receipts.push({ command_id: 'c-pending', outcome: 'pending', aggregate_id: null, message: null })

  const res = await transitionTransactionDocumentState(id, { commandId: 'c-pending', to: 'ready' }, f.runner)
  assert.equal(res.outcome, 'conflict')
  assert.equal(res.replayed, true)
  assert.equal(f.documents.find((d) => d.id === id)!.state, 'draft', 'pending receipt never mutates')
})

test('a completed receipt replays its stored outcome without re-applying', async () => {
  const f = new FakeDb()
  const id = await seedDraft(f)
  f.receipts.push({ command_id: 'c-done', outcome: 'success', aggregate_id: id, message: null })

  const res = await transitionTransactionDocumentState(id, { commandId: 'c-done', to: 'ready' }, f.runner)
  assert.equal(res.outcome, 'success')
  assert.equal(res.replayed, true)
  assert.equal(f.documents.find((d) => d.id === id)!.state, 'draft', 'replay does not re-mutate')
})
