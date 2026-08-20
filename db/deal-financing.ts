import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import { claimReceipt, finalizeReceipt, readFinalReceipt } from './workflow-command-receipt'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// Canonical SET DEAL FINANCING TYPE (CRM-14C).
//
// The smallest application operation for resolving the canonical financing
// fact. The application owns legality; a workflow merely requests it. Safe
// retry via the claim-first receipt; no unrelated deal mutation. Financing
// type can be changed later by an explicit application command if business
// circumstances change.
// ---------------------------------------------------------------------------

export type SetDealFinancingTypeInput = {
  dealId: string
  financingType: 'cash' | 'financed'
  commandId: string
}

export async function setDealFinancingType(
  input: SetDealFinancingTypeInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  if (input.financingType !== 'cash' && input.financingType !== 'financed') {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: null,
      message: 'financingType must be cash or financed.',
      replayed: false,
    }
  }

  return run(async (tx) => {
    const claimed = await claimReceipt(tx, input.commandId)
    if (!claimed) {
      const receipt = await readFinalReceipt(tx, input.commandId)
      return {
        commandId: input.commandId,
        outcome: (receipt?.outcome ?? 'success') as CommandOutcome,
        emittedEvents: [],
        aggregateId: receipt?.aggregateId ?? null,
        message: receipt?.message ?? null,
        replayed: true,
      }
    }

    let outcome: CommandOutcome = 'success'
    let aggregateId: string | null = input.dealId
    let message: string | null = null

    const rows = await tx`
      update deal
      set financing_type = ${input.financingType}, updated_at = now()
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
