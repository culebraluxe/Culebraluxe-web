import type {
  ApplicationPort,
  ApplicationFacts,
  WorkflowSubject,
} from '../workflow_engine/lib/workflow/types'
import { toCommandEnvelope, toApplicationCommandResult } from './engine-bridge'
import { routeCommand } from './command-router'
import { getDealWorkflowFacts } from './facts'

// ---------------------------------------------------------------------------
// CulebraLuxe ApplicationPort — the concrete adapter behind the engine's
// generic integration seam. The engine never imports this file.
//
// executeCommand: engine request -> CommandEnvelope -> router -> canonical
//                 service -> CommandResult -> engine result.
// readFacts:      subject -> canonical DealWorkflowFacts projection.
// ---------------------------------------------------------------------------

export function createApplicationPort(): ApplicationPort {
  return {
    async executeCommand(req) {
      const envelope = toCommandEnvelope(req, null)
      const result = await routeCommand(envelope)
      return toApplicationCommandResult(result, req.commandId)
    },

    async readFacts(subject: WorkflowSubject): Promise<ApplicationFacts> {
      if (subject.subjectType !== 'deal') return {}
      const facts = await getDealWorkflowFacts(subject.subjectId)
      return (facts ?? {}) as unknown as ApplicationFacts
    },
  }
}
