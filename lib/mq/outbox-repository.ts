// ---------------------------------------------------------------------------
// MQ-01 — Postgres outbox repository: the durable implementation of the
// CRM-14J `OutboxEventRepository` contract.
//
// append() runs in the SAME application transaction as the canonical business
// mutation + command receipt (the CommandDispatcher eventSink hook) — a
// committed outbox message becomes available for delivery; a rolled-back
// transaction leaves nothing. markDelivered/markFailed/claimBatch are contract
// shims over the SINGLE delivery-state table (mq_delivery) — no second
// tracking system exists.
// ---------------------------------------------------------------------------

import type { DomainEvent } from '../commands/contracts'
import type { QueryExecutor } from '../../db/query-executor'
import type {
  OutboxEventRepository,
  OutboxRecord,
} from '../events/outbox-contracts'

type OutboxRow = {
  id: string
  event_type: string
  aggregate_type: string | null
  aggregate_id: string | null
  correlation_id: string | null
  causation_id: string | null
  occurred_at: string
  payload: Record<string, unknown>
}

function mapOutbox(row: OutboxRow & Record<string, unknown>): OutboxRecord {
  return {
    eventId: row.id,
    eventType: row.event_type,
    aggregateType: row.aggregate_type,
    aggregateId: row.aggregate_id,
    correlationId: row.correlation_id,
    causationId: row.causation_id,
    occurredAt: row.occurred_at,
    payload: row.payload,
    status: (row.state as OutboxRecord['status']) ?? 'pending',
    attempts: Number(row.attempt_count ?? 0),
    leaseUntil: (row.lease_until as string) ?? null,
    lockedBy: (row.claimed_by as string) ?? null,
    nextAttemptAt: (row.available_at as string) ?? null,
    lastError: (row.last_error as string) ?? null,
  }
}

export class PostgresOutboxEventRepository implements OutboxEventRepository {
  constructor(
    private readonly executor: () => Promise<QueryExecutor> = defaultExecutor,
  ) {}

  /** Append committed domain events in the business transaction (idempotent:
   * a replayed already-committed event inserts nothing). */
  async append(events: DomainEvent[], tx: QueryExecutor): Promise<void> {
    for (const event of events) {
      await tx`
        insert into outbox_message (
          id, event_type, aggregate_type, aggregate_id,
          correlation_id, causation_id, actor_app_user_id,
          occurred_at, payload
        ) values (
          ${event.eventId}, ${event.eventType}, ${event.aggregateType ?? null},
          ${event.aggregateId}, ${event.correlationId ?? null},
          ${event.causationId ?? null}, ${event.actorAppUserId ?? null},
          ${event.occurredAt}, ${JSON.stringify(event.payload)}::jsonb
        )
        on conflict (id) do nothing
      `
    }
  }
  /** Contract shim: claim up to `limit` due deliveries under a durable lease.
   * Returns the underlying outbox rows with the delivery lease mapped on. */
  async claimBatch(
    workerId: string,
    limit: number,
    leaseUntil: Date,
  ): Promise<OutboxRecord[]> {
    const q = await this.executor()
    await q`
      insert into mq_delivery (message_id, subscription_id)
      select m.id, s.id
      from outbox_message m
      join mq_subscription s on s.routing_key = m.event_type and s.enabled
      on conflict (message_id, subscription_id) do nothing
    `
    const rows = await q`
      update mq_delivery d
      set state = 'claimed',
          claimed_at = now(),
          claimed_by = ${workerId},
          lease_until = ${leaseUntil.toISOString()},
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
      returning id, message_id, state, attempt_count, lease_until, claimed_by, available_at, last_error,
        subscription_id, created_at, updated_at
    `
    return Promise.all(
      (rows as unknown as Array<{ message_id: string }>).map(async (r) => {
        const msg = await q`
          select id, event_type, aggregate_type, aggregate_id, correlation_id,
            causation_id, occurred_at, payload
          from outbox_message where id = ${r.message_id}
        `
        return mapOutbox(msg[0] as OutboxRow & Record<string, unknown>)
      }),
    )
  }

  /** Contract shim: mark a delivery acknowledged for one subscription. */
  async markDelivered(eventId: string, subscriberId: string): Promise<void> {
    const q = await this.executor()
    await q`
      update mq_delivery
      set state = 'delivered',
          acknowledged_at = now(),
          lease_until = null,
          claimed_at = null,
          claimed_by = null,
          last_error = null,
          updated_at = now()
      where message_id = ${eventId}
        and subscription_id = ${subscriberId}
        and state = 'claimed'
    `
  }

  /** Contract shim: record a failed delivery attempt (bounded retry / dead). */
  async markFailed(
    eventId: string,
    subscriberId: string,
    error: string,
    nextAttemptAt?: Date,
  ): Promise<void> {
    const q = await this.executor()
    const row = await q`
      select attempt_count, max_attempts
      from mq_delivery
      where message_id = ${eventId}
        and subscription_id = ${subscriberId}
      limit 1
    `
    const attempt = Number((row[0] as { attempt_count?: number } | undefined)?.attempt_count ?? 0)
    const maxAttempts = Number((row[0] as { max_attempts?: number } | undefined)?.max_attempts ?? 5)
    const terminal = attempt >= maxAttempts
    await q`
      update mq_delivery
      set state = ${terminal ? 'dead' : 'failed'},
          last_error = ${error.slice(0, 2000)},
          lease_until = null,
          claimed_at = null,
          claimed_by = null,
          available_at = case when ${terminal} then available_at
            else coalesce(${nextAttemptAt?.toISOString() ?? null}::timestamptz, now() + interval '30 seconds') end,
          updated_at = now()
      where message_id = ${eventId}
        and subscription_id = ${subscriberId}
        and state = 'claimed'
    `
  }
}

/** Default executor: the shared application DB handle. */
async function defaultExecutor(): Promise<QueryExecutor> {
  const client = await import('../../db/client')
  return client.sql as unknown as QueryExecutor
}

