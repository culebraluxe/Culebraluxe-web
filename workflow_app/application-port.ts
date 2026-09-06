import type {
  ApplicationPort,
  ApplicationFacts,
  ApplicationCommandRequest,
  WorkflowSubject,
} from '../workflow_engine/lib/workflow/types'
import { toCommandEnvelope, toApplicationCommandResult } from './engine-bridge'
import { routeCommand } from './command-router'
import { getDealWorkflowFacts } from './facts'
import {
  getContractWorkflowFacts,
  resolveLegacyDealIdForContract,
} from './contract-facts'

// ---------------------------------------------------------------------------
// CulebraLuxe ApplicationPort — the concrete adapter behind the engine's
// generic integration seam. The engine never imports this file.
//
// Contract-subject strangler rule (WORKFLOW-CONTRACT-01):
//   READS    Contract owns P&S truth; Deal supplies only not-yet-retired facts.
//   COMMANDS existing RE command handlers still expect the historical Deal
//            subject, so the port translates Contract -> source Form -> Deal
//            only at this compatibility boundary. No heuristic Deal lookup.
//
// This lets new workflow instances be Contract-scoped without pretending the
// old Deal command catalog has already been refactored. As native Contract
// commands replace Deal commands, this translation shrinks and disappears.
// ---------------------------------------------------------------------------

async function compatibilityCommandRequest(
  req: ApplicationCommandRequest,
): Promise<ApplicationCommandRequest | null> {
  if (req.subjectType !== 'contract') return req
  if (!req.subjectId) return null

  const dealId = await resolveLegacyDealIdForContract(req.subjectId)
  if (!dealId) return null

  return {
    ...req,
    subjectType: 'deal',
    subjectId: dealId,
  }
}

export function createApplicationPort(): ApplicationPort {
  return {
    async executeCommand(req) {
      const routedRequest = await compatibilityCommandRequest(req)
      if (!routedRequest) {
        return {
          commandId: req.commandId,
          outcome: 'precondition_failure',
          message: req.subjectType === 'contract'
            ? `Contract ${req.subjectId ?? '(missing)'} has no unambiguous legacy Deal correlation for command ${req.commandType}.`
            : 'Workflow command subject is missing.',
          emittedEvents: [],
        }
      }

      const envelope = toCommandEnvelope(routedRequest, null)
      const result = await routeCommand(envelope)
      return toApplicationCommandResult(result, req.commandId)
    },

    async readFacts(subject: WorkflowSubject): Promise<ApplicationFacts> {
      if (subject.subjectType === 'contract') {
        const facts = await getContractWorkflowFacts(subject.subjectId)
        return (facts ?? {}) as ApplicationFacts
      }
      if (subject.subjectType === 'deal') {
        const facts = await getDealWorkflowFacts(subject.subjectId)
        return (facts ?? {}) as unknown as ApplicationFacts
      }
      return {}
    },
  }
}
