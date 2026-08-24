import { test } from 'node:test'
import assert from 'node:assert/strict'

// CRM-27 — Phase 1 corrective proof: canonical `agreement.execution.claim`
// command through the REAL production dispatcher factory
// (`createCommandDispatcher`, with the real PostgresOutboxEventRepository sink
// wired in by default). Exercises it against an in-memory transaction fake so
// that the marker + command receipt + outbox append share ONE commit, and a
// forced rollback leaves NONE of them.

import { createCommandDispatcher } from '../../lib/commands'
import { createCommandRegistry } from '../../lib/commands/register'
import { PostgresCommandReceiptRepository } from '../../db/command-receipt-repository'
import { PostgresOutboxEventRepository } from '../../lib/mq/outbox-repository'
import { AGREEMENT_EXECUTION_CLAIM } from '../../lib/commands/command-types'
import { evaluateAgreementViaCommand } from '../../lib/agreements/re-drive'
import { runAgreementExecutionRecovery } from '../../lib/agreements/recovery'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'
import type { CommandEnvelope } from '../../lib/workflow/contracts'
import type { OutboxEventRepository } from '../../lib/events/outbox-contracts'

type Row = Record<string, any>
const FIXED_NOW = () => new Date('2026-08-24T12:00:00.000Z')
const SLOTS = ['BUYER:1', 'SELLER:1', 'SELLER_BROKER:1']

// AgreementCommandDb — in-memory transaction applying writes only on commit.
class AgreementCommandDb {
  private committed: { receipts: Row[]; markers: Row[]; outbox: Row[] } = {
    receipts: [],
    markers: [],
    outbox: [],
  }
  commits = 0
  failNextCommit = false
  docExists = true

  doc: {
    id: string
    template_id: string
    issued_version: number
    deal_id: string | null
    document_type: string
  }
  satisfiedSlots: string[] = []
  completedDocumentIds: string[] = []

  constructor(doc: AgreementCommandDb['doc']) {
    this.doc = doc
  }

  get receipts() {
    return this.committed.receipts
  }
  get markers() {
    return this.committed.markers
  }
  get outbox() {
    return this.committed.outbox
  }

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  private makeTx(working: AgreementCommandDb['committed']): QueryExecutor {
    const tx: QueryExecutor = (strings, ...params) => {
      const t = this.norm(
        strings.reduce((acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''), ''),
      )
      const p = params as any[]

      if (t.includes('from transaction_document') && !t.includes('distinct td.id')) {
        if (!this.docExists) return Promise.resolve([])
        return Promise.resolve([
          {
            id: this.doc.id,
            template_id: this.doc.template_id,
            issued_version: this.doc.issued_version,
            deal_id: this.doc.deal_id,
            document_type: this.doc.document_type,
          },
        ])
      }
      if (t.includes('from signature_request') && !t.includes('distinct td.id')) {
        return Promise.resolve(this.satisfiedSlots.map((slot) => ({ execution_slot_id: slot })))
      }
      if (t.includes('distinct td.id')) {
        const already = new Set(this.markers.map((m) => `${m.document_id}:${m.issued_version}`))
        return Promise.resolve(
          this.completedDocumentIds
            .filter((id) => !already.has(`${id}:${this.doc.issued_version}`))
            .map((id) => ({ id })),
        )
      }
      if (t.includes('insert into agreement_execution')) {
        const key = `${String(p[0])}:${String(p[1])}`
        if (working.markers.some((m) => m.key === key)) return Promise.resolve([])
        working.markers.push({ key, document_id: p[0], issued_version: p[1], event_id: p[2] })
        return Promise.resolve([{ id: 'm1' }])
      }
      if (t.includes('insert into workflow_command_receipt') && t.includes('on conflict')) {
        if (working.receipts.some((r) => r.command_id === p[0])) return Promise.resolve([])
        working.receipts.push({
          command_id: p[0],
          outcome: 'pending',
          aggregate_id: null,
          message: null,
          actor_app_user_id: p[1] ?? null,
        })
        return Promise.resolve([{ command_id: p[0] }])
      }
      if (t.includes('update workflow_command_receipt set outcome')) {
        const r = working.receipts.find((x) => x.command_id === p[4])
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
        const r = working.receipts.find((x) => x.command_id === p[0])
        return Promise.resolve(
          r
            ? [
                {
                  command_id: r.command_id,
                  outcome: r.outcome,
                  aggregate_id: r.aggregate_id,
                  message: r.message,
                  actor_app_user_id: r.actor_app_user_id ?? null,
                },
              ]
            : [],
        )
      }
      if (t.includes('insert into outbox_message')) {
        working.outbox.push({
          id: String(p[0]),
          event_type: String(p[1]),
          payload: p[3],
          aggregate_id: null,
        })
        return Promise.resolve([])
      }
      return Promise.resolve([])
    }
    return tx
  }


  get evidenceTx(): QueryExecutor {
    const self = this
    return ((strings, ...params) => {
      const t = self.norm(
        strings.reduce((acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''), ''),
      )
      if (t.includes('distinct td.id')) {
        const already = new Set(self.markers.map((m) => `${m.document_id}:${m.issued_version}`))
        return Promise.resolve(
          self.completedDocumentIds
            .filter((id) => !already.has(`${id}:${self.doc.issued_version}`))
            .map((id) => ({ id })),
        )
      }
      if (t.includes('from signature_request')) {
        return Promise.resolve(self.satisfiedSlots.map((slot) => ({ execution_slot_id: slot })))
      }
      return Promise.resolve([])
    }) as QueryExecutor
  }

  runner: TxRunner = async (cb) => {
    const working: AgreementCommandDb['committed'] = {
      receipts: this.committed.receipts.map((r) => ({ ...r })),
      markers: this.committed.markers.map((m) => ({ ...m })),
      outbox: this.committed.outbox.map((o) => ({ ...o })),
    }
    const result = await cb(this.makeTx(working))
    if (this.failNextCommit) {
      this.failNextCommit = false
      throw new Error('forced transaction rollback')
    }
    this.committed = working
    this.commits++
    return result
  }
}

function envelope(overrides: Partial<CommandEnvelope> = {}): CommandEnvelope {
  return {
    commandId: 'cmd-1',
    commandType: AGREEMENT_EXECUTION_CLAIM,
    actorAppUserId: 'user-1',
    aggregateType: 'transaction_document',
    aggregateId: 'doc-1',
    correlationId: 'corr-1',
    causationId: null,
    requestedAt: '2026-08-24T12:00:00.000Z',
    input: { transactionDocumentId: 'doc-1' },
    ...overrides,
  } as CommandEnvelope
}

const eventSink: OutboxEventRepository = {
  async append(events, tx) {
    for (const e of events) {
      await tx`insert into outbox_message (id, event_type, aggregate_id, payload) values (${e.eventId}, ${e.eventType}, ${e.aggregateId ?? null}, ${JSON.stringify(e.payload)}::jsonb) on conflict (id) do nothing`
    }
  },
  async claimBatch() {
    return []
  },
  async markDelivered() {},
  async markFailed() {},
}

// Use the REAL production dispatcher factory; only the tx/receipt seams are fake.
function makeDispatcher(db: AgreementCommandDb, sink: OutboxEventRepository) {
  return createCommandDispatcher({
    registry: createCommandRegistry(),
    receipts: new PostgresCommandReceiptRepository(),
    run: db.runner,
    now: FIXED_NOW,
    eventSink: sink,
  })
}

function doc(overrides: Partial<AgreementCommandDb['doc']> = {}): AgreementCommandDb['doc'] {
  return {
    id: 'doc-1',
    template_id: 'PR-PNS',
    issued_version: 1,
    deal_id: 'deal-1',
    document_type: 'agreement',
    ...overrides,
  }
}

test('CRM-27 (BLOCKER 1): real production factory wires a PostgresOutboxEventRepository sink', () => {
  const dispatcher = createCommandDispatcher()
  assert.ok(
    dispatcher.eventSink instanceof PostgresOutboxEventRepository,
    'production factory must wire the real Postgres outbox sink',
  )
  assert.notEqual(dispatcher.eventSink, null)
})

test('CRM-27: full evidence -> marker + receipt + outbox atomically; event id equals outbox id', async () => {
  const db = new AgreementCommandDb(doc())
  db.satisfiedSlots = SLOTS
  const dispatcher = makeDispatcher(db, eventSink)

  const first = await dispatcher.execute(envelope())
  assert.equal(first.outcome, 'success')
  assert.equal(first.replayed, false)
  assert.equal(first.emittedEvents.length, 1)
  assert.equal(first.emittedEvents[0].eventType, 'AGREEMENT_FULLY_EXECUTED')
  assert.equal(db.receipts.length, 1)
  assert.equal(db.markers.length, 1)
  assert.equal(db.outbox.length, 1)
  assert.equal(db.outbox[0].event_type, 'AGREEMENT_FULLY_EXECUTED')

  // EVENT-ID EQUALITY: marker.event_id == outbox.id == DomainEvent.eventId.
  assert.equal(db.markers[0].event_id, db.outbox[0].id)
  assert.equal(first.emittedEvents[0].eventId, db.outbox[0].id)
  assert.equal(db.markers[0].event_id, first.emittedEvents[0].eventId)
})

test('CRM-27: same commandId replay -> no second marker, no second outbox row', async () => {
  const db = new AgreementCommandDb(doc())
  db.satisfiedSlots = SLOTS
  const dispatcher = makeDispatcher(db, eventSink)

  await dispatcher.execute(envelope())
  const replay = await dispatcher.execute(envelope())
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
  assert.equal(db.receipts.length, 1)
  assert.equal(db.markers.length, 1)
  assert.equal(db.outbox.length, 1, 'no duplicate outbox event on replay')
})

test('CRM-27: forced rollback leaves neither marker, receipt, nor outbox row', async () => {
  const db = new AgreementCommandDb(doc())
  db.satisfiedSlots = SLOTS
  db.failNextCommit = true
  const dispatcher = makeDispatcher(db, eventSink)

  await assert.rejects(() => dispatcher.execute(envelope()))
  assert.equal(db.receipts.length, 0, 'no receipt after rollback')
  assert.equal(db.markers.length, 0, 'no marker after rollback')
  assert.equal(db.outbox.length, 0, 'no outbox event after rollback')
  assert.equal(db.commits, 0)
})

test('CRM-27: partial evidence -> no marker, no outbox event', async () => {
  const db = new AgreementCommandDb(doc())
  db.satisfiedSlots = ['BUYER:1']
  const dispatcher = makeDispatcher(db, eventSink)

  const result = await dispatcher.execute(envelope())
  assert.equal(result.outcome, 'success')
  assert.equal(result.emittedEvents.length, 0)
  assert.equal(db.markers.length, 0)
  assert.equal(db.outbox.length, 0)
})

test('CRM-27: event payload resolves document, version, template, deal, correlation', async () => {
  const db = new AgreementCommandDb(doc({ issued_version: 3, deal_id: 'deal-9' }))
  db.satisfiedSlots = SLOTS
  const dispatcher = makeDispatcher(db, eventSink)

  const result = await dispatcher.execute(
    envelope({ commandId: 'cmd-payload', correlationId: 'corr-42' }),
  )
  assert.equal(result.outcome, 'success')
  assert.equal(result.emittedEvents.length, 1)
  const ev = result.emittedEvents[0]
  assert.equal(ev.correlationId, 'corr-42')
  assert.equal(ev.causationId, 'cmd-payload')
  assert.equal(ev.aggregateType, 'transaction_document')
  assert.equal(ev.aggregateId, 'doc-1')
  assert.equal(ev.payload.transactionDocumentId, 'doc-1')
  assert.equal(ev.payload.issuedVersion, 3)
  assert.equal(ev.payload.templateId, 'PR-PNS')
  assert.equal(ev.payload.dealId, 'deal-9')
  assert.equal(ev.payload.agreementVersion, 'PR-PNS-v3')
})


test('CRM-27: missing/invalid/ineligible documents return truthful outcomes and never emit', async () => {
  const db1 = new AgreementCommandDb(doc())
  db1.docExists = false
  const d1 = makeDispatcher(db1, eventSink)
  const r1 = await d1.execute(envelope())
  assert.equal(r1.outcome, 'not_found')
  assert.equal(db1.markers.length, 0)
  assert.equal(db1.outbox.length, 0)

  const db2 = new AgreementCommandDb(doc({ issued_version: 0 }))
  const d2 = makeDispatcher(db2, eventSink)
  const r2 = await d2.execute(envelope())
  assert.equal(r2.outcome, 'validation_failure')
  assert.equal(db2.markers.length, 0)

  const db2b = new AgreementCommandDb(doc({ template_id: 'NOPE-99' }))
  const d2b = makeDispatcher(db2b, eventSink)
  const r2b = await d2b.execute(envelope())
  assert.equal(r2b.outcome, 'validation_failure')

  const db3 = new AgreementCommandDb(doc({ template_id: 'OFFER-01' }))
  const d3 = makeDispatcher(db3, eventSink)
  const r3 = await d3.execute(envelope())
  assert.equal(r3.outcome, 'precondition_failure')
  assert.equal(db3.outbox.length, 0)

  const db4 = new AgreementCommandDb(doc({ deal_id: null }))
  const d4 = makeDispatcher(db4, eventSink)
  const r4 = await d4.execute(envelope())
  assert.equal(r4.outcome, 'precondition_failure')

  const db5 = new AgreementCommandDb(doc({ document_type: 'disclosure' }))
  const d5 = makeDispatcher(db5, eventSink)
  const r5 = await d5.execute(envelope())
  assert.equal(r5.outcome, 'precondition_failure')

  for (const [label, db] of [
    ['db1', db1],
    ['db2', db2],
    ['db3', db3],
    ['db4', db4],
    ['db5', db5],
  ] as const) {
    for (const r of db.receipts) {
      assert.notEqual(
        r.outcome,
        'success',
        `${label} must not finalize a success receipt for an invalid document`,
      )
    }
  }
})

test('CRM-27 (BLOCKER 2): incomplete -> later complete re-drive reevaluates; exactly one marker/event', async () => {
  const db = new AgreementCommandDb(doc())
  const dispatcher = makeDispatcher(db, eventSink)

  // First evaluation has only BUYER evidence.
  db.satisfiedSlots = ['BUYER:1']
  const r1 = await evaluateAgreementViaCommand(
    { dispatcher, execute: db.evidenceTx },
    'doc-1',
    'completed-event-1',
  )
  assert.equal(r1.outcome, 'success')
  assert.equal(r1.shouldEmit, false)
  assert.equal(db.markers.length, 0, 'no marker while incomplete')
  assert.equal(db.outbox.length, 0, 'no event while incomplete')
  assert.equal(db.receipts.length, 1, 'the incomplete attempt committed its receipt')

  // SELLER + SELLER_BROKER evidence later becomes available; re-drive reevaluates.
  db.satisfiedSlots = SLOTS
  const r2 = await evaluateAgreementViaCommand(
    { dispatcher, execute: db.evidenceTx },
    'doc-1',
    'completed-event-2',
  )
  assert.equal(r2.outcome, 'success')
  assert.equal(r2.shouldEmit, true)
  assert.equal(db.receipts.length, 2, 'a new evidence fingerprint is a new attempt, not a replay')
  // Exactly one marker and one outbox event.
  assert.equal(db.markers.length, 1)
  assert.equal(db.outbox.length, 1)

  // Further re-drive (same evidence) produces no duplicate.
  const r3 = await evaluateAgreementViaCommand(
    { dispatcher, execute: db.evidenceTx },
    'doc-1',
    'completed-event-3',
  )
  assert.equal(r3.shouldEmit, false)
  assert.equal(db.markers.length, 1)
  assert.equal(db.outbox.length, 1, 'no duplicate business event on further re-drive')
})

test('CRM-27 (BLOCKER 3): durable recovery discovers completed evidence and re-drives', async () => {
  const db = new AgreementCommandDb(doc())
  db.satisfiedSlots = SLOTS
  db.completedDocumentIds = ['doc-1']
  const dispatcher = makeDispatcher(db, eventSink)

  const summary = await runAgreementExecutionRecovery({
    dispatcher,
    execute: db.evidenceTx,
  })
  assert.equal(summary.evaluated, 1)
  assert.equal(summary.emitted, 1)
  assert.equal(summary.failed.length, 0)
  assert.equal(db.markers.length, 1)
  assert.equal(db.outbox.length, 1)

  // A second pass is a no-op (marker exists -> nothing to recover / emit).
  const again = await runAgreementExecutionRecovery({
    dispatcher,
    execute: db.evidenceTx,
  })
  assert.equal(again.evaluated, 0)
  assert.equal(db.markers.length, 1)
  assert.equal(db.outbox.length, 1, 'recovery pass is idempotent')
})
