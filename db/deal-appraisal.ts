import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import { claimReceipt, finalizeReceipt, readFinalReceipt, replayOutcome } from './workflow-command-receipt'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// Canonical SET DEAL APPRAISAL REQUIRED (CRM-19).
//
// The smallest application operation for resolving the canonical appraisal
// applicability fact (deal.appraisal_required). The application owns legality;
// a workflow merely requests it. Safe retry via the claim-first receipt; no
// unrelated deal mutation. The fact can be changed later by an explicit
// application command if business circumstances change.
//
// Mirrors db/deal-financing.ts (deal.set_financing_type): appraisal
// applicability is INDEPENDENT of financing (Story 123) — a cash deal may
// require an appraisal (buyer/seller request) and a financed deal may not.
// ---------------------------------------------------------------------------

export type SetDealAppraisalRequiredInput = {
  dealId: string
  appraisalRequired: boolean
  commandId: string
}

export async function setDealAppraisalRequired(
  input: SetDealAppraisalRequiredInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  if (typeof input.appraisalRequired !== 'boolean') {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: null,
      message: 'appraisalRequired must be a boolean.',
      replayed: false,
    }
  }

  return run(async (tx) => {
    const claimed = await claimReceipt(tx, input.commandId)
    if (!claimed) {
      const receipt = await readFinalReceipt(tx, input.commandId)
      const replay = replayOutcome(receipt)
      return {
        commandId: input.commandId,
        outcome: replay.outcome,
        emittedEvents: [],
        aggregateId: receipt?.aggregateId ?? null,
        message: replay.message,
        replayed: true,
      }
    }

    let outcome: CommandOutcome = 'success'
    let aggregateId: string | null = input.dealId
    let message: string | null = null

    const rows = await tx`
      update deal
      set appraisal_required = ${input.appraisalRequired}, updated_at = now()
      where id = ${input.dealId}
      returning id
    `
    if (!rows[0]) {
      outcome = 'not_found'
      aggregateId = null
      message = 'Deal not found.'
    }

    await finalizeReceipt(tx, input.commandId, outcome, aggregateId, message)

    return {
      commandId: input.commandId,
      outcome,
      emittedEvents: [],
      aggregateId,
      message,
      replayed: false,
    }
  })
}
