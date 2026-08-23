// ---------------------------------------------------------------------------
// CRM-14J — Canonical command wrapper: deal.set_closing_date.
//
// Thin adapter over the existing canonical service db/deal-closing-date.ts.
// The service owns legality (date validity), the claim-first receipt and the
// canonical mutation; this handler only translates the envelope into the
// service call, preserving the router's transport-level presence pre-check
// (dealId + closingDate supplied). No business rules live here. Registration
// happens in lib/commands/register.ts.
// ---------------------------------------------------------------------------

import { setDealClosingDate } from '../../../db/deal-closing-date'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'
import { DEAL_SET_CLOSING_DATE } from '../command-types'

export { DEAL_SET_CLOSING_DATE }

export class SetDealClosingDateCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const { closingDate } = envelope.input as { closingDate?: string }
    const dealId = envelope.aggregateId
    if (!dealId || !closingDate) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: dealId ?? null,
        message: 'deal.set_closing_date requires dealId and a closingDate.',
        replayed: false,
      }
    }
    return setDealClosingDate(
      {
        dealId,
        closingDate,
        commandId: envelope.commandId,
        // AUTH-05: thread the acting user into the receipt so the receipt
        // itself records who changed the closing date.
        actorAppUserId: envelope.actorAppUserId ?? null,
      },
      ctx.run,
    )
  }
}
