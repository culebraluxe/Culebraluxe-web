// ---------------------------------------------------------------------------
// MQ-01 — Durable Postgres Message Broker V1 (public surface).
// ---------------------------------------------------------------------------

export { PostgresMessageBroker } from './broker'
export { PostgresOutboxEventRepository } from './outbox-repository'
export { MqProofConsumer } from './proof-consumer'
export type {
  MqConsumer,
  MqDeliveryContext,
  MqDeliveryState,
  MqDispatchSummary,
  MqMessage,
} from './types'
