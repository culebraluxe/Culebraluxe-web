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
import { neonTx } from '../../db/tx'
import type {
  CommandDispatcher,
  CommandResult,
  TypedCommandEnvelope,
} from './contracts'
import { CommandDispatcherImpl } from './dispatcher'
import { createCommandRegistry } from './register'

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

/** Build a dispatcher bound to the Postgres V1 transport and the canonical registry. */
export function createCommandDispatcher(): CommandDispatcherImpl {
  return new CommandDispatcherImpl({
    registry: createCommandRegistry(),
    receipts: new PostgresCommandReceiptRepository(),
    run: neonTx,
  })
}

/**
 * The application-wide canonical dispatcher — the single seam UI/workflow/API/
 * agent callers share. (Workflow_app routes through this via command-router.)
 */
export const commandDispatcher: CommandDispatcherImpl = createCommandDispatcher()

/** Convenience facade: execute an envelope through the canonical dispatcher. */
export function executeCommand<TPayload extends Record<string, unknown>>(
  envelope: TypedCommandEnvelope<TPayload>,
): Promise<CommandResult> {
  return commandDispatcher.execute(envelope)
}
