import { test } from 'node:test'
import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'

// CRM-14J — canonical command layer persistence proof (real Postgres, DEV
// Neon branch via lib/neon-interactive).
//
// Proves the dispatcher's transaction boundary with the SAME durability model
// production uses (db/tx neonTx): business mutation + command receipt + outbox
// rows commit atomically in one transaction; a failure after the mutation but
// before commit rolls back ALL of them; the same commandId replays exactly
// once. The outbox here is a TEST-ONLY sink writing to a test-only table —
// the production outbox remains DEFINED but NOT IMPLEMENTED (CRM-14I defer).

import { interactiveSql } from '../../../lib/neon-interactive'
import { neonTx } from '../../../db/tx'
import { CommandDispatcherImpl } from '../../../lib/commands/dispatcher'
import { InMemoryCommandRegistry } from '../../../lib/commands/registry'
import { PostgresCommandReceiptRepository } from '../../../db/command-receipt-repository'
import { createDomainEventFromCommand } from '../../../lib/commands/domain-events'
import { replayOutcome } from '../../../db/workflow-command-receipt'
import type { CommandEnvelope } from '../../../lib/workflow/contracts'

class CommandLayerPersistenceFixture {
  readonly tenantId = randomUUID()
  readonly commandIds: string[] = []

  private newCommandId(seed: string): string {
    const id = `tunit-${this.tenantId}-${seed}`
    this.commandIds.push(id)
    return id
  }

  async assertReceiptTable(): Promise<void> {
    const rows = await interactiveSql`
      select to_regclass('public.workflow_command_receipt') as r
    `
    if (!rows[0]?.r) {
      throw new Error(
        'workflow_command_receipt is missing from the DEV database (apply db/manual/2026-08-20_v4_crm14_workflow_activation.sql first).',
      )
    }
  }

  async ensureTestTables(): Promise<void> {
    await interactiveSql`drop table if exists tunit_dispatcher_effect`
    await interactiveSql`
      create table tunit_dispatcher_effect (
        command_id text primary key,
        tenant_id uuid not null,
        effect_count integer not null default 1
      )
    `
    await interactiveSql`drop table if exists tunit_dispatcher_outbox`
    await interactiveSql`
      create table tunit_dispatcher_outbox (
        event_id text primary key,
        tenant_id uuid not null,
        event_type text not null,
        correlation_id text,
        causation_id text
      )
    `
  }

  /** Test-only outbox sink: appends rows in the SAME transaction. */
  makeEventSink() {
    return {
      append: async (events: any[], tx: any) => {
        for (const e of events) {
          await tx`
            insert into tunit_dispatcher_outbox (
              event_id, tenant_id, event_type, correlation_id, causation_id
            ) values (
              ${e.eventId}, ${this.tenantId}, ${e.eventType},
              ${e.correlationId}, ${e.causationId}
            )
          `
        }
      },
    }
  }

  makeDispatcher(registry: InMemoryCommandRegistry, eventSink?: any) {
    return new CommandDispatcherImpl({
      registry,
      receipts: new PostgresCommandReceiptRepository(),
      run: neonTx,
      eventSink,
    })
  }

  /**
   * New-style canonical handler: claim-first receipt + canonical mutation +
   * emitted domain event — the exact pattern a future command author writes.
   */
  registerSetStageHandler(registry: InMemoryCommandRegistry) {
    const self = this
    registry.register('test.set_stage', {
      async handle(envelope: CommandEnvelope, ctx: any) {
        const claimed = await ctx.receipts.claim(envelope.commandId, ctx.tx)
        if (!claimed) {
          const receipt = await ctx.receipts.find(envelope.commandId, ctx.tx)
          const decision = replayOutcome(
            receipt
              ? {
                  commandId: receipt.commandId,
                  outcome: receipt.outcome,
                  aggregateId: receipt.aggregateId,
                  message: receipt.message,
                }
              : null,
          )
          return {
            commandId: envelope.commandId,
            outcome: decision.outcome,
            emittedEvents: [],
            aggregateId: receipt?.aggregateId ?? null,
            message: decision.message,
            replayed: true,
          }
        }
        // Canonical business mutation (test-only effect table, tenant-scoped).
        await ctx.tx`
          insert into tunit_dispatcher_effect (command_id, tenant_id, effect_count)
          values (${envelope.commandId}, ${self.tenantId}, 1)
        `
        await ctx.receipts.save(
          {
            commandId: envelope.commandId,
            outcome: 'success',
            status: 'Succeeded',
            aggregateId: envelope.aggregateId ?? null,
            message: null,
            createdAt: null,
          },
          ctx.tx,
        )
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
          aggregateId: envelope.aggregateId ?? null,
          message: null,
          replayed: false,
        }
      },
    })
  }

  /** Handler that mutates + receipts + emits, then FAILS before commit. */
  registerBoomHandler(registry: InMemoryCommandRegistry) {
    const self = this
    registry.register('test.boom', {
      async handle(envelope: CommandEnvelope, ctx: any) {
        await ctx.receipts.claim(envelope.commandId, ctx.tx)
        await ctx.tx`
          insert into tunit_dispatcher_effect (command_id, tenant_id, effect_count)
          values (${envelope.commandId}, ${self.tenantId}, 1)
        `
        await ctx.receipts.save(
          {
            commandId: envelope.commandId,
            outcome: 'success',
            status: 'Succeeded',
            aggregateId: null,
            message: null,
            createdAt: null,
          },
          ctx.tx,
        )
        ctx.events.add(
          createDomainEventFromCommand(envelope, {
            eventType: 'DEAL_STAGE_CHANGED',
            payload: {},
            eventId: `evt-${envelope.commandId}`,
          }),
        )
        throw new Error('infrastructure failure after domain mutation (test)')
      },
    })
  }

  async effectCount(commandId: string): Promise<number> {
    const rows = await interactiveSql`
      select count(*)::int as c from tunit_dispatcher_effect where command_id = ${commandId}
    `
    return rows[0].c as number
  }

  async outboxCount(commandId: string): Promise<number> {
    const rows = await interactiveSql`
      select count(*)::int as c from tunit_dispatcher_outbox where event_id = ${'evt-' + commandId}
    `
    return rows[0].c as number
  }

  async receipt(commandId: string): Promise<Record<string, any> | null> {
    const rows = await interactiveSql`
      select command_id, outcome, aggregate_id, message
      from workflow_command_receipt
      where command_id = ${commandId}
      limit 1
    `
    return rows[0] ?? null
  }

  async cleanup(): Promise<void> {
    // Helper tables may not exist if a test failed before ensureTestTables;
    // guard each delete like the shared persistence harness does.
    try {
      await interactiveSql`delete from tunit_dispatcher_effect where tenant_id = ${this.tenantId}`
    } catch {
      /* table absent */
    }
    try {
      await interactiveSql`delete from tunit_dispatcher_outbox where tenant_id = ${this.tenantId}`
    } catch {
      /* table absent */
    }
    for (const id of this.commandIds) {
      await interactiveSql`delete from workflow_command_receipt where command_id = ${id}`
    }
    await interactiveSql`drop table if exists tunit_dispatcher_effect`
    await interactiveSql`drop table if exists tunit_dispatcher_outbox`
  }
}

function envelope(
  commandId: string,
  commandType: string,
  overrides: Partial<CommandEnvelope> = {},
): CommandEnvelope {
  return {
    commandId,
    commandType,
    actorAppUserId: null,
    aggregateType: 'deal',
    // Valid UUID: workflow_command_receipt.aggregate_id is a uuid column.
    aggregateId: '00000000-0000-4000-8000-000000000001',
    correlationId: 'corr-persist',
    causationId: 'cause-parent',
    requestedAt: '2026-08-22T00:00:00.000Z',
    input: {},
    ...overrides,
  } as CommandEnvelope
}

test('CRM-14J: mutation + receipt + outbox rows commit atomically; same commandId replays once (real Postgres)', async () => {
  const f = new CommandLayerPersistenceFixture()
  await f.assertReceiptTable()
  await f.ensureTestTables()
  const cmd1 = f.newCommandId('cmd-1')
  try {
    const registry = new InMemoryCommandRegistry()
    f.registerSetStageHandler(registry)
    const dispatcher = f.makeDispatcher(registry, f.makeEventSink())

    // First execution: business mutation + receipt + outbox row all commit.
    const first = await dispatcher.execute(envelope(cmd1, 'test.set_stage'))
    assert.equal(first.outcome, 'success')
    assert.equal(first.replayed, false)
    assert.equal(first.receiptId, cmd1)
    assert.equal(await f.effectCount(cmd1), 1)
    assert.equal(await f.outboxCount(cmd1), 1)
    const receipt = await f.receipt(cmd1)
    assert.equal(receipt?.outcome, 'success')

    // Second execution with the same commandId: replay, nothing duplicated.
    const replay = await dispatcher.execute(envelope(cmd1, 'test.set_stage'))
    assert.equal(replay.outcome, 'success')
    assert.equal(replay.replayed, true)
    assert.equal(await f.effectCount(cmd1), 1, 'mutation applied exactly once')
    assert.equal(await f.outboxCount(cmd1), 1, 'outbox row written exactly once')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14J: failure after mutation but before commit rolls back mutation + receipt + outbox (real Postgres)', async () => {
  const f = new CommandLayerPersistenceFixture()
  await f.assertReceiptTable()
  await f.ensureTestTables()
  const cmd2 = f.newCommandId('cmd-2')
  try {
    const registry = new InMemoryCommandRegistry()
    f.registerBoomHandler(registry)
    const dispatcher = f.makeDispatcher(registry, f.makeEventSink())

    await assert.rejects(() => dispatcher.execute(envelope(cmd2, 'test.boom')))

    assert.equal(await f.effectCount(cmd2), 0, 'no durable business mutation')
    assert.equal(await f.outboxCount(cmd2), 0, 'no durable outbox event')
    assert.equal(await f.receipt(cmd2), null, 'no success receipt')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14J: infra failure on first attempt -> rollback -> same commandId retry re-executes exactly once (real Postgres)', async () => {
  const f = new CommandLayerPersistenceFixture()
  await f.assertReceiptTable()
  await f.ensureTestTables()
  const cmd3 = f.newCommandId('cmd-3')
  try {
    const registry = new InMemoryCommandRegistry()
    let armed = true
    const self = f
    registry.register('test.flaky', {
      async handle(envelope: CommandEnvelope, ctx: any) {
        const claimed = await ctx.receipts.claim(envelope.commandId, ctx.tx)
        if (!claimed) {
          const receipt = await ctx.receipts.find(envelope.commandId, ctx.tx)
          const decision = replayOutcome(
            receipt
              ? {
                  commandId: receipt.commandId,
                  outcome: receipt.outcome,
                  aggregateId: receipt.aggregateId,
                  message: receipt.message,
                }
              : null,
          )
          return {
            commandId: envelope.commandId,
            outcome: decision.outcome,
            emittedEvents: [],
            aggregateId: receipt?.aggregateId ?? null,
            message: decision.message,
            replayed: true,
          }
        }
        await ctx.tx`
          insert into tunit_dispatcher_effect (command_id, tenant_id, effect_count)
          values (${envelope.commandId}, ${self.tenantId}, 1)
        `
        await ctx.receipts.save(
          {
            commandId: envelope.commandId,
            outcome: 'success',
            status: 'Succeeded',
            aggregateId: envelope.aggregateId ?? null,
            message: null,
            createdAt: null,
          },
          ctx.tx,
        )
        ctx.events.add(
          createDomainEventFromCommand(envelope, {
            eventType: 'DEAL_STAGE_CHANGED',
            payload: {},
            eventId: `evt-${envelope.commandId}`,
          }),
        )
        if (armed) {
          armed = false
          throw new Error('simulated infrastructure failure on first attempt')
        }
        return {
          commandId: envelope.commandId,
          outcome: 'success',
          emittedEvents: [],
          aggregateId: envelope.aggregateId ?? null,
          message: null,
          replayed: false,
        }
      },
    })
    const dispatcher = f.makeDispatcher(registry, f.makeEventSink())

    // Attempt 1: mutates + receipts + emits, then fails BEFORE commit.
    await assert.rejects(() => dispatcher.execute(envelope(cmd3, 'test.flaky')))
    assert.equal(await f.effectCount(cmd3), 0, 'first attempt rolled back')
    assert.equal(await f.outboxCount(cmd3), 0, 'first attempt outbox rolled back')
    assert.equal(await f.receipt(cmd3), null, 'no receipt from the rolled-back attempt')

    // Attempt 2: SAME commandId re-submitted — a fresh valid execution, not a
    // replay (nothing ever committed), mutation applied exactly once total.
    const retry = await dispatcher.execute(envelope(cmd3, 'test.flaky'))
    assert.equal(retry.outcome, 'success')
    assert.equal(retry.replayed, false)
    assert.equal(await f.effectCount(cmd3), 1, 'mutation applied exactly once across both attempts')
    assert.equal(await f.outboxCount(cmd3), 1, 'outbox event committed with the retry')
    assert.equal((await f.receipt(cmd3))?.outcome, 'success')
  } finally {
    await f.cleanup()
  }
})

test('CRM-14J: correlationId/causationId survive into the committed outbox row (real Postgres)', async () => {
  const f = new CommandLayerPersistenceFixture()
  await f.assertReceiptTable()
  await f.ensureTestTables()
  const cmd4 = f.newCommandId('cmd-4')
  try {
    const registry = new InMemoryCommandRegistry()
    f.registerSetStageHandler(registry)
    const dispatcher = f.makeDispatcher(registry, f.makeEventSink())

    await dispatcher.execute(
      envelope(cmd4, 'test.set_stage', {
        correlationId: 'corr-persist-4',
        causationId: 'parent-cause-4',
      }),
    )

    const rows = await interactiveSql`
      select event_id, event_type, correlation_id, causation_id
      from tunit_dispatcher_outbox
      where event_id = ${'evt-' + cmd4}
    `
    assert.equal(rows.length, 1)
    assert.equal(rows[0].correlation_id, 'corr-persist-4', 'event carries the command correlationId')
    assert.equal(rows[0].causation_id, cmd4, 'the command caused the fact (causationId = commandId)')
  } finally {
    await f.cleanup()
  }
})
