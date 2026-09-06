import type {
  ApplicationPort,
  ApplicationFacts,
  ApplicationCommandRequest,
  WorkflowSubject,
} from '../workflow_engine/lib/workflow/types'
import { DEAL_SET_STAGE_UNDER_CONTRACT } from '../lib/commands/command-types'
import { toCommandEnvelope, toApplicationCommandResult } from './engine-bridge'
import { routeCommand } from './command-router'
import { getDealWorkflowFacts } from './facts'
import {
  getContractWorkflowFacts,
  resolveLegacyDealIdForContract,
} from './contract-facts'

// ---------------------------------------------------------------------------
// Contract-subject strangler rule:
//   - Contract owns P&S truth and execution.
//   - The old Deal stage write at mark_under_contract is redundant for Contract
//     workflows and is intentionally suppressed.
//   - Other not-yet-strangled workflow commands may still translate through the
//     explicit source-Form -> Deal compatibility seam until their own stories.
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
      // CONTRACT-CUT: Contract.execute is now the authoritative under-contract
      // lifecycle transition. Do not dual-write deal.stage for a Contract flow.
      if (
        req.subjectType === 'contract' &&
        req.commandType === DEAL_SET_STAGE_UNDER_CONTRACT
      ) {
        return {
          commandId: req.commandId,
          outcome: 'success',
          message: 'Contract execution already owns the under-contract transition.',
          emittedEvents: [],
        }
      }

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
