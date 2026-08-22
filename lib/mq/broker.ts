// ---------------------------------------------------------------------------
// MQ-01 — Durable Postgres Message Broker V1 runtime.
//
// ONE pass: materialize deliveries (outbox message x enabled subscription,
// exact routing-key match) -> claim due deliveries under a durable lease
// (SKIP LOCKED, no concurrent double-claim) -> handle each via its consumer ->
// ack / bounded retry / terminal dead. Transport state lives ONLY in
// mq_delivery; no business table is touched here.
// ---------------------------------------------------------------------------

import type { QueryExecutor } from '../../db/query-executor'
import type {
  MqConsumer,
  MqDeliveryContext,
  MqDispatchSummary,
  MqMessage,
} from './types'

/** Claim lease duration: an abandoned 'claimed' delivery becomes reclaimable
 * after this. (Mirrors the integration-inbox stale-claim window.) */
export const MQ_LEASE_SECONDS = 300

/** Default claim cap per pass. */
export const MQ_DEFAULT_LIMIT = 20

export type MqBrokerOptions = {
  executor: () => Promise<QueryExecutor>
  /** Lease duration seconds (tests shrink it). */
  leaseSeconds?: number
  /** Max deliveries claimed per pass. */
  limit?: number
}

function normalizeError(error: unknown): string {
  const raw = String((error as Error)?.message ?? error)
  const text = raw.trim()
  return (text || 'unknown consumer failure').slice(0, 2000)
}

type ClaimedDelivery = {
  id: string
  message_id: string
  subscription_id: string
  attempt_count: number
}

export class PostgresMessageBroker {
  private readonly consumers = new Map<string, MqConsumer>()

  constructor(private readonly options: MqBrokerOptions) {}

  /** Register a consumer (upserts its durable subscription row). */
  async registerConsumer(consumer: MqConsumer): Promise<void> {
    if (this.consumers.has(consumer.subscriptionId)) {
      throw new Error(`mq consumer already registered: ${consumer.subscriptionId}`)
    }
    const q = await this.options.executor()
    await q`
      insert into mq_subscription (
        id, routing_key, description, max_attempts, retry_backoff_seconds, enabled
      ) values (
        ${consumer.subscriptionId}, ${consumer.routingKey}, null,
        ${consumer.maxAttempts}, ${consumer.retryBackoffSeconds}, true
      )
      on conflict (id) do update set
        routing_key = excluded.routing_key,
        max_attempts = excluded.max_attempts,
        retry_backoff_seconds = excluded.retry_backoff_seconds,
        enabled = true
    `
    this.consumers.set(consumer.subscriptionId, consumer)
  }

  /**
   * Run ONE dispatch pass: materialize -> claim -> handle -> ack/retry/dead.
   * Safe to call from a poller; overlapping workers cannot double-claim (the
   * lease + SKIP LOCKED guard).
   */
  async dispatchOnce(workerId = 'mq-worker'): Promise<MqDispatchSummary> {
    const q = await this.options.executor()
    const limit = this.options.limit ?? MQ_DEFAULT_LIMIT
    const leaseSeconds = this.options.leaseSeconds ?? MQ_LEASE_SECONDS
    const summary: MqDispatchSummary = {
      claimed: 0,
      delivered: 0,
      failed: 0,
      deferred: 0,
      deadLettered: 0,
    }

    // 1. Materialize delivery rows for every committed message x enabled
    //    subscription whose routing key matches (exact match, V1). Idempotent:
    //    reprocessing never duplicates a delivery row.
    await q`
      insert into mq_delivery (message_id, subscription_id)
      select m.id, s.id
      from outbox_message m
      join mq_subscription s on s.routing_key = m.event_type and s.enabled
      on conflict (message_id, subscription_id) do nothing
    `

    // 2. Claim due deliveries under a durable lease. A pending/failed delivery
    //    whose available_at is due, or a claimed delivery whose lease expired
    //    (crashed worker), is claimed exactly once by one worker (SKIP LOCKED).
    const rows = await q`
      update mq_delivery d
      set state = 'claimed',
          claimed_at = now(),
          claimed_by = ${workerId},
          lease_until = now() + (${leaseSeconds} || ' seconds')::interval,
          attempt_count = attempt_count + 1,
          updated_at = now()
      where d.id in (
        select id from mq_delivery
        where (state = 'pending' and available_at <= now())
           or (state = 'failed' and available_at <= now())
           or (state = 'claimed' and lease_until <= now())
        order by available_at asc, id asc
        limit ${limit}
        for update skip locked
      )
      returning d.id, d.message_id, d.subscription_id, d.attempt_count
    `
    const claimed = rows as unknown as ClaimedDelivery[]
    summary.claimed = claimed.length

    // 3. Handle each claimed delivery INDEPENDENTLY (failure isolation: one
    //    failing subscription never blocks another for the same message).
    for (const delivery of claimed) {
      const consumer = this.consumers.get(delivery.subscription_id)
      if (!consumer) {
        summary.deferred++
        await q`
          update mq_delivery
          set state = 'failed',
              last_error = ${`mq consumer not registered in worker ${workerId}`},
              lease_until = null,
              claimed_at = null,
              claimed_by = null,
              available_at = now() + interval '60 seconds',
              updated_at = now()
          where id = ${delivery.id} and state = 'claimed'
        `
        continue
      }


      const message = await this.loadMessage(q, delivery.message_id)
      if (!message) {
        summary.failed++
        await q`
          update mq_delivery
          set state = 'dead',
              last_error = 'outbox message missing for delivery',
              lease_until = null,
              claimed_at = null,
              claimed_by = null,
              updated_at = now()
          where id = ${delivery.id} and state = 'claimed'
        `
        continue
      }

      const context: MqDeliveryContext = {
        deliveryId: delivery.id,
        subscriptionId: consumer.subscriptionId,
        attempt: delivery.attempt_count,
        maxAttempts: consumer.maxAttempts,
      }

      try {
        await consumer.handle(message, context)
        await q`
          update mq_delivery
          set state = 'delivered',
              acknowledged_at = now(),
              lease_until = null,
              claimed_at = null,
              claimed_by = null,
              last_error = null,
              updated_at = now()
          where id = ${delivery.id} and state = 'claimed'
        `
        summary.delivered++
      } catch (error) {
        const err = normalizeError(error)
        const terminal = delivery.attempt_count >= consumer.maxAttempts
        await q`
          update mq_delivery
          set state = ${terminal ? 'dead' : 'failed'},
              last_error = ${err},
              lease_until = null,
              claimed_at = null,
              claimed_by = null,
              available_at = case when ${terminal} then available_at
                else now() + (${consumer.retryBackoffSeconds} || ' seconds')::interval end,
              updated_at = now()
          where id = ${delivery.id} and state = 'claimed'
        `
        if (terminal) summary.deadLettered++
        else summary.failed++
      }
    }

    return summary
  }


  private async loadMessage(
    q: QueryExecutor,
    messageId: string,
  ): Promise<MqMessage | null> {
    const rows = await q`
      select id, event_type, payload, occurred_at, correlation_id, causation_id,
        aggregate_type, aggregate_id
      from outbox_message
      where id = ${messageId}
      limit 1
    `
    const row = rows[0] as
      | {
          id: string
          event_type: string
          payload: Record<string, unknown>
          occurred_at: string
          correlation_id: string | null
          causation_id: string | null
          aggregate_type: string | null
          aggregate_id: string | null
        }
      | undefined
    if (!row) return null
    return {
      messageId: row.id,
      routingKey: row.event_type,
      payload: row.payload,
      occurredAt: row.occurred_at,
      correlationId: row.correlation_id,
      causationId: row.causation_id,
      aggregateType: row.aggregate_type,
      aggregateId: row.aggregate_id,
    }
  }
}

