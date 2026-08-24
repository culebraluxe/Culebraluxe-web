import { test } from 'node:test'
import assert from 'node:assert/strict'

// CRM-27 — Phase 1 durability repair: canonical `agreement.execution.claim`
// command proof. Exercises the CommandDispatcher against an in-memory
// transaction fake so that the marker + command receipt + outbox append share
// ONE commit, and a forced rollback leaves NONE of them.

import { CommandDispatcherImpl } from '../../lib/commands/dispatcher'
import { createCommandRegistry } from '../../lib/commands/register'
import { PostgresCommandReceiptRepository } from '../../db/command-receipt-repository'
import { AGREEMENT_EXECUTION_CLAIM } from '../../lib/commands/command-types'
import { evaluateAgreementViaCommand } from '../../lib/agreements/re-drive'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'
import type { CommandEnvelope } from '../../lib/workflow/contracts'
import type { OutboxEventRepository } from '../../lib/events/outbox-contracts'

type Row = Record<string, any>
const FIXED_NOW = () => new Date('2026-08-24T12:00:00.000Z')

// ---------------------------------------------------------------------------
// AgreementCommandDb — in-memory transaction that applies writes only on
// commit. A forced failure discards the working copy (a rolled-back Postgres
// transaction): neither marker, receipt, nor outbox row survives.
// ---------------------------------------------------------------------------

class AgreementCommandDb {
  private committed: { receipts: Row[]; markers: Row[]; outbox: Row[] } = {
    receipts: [],
    markers: [],
    outbox: [],
  }
  commits = 0
  failNextCommit = false

  // Seeded immutable issued document + completed signature-role evidence.
  doc: { id: string; template_id: string; issued_version: number; deal_id: string | null }
  satisfiedRoles: string[] = []

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

      if (t.includes('from transaction_document')) {
        return Promise.resolve([
          {
            template_id: this.doc.template_id,
            issued_version: this.doc.issued_version,
            deal_id: this.doc.deal_id,
          },
        ])
      }
      if (t.includes('from signature_request')) {
        return Promise.resolve(this.satisfiedRoles.map((role) => ({ execution_role: role })))
      }
      if (t.includes('insert into agreement_execution')) {
        const key = `${String(p[0])}:${String(p[1])}`
        if (working.markers.some((m) => m.key === key)) return Promise.resolve([])
        working.markers.push({ key, document_id: p[0], issued_version: p[1] })
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

function makeDispatcher(db: AgreementCommandDb, eventSink?: any) {
  return new CommandDispatcherImpl({
    registry: createCommandRegistry(),
    receipts: new PostgresCommandReceiptRepository(),
    run: db.runner,
    now: FIXED_NOW,
    eventSink,
  })
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

const ROLES = ['BUYER', 'SELLER', 'SELLER_BROKER']

test('CRM-27: full evidence -> command commits marker + receipt + outbox atomically, emits once', async () => {
  const db = new AgreementCommandDb({
    id: 'doc-1',
    template_id: 'PR-PNS',
    issued_version: 1,
    deal_id: 'deal-1',
  })
  db.satisfiedRoles = ROLES
  const dispatcher = makeDispatcher(db, eventSink)

  const first = await dispatcher.execute(envelope())
  assert.equal(first.outcome, 'success')
  assert.equal(first.replayed, false)
  assert.equal(first.emittedEvents.length, 1)
  assert.equal(first.emittedEvents[0].eventType, 'AGREEMENT_FULLY_EXECUTED')

  // One atomic commit produced exactly one of each.
  assert.equal(db.receipts.length, 1)
  assert.equal(db.markers.length, 1)
  assert.equal(db.outbox.length, 1)
  assert.equal(db.outbox[0].event_type, 'AGREEMENT_FULLY_EXECUTED')
})

test('CRM-27: same commandId replay -> no second marker, no second outbox row', async () => {
  const db = new AgreementCommandDb({
    id: 'doc-1',
    template_id: 'PR-PNS',
    issued_version: 1,
    deal_id: 'deal-1',
  })
  db.satisfiedRoles = ROLES
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
  const db = new AgreementCommandDb({
    id: 'doc-1',
    template_id: 'PR-PNS',
    issued_version: 1,
    deal_id: 'deal-1',
  })
  db.satisfiedRoles = ROLES
  db.failNextCommit = true
  const dispatcher = makeDispatcher(db, eventSink)

  await assert.rejects(() => dispatcher.execute(envelope()))
  assert.equal(db.receipts.length, 0, 'no receipt after rollback')
  assert.equal(db.markers.length, 0, 'no marker after rollback')
  assert.equal(db.outbox.length, 0, 'no outbox event after rollback')
  assert.equal(db.commits, 0)
})

test('CRM-27: partial evidence -> no marker, no outbox event', async () => {
  const db = new AgreementCommandDb({
    id: 'doc-1',
    template_id: 'PR-PNS',
    issued_version: 1,
    deal_id: 'deal-1',
  })
  db.satisfiedRoles = ['BUYER']
  const dispatcher = makeDispatcher(db, eventSink)

  const result = await dispatcher.execute(envelope())
  assert.equal(result.outcome, 'success')
  assert.equal(result.emittedEvents.length, 0)
  assert.equal(db.markers.length, 0)
  assert.equal(db.outbox.length, 0)
})

test('CRM-27: event payload resolves document, version, template, deal, correlation', async () => {
  const db = new AgreementCommandDb({
    id: 'doc-1',
    template_id: 'PR-PNS',
    issued_version: 3,
    deal_id: 'deal-9',
  })
  db.satisfiedRoles = ROLES
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

test('CRM-27 (F): re-drive adapter dispatches the durable command and maps the completion', async () => {
  const db = new AgreementCommandDb({
    id: 'doc-1',
    template_id: 'PR-PNS',
    issued_version: 1,
    deal_id: 'deal-1',
  })
  db.satisfiedRoles = ROLES
  const dispatcher = makeDispatcher(db, eventSink)
  const completion = await evaluateAgreementViaCommand(
    { dispatcher },
    'doc-1',
    'completed-event-1',
  )
  assert.equal(completion.shouldEmit, true)
  assert.equal(completion.verdict.fullyExecuted, true)
  assert.equal(db.outbox.length, 1, 're-drive emits the outbox event durably')

  // Re-driving the SAME completed event (same commandId) is a replay: no duplicate.
  const again = await evaluateAgreementViaCommand({ dispatcher }, 'doc-1', 'completed-event-1')
  assert.equal(again.shouldEmit, false)
  assert.equal(db.outbox.length, 1)
})
