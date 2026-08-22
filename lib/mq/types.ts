// ---------------------------------------------------------------------------
// MQ-01 — Durable Postgres Message Broker V1: contracts.
//
// MESSAGE = committed fact (outbox_message row). DELIVERY = transport state
// per (message x subscription). CONSUMER = one independent business reaction.
//
// Transport truth only: no broker table is canonical CRM/workflow truth.
// ---------------------------------------------------------------------------

/** A committed message handed to a consumer. Stable identity + routing key +
 * payload + correlation metadata. The payload is the canonical outbox copy —
 * consumers never receive raw database transaction internals. */
export type MqMessage = {
  messageId: string
  routingKey: string
  payload: Record<string, unknown>
  occurredAt: string
  correlationId: string | null
  causationId: string | null
  aggregateType: string | null
  aggregateId: string | null
}

/** Per-delivery context the consumer can use for idempotency/evidence. */
export type MqDeliveryContext = {
  deliveryId: string
  subscriptionId: string
  attempt: number
  maxAttempts: number
}

/**
 * Consumer contract: one subscription, one exact routing key, bounded attempts.
 * `handle` performs the business reaction through existing canonical seams.
 * A throw = a failed attempt (bounded retry, then dead). Duplicate delivery is
 * possible under at-least-once; consumers use existing idempotency /
 * command-receipt / correlation facilities to avoid duplicate business effects.
 */
export interface MqConsumer {
  readonly subscriptionId: string
  readonly routingKey: string
  readonly maxAttempts: number
  /** Durable delay (seconds) before the next attempt after a failure. */
  readonly retryBackoffSeconds: number
  handle(message: MqMessage, ctx: MqDeliveryContext): Promise<void>
}

export type MqDeliveryState =
  | 'pending'
  | 'claimed'
  | 'delivered'
  | 'failed'
  | 'dead'

/** One pass over the queue (callable worker + poller entry point). */
export type MqDispatchSummary = {
  claimed: number
  delivered: number
  failed: number
  deferred: number
  deadLettered: number
}
