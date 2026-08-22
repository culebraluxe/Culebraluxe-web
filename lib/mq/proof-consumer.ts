// ---------------------------------------------------------------------------
// MQ-01 — Proof consumer.
//
// Writes NON-canonical diagnostic evidence (mq_proof_effect) for one routing
// key. Idempotent under at-least-once delivery: the effect row is keyed by
// message_id, so a duplicate delivery/replay can never create a duplicate
// business effect (here: a duplicate diagnostic row). This consumer's effect
// is explicitly diagnostic — it is not canonical CRM/workflow truth.
// ---------------------------------------------------------------------------

import type { QueryExecutor } from '../../db/query-executor'
import type { MqConsumer, MqDeliveryContext, MqMessage } from './types'

export class MqProofConsumer implements MqConsumer {
  constructor(
    private readonly executor: () => Promise<QueryExecutor>,
    private readonly options: {
      subscriptionId?: string
      routingKey?: string
      maxAttempts?: number
      retryBackoffSeconds?: number
    } = {},
  ) {}

  readonly subscriptionId = this.options.subscriptionId ?? 'mq-proof'
  readonly routingKey = this.options.routingKey ?? 'mq.proof'
  readonly maxAttempts = this.options.maxAttempts ?? 3
  readonly retryBackoffSeconds = this.options.retryBackoffSeconds ?? 1

  async handle(message: MqMessage, ctx: MqDeliveryContext): Promise<void> {
    const q = await this.executor()
    await q`
      insert into mq_proof_effect (
        message_id, subscription_id, routing_key, attempt
      ) values (
        ${message.messageId}, ${ctx.subscriptionId}, ${message.routingKey}, ${ctx.attempt}
      )
      on conflict (message_id) do nothing
    `
  }
}
