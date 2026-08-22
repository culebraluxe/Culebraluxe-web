// ---------------------------------------------------------------------------
// CRM-14J — Canonical command wrapper: deal.set_stage_under_contract.
//
// Thin adapter over the existing canonical service db/deal-stage.ts
// (compare-and-set offer -> under_contract). The service owns legality,
// invariant enforcement, the claim-first receipt and the canonical mutation;
// this handler only translates the envelope into the service call. No business
// rules live here. Registration happens in lib/commands/register.ts.
// ---------------------------------------------------------------------------

import { setDealStage } from '../../../db/deal-stage'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'
import { DEAL_SET_STAGE_UNDER_CONTRACT } from '../command-types'
import { createDomainEventFromCommand } from '../domain-events'

export { DEAL_SET_STAGE_UNDER_CONTRACT }

export class SetDealStageUnderContractCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const from = 'offer'
    const to = 'under_contract'
    const result = await setDealStage(
      {
        dealId: envelope.aggregateId ?? '',
        from,
        to,
        commandId: envelope.commandId,
      },
      ctx.run,
    )
    // CMD-01 — a COMMITTED stage change is a FACT. Emit the existing
    // DEAL_STAGE_CHANGED domain event through the dispatcher's collector so it
    // is appended to the outbox in the SAME transaction as the mutation +
    // receipt. Only on success — a failed command never emits a fact.
    if (result.outcome === 'success') {
      ctx.events.add(
        createDomainEventFromCommand(envelope, {
          eventType: 'DEAL_STAGE_CHANGED',
          payload: { dealId: envelope.aggregateId ?? '', from, to },
        }),
      )
    }
    return result
  }
}
