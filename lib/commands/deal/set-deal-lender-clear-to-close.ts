// ---------------------------------------------------------------------------
// CRM-14J — Canonical command wrapper: deal.set_lender_clear_to_close
// (CRM-20).
//
// Thin adapter over the existing canonical service
// db/deal-lender-clearance.ts. The service owns legality, the claim-first
// receipt and the canonical mutation; this handler only translates the
// envelope into the service call, preserving the router's transport-level
// presence/boolean pre-check. No business rules live here. Registration
// happens in lib/commands/register.ts.
// ---------------------------------------------------------------------------

import { setDealLenderClearToClose } from '../../../db/deal-lender-clearance'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'
import { DEAL_SET_LENDER_CLEAR_TO_CLOSE } from '../command-types'

export { DEAL_SET_LENDER_CLEAR_TO_CLOSE }

export class SetDealLenderClearToCloseCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const { lenderClearToClose } = envelope.input as {
      lenderClearToClose?: unknown
    }
    const dealId = envelope.aggregateId
    if (!dealId || typeof lenderClearToClose !== 'boolean') {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: dealId ?? null,
        message:
          'deal.set_lender_clear_to_close requires dealId and a boolean lenderClearToClose.',
        replayed: false,
      }
    }
    return setDealLenderClearToClose(
      { dealId, lenderClearToClose, commandId: envelope.commandId },
      ctx.run,
    )
  }
}
