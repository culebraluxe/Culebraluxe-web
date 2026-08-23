import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import { claimReceipt, finalizeReceipt, readFinalReceipt, replayOutcome } from './workflow-command-receipt'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// Canonical SET DEAL FINANCING TYPE (CRM-14C).
//
// The smallest application operation for resolving the canonical financing
// fact. The application owns legality; a workflow merely requests it. Safe
// retry via the claim-first receipt; no unrelated deal mutation. Financing
// type can be changed later by an explicit application command if business
// circumstances change.
//
// CRM-14J: callers (UI/API/agent/workflow) reach this service through the
// canonical command seam (lib/commands — thin wrapper
// SetDealFinancingTypeCommand registered for deal.set_financing_type), never
// by one-off direct service calls.
//
// AUTH-05: the optional actorAppUserId is threaded into the receipt so the
// receipt itself records WHO set the financing type.
// ---------------------------------------------------------------------------

export type SetDealFinancingTypeInput = {
  dealId: string
  financingType: 'cash' | 'financed'
  commandId: string
  actorAppUserId?: string | null
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
    const claimed = await claimReceipt(
      tx,
      input.commandId,
      input.actorAppUserId ?? null,
    )
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
      set financing_type = ${input.financingType}, updated_at = now()
      where id = ${input.dealId}
      returning id
    `
    if (!rows[0]) {
      outcome = 'not_found'
      aggregateId = null
      message = 'Deal not found.'
    }

    await finalizeReceipt(
      tx,
      input.commandId,
      outcome,
      aggregateId,
      message,
      input.actorAppUserId ?? null,
    )

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
