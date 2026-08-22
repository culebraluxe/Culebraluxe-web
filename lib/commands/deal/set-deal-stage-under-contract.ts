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

export { DEAL_SET_STAGE_UNDER_CONTRACT }

export class SetDealStageUnderContractCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    return setDealStage(
      {
        dealId: envelope.aggregateId ?? '',
        from: 'offer',
        to: 'under_contract',
        commandId: envelope.commandId,
      },
      ctx.run,
    )
  }
}
