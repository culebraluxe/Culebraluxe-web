import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import { claimReceipt, finalizeReceipt, readFinalReceipt } from './workflow-command-receipt'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// Canonical SET DEAL STAGE (CRM-14).
//
// Compare-and-set semantics for the two consequential transitions:
//   offer -> under_contract   (deal.set_stage_under_contract)
//   under_contract -> closed  (deal.set_stage_closed)
//
// Idempotency uses the claim-first receipt: the same commandId executes its
// business effect at most once, and every caller observes the winner's result.
// ---------------------------------------------------------------------------

const ALLOWED_TRANSITIONS: Array<[string, string]> = [
  ['offer', 'under_contract'],
  ['under_contract', 'closed'],
]

export type SetDealStageInput = {
  dealId: string
  from: string
  to: string
  commandId: string
}

export async function setDealStage(
  input: SetDealStageInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  const allowed = ALLOWED_TRANSITIONS.some(
    ([from, to]) => from === input.from && to === input.to,
  )
  if (!allowed) {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: null,
      message: `Transition ${input.from} -> ${input.to} is not allowed.`,
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
      set stage = ${input.to},
          closed_at = case when ${input.to} = 'closed' then now() else closed_at end,
          updated_at = now()
      where id = ${input.dealId} and stage = ${input.from}
      returning id, stage
    `
    if (!rows[0]) {
      const curRows = await tx`
        select stage from deal where id = ${input.dealId} limit 1
      `
      if (!curRows[0]) {
        outcome = 'not_found'
        aggregateId = null
        message = 'Deal not found.'
      } else {
        outcome = 'conflict'
        aggregateId = null
        message = `Expected stage '${input.from}' but was '${String(curRows[0].stage)}'.`
      }
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
