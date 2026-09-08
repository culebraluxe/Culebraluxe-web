// -----------------------------------------------------------------------------
// Neon-backed ServiceErrorSink. The domain kernel (services/core) never imports
// capture code; callers compose CoreServices with this sink via ServiceInfrastructure.errors.
// Maps an unhandled service exception (already domain/operation/correlation tagged
// by BaseService) into a durable app_error row at 'error' severity. Best-effort:
// captureError never throws into the caller.
// -----------------------------------------------------------------------------
import { captureError } from '../db/app-error'
import type { ServiceErrorSink } from '../services/core'

/** Build a ServiceErrorSink that writes to the durable app_error table. */
export function createNeonServiceErrorSink(): ServiceErrorSink {
  return {
    record: async (failure) => {
      captureError({
        kind: `service:${failure.code}`,
        operation: `service:${failure.domain}:${failure.operation}`,
        message: failure.message,
        stack: failure.stack,
        storyId: null,
        route: null,
        level: 'error',
      })
    },
  }
}

let sharedSink: ServiceErrorSink | null = null

/** Lazily-created app-wide singleton so every composed kernel shares one sink. */
export function appServiceErrorSink(): ServiceErrorSink {
  if (!sharedSink) sharedSink = createNeonServiceErrorSink()
  return sharedSink
}
