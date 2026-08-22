// ---------------------------------------------------------------------------
// MQ-01 — Durable Postgres Message Broker V1: SCOPED focused tests (5).
//
// Dangerous semantics only (per story test policy):
//   1. COMMIT VISIBILITY + FAN-OUT + INDEPENDENT FAILURE + ACK
//   2. ROLLBACK SAFETY (rolled-back tx produces no deliverable message)
//   3. RETRY (durable backoff timing, bounded attempts, non-empty last_error)
//   4. CONCURRENT CLAIM (two workers cannot double-claim the same delivery)
//   5. LEASE RECOVERY + DUPLICATE SAFETY (stale claim reclaim + idempotent
//      proof-consumer effect under duplicate delivery)
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'

import { interactiveSql } from '../../../lib/neon-interactive'
import type { QueryExecutor } from '../../../db/query-executor'
import type { DomainEvent } from '../../../lib/commands/contracts'
import type {
  MqConsumer,
  MqDeliveryContext,
  MqMessage,
} from '../../../lib/mq/types'
import { PostgresMessageBroker } from '../../../lib/mq/broker'
import { PostgresOutboxEventRepository } from '../../../lib/mq/outbox-repository'
import { MqProofConsumer } from '../../../lib/mq/proof-consumer'

const executor = interactiveSql as unknown as QueryExecutor
const dbExecutor = () => Promise.resolve(executor)
let seq = 0

async function resetMq(): Promise<void> {
  await interactiveSql`delete from mq_delivery`
  await interactiveSql`delete from mq_proof_effect`
  await interactiveSql`delete from mq_subscription`
  await interactiveSql`delete from outbox_message`
}

function makeEvent(eventType = 'mq.test.event'): DomainEvent {
  seq += 1
  return {
    eventId: randomUUID(),
    eventType,
    occurredAt: new Date().toISOString(),
    actorAppUserId: null,
    aggregateType: 'TestAggregate',
    aggregateId: 'agg-1',
    correlationId: 'corr-1',
    causationId: 'cmd-1',
    payload: { note: 'mq-01 proof', seq },
  }
}

async function appendCommitted(event: DomainEvent): Promise<void> {
  const repo = new PostgresOutboxEventRepository(dbExecutor)
  await interactiveSql.begin((tx) => repo.append([event], tx))
}

function failConsumer(
  subscriptionId: string,
  error = 'consumer boom',
  maxAttempts = 1,
): MqConsumer {
  return {
    subscriptionId,
    routingKey: 'mq.test.event',
    maxAttempts,
    retryBackoffSeconds: 1,
    async handle(): Promise<void> {
      throw new Error(error)
    },
  }
}

function okConsumer(subscriptionId: string, maxAttempts = 3): MqConsumer {
  return {
    subscriptionId,
    routingKey: 'mq.test.event',
    maxAttempts,
    retryBackoffSeconds: 1,
    async handle(): Promise<void> {
      // no-op: an ACK is the business effect for the fan-out proof
    },
  }
}

async function makeBroker(
  consumers: MqConsumer[],
  options: { leaseSeconds?: number; limit?: number } = {},
): Promise<PostgresMessageBroker> {
  const broker = new PostgresMessageBroker({
    executor: dbExecutor,
    leaseSeconds: options.leaseSeconds ?? 5,
    limit: options.limit ?? 20,
  })
  for (const c of consumers) await broker.registerConsumer(c)
  return broker
}

test('MQ-01 commit visibility + fan-out + independent failure + ack', async () => {
  await resetMq()
  const event = makeEvent()
  const broker = await makeBroker([
    failConsumer('mq-test-sub-a', 'sub-a boom', 1),
    okConsumer('mq-test-sub-b'),
  ])

  await appendCommitted(event)
  const summary = await broker.dispatchOnce('worker-1')

  const deliveries = await interactiveSql`
    select message_id, subscription_id, state, attempt_count, last_error,
      acknowledged_at
    from mq_delivery
    order by subscription_id
  `
  assert.equal(deliveries.length, 2)
  assert.equal(summary.claimed, 2)
  assert.equal(summary.delivered, 1)
  assert.equal(summary.deadLettered, 1)

  const a = deliveries.find((d: any) => d.subscription_id === 'mq-test-sub-a')
  const b = deliveries.find((d: any) => d.subscription_id === 'mq-test-sub-b')
  assert.equal(a.state, 'dead')
  assert.ok(String(a.last_error).includes('sub-a boom'))
  assert.equal(a.attempt_count, 1)
  assert.equal(a.acknowledged_at, null)
  assert.equal(b.state, 'delivered')
  assert.equal(b.last_error, null)
  assert.ok(b.acknowledged_at !== null)
})

test('MQ-01 rollback safety: rolled-back tx produces no deliverable message', async () => {
  await resetMq()
  const event = makeEvent()
  const repo = new PostgresOutboxEventRepository(dbExecutor)
  const broker = await makeBroker([okConsumer('mq-test-sub-rb')])

  class RollbackSentinel extends Error {}
  await assert.rejects(
    interactiveSql.begin(async (tx) => {
      await repo.append([event], tx)
      throw new RollbackSentinel('deliberate rollback')
    }),
    RollbackSentinel,
  )

  const summary = await broker.dispatchOnce('worker-1')
  const messages = await interactiveSql`
    select id from outbox_message where id = ${event.eventId}
  `
  assert.equal(messages.length, 0)
  assert.equal(summary.claimed, 0)
  const deliveries = await interactiveSql`select id from mq_delivery`
  assert.equal(deliveries.length, 0)
})


test('MQ-01 retry: durable backoff, bounded attempts, non-empty error, eventual ack', async () => {
  await resetMq()
  const event = makeEvent()
  let calls = 0
  const flaky: MqConsumer = {
    subscriptionId: 'mq-test-sub-flaky',
    routingKey: 'mq.test.event',
    maxAttempts: 3,
    retryBackoffSeconds: 1,
    async handle(_m: MqMessage, _c: MqDeliveryContext): Promise<void> {
      calls += 1
      if (calls < 2) throw new Error('transient blip')
    },
  }
  const broker = await makeBroker([flaky])
  await appendCommitted(event)

  const pass1 = await broker.dispatchOnce('worker-1')
  assert.equal(pass1.claimed, 1)
  assert.equal(pass1.failed, 1)
  const afterFail = await interactiveSql`
    select state, attempt_count, last_error, available_at from mq_delivery
    where subscription_id = 'mq-test-sub-flaky'
  `
  assert.equal(afterFail[0].state, 'failed')
  assert.equal(afterFail[0].attempt_count, 1)
  assert.ok(String(afterFail[0].last_error).includes('transient blip'))

  const pass2 = await broker.dispatchOnce('worker-1')
  assert.equal(pass2.claimed, 0)

  await interactiveSql`
    update mq_delivery set available_at = now() - interval '1 second'
    where subscription_id = 'mq-test-sub-flaky'
  `
  const pass3 = await broker.dispatchOnce('worker-1')
  assert.equal(pass3.claimed, 1)
  assert.equal(pass3.delivered, 1)
  assert.equal(calls, 2)
  const done = await interactiveSql`
    select state, attempt_count, last_error from mq_delivery
    where subscription_id = 'mq-test-sub-flaky'
  `
  assert.equal(done[0].state, 'delivered')
  assert.equal(done[0].attempt_count, 2)
  assert.equal(done[0].last_error, null)
})


test('MQ-01 concurrency: two workers cannot double-claim the same delivery', async () => {
  await resetMq()
  const event = makeEvent()
  let handled = 0
  const counting: MqConsumer = {
    subscriptionId: 'mq-test-sub-race',
    routingKey: 'mq.test.event',
    maxAttempts: 3,
    retryBackoffSeconds: 1,
    async handle(_m: MqMessage, _c: MqDeliveryContext): Promise<void> {
      handled += 1
    },
  }
  const w1 = await makeBroker([counting])
  const w2 = await makeBroker([counting])
  await appendCommitted(event)

  const [s1, s2] = await Promise.all([
    w1.dispatchOnce('worker-1'),
    w2.dispatchOnce('worker-2'),
  ])

  assert.equal(s1.claimed + s2.claimed, 1)
  assert.equal(s1.delivered + s2.delivered, 1)
  assert.equal(handled, 1)
  const row = await interactiveSql`
    select state, claimed_by, attempt_count from mq_delivery
    where subscription_id = 'mq-test-sub-race'
  `
  assert.equal(row[0].state, 'delivered')
  assert.equal(row[0].attempt_count, 1)
})



test('MQ-01 lease recovery + duplicate safety (idempotent proof effect)', async () => {
  await resetMq()
  const event = makeEvent()
  const proof = new MqProofConsumer(dbExecutor, {
    subscriptionId: 'mq-test-proof',
    routingKey: 'mq.test.event',
  })
  const broker = await makeBroker([proof])
  await appendCommitted(event)

  await interactiveSql`
    update mq_delivery
    set state = 'claimed', claimed_by = 'crashed-worker',
        lease_until = now() - interval '1 hour', attempt_count = 1,
        claimed_at = now() - interval '1 hour'
    where subscription_id = 'mq-test-proof'
  `

  const summary = await broker.dispatchOnce('worker-2')
  assert.equal(summary.claimed, 1)
  assert.equal(summary.delivered, 1)
  const row = await interactiveSql`
    select state, claimed_by, attempt_count from mq_delivery
    where subscription_id = 'mq-test-proof'
  `
  assert.equal(row[0].state, 'delivered')
  assert.equal(row[0].attempt_count, 1)

  const again = await broker.dispatchOnce('worker-2')
  assert.equal(again.claimed, 0)
  const effects = await interactiveSql`
    select message_id, attempt from mq_proof_effect
  `
  assert.equal(effects.length, 1)

  const msg = {
    messageId: event.eventId,
    routingKey: event.eventType,
    payload: event.payload,
    occurredAt: event.occurredAt,
    correlationId: event.correlationId,
    causationId: event.causationId,
    aggregateType: event.aggregateType,
    aggregateId: event.aggregateId,
  }
  await proof.handle(msg, {
    deliveryId: 'replayed-delivery',
    subscriptionId: 'mq-test-proof',
    attempt: 1,
    maxAttempts: 3,
  })
  const afterReplay = await interactiveSql`
    select message_id, attempt from mq_proof_effect
  `
  assert.equal(afterReplay.length, 1)
  assert.equal(afterReplay[0].message_id, event.eventId)
})
