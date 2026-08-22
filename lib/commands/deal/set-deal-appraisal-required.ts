// ---------------------------------------------------------------------------
// CRM-14J — Canonical command wrapper: deal.set_appraisal_required (CRM-19).
//
// Thin adapter over the existing canonical service db/deal-appraisal.ts.
// The service owns legality, the claim-first receipt and the canonical
// mutation; this handler only translates the envelope into the service call,
// preserving the router's transport-level presence/boolean pre-check. No
// business rules live here. Registration happens in lib/commands/register.ts.
// ---------------------------------------------------------------------------

import { setDealAppraisalRequired } from '../../../db/deal-appraisal'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'
import { DEAL_SET_APPRAISAL_REQUIRED } from '../command-types'

export { DEAL_SET_APPRAISAL_REQUIRED }

export class SetDealAppraisalRequiredCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const { appraisalRequired } = envelope.input as {
      appraisalRequired?: unknown
    }
    const dealId = envelope.aggregateId
    if (!dealId || typeof appraisalRequired !== 'boolean') {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: dealId ?? null,
        message:
          'deal.set_appraisal_required requires dealId and a boolean appraisalRequired.',
        replayed: false,
      }
    }
    return setDealAppraisalRequired(
      { dealId, appraisalRequired, commandId: envelope.commandId },
      ctx.run,
    )
  }
}
