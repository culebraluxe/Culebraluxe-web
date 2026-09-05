import {
  FORGE_ROUTED_COMMAND_TYPES,
  FORGE_RUN_APPEND_DETAIL,
  FORGE_STORY_MARK_COMPLETE,
  FORGE_STORY_MARK_HOLD,
  FORGE_STORY_MARK_IN_PROGRESS,
} from '../forge-command-types'
import type {
  ForgeCommandEnvelope,
  ForgeCommandHandler,
  ForgeCommandRegistry,
  ForgeCommandResult,
  ForgeStateWriter,
} from './forge-state-writer'
import { optionalString, requiredString } from './forge-state-writer'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 Stage 3 — Forge command handlers + registry (DB-free core).
//
// Handlers are thin adapters: envelope -> required/optional fields ->
// ForgeStateWriter call -> normalized result. NO business rules live here; they
// stay in the writer (canonical Forge DB services) or the caller's decision
// gate. Malformed input is a stable `validation_failure` result; a writer /
// infrastructure failure throws upward (the engine step rolls back — fail
// closed). This module imports no database and no RE registry.
// ---------------------------------------------------------------------------

function ok(commandType: string): ForgeCommandResult {
  return { commandType, outcome: 'success' }
}

function malformed(commandType: string, message: string): ForgeCommandResult {
  return { commandType, outcome: 'validation_failure', message }
}

function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : String(err)
}

function markHoldHandler(writer: ForgeStateWriter): ForgeCommandHandler {
  return {
    async handle(envelope) {
      let storyId: string
      let reason: string
      try {
        storyId = requiredString(envelope.input, 'storyId')
        reason = optionalString(envelope.input, 'reason')
      } catch (err) {
        return malformed(FORGE_STORY_MARK_HOLD, messageOf(err))
      }
      await writer.markStoryHumanHold(storyId, reason || 'Forge engine hold command')
      return ok(FORGE_STORY_MARK_HOLD)
    },
  }
}

function markCompleteHandler(writer: ForgeStateWriter): ForgeCommandHandler {
  return {
    async handle(envelope) {
      let storyId: string
      try {
        storyId = requiredString(envelope.input, 'storyId')
      } catch (err) {
        return malformed(FORGE_STORY_MARK_COMPLETE, messageOf(err))
      }
      await writer.markStoryComplete(storyId)
      return ok(FORGE_STORY_MARK_COMPLETE)
    },
  }
}

function markInProgressHandler(writer: ForgeStateWriter): ForgeCommandHandler {
  return {
    async handle(envelope) {
      let storyId: string
      try {
        storyId = requiredString(envelope.input, 'storyId')
      } catch (err) {
        return malformed(FORGE_STORY_MARK_IN_PROGRESS, messageOf(err))
      }
      await writer.markStoryInProgress(storyId)
      return ok(FORGE_STORY_MARK_IN_PROGRESS)
    },
  }
}

function appendRunDetailHandler(writer: ForgeStateWriter): ForgeCommandHandler {
  return {
    async handle(envelope) {
      let runId: string
      let detail: string
      try {
        runId = requiredString(envelope.input, 'runId')
        detail = requiredString(envelope.input, 'detail')
      } catch (err) {
        return malformed(FORGE_RUN_APPEND_DETAIL, messageOf(err))
      }
      await writer.appendRunDetail(runId, detail)
      return ok(FORGE_RUN_APPEND_DETAIL)
    },
  }
}

/** Registry of the four forge.* handlers bound to the given writer. */
export function buildForgeCommandRegistry(writer: ForgeStateWriter): ForgeCommandRegistry {
  const handlers = new Map<string, ForgeCommandHandler>([
    [FORGE_STORY_MARK_HOLD, markHoldHandler(writer)],
    [FORGE_STORY_MARK_COMPLETE, markCompleteHandler(writer)],
    [FORGE_STORY_MARK_IN_PROGRESS, markInProgressHandler(writer)],
    [FORGE_RUN_APPEND_DETAIL, appendRunDetailHandler(writer)],
  ])

  // ENG-FORGE-V9 Item 1 — route EVERY forge.* command in the inventory so a
  // FORGE_SDLC command-node never returns not_found (which would abort the
  // engine step). Role-execution commands (run_* / classify / publish /
  // migrate / deploy / verify_*) are routed here as IDEMPOTENT intent handlers:
  // they validate their payload and return a stable success. Their REAL
  // execution (launching the Scout/Smith/QA role, git publish, Neon migration,
  // production smoke) plugs into these seams from the async Forge execution
  // layer (ENG-FORGE-V9 Item 3); the engine's own process_commands uniqueness
  // already guarantees per-node-visit idempotency (invariant #10). Until that
  // layer is wired these advance the token so the full topology is traversable.
  const special = new Set<string>(handlers.keys())
  for (const commandType of FORGE_ROUTED_COMMAND_TYPES) {
    if (special.has(commandType)) continue
    handlers.set(commandType, genericIntentHandler(commandType))
  }

  return {
    register(commandType: string, handler: ForgeCommandHandler): void {
      if (handlers.has(commandType)) {
        throw new Error(`Forge command already registered: ${commandType}`)
      }
      handlers.set(commandType, handler)
    },
    resolve(commandType: string): ForgeCommandHandler | undefined {
      return handlers.get(commandType)
    },
    list(): string[] {
      return [...handlers.keys()].sort()
    },
  }
}

/**
 * Generic idempotent intent handler for a routed forge.* command. Validates
 * required context when supplied and returns a stable success. Real side
 * effects are attached by the async Forge execution layer.
 */
function genericIntentHandler(commandType: string): ForgeCommandHandler {
  return {
    async handle(envelope) {
      // storyId/runId are optional context; a present storyId must be non-empty.
      const storyId = optionalString(envelope.input, 'storyId')
      const runId = optionalString(envelope.input, 'runId')
      if (storyId === '' && runId === '' && (envelope.input.storyId !== undefined || envelope.input.runId !== undefined)) {
        return {
          commandType,
          outcome: 'validation_failure',
          message: `${commandType} requires a non-empty storyId or runId`,
        }
      }
      return { commandType, outcome: 'success' }
    },
  }
}

/**
 * Dispatch a Forge command envelope to its handler. Returns not_found when the
 * command is not a Forge command — so an RE command can never execute here.
 */
export async function dispatchForgeCommand(
  envelope: ForgeCommandEnvelope,
  registry: ForgeCommandRegistry,
): Promise<ForgeCommandResult> {
  const handler = registry.resolve(envelope.commandType)
  if (!handler) {
    return {
      commandType: envelope.commandType,
      outcome: 'not_found',
      message: `Unknown Forge command: ${envelope.commandType}`,
    }
  }
  return handler.handle(envelope)
}
