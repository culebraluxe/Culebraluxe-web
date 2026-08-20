import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import { claimReceipt, finalizeReceipt, readFinalReceipt, replayOutcome } from './workflow-command-receipt'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// Canonical SET DEAL CLOSING DATE (CRM-14F / Story 135).
//
// `deal.set_closing_date` changes the canonical P&S target closing date. The
// application owns legality; the workflow merely requests it. Idempotent via
// the claim-first receipt; safe retry; no unrelated deal mutation. The SAME
// workflow instance continues — the workflow never restarts because a date
// changed; reconciliation reschedules the closing-deadline timer.
// ---------------------------------------------------------------------------

export type SetDealClosingDateInput = {
  dealId: string
  closingDate: string
  commandId: string
}

function isIsoDate(value: string): boolean {
  if (!value || typeof value !== 'string') return false
  const t = new Date(value).getTime()
  return !Number.isNaN(t)
}

export async function setDealClosingDate(
  input: SetDealClosingDateInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  if (!isIsoDate(input.closingDate)) {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: null,
      message: 'closingDate must be a valid date.',
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
      set closing_date = ${input.closingDate}::date, updated_at = now()
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
