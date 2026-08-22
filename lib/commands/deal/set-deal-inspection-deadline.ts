// ---------------------------------------------------------------------------
// CRM-22 — Canonical command wrapper: deal.set_inspection_deadline.
//
// Thin adapter over the canonical service db/deal-deadline.ts. The service
// owns legality (milestone whitelist + date validity), the claim-first
// receipt and the canonical mutation; this handler only translates the
// envelope into the service call, preserving the router's transport-level
// presence pre-check (dealId + deadline supplied). No business rules live
// here. Registration happens in lib/commands/register.ts.
// ---------------------------------------------------------------------------

import { setDealMilestoneDeadline } from '../../../db/deal-deadline'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'
import { DEAL_SET_INSPECTION_DEADLINE } from '../command-types'

export { DEAL_SET_INSPECTION_DEADLINE }

export class SetDealInspectionDeadlineCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const { inspectionDeadline } = envelope.input as { inspectionDeadline?: string }
    const dealId = envelope.aggregateId
    if (!dealId || !inspectionDeadline) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: dealId ?? null,
        message: 'deal.set_inspection_deadline requires dealId and an inspectionDeadline.',
        replayed: false,
      }
    }
    return setDealMilestoneDeadline(
      {
        dealId,
        milestone: 'inspection',
        deadline: inspectionDeadline,
        commandId: envelope.commandId,
      },
      ctx.run,
    )
  }
}
