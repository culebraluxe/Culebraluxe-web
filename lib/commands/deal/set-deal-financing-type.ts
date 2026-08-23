// ---------------------------------------------------------------------------
// CRM-14J — Canonical command wrapper: deal.set_financing_type.
//
// Thin adapter over the existing canonical service db/deal-financing.ts.
// The service owns legality (cash|financed), the claim-first receipt and the
// canonical mutation; this handler only translates the envelope into the
// service call, preserving the router's transport-level presence/enum
// pre-check. No business rules live here. Registration happens in
// lib/commands/register.ts.
// ---------------------------------------------------------------------------

import { setDealFinancingType } from '../../../db/deal-financing'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'
import { DEAL_SET_FINANCING_TYPE } from '../command-types'

export { DEAL_SET_FINANCING_TYPE }

export class SetDealFinancingTypeCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const { financingType } = envelope.input as { financingType?: string }
    const dealId = envelope.aggregateId
    if (!dealId || (financingType !== 'cash' && financingType !== 'financed')) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: dealId ?? null,
        message:
          'deal.set_financing_type requires dealId and a cash|financed value.',
        replayed: false,
      }
    }
    return setDealFinancingType(
      {
        dealId,
        financingType,
        commandId: envelope.commandId,
        // AUTH-05: thread the acting user into the receipt so the receipt
        // itself records who set the financing type.
        actorAppUserId: envelope.actorAppUserId ?? null,
      },
      ctx.run,
    )
  }
}
