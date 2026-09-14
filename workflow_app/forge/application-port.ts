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
  ForgeReleaseExecutor,
} from './forge-state-writer'
import { projectForgeGateFacts, type ForgeGateEvidence } from './forge-facts'

// ---------------------------------------------------------------------------
// ENG-FORGE-V10 — Forge ApplicationPort (the engine-owned Forge seam).
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
// The engine stays domain-neutral; the live Forge engine runtime constructs
// this ApplicationPort and passes it to WorkflowEngine.
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
  /**
   * The evidence of the task completing RIGHT NOW, when a transition is being evaluated.
   *
   * The runtime runs the transition BEFORE merging the evidence row — deliberately, because the
   * transition is the CAS that decides which worker won and a losing worker must not commit its
   * result. The unhandled consequence was that each gate judged this turn's work against the
   * PREVIOUS turn's row: `qa_verify` computed `candidate && verified === candidate` from a
   * `candidateSha` predating the Smith's commit, `qaPassed` came out false, and a passing candidate
   * was sent to `repair_smith` until the turn cap (measured live 2026-09-14, twice).
   *
   * This is merged OVER the durable row because it is strictly more recent.
   */
  pendingEvidence?: ForgeGateEvidence
  /** Real executor for release-critical command nodes. */
  releaseExecutor?: ForgeReleaseExecutor
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
  const registry = buildForgeCommandRegistry(writer, opts.releaseExecutor)

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
        {
          commandId: request.commandId,
          commandType: request.commandType,
          processInstanceId: request.correlationId,
          storyId: request.subjectType === 'story' ? request.subjectId : null,
          input: request.input,
        },
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
      if (subject.subjectType === 'story') {
        const reader =
          opts.evidenceReader ??
          (await import('./forge-evidence-db')).createStoryGateEvidenceReader()
        const evidence = await reader(subject.subjectId)
        // THE EVIDENCE THIS TASK IS SUBMITTING WINS OVER THE DURABLE ROW.
        //
        // The runtime completes the TRANSITION FIRST and merges the evidence row afterwards, on
        // purpose: the transition is the CAS that decides which worker won, and a losing worker
        // must not commit its result. The consequence, unhandled until now, was that every gate
        // judged this turn's work against the PREVIOUS turn's row — so `qa_verify` computed
        // `candidate && verified === candidate` from a `candidateSha` that predated the Smith's
        // commit, `qaPassed` came out false, the engine took the fail branch to `repair_smith`, the
        // repair reproduced the same candidate, and the loop ran to the turn cap (measured live
        // 2026-09-14: qa reported `qaPassed:true` at 05:35:19 and again at 08:28:25, and the router
        // took `fail` one second later both times, while the durable row said `qa_passed=true`).
        //
        // `pendingEvidence` is the evidence of the task completing RIGHT NOW. It is strictly more
        // recent than the row, so it is merged over it: this turn's facts are visible to this turn's
        // transition, and the durable row still supplies everything earlier nodes persisted. The CAS
        // order in the runtime is untouched.
        const merged = opts.pendingEvidence ? { ...evidence, ...opts.pendingEvidence } : evidence
        return projectForgeGateFacts(merged)
      }
      return {}
    },
  }
}
