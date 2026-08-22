// ---------------------------------------------------------------------------
// CRM-14J — Transactional Domain Event Outbox: contracts (DEFINED, NOT
// IMPLEMENTED).
//
// EVENT = FACT; OUTBOX = DURABLE HANDOFF; SUBSCRIBER = REACTION.
//
// This module is the compile-ready contract surface for the FUTURE outbox
// seam. Per the CRM-14I defer decision (docs/domain-event-persistence-
// decision.md) there is NO outbox table, NO dispatcher loop and NO subscriber
// registry today: no cross-cutting consumer (alerting/notifications, external
// integration, cross-workflow triggers, multiple independent subscribers,
// durable event-driven automation) exists yet. Build the implementation ONLY
// when a real consumer is being built in the same batch.
//
// Delivery semantics (invariants, from the architect brief):
//   - At-least-once delivery to each subscriber; subscriber idempotency keyed
//     by (eventId, subscriberId) via DeliveryReceiptRepository.
//   - Durable lease/claim (Postgres SKIP LOCKED when implemented) so a crashed
//     worker cannot silently drop events.
//   - Bounded retry, then terminal escalation/dead-letter state.
//   - A subscriber failure affects DELIVERY state only — it can never roll
//     back an already-committed business transaction (the event rows were
//     committed atomically with the business mutation + command receipt).
//   - Postgres is the V1 transport (transactionality, SKIP LOCKED/leases,
//     indexes, retry state, durable rows are proven in this system). Do not
//     pretend it is Kafka/RabbitMQ; an external broker is a future option and
//     would sit BELOW this contract (the interfaces stay; the transport
//     implementation changes).
// ---------------------------------------------------------------------------

import type { DomainEvent } from '../commands/contracts'
import type { ApplicationTransaction } from '../commands/contracts'

/** A durable outbox row (the future outbox table projection). */
export type OutboxRecord = {
  eventId: string
  eventType: string
  aggregateType: string | null
  aggregateId: string | null
  correlationId: string | null
  causationId: string | null
  occurredAt: string
  payload: Record<string, unknown>
  status: 'pending' | 'delivered' | 'failed' | 'dead_letter'
  attempts: number
  leaseUntil: string | null
  lockedBy: string | null
  nextAttemptAt: string | null
  lastError: string | null
}

/**
 * Durable outbox handoff. `append` runs in the SAME application transaction
 * as the canonical business mutation + command receipt (atomic commit: an
 * event can never outlive — or precede — the business fact it records).
 * Claim/dispatch happens AFTER commit by a separate worker.
 */
export interface OutboxEventRepository {
  /** Append committed events to the outbox within the business transaction. */
  append(events: DomainEvent[], tx: ApplicationTransaction): Promise<void>
  /**
   * Claim up to `limit` pending events for a worker under a durable lease
   * (implement with SKIP LOCKED when built). Claimed rows are the delivery
   * unit; a crashed worker's lease expires and the rows are reclaimable.
   */
  claimBatch(workerId: string, limit: number, leaseUntil: Date): Promise<OutboxRecord[]>
  /** Record delivery to a subscriber (idempotency key (eventId, subscriberId)). */
  markDelivered(eventId: string, subscriberId: string): Promise<void>
  /** Record a failed delivery attempt; schedule the next attempt / escalation. */
  markFailed(
    eventId: string,
    subscriberId: string,
    error: string,
    nextAttemptAt?: Date,
  ): Promise<void>
}

/** One independent reaction to committed facts. Subscriber failure never
 *  rolls back business truth — it only affects delivery state. */
export interface EventSubscriber<E extends DomainEvent = DomainEvent> {
  readonly subscriberId: string
  /** Type guard: does this subscriber react to this event? */
  supports(event: DomainEvent): event is E
  handle(event: E): Promise<void>
}

/** Summary of one outbox dispatch pass (worker invocation). */
export type DispatchSummary = {
  claimed: number
  delivered: number
  failed: number
  deferred: number
  deadLettered: number
}

/** The outbox worker: claim -> fan out to subscribers -> record delivery. */
export interface OutboxDispatcher {
  dispatchOnce(): Promise<DispatchSummary>
}

/**
 * Subscriber-side idempotency: an (eventId, subscriberId) pair is processed
 * at most once even under at-least-once delivery (duplicate dispatch,
 * retries, worker overlap).
 */
export interface DeliveryReceiptRepository {
  hasDelivered(eventId: string, subscriberId: string): Promise<boolean>
  recordDelivered(eventId: string, subscriberId: string): Promise<void>
}
