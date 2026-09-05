import type {
  ApplicationCommandResult,
  ApplicationFacts,
  ApplicationPort,
  WorkflowSubject,
} from '../../workflow_engine/lib/workflow/types'
import { FORGE_ROUTED_COMMAND_TYPES } from '../forge-command-types'
import { buildForgeCommandRegistry, dispatchForgeCommand } from './forge-command'
import type {
  ForgeFactReader,
  ForgeStateWriter,
} from './forge-state-writer'
import { projectForgeGateFacts, type ForgeGateEvidence } from './forge-facts'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 Stage 3 — Forge ApplicationPort (the "B" engine seam).
//
// This is the concrete adapter the generic workflow engine uses when it runs a
// FORGE_SDLC instance — the exact mirror of RE's createApplicationPort in
// workflow_app/application-port.ts, but for the Forge domain. It is a hard
// A/B boundary:
//   - executeCommand routes ONLY forge.* commands to the Forge dispatcher. Any
//     RE command (deal.*, offer.*, task.*) returns not_found here and is NEVER
//     dispatched through the RE CommandDispatcher.
//   - readFacts returns Forge subject facts when a reader is supplied, else {}
//     for non-Forge subjects.
//
// The engine never imports this file; a future Forge-on-engine integration
// constructs an ApplicationPort from here and passes it to WorkflowEngine.
// ---------------------------------------------------------------------------

export type ForgeApplicationPortOptions = {
  /** Injected writer. Absent => lazily resolved Neon writer (db-state-writer). */
  writer?: ForgeStateWriter | (() => Promise<ForgeStateWriter>)
  /** Optional subject fact reader (raw facts). */
  readFacts?: ForgeFactReader
  /**
   * Optional Forge evidence loader keyed by story. When present and the subject
   * is a story, readFacts projects the FORGE_SDLC decision-gate facts from the
   * story's execution evidence (Item 2). Mutually exclusive with `readFacts`.
   */
  evidenceReader?: (storyId: string) => Promise<ForgeGateEvidence>
}

async function resolveWriter(
  opts: ForgeApplicationPortOptions,
): Promise<ForgeStateWriter> {
  if (!opts.writer) {
    const { createDbForgeStateWriter } = await import('./db-state-writer')
    return createDbForgeStateWriter()
  }
  if (typeof opts.writer === 'function') return opts.writer()
  return opts.writer
}

export async function createForgeApplicationPort(
  opts: ForgeApplicationPortOptions = {},
): Promise<ApplicationPort> {
  const writer = await resolveWriter(opts)
  const registry = buildForgeCommandRegistry(writer)

  return {
    async executeCommand(request): Promise<ApplicationCommandResult> {
      if (!FORGE_ROUTED_COMMAND_TYPES.has(request.commandType)) {
        return {
          commandId: request.commandId,
          outcome: 'not_found',
          message: `Not a Forge command: ${request.commandType} (Forge executes forge.* commands only)`,
        }
      }
      const result = await dispatchForgeCommand(
        { commandId: request.commandId, commandType: request.commandType, input: request.input },
        registry,
      )
      return {
        commandId: request.commandId,
        outcome: result.outcome,
        message: result.message,
      }
    },

    async readFacts(subject: WorkflowSubject): Promise<ApplicationFacts> {
      if (opts.readFacts) return opts.readFacts(subject)
      if (subject.subjectType !== 'story') return {}
      // Explicit evidence wins (used by the executor for a single completion);
      // otherwise fall back to the DURABLE reader over the run tables (#1).
      const reader =
        opts.evidenceReader ??
        (await import('./forge-evidence-db')).createStoryGateEvidenceReader()
      const evidence = await reader(subject.subjectId)
      return projectForgeGateFacts(evidence)
    },
  }
}
