// ---------------------------------------------------------------------------
// CRM-22 — Canonical command wrapper: deal.set_financing_deadline.
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
import { DEAL_SET_FINANCING_DEADLINE } from '../command-types'

export { DEAL_SET_FINANCING_DEADLINE }

export class SetDealFinancingDeadlineCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const { financingDeadline } = envelope.input as { financingDeadline?: string }
    const dealId = envelope.aggregateId
    if (!dealId || !financingDeadline) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: dealId ?? null,
        message: 'deal.set_financing_deadline requires dealId and a financingDeadline.',
        replayed: false,
      }
    }
    return setDealMilestoneDeadline(
      {
        dealId,
        milestone: 'financing',
        deadline: financingDeadline,
        commandId: envelope.commandId,
      },
      ctx.run,
    )
  }
}
