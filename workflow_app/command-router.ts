import type {
  CommandEnvelope,
  CommandResult,
  CommandOutcome,
} from '../lib/workflow/contracts'
import { PortalWriteError } from '../lib/portal-write-error'
import { createTask as createCanonicalTask } from '../db/tasks'
import {
  completeTask as completeCanonicalTask,
  cancelTask as cancelCanonicalTask,
} from '../db/portal-writes'
import { acceptOffer } from '../db/offer-acceptance'
import { setDealStage } from '../db/deal-stage'
import { setDealFinancingType } from '../db/deal-financing'

// ---------------------------------------------------------------------------
// Command router for transaction-close-v1.
//
// Existing canonical services are routed through here. Commands without a safe
// CulebraLuxe service would be declared as gaps; in this tranche the offer and
// deal-stage gaps are closed with the real canonical services below.
// ---------------------------------------------------------------------------

export type GapContract = {
  commandType: string
  title: string
  invariants: string[]
  schemaChange?: string
}

/** Retained for documentation; the offer/stage gaps are now implemented. */
export const ACCEPT_OFFER_CONTRACT: GapContract = {
  commandType: 'offer.accept',
  title: 'Accept Offer (canonical)',
  invariants: [
    'offer exists and belongs to the deal',
    'offer.status is actionable (submitted)',
    'one accepted/primary offer per deal',
    'competing offers preserved unchanged',
  ],
  schemaChange: 'none required — offer.status already supports `accepted`.',
}

export const SET_DEAL_STAGE_CONTRACT: GapContract = {
  commandType: 'deal.set_stage',
  title: 'Set Deal Stage (canonical)',
  invariants: [
    'compare-and-set on current stage',
    'only offer -> under_contract and under_contract -> closed',
  ],
  schemaChange: 'none required — deal.stage already exists.',
}

function outcomeFromError(err: unknown): CommandOutcome {
  if (err instanceof PortalWriteError) {
    switch (err.code) {
      case 'conflict':
        return 'conflict'
      case 'not-found':
        return 'not_found'
      case 'validation':
        return 'validation_failure'
      default:
        return 'precondition_failure'
    }
  }
  return 'precondition_failure'
}

function success(
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

export async function routeCommand(
  envelope: CommandEnvelope,
): Promise<CommandResult> {
  switch (envelope.commandType) {
    case 'offer.accept': {
      const { offerId } = envelope.input as { offerId?: string }
      const dealId = envelope.aggregateId
      if (!dealId || !offerId) {
        return {
          commandId: envelope.commandId,
          outcome: 'validation_failure',
          emittedEvents: [],
          aggregateId: dealId,
          message: 'offer.accept requires dealId and offerId.',
          replayed: false,
        }
      }
      return acceptOffer({
        dealId,
        offerId,
        commandId: envelope.commandId,
      })
    }
    case 'deal.set_stage_under_contract': {
      return setDealStage({
        dealId: envelope.aggregateId ?? '',
        from: 'offer',
        to: 'under_contract',
        commandId: envelope.commandId,
      })
    }
    case 'deal.set_stage_closed': {
      return setDealStage({
        dealId: envelope.aggregateId ?? '',
        from: 'under_contract',
        to: 'closed',
        commandId: envelope.commandId,
      })
    }
    case 'deal.set_financing_type': {
      const { financingType } = envelope.input as { financingType?: string }
      const dealId = envelope.aggregateId
      if (!dealId || (financingType !== 'cash' && financingType !== 'financed')) {
        return {
          commandId: envelope.commandId,
          outcome: 'validation_failure',
          emittedEvents: [],
          aggregateId: dealId ?? null,
          message: 'deal.set_financing_type requires dealId and a cash|financed value.',
          replayed: false,
        }
      }
      return setDealFinancingType({
        dealId,
        financingType,
        commandId: envelope.commandId,
      })
    }
    case 'task.create': {
      const input = envelope.input as {
        title: string
        detail?: string
        personId?: string
        propertyId?: string
        dealId?: string
        dueAt?: string
        priority?: number
        taskKind?: 'human' | 'system'
      }
      try {
        const task = await createCanonicalTask({
          title: input.title,
          detail: input.detail,
          personId: input.personId,
          propertyId: input.propertyId,
          dealId: input.dealId,
          dueAt: input.dueAt,
          priority: input.priority,
          taskKind: input.taskKind,
        })
        return success(envelope, task.id)
      } catch (err) {
        return {
          commandId: envelope.commandId,
          outcome: outcomeFromError(err),
          emittedEvents: [],
          aggregateId: null,
          message: err instanceof Error ? err.message : 'task.create failed',
          replayed: false,
        }
      }
    }
    case 'task.complete': {
      const { taskId } = envelope.input as { taskId: string }
      try {
        await completeCanonicalTask(taskId)
        return success(envelope, taskId)
      } catch (err) {
        return {
          commandId: envelope.commandId,
          outcome: outcomeFromError(err),
          emittedEvents: [],
          aggregateId: taskId,
          message: err instanceof Error ? err.message : 'task.complete failed',
          replayed: false,
        }
      }
    }
    case 'task.cancel': {
      const { taskId } = envelope.input as { taskId: string }
      try {
        await cancelCanonicalTask(taskId)
        return success(envelope, taskId)
      } catch (err) {
        return {
          commandId: envelope.commandId,
          outcome: outcomeFromError(err),
          emittedEvents: [],
          aggregateId: taskId,
          message: err instanceof Error ? err.message : 'task.cancel failed',
          replayed: false,
        }
      }
    }
    default:
      return {
        commandId: envelope.commandId,
        outcome: 'not_found',
        emittedEvents: [],
        aggregateId: null,
        message: `Unknown command type: ${envelope.commandType}`,
        replayed: false,
      }
  }
}
