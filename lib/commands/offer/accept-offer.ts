// ---------------------------------------------------------------------------
// CRM-14J — Canonical command wrapper: offer.accept.
//
// Thin adapter over the existing canonical service db/offer-acceptance.ts
// (acceptOffer). The service owns legality (offer belongs to the deal, is
// actionable, no competing accepted offer), the claim-first receipt and the
// canonical mutation; this handler only translates the envelope into the
// service call, preserving the router's transport-level presence pre-check
// (dealId + offerId). No business rules live here. Registration happens in
// lib/commands/register.ts.
// ---------------------------------------------------------------------------

import { acceptOffer } from '../../../db/offer-acceptance'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'
import { OFFER_ACCEPT } from '../command-types'

export { OFFER_ACCEPT }

export class AcceptOfferCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const { offerId } = envelope.input as { offerId?: string }
    const dealId = envelope.aggregateId
    if (!dealId || !offerId) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: dealId ?? null,
        message: 'offer.accept requires dealId and offerId.',
        replayed: false,
      }
    }
    return acceptOffer(
      { dealId, offerId, commandId: envelope.commandId },
      ctx.run,
    )
  }
}
