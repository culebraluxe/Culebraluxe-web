// ---------------------------------------------------------------------------
// CRM-14J — Shared result/error helpers for canonical command handlers.
//
// Transport-level mapping only: turning a canonical service outcome or a
// PortalWriteError into the normalized CommandResult shape. These helpers
// carry NO business rules — domain validation stays in the canonical services.
// ---------------------------------------------------------------------------

import type {
  CommandEnvelope,
  CommandResult,
  CommandOutcome,
} from '../workflow/contracts'
import { PortalWriteError } from '../portal-write-error'

/** Map a thrown PortalWriteError onto the existing CommandOutcome vocabulary. */
export function outcomeFromPortalWriteError(err: unknown): CommandOutcome {
  if (err instanceof PortalWriteError) {
    switch (err.code) {
      case 'conflict':
        return 'conflict'
      case 'not-found':
        return 'not_found'
      case 'validation':
        return 'validation_failure'
    }
  }
  return 'precondition_failure'
}

/** Build a success result for commands that do not return a richer value. */
export function successResult(
  envelope: CommandEnvelope,
  aggregateId: string | null,
): CommandResult {
  return {
    commandId: envelope.commandId,
    outcome: 'success',
    emittedEvents: [],
    aggregateId,
    message: null,
    replayed: false,
  }
}

/** Build a failure result from a caught error (task-style commands). */
export function failureResult(
  envelope: CommandEnvelope,
  err: unknown,
  aggregateId: string | null,
  fallbackMessage: string,
): CommandResult {
  return {
    commandId: envelope.commandId,
    outcome: outcomeFromPortalWriteError(err),
    emittedEvents: [],
    aggregateId,
    message: err instanceof Error ? err.message : fallbackMessage,
    replayed: false,
  }
}
