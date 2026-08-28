import { test } from 'node:test'
import assert from 'node:assert/strict'

import { recordTraceEvent, listTraceEvents } from '../../db/workflow-trace'
import { sanitizeMetadata } from '../../lib/workflow-trace'
import { CommandDispatcherImpl } from '../../lib/commands/dispatcher'
import type { CommandReceiptRepository, CommandRegistry, CommandResult } from '../../lib/commands/contracts'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'

// ---------------------------------------------------------------------------
// WORKFLOW-TRACE — Flight Recorder + command-dispatcher instrumentation.
// ---------------------------------------------------------------------------

function capturingExecutor() {
  const calls: { sql: string; params: unknown[] }[] = []
  const execute: QueryExecutor = ((strings: TemplateStringsArray, ...params: unknown[]) => {
    calls.push({ sql: strings.join('?'), params })
    return Promise.resolve([])
  }) as QueryExecutor
  return { execute, calls }
}

test('recordTraceEvent persists a trace event with replay source identity', async () => {
  const { execute, calls } = capturingExecutor()
  await recordTraceEvent(
    {
      eventType: 'NODE_ENTERED',
      system: 'workflow',
      occurredAt: '2026-08-28T14:00:00.000Z',
      workflowInstanceId: 'wf-1',
      workflowNodeId: 'task_a',
      correlationId: 'wf-1',
      causationId: 'evt-9',
      sourceSystem: 'workflow_engine',
      sourceEventId: 'node-enter-1',
      summary: 'Entered task_a',
    },
    execute,
  )
  assert.equal(calls.length, 1)
  const params = calls[0].params
  // source_system + source_event_id are the last two params -> replay backstop.
  assert.equal(params[params.length - 2], 'workflow_engine')
  assert.equal(params[params.length - 1], 'node-enter-1')
})

test('recorder failure is contained and never throws', async () => {
  const execute: QueryExecutor = (async () => {
    throw new Error('simulated DB outage')
  }) as QueryExecutor
  await assert.doesNotReject(
    recordTraceEvent({ eventType: 'FAILURE', system: 'workflow', occurredAt: '2026-08-28T14:00:00.000Z' }, execute),
  )
})

test('sanitizeMetadata strips credentials and caps nested depth', () => {
  const safe = sanitizeMetadata({
    dealId: 'deal-1',
    commandType: 'set-stage',
    token: 'abc123',
    auth: { Authorization: 'Bearer x', apiKey: 'k' },
    nested: { a: { b: { c: { d: { e: { f: { g: 'deep' } } } } } } },
    long: 'x'.repeat(800),
  })!
  assert.equal(safe.dealId, 'deal-1')
  assert.equal(safe.token, '[redacted]')
  assert.equal((safe.auth as Record<string, unknown>).Authorization, '[redacted]')
  assert.equal((safe.auth as Record<string, unknown>).apiKey, '[redacted]')
  assert.equal((safe.long as string).length, 500 + 1) // truncated + ellipsis
  assert.ok(safe.nested) // depth is capped, not a crash
})

test('listTraceEvents filters by workflow instance and returns normalized events', async () => {
  const rows = [
    {
      id: '1', trace_id: 't1', correlation_id: 'wf-1', causation_id: null,
      deal_id: null, person_id: null, property_id: null, transaction_document_id: null,
      workflow_instance_id: 'wf-1', workflow_definition_key: 'purchase', workflow_definition_version: 2,
      workflow_node_id: 'task_a', workflow_transition_id: null,
      event_type: 'NODE_ENTERED', system: 'workflow', occurred_at: new Date('2026-08-28T14:00:00.000Z'),
      completed_at: null, duration_ms: null, outcome: null,
      command_id: null, domain_event_id: null, task_id: null, timer_job_id: null, signature_request_id: null, external_reference: null,
      summary: 'Entered', metadata: null, source_system: 'engine', source_event_id: 'e1', recorded_at: new Date(),
    },
  ]
  const execute: QueryExecutor = (async () => rows) as QueryExecutor
  const events = await listTraceEvents({ workflowInstanceId: 'wf-1' }, execute)
  assert.equal(events.length, 1)
  assert.equal(events[0].eventType, 'NODE_ENTERED')
  assert.equal(events[0].workflowNodeId, 'task_a')
  assert.equal(events[0].occurredAt, '2026-08-28T14:00:00.000Z') // Date normalized to ISO string
  assert.equal(events[0].workflowDefinitionKey, 'purchase')
})

// ---------------------------------------------------------------------------
// Command dispatcher instrumentation — observer-only, failure-contained.
// ---------------------------------------------------------------------------

type Recorded = { eventType: string; commandId?: string | null; domainEventId?: string | null }

function makeDispatcher(recorder: ((input: Recorded) => Promise<void>) | null, failCommit = false) {
  const registry = {
    resolve: () => ({
      handle: async (): Promise<CommandResult> => ({
        commandId: 'cmd-1',
        outcome: 'success',
        emittedEvents: [
          { eventId: 'evt-1', eventType: 'DEAL_STAGE_CHANGED', occurredAt: '2026-08-28T14:00:05.000Z', actorAppUserId: null, aggregateType: 'deal', aggregateId: 'deal-1', correlationId: 'wf-1', causationId: 'cmd-1', payload: {} },
        ],
        aggregateId: 'deal-1',
        message: null,
        replayed: false,
      }),
    }),
  } as unknown as CommandRegistry
  const receipts = { find: async () => null } as unknown as CommandReceiptRepository
  const tx: QueryExecutor = (async () => []) as QueryExecutor
  const run: TxRunner = async (cb) => {
    if (failCommit) throw new Error('commit failed')
    return cb(tx)
  }
  const dispatcher = new CommandDispatcherImpl({
    registry,
    receipts,
    run,
    now: () => new Date('2026-08-28T14:00:00.000Z'),
    traceRecorder: recorder as unknown as NonNullable<ConstructorParameters<typeof CommandDispatcherImpl>[0]['traceRecorder']>,
  })
  return { dispatcher }
}

const ENVELOPE = {
  commandId: 'cmd-1', commandType: 'x', actorAppUserId: null, aggregateType: 'deal',
  aggregateId: 'deal-1', correlationId: 'wf-1', causationId: null,
  requestedAt: '2026-08-28T14:00:00.000Z', input: {},
} as never

test('dispatcher records COMMAND_RECEIVED / COMPLETED and DOMAIN_EVENT_EMITTED', async () => {
  const recorded: Recorded[] = []
  const { dispatcher } = makeDispatcher(async (r) => {
    recorded.push(r)
  })
  const result = await dispatcher.execute(ENVELOPE)
  assert.equal(result.outcome, 'success')
  assert.ok(recorded.some((r) => r.eventType === 'COMMAND_RECEIVED'))
  assert.ok(recorded.some((r) => r.eventType === 'COMMAND_COMPLETED'))
  assert.ok(recorded.some((r) => r.eventType === 'DOMAIN_EVENT_EMITTED' && r.domainEventId === 'evt-1'))
})

test('a throwing recorder never breaks the command', async () => {
  const { dispatcher } = makeDispatcher(async () => {
    throw new Error('recorder blew up')
  })
  const result = await dispatcher.execute(ENVELOPE)
  assert.equal(result.outcome, 'success')
})

test('dispatcher without a recorder is a no-op (no behavior change)', async () => {
  const { dispatcher } = makeDispatcher(null)
  const result = await dispatcher.execute(ENVELOPE)
  assert.equal(result.outcome, 'success')
})

test('dispatcher records COMMAND_FAILED on infrastructure failure and rethrows', async () => {
  const recorded: Recorded[] = []
  const { dispatcher } = makeDispatcher(async (r) => {
    recorded.push(r)
  }, true)
  await assert.rejects(() => dispatcher.execute(ENVELOPE))
  assert.ok(recorded.some((r) => r.eventType === 'COMMAND_FAILED'))
})

