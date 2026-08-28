// ---------------------------------------------------------------------------
// CRM-14J — Canonical business command seam (public entry).
//
// UI / workflow / API / agent / recovery callers all converge on the same
// dispatcher: build a CommandEnvelope and execute it. The dispatcher owns the
// transaction, the replay fast-path, the receipt boundary and the (deferred)
// outbox handoff; canonical domain services own truth and rules.
//
// Importing this module is safe without a DATABASE_URL: the Neon pool and the
// receipt repository are resolved lazily on first use (db/tx, db/client).
// ---------------------------------------------------------------------------

import { PostgresCommandReceiptRepository } from '../../db/command-receipt-repository'
import { PostgresOutboxEventRepository } from '../mq/outbox-repository'
import { neonTx } from '../../db/tx'
import type {
  CommandDispatcher,
  CommandResult,
  TypedCommandEnvelope,
} from './contracts'
import type { CommandDispatcherOptions } from './dispatcher'
import { CommandDispatcherImpl } from './dispatcher'
import { createCommandRegistry } from './register'
import { flightRecorder } from '../../db/workflow-trace'

export type { CommandDispatcher }
export {
  CommandDispatcherImpl,
} from './dispatcher'
export {
  createCommandRegistry,
  registerCanonicalCommands,
} from './register'
export {
  InMemoryCommandRegistry,
} from './registry'
export {
  InMemoryDomainEventCollector,
  createDomainEventFromCommand,
} from './domain-events'
export { commandReceiptStatus } from './contracts'
export { PostgresCommandReceiptRepository } from '../../db/command-receipt-repository'

/**
 * Build a dispatcher bound to the Postgres V1 transport and the canonical registry.
 *
 * CRM-27 (BLOCKER 1): every production dispatcher is wired with the REAL
 * PostgresOutboxEventRepository as its durable event sink, so emitted DomainEvents
 * (e.g. AGREEMENT_FULLY_EXECUTED) are appended to `outbox_message` in the SAME
 * transaction as the canonical mutation + command receipt — the lost-event defect
 * cannot recur. `overrides` exist for tests to swap the run/receipts/sink seams;
 * the production default (below) is the outbox-enabled construction every
 * application caller shares.
 */
export function createCommandDispatcher(
  overrides: Partial<CommandDispatcherOptions> = {},
): CommandDispatcherImpl {
  return new CommandDispatcherImpl({
    registry: createCommandRegistry(),
    receipts: new PostgresCommandReceiptRepository(),
    run: neonTx,
    eventSink: new PostgresOutboxEventRepository(),
    ...overrides,
  })
}

/**
 * The application-wide canonical dispatcher — the single seam UI/workflow/API/
 * agent callers share. (Workflow_app routes through this via command-router.)
 *
 * It is wired with the observer-only Flight Recorder so every command records
 * COMMAND_RECEIVED / COMPLETED / FAILED + DOMAIN_EVENT_EMITTED trace evidence.
 * The recorder is contained (never throws) and replay-safe, so it can never
 * gate or slow-lock a business transaction.
 */
export const commandDispatcher: CommandDispatcherImpl = createCommandDispatcher({
  traceRecorder: flightRecorder.record,
})

/** Convenience facade: execute an envelope through the canonical dispatcher. */
export function executeCommand<TPayload extends Record<string, unknown>>(
  envelope: TypedCommandEnvelope<TPayload>,
): Promise<CommandResult> {
  return commandDispatcher.execute(envelope)
}
