// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
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
import type { QueryExecutor } from '@/legacy/db/query-executor'
import { PostgresMessageBroker } from '../lib/mq/broker'
import { MqProofConsumer } from '../lib/mq/proof-consumer'
import { createCrm26Consumer } from '../lib/agreements/crm26-consumer'

async function main(): Promise<void> {
  const executor = async () => interactiveSql as unknown as QueryExecutor
  const broker = new PostgresMessageBroker({ executor })

  // Proof consumer (diagnostic) + CRM-26 agreement-execution consumer (production
  // business reaction). CRM-26 registers its stable subscription
  // `crm26-agreement-execution` and routes ONLY the exact AGREEMENT_FULLY_EXECUTED
  // event type. The CRM-26 consumer is part of the REAL worker composition so the
  // deployed worker loads it — not only tests/factories. Run with APP_ENV=production
  // (pnpm mq:worker:prod) so the broker and the canonical dispatcher both target
  // the production Neon database.
  await broker.registerConsumer(new MqProofConsumer(executor))
  const crm26 = await createCrm26Consumer()
  await broker.registerConsumer(crm26)

  const summary = await broker.dispatchOnce('mq-worker')
  console.log('MQ-01 dispatch pass:', JSON.stringify(summary))
}

main().catch((e) => {
  console.error(String(e).slice(0, 2000))
  process.exit(1)
})
