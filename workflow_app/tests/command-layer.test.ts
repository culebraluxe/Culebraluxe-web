import { test } from 'node:test'
import assert from 'node:assert/strict'

// Canonical command layer (CRM-14J) — unit proof against an in-memory
// transaction fake. The REAL receipt repository and REAL deal wrappers run
// against a fake QueryExecutor that mirrors the claim-first receipt SQL and
// the canonical deal mutations, so replay/rollback/correlation semantics are
// exercised end-to-end without a database.

import { PostgresCommandReceiptRepository } from '../../db/command-receipt-repository'
import type { TxRunner } from '../../db/tx'
import type { QueryExecutor } from '../../db/query-executor'
import type { CommandReceipt } from '../../lib/commands/contracts'
import { CommandDispatcherImpl } from '../../lib/commands/dispatcher'
import { InMemoryCommandRegistry } from '../../lib/commands/registry'
import { InMemoryDomainEventCollector, createDomainEventFromCommand } from '../../lib/commands/domain-events'
import { createCommandRegistry } from '../../lib/commands/register'
import {
  DEAL_SET_CLOSING_DATE,
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_STAGE_CLOSED,
  DEAL_SET_STAGE_UNDER_CONTRACT,
} from '../../lib/commands/command-types'
import { toCommandEnvelope } from '../engine-bridge'
import type { CommandEnvelope } from '../../lib/workflow/contracts'

type Row = Record<string, any>
const FIXED_NOW = () => new Date('2026-08-22T00:00:00.000Z')

// ---------------------------------------------------------------------------
// FakeCommandDb — an in-memory transaction that applies writes only on
// commit. A thrown handler / failed commit discards the working copy, exactly
// like a rolled-back Postgres transaction.
// ---------------------------------------------------------------------------

class FakeCommandDb {
  private committedReceipts: Row[] = []
  private committedDeals: Row[] = []
  commits = 0
  failNextCommit = false

  get receipts(): Row[] {
    return this.committedReceipts
  }

  get deals(): Row[] {
    return this.committedDeals
  }

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  private makeTx(working: { receipts: Row[]; deals: Row[] }): QueryExecutor {
    const tx: QueryExecutor = (strings, ...params) => {
      const t = this.norm(
        strings.reduce((acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''), ''),
      )
      const p = params as any[]

      if (t.includes('insert into workflow_command_receipt') && t.includes('on conflict')) {
        if (working.receipts.some((r) => r.command_id === p[0])) return Promise.resolve([])
        working.receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null, actor_app_user_id: p[1] ?? null })
        return Promise.resolve([{ command_id: p[0] }])
      }
      if (t.includes('update workflow_command_receipt set outcome =')) {
        const r = working.receipts.find((x) => x.command_id === p[4])
        if (r) {
          r.outcome = p[0]
          r.aggregate_id = p[1]
          r.message = p[2]
          r.actor_app_user_id = p[3] ?? null
        }
        return Promise.resolve([])
      }
      if (t.includes('select command_id, outcome, aggregate_id, message from workflow_command_receipt') && t.includes('where command_id')) {
        const r = working.receipts.find((x) => x.command_id === p[0])
        return Promise.resolve(r ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message, actor_app_user_id: r.actor_app_user_id ?? null }] : [])
      }
      if (t.includes('update deal set stage =') && t.includes('returning id, stage')) {
        const row = working.deals.find((d) => d.id === p[2] && d.stage === p[3])
        if (!row) return Promise.resolve([])
        row.stage = p[0]
        return Promise.resolve([{ id: row.id, stage: row.stage }])
      }
      if (t.includes('select stage from deal where id =')) {
        const row = working.deals.find((d) => d.id === p[0])
        return Promise.resolve(row ? [{ stage: row.stage }] : [])
      }
      if (t.includes('update deal set closing_date =') && t.includes('returning id')) {
        const row = working.deals.find((d) => d.id === p[1])
        if (!row) return Promise.resolve([])
        row.closing_date = p[0]
        return Promise.resolve([{ id: row.id }])
      }
      if (t.includes('update deal set financing_type =') && t.includes('returning id')) {
        const row = working.deals.find((d) => d.id === p[1])
        if (!row) return Promise.resolve([])
        row.financing_type = p[0]
        return Promise.resolve([{ id: row.id }])
      }

      throw new Error(`FAKE_UNHANDLED: ${t}`)
    }
    return tx
  }

  runner: TxRunner = async (cb) => {
    const working = {
      receipts: this.committedReceipts.map((r) => ({ ...r })),
      deals: this.committedDeals.map((d) => ({ ...d })),
    }
    const tx = this.makeTx(working)
    const result = await cb(tx)
    if (this.failNextCommit) {
      this.failNextCommit = false
      throw new Error('simulated commit failure (after handler, before commit)')
    }
    this.committedReceipts = working.receipts
    this.committedDeals = working.deals
    this.commits += 1
    return result
  }
}

function envelope(overrides: Partial<CommandEnvelope>): CommandEnvelope {
  return {
    commandId: 'cmd-1',
    commandType: DEAL_SET_STAGE_UNDER_CONTRACT,
    actorAppUserId: null,
    aggregateType: 'deal',
    aggregateId: 'deal-1',
    correlationId: 'corr-1',
    causationId: null,
    requestedAt: '2026-08-22T00:00:00.000Z',
    input: {},
    ...overrides,
  } as CommandEnvelope
}

function makeDispatcher(db: FakeCommandDb, registry = createCommandRegistry(), eventSink?: any) {
  return new CommandDispatcherImpl({
    registry,
    receipts: new PostgresCommandReceiptRepository(),
    run: db.runner,
    now: FIXED_NOW,
    eventSink,
  })
}

// ---------------------------------------------------------------------------
// A — Same commandId twice => one canonical mutation; second result is replay.
// ---------------------------------------------------------------------------

test('A: same commandId executed twice => one mutation, second result is a replay', async () => {
  const db = new FakeCommandDb()
  db.deals.push({ id: 'deal-1', stage: 'offer' })
  const dispatcher = makeDispatcher(db)

  const first = await dispatcher.execute(envelope({ commandId: 'cmd-1' }))
  assert.equal(first.outcome, 'success')
  assert.equal(first.replayed, false)
  assert.equal(first.receiptId, 'cmd-1')
  assert.equal(db.deals[0].stage, 'under_contract')
  assert.equal(db.receipts.length, 1)

  const replay = await dispatcher.execute(envelope({ commandId: 'cmd-1' }))
  assert.equal(replay.outcome, 'success')
  assert.equal(replay.replayed, true)
  assert.equal(replay.receiptId, 'cmd-1')
  // The claim-first receipt was inserted exactly once (no second handler run).
  assert.equal(db.receipts.length, 1)
  assert.equal(db.deals[0].stage, 'under_contract')
  assert.equal(db.commits, 2)
})

// ---------------------------------------------------------------------------
// B — Same business intent with a NEW commandId is a NEW request, validated
//     normally against current canonical truth.
// ---------------------------------------------------------------------------

test('B: same intent with a new commandId is a new request, validated normally', async () => {
  const db = new FakeCommandDb()
  db.deals.push({ id: 'deal-1', stage: 'offer' })
  const dispatcher = makeDispatcher(db)

  await dispatcher.execute(envelope({ commandId: 'cmd-a' }))
  assert.equal(db.deals[0].stage, 'under_contract')

  // Same intent, new commandId: the deal is now at under_contract, so the
  // offer -> under_contract CAS is validated against CURRENT truth -> conflict.
  const second = await dispatcher.execute(
    envelope({ commandId: 'cmd-b', correlationId: 'corr-2' }),
  )
  assert.equal(second.outcome, 'conflict')
  assert.equal(second.replayed, false)
  assert.match(second.message ?? '', /Expected stage 'offer'/)
  assert.equal(db.receipts.length, 2, 'each new commandId claims its own receipt')
})

// ---------------------------------------------------------------------------
// C — Domain validation remains in canonical services, not command handlers.
// ---------------------------------------------------------------------------

test('C: domain validation stays in the canonical service (date validity)', async () => {
  const db = new FakeCommandDb()
  db.deals.push({ id: 'deal-1', stage: 'under_contract' })
  const dispatcher = makeDispatcher(db)

  // Transport pre-check passes (closingDate present); the SERVICE rejects the
  // invalid date with its own message — proof the domain owns validation.
  const res = await dispatcher.execute(
    envelope({
      commandId: 'cmd-1',
      commandType: DEAL_SET_CLOSING_DATE,
      input: { closingDate: 'not-a-date' },
    }),
  )
  assert.equal(res.outcome, 'validation_failure')
  assert.equal(res.message, 'closingDate must be a valid date.')
  assert.equal(db.deals[0].closing_date, undefined, 'no mutation on domain rejection')
})

// ---------------------------------------------------------------------------
// D — The dispatcher knows command types only, never workflow node names or
//     provider integration names.
// ---------------------------------------------------------------------------

test('D: dispatcher never knows workflow node names or provider names', async () => {
  const db = new FakeCommandDb()
  const dispatcher = makeDispatcher(db)

  // Workflow XML node name (mark_under_contract) is NOT a command type.
  const node = await dispatcher.execute(
    envelope({ commandId: 'cmd-1', commandType: 'mark_under_contract' }),
  )
  assert.equal(node.outcome, 'not_found')
  assert.match(node.message ?? '', /Unknown command type/)

  // Provider integration name is NOT a command type either.
  const provider = await dispatcher.execute(
    envelope({ commandId: 'cmd-2', commandType: 'mux.upload' }),
  )
  assert.equal(provider.outcome, 'not_found')
})

// ---------------------------------------------------------------------------
// E — Workflow / UI / API / agent callers converge on the same dispatcher.
// ---------------------------------------------------------------------------

test('E: workflow-style and UI/API-style callers converge on the same dispatcher', async () => {
  const db = new FakeCommandDb()
  db.deals.push({ id: 'deal-1', stage: 'offer' })
  const dispatcher = makeDispatcher(db)

  // Workflow caller: engine request -> toCommandEnvelope (engine-bridge) ->
  // the SAME dispatcher instance.
  const workflowEnvelope = toCommandEnvelope(
    {
      commandId: 'cmd-1',
      commandType: DEAL_SET_STAGE_UNDER_CONTRACT,
      subjectType: 'deal',
      subjectId: 'deal-1',
      correlationId: 'wf-1',
      causationId: null,
      input: {},
    },
    null,
  )
  const viaWorkflow = await dispatcher.execute(workflowEnvelope)
  assert.equal(viaWorkflow.outcome, 'success')

  // UI/API caller: a directly-built envelope on the same dispatcher.
  const uiEnvelope = envelope({ commandId: 'cmd-1', correlationId: 'ui-1' })
  const viaUi = await dispatcher.execute(uiEnvelope)
  assert.equal(viaUi.outcome, 'success')
  assert.equal(viaUi.replayed, true, 'UI caller observes the workflow winner as a replay')

  // A third caller (agent) with a NEW commandId is a fresh validated request.
  const viaAgent = await dispatcher.execute(
    envelope({ commandId: 'cmd-2', correlationId: 'agent-1' }),
  )
  assert.equal(viaAgent.outcome, 'conflict', 'deal already at under_contract')
  assert.equal(viaAgent.replayed, false)
})

// ---------------------------------------------------------------------------
// F — correlationId / causationId survive command execution and the
//     normalized result / emitted events.
// ---------------------------------------------------------------------------

test('F: correlationId and causationId survive into emitted domain events', async () => {
  const db = new FakeCommandDb()
  const registry = new InMemoryCommandRegistry()
  const appended: Array<{ events: any[]; tx: unknown }> = []
  const eventSink = {
    async append(events: any[], tx: unknown) {
      appended.push({ events, tx })
    },
  }
  registry.register('test.emit', {
    async handle(envelope: CommandEnvelope, ctx: any) {
      ctx.events.add(
        createDomainEventFromCommand(envelope, {
          eventType: 'DEAL_STAGE_CHANGED',
          payload: { stage: 'under_contract' },
          eventId: `evt-${envelope.commandId}`,
        }),
      )
      return {
        commandId: envelope.commandId,
        outcome: 'success',
        emittedEvents: [],
        aggregateId: envelope.aggregateId,
        message: null,
        replayed: false,
      }
    },
  })
  const dispatcher = makeDispatcher(db, registry, eventSink)

  const res = await dispatcher.execute(
    envelope({ commandId: 'cmd-1', commandType: 'test.emit' }),
  )
  assert.equal(res.outcome, 'success')
  assert.equal(res.emittedEvents.length, 1)
  const ev = res.emittedEvents[0]
  assert.equal(ev.eventId, 'evt-cmd-1')
  assert.equal(ev.correlationId, 'corr-1', 'event carries the envelope correlationId')
  assert.equal(ev.causationId, 'cmd-1', 'the command caused the fact (causationId = commandId)')
  assert.equal(ev.aggregateId, 'deal-1')

  // Outbox sink receives the same events inside the business transaction.
  assert.equal(appended.length, 1)
  assert.equal(appended[0].events[0].eventId, 'evt-cmd-1')
})

test('F2: outbox append happens in the same transaction as the mutation, and not on replay', async () => {
  const db = new FakeCommandDb()
  const registry = new InMemoryCommandRegistry()
  const appended: Array<{ events: any[]; tx: unknown }> = []
  const txSeenByHandler: unknown[] = []
  const eventSink = {
    async append(events: any[], tx: unknown) {
      appended.push({ events, tx })
    },
  }
  registry.register('test.emit', {
    async handle(envelope: CommandEnvelope, ctx: any) {
      txSeenByHandler.push(ctx.tx)
      // New-style handler: claim + finalize the receipt (idempotent replay),
      // emit a committed fact into the collector.
      const claimed = await ctx.receipts.claim(envelope.commandId, ctx.tx)
      if (!claimed) {
        const receipt = await ctx.receipts.find(envelope.commandId, ctx.tx)
        return {
          commandId: envelope.commandId,
          outcome: 'success',
          emittedEvents: [],
          aggregateId: null,
          message: null,
          replayed: true,
        }
      }
      ctx.events.add(
        createDomainEventFromCommand(envelope, {
          eventType: 'DEAL_STAGE_CHANGED',
          payload: {},
          eventId: `evt-${envelope.commandId}`,
        }),
      )
      await ctx.receipts.save(
        {
          commandId: envelope.commandId,
          outcome: 'success',
          status: 'Succeeded',
          aggregateId: null,
          message: null,
          createdAt: null,
        } as CommandReceipt,
        ctx.tx,
      )
      return {
        commandId: envelope.commandId,
        outcome: 'success',
        emittedEvents: [],
        aggregateId: null,
        message: null,
        replayed: false,
      }
    },
  })
  const dispatcher = makeDispatcher(db, registry, eventSink)

  // First run: the handler receives ctx.tx; the sink receives the SAME tx.
  await dispatcher.execute(envelope({ commandId: 'cmd-1', commandType: 'test.emit' }))
  assert.equal(appended.length, 1)
  assert.equal(appended[0].tx, txSeenByHandler[0], 'outbox rows share the business transaction')

  // Replay: no new events, no outbox append.
  const replay = await dispatcher.execute(envelope({ commandId: 'cmd-1', commandType: 'test.emit' }))
  assert.equal(replay.replayed, true)
  assert.equal(appended.length, 1, 'no outbox append on replay')
})

// ---------------------------------------------------------------------------
// G — Transaction failure after the domain mutation but before commit =>
//     no durable mutation, no success receipt, no outbox event.
// ---------------------------------------------------------------------------

test('G: handler throw after mutation rolls back mutation + receipt + outbox', async () => {
  const db = new FakeCommandDb()
  db.deals.push({ id: 'deal-1', stage: 'under_contract' })
  const registry = new InMemoryCommandRegistry()
  const appended: Array<{ events: any[] }> = []
  const eventSink = {
    async append(events: any[]) {
      appended.push({ events })
    },
  }
  registry.register('test.boom', {
    async handle(envelope: CommandEnvelope, ctx: any) {
      // 1. claim the commandId (winner)
      const claimed = await ctx.receipts.claim(envelope.commandId, ctx.tx)
      assert.equal(claimed, true)
      // 2. mutate canonical truth (simulated domain mutation in this tx)
      await ctx.tx`update deal set closing_date = ${'2026-09-01'} where id = ${'deal-1'} returning id`
      // 3. persist a receipt (same tx)
      await ctx.receipts.save(
        {
          commandId: envelope.commandId,
          outcome: 'success',
          status: 'Succeeded',
          aggregateId: null,
          message: null,
          createdAt: null,
        } as CommandReceipt,
        ctx.tx,
      )
      // 4. collect an event for the outbox (same tx)
      ctx.events.add(
        createDomainEventFromCommand(envelope, {
          eventType: 'DEAL_STAGE_CHANGED',
          payload: {},
          eventId: `evt-${envelope.commandId}`,
        }),
      )
      // 5. infrastructure failure BEFORE commit
      throw new Error('infrastructure failure after domain mutation')
    },
  })
  const dispatcher = makeDispatcher(db, registry, eventSink)

  await assert.rejects(() =>
    dispatcher.execute(envelope({ commandId: 'cmd-1', commandType: 'test.boom' })),
  )

  assert.equal(db.receipts.length, 0, 'no success receipt survives the rollback')
  assert.equal(db.deals[0].closing_date, undefined, 'no durable business mutation')
  assert.equal(appended.length, 0, 'no outbox rows survive the rollback')
  assert.equal(db.commits, 0, 'transaction never committed')
})

test('G2: simulated commit failure leaves nothing durable', async () => {
  const db = new FakeCommandDb()
  db.deals.push({ id: 'deal-1', stage: 'offer' })
  const dispatcher = makeDispatcher(db)
  db.failNextCommit = true

  await assert.rejects(() => dispatcher.execute(envelope({ commandId: 'cmd-1' })))
  assert.equal(db.deals[0].stage, 'offer', 'mutation rolled back')
  assert.equal(db.receipts.length, 0, 'receipt rolled back')
  assert.equal(db.commits, 0)

  // Re-submitting the SAME commandId is a fresh, valid execution (no success
  // receipt was ever committed) — the canonical mutation runs once.
  const retry = await dispatcher.execute(envelope({ commandId: 'cmd-1' }))
  assert.equal(retry.outcome, 'success')
  assert.equal(retry.replayed, false)
  assert.equal(db.deals[0].stage, 'under_contract')
})

// ---------------------------------------------------------------------------
// Replay of terminal failure receipts / poisoned pending receipts — the
// dispatcher must never re-run the mutation.
// ---------------------------------------------------------------------------

test('a terminal failure receipt replays its stored outcome without re-running', async () => {
  const db = new FakeCommandDb()
  db.deals.push({ id: 'deal-1', stage: 'offer' })
  const dispatcher = makeDispatcher(db)

  // deal.set_stage_closed on a deal at 'offer' is a domain conflict.
  const first = await dispatcher.execute(
    envelope({ commandId: 'cmd-1', commandType: DEAL_SET_STAGE_CLOSED }),
  )
  assert.equal(first.outcome, 'conflict')
  assert.equal(first.replayed, false)

  const replay = await dispatcher.execute(
    envelope({ commandId: 'cmd-1', commandType: DEAL_SET_STAGE_CLOSED }),
  )
  assert.equal(replay.outcome, 'conflict')
  assert.equal(replay.replayed, true)
  assert.equal(db.receipts.length, 1, 'no second claim on replay')
  assert.equal(db.deals[0].stage, 'offer', 'no mutation on replay')
})

test('a poisoned pending receipt is an in-flight conflict, never re-run', async () => {
  const db = new FakeCommandDb()
  db.deals.push({ id: 'deal-1', stage: 'offer' })
  db.receipts.push({ command_id: 'cmd-1', outcome: 'pending', aggregate_id: null, message: null })
  const dispatcher = makeDispatcher(db)

  const res = await dispatcher.execute(envelope({ commandId: 'cmd-1' }))
  assert.equal(res.outcome, 'conflict')
  assert.equal(res.replayed, true)
  assert.match(res.message ?? '', /pending/i)
  assert.equal(db.deals[0].stage, 'offer', 'deal must not be mutated on pending replay')
  assert.equal(db.receipts.length, 1)
})

// ---------------------------------------------------------------------------
// Wrappers prove the pattern against the real canonical services' contracts.
// ---------------------------------------------------------------------------

test('deal.set_financing_type wrapper validates transport shape, domain validates value', async () => {
  const db = new FakeCommandDb()
  db.deals.push({ id: 'deal-1', financing_type: null })
  const dispatcher = makeDispatcher(db)

  const ok = await dispatcher.execute(
    envelope({ commandId: 'cmd-1', commandType: DEAL_SET_FINANCING_TYPE, input: { financingType: 'financed' } }),
  )
  assert.equal(ok.outcome, 'success')
  assert.equal(db.deals[0].financing_type, 'financed')

  // Same commandId replays; a NEW commandId with a bogus value is rejected by
  // the transport pre-check (same as the pre-CRM-14J router).
  const replay = await dispatcher.execute(
    envelope({ commandId: 'cmd-1', commandType: DEAL_SET_FINANCING_TYPE, input: { financingType: 'financed' } }),
  )
  assert.equal(replay.replayed, true)

  const bad = await dispatcher.execute(
    envelope({ commandId: 'cmd-2', commandType: DEAL_SET_FINANCING_TYPE, input: { financingType: 'bogus' } }),
  )
  assert.equal(bad.outcome, 'validation_failure')
})

// ---------------------------------------------------------------------------
// Registry contract.
// ---------------------------------------------------------------------------

test('registry rejects duplicate command types', () => {
  const registry = new InMemoryCommandRegistry()
  const handler = { handle: async () => ({}) }
  registry.register('x.y', handler as any)
  assert.throws(() => registry.register('x.y', handler as any), /already registered/)
  assert.equal(registry.resolve('x.y'), handler)
  assert.equal(registry.resolve('nope'), undefined)
})

test('InMemoryDomainEventCollector drains exactly once', () => {
  const c = new InMemoryDomainEventCollector()
  c.add({} as any)
  assert.equal(c.drain().length, 1)
  assert.equal(c.drain().length, 0)
})
