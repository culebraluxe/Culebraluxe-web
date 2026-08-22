import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import { claimReceipt, finalizeReceipt, readFinalReceipt, replayOutcome } from './workflow-command-receipt'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// Canonical SET DEAL MILESTONE DEADLINE (CRM-22).
//
// `deal.set_inspection_deadline` and `deal.set_financing_deadline` change the
// canonical P&S contingency deadline (inspection period / financing
// commitment) recorded on the deal. The application owns legality; the
// workflow merely requests it. Idempotent via the claim-first receipt; safe
// retry; no unrelated deal mutation. The SAME workflow instance continues —
// the workflow never restarts because a deadline changed; the generic
// deadline-timer seam (workflow_app/deadline-timer.ts) reschedules the
// instance's pending timer for that milestone deterministically.
//
// CRM-22 scope: only milestones with a REAL business date source are
// supported. The milestone whitelist below maps a stable milestone key to the
// canonical deal column — there is NO arbitrary-column path (a caller can
// never write a column that is not a canonical deadline fact source). No
// artificial dates and no parallel SLA framework are created.
//
// CRM-14J: callers (UI/API/agent/workflow) reach this service through the
// canonical command seam (lib/commands — thin wrappers
// SetDealInspectionDeadlineCommand / SetDealFinancingDeadlineCommand
// registered for deal.set_inspection_deadline / deal.set_financing_deadline),
// never by one-off direct service calls.
// ---------------------------------------------------------------------------

export type DealMilestoneKey = 'inspection' | 'financing'

/** Stable milestone key -> canonical deal column (the ONLY writable sources). */
export const DEAL_DEADLINE_COLUMNS: Record<DealMilestoneKey, string> = {
  inspection: 'inspection_deadline',
  financing: 'financing_deadline',
} as const

export type SetDealMilestoneDeadlineInput = {
  dealId: string
  milestone: DealMilestoneKey
  deadline: string
  commandId: string
}

function isIsoDate(value: string): boolean {
  if (!value || typeof value !== 'string') return false
  const t = new Date(value).getTime()
  return !Number.isNaN(t)
}

export function isDealMilestoneKey(value: string): value is DealMilestoneKey {
  return value === 'inspection' || value === 'financing'
}

export async function setDealMilestoneDeadline(
  input: SetDealMilestoneDeadlineInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  const column = DEAL_DEADLINE_COLUMNS[input.milestone]
  if (!column) {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: input.dealId,
      message: `milestone must be one of: ${Object.keys(DEAL_DEADLINE_COLUMNS).join(', ')}.`,
      replayed: false,
    }
  }
  if (!isIsoDate(input.deadline)) {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: input.dealId,
      message: 'deadline must be a valid date.',
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

    // The milestone whitelist (DEAL_DEADLINE_COLUMNS) is resolved above; the
    // column is a module constant and the branch below is chosen by it, so a
    // caller-supplied milestone key can never reach SQL as an identifier.
    const rows =
      column === 'inspection_deadline'
        ? await tx`
            update deal
            set inspection_deadline = ${input.deadline}::date, updated_at = now()
            where id = ${input.dealId}
            returning id
          `
        : await tx`
            update deal
            set financing_deadline = ${input.deadline}::date, updated_at = now()
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
