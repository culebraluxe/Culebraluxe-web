// ---------------------------------------------------------------------------
// MQ-01 — broker poller entry point: ONE dispatch pass.
//
//   pnpm mq:worker
//
// Registers the proof consumer and runs one claim->deliver->ack/retry pass.
// Reuse via launchd/cron for a durable 5-minute poll (like agent-worker).
// No daemon; each invocation is one bounded pass.
// ---------------------------------------------------------------------------

import { interactiveSql } from '../lib/neon-interactive'
import type { QueryExecutor } from '../db/query-executor'
import { PostgresMessageBroker } from '../lib/mq/broker'
import { MqProofConsumer } from '../lib/mq/proof-consumer'

async function main(): Promise<void> {
  const executor = async () => interactiveSql as unknown as QueryExecutor
  const broker = new PostgresMessageBroker({ executor })
  await broker.registerConsumer(new MqProofConsumer(executor))
  const summary = await broker.dispatchOnce('mq-worker')
  console.log('MQ-01 dispatch pass:', JSON.stringify(summary))
}

main().catch((e) => {
  console.error(String(e).slice(0, 2000))
  process.exit(1)
})
