import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import { claimReceipt, finalizeReceipt, readFinalReceipt, replayOutcome } from './workflow-command-receipt'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// Canonical SET DEAL LENDER CLEAR-TO-CLOSE (CRM-20).
//
// The smallest application operation for recording the canonical lender
// clear-to-close fact (deal.lender_clear_to_close). The application owns
// legality; a workflow merely requests it. Safe retry via the claim-first
// receipt; no unrelated deal mutation. The fact can be changed later by an
// explicit application command if business circumstances change.
//
// Mirrors db/deal-financing.ts (deal.set_financing_type) and
// db/deal-appraisal.ts (deal.set_appraisal_required): the command is routed by
// workflow_app but never referenced by a workflow command-node — the workflow
// reads the fact as the lenderClearToClose decision input
// (workflow_app/facts.ts). Lender provider behavior is never modeled inside
// the workflow engine.
// ---------------------------------------------------------------------------

export type SetDealLenderClearToCloseInput = {
  dealId: string
  lenderClearToClose: boolean
  commandId: string
}

export async function setDealLenderClearToClose(
  input: SetDealLenderClearToCloseInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  if (typeof input.lenderClearToClose !== 'boolean') {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: null,
      message: 'lenderClearToClose must be a boolean.',
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
      set lender_clear_to_close = ${input.lenderClearToClose}, updated_at = now()
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
