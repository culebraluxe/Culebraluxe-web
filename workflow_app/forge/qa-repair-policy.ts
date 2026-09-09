// ---------------------------------------------------------------------------
// ENG-FORGE-V11-S1 — QA failure disposition + bounded repair/replan accounting.
//
// Pure, DB-free policy for turning a QA result into a machine-routable outcome.
// It does NOT decide what a story "means" — it only maps an explicit QA
// disposition to a legal lifecycle action given the current durable attempt
// state and budget, and fails closed when a decision is missing/invalid.
//
// Engine owns the state (the caller supplies the durable repair/replan attempt
// counts). This module is restart-neutral: the same state always yields the
// same decision, so nothing about attempt truth lives in a process-local
// executor variable here.
//
// Legal routes (V11-S1):
//   PASS                          -> pass (continue toward DEV_OPS/release)
//   FAIL + REPAIR  (in budget)    -> smith (REPAIR: architecture stays valid)
//   FAIL + REPLAN  (in budget)    -> architect (REPLAN: plan no longer valid)
//   FAIL + ESCALATE               -> hold (Lead / operator)
//   FAIL + missing/invalid disp.  -> hold (fail safe — never a silent success)
//   FAIL + REPAIR/REPLAN exhausted-> hold (bounded autonomy, durable HOLD)
//
// A REPAIR or REPLAN decision increments its own durable counter. Exhaustion of
// one budget routes THAT attempt to HOLD; it never auto-converts into the other
// lane and never bypasses the operator.
// ---------------------------------------------------------------------------

export type QaVerdict = 'PASS' | 'FAIL'

export type QaDisposition = 'REPAIR' | 'REPLAN' | 'ESCALATE'

/** Human-readable + machine-visible failure evidence QA attaches to a FAIL. */
export type QaFailureEvidence = {
  reason: string
  failedCriteria?: string[]
  failedCommands?: string[]
}

/** Durable, restart-surviving attempt counters for a story. */
export type RepairAttemptState = {
  repairAttempts: number
  replanAttempts: number
}

/** Bounded-autonomy budgets. Defaults per V11 §1.6. */
export type RepairBudget = {
  maxRepairAttempts: number
  maxReplanAttempts: number
}

export const DEFAULT_REPAIR_BUDGET: RepairBudget = {
  maxRepairAttempts: 3,
  maxReplanAttempts: 2,
}

export type RepairRouting =
  | { action: 'pass' }
  | { action: 'smith'; repairAttempts: number }
  | { action: 'architect'; replanAttempts: number }
  | { action: 'hold'; reason: string }

export function routeQaResult(input: {
  verdict: QaVerdict
  disposition: QaDisposition | null | undefined
  state: RepairAttemptState
  budget?: RepairBudget
  /** Scope B no-progress: same SHA re-failed same machine class with no new candidate. */
  noProgress?: boolean
  /**
   * Verification/config gap (claude-orchestrate): the failure is NOT a candidate
   * defect — Assay could not form/run a valid command plan (missing ## Assay
   * commands, no frozen plan). Repairing or upgrading the model cannot help, so
   * this must route to a durable HOLD, never to smith/architect. This is what
   * stops the "assay can never pass -> repair forever" deadlock.
   */
  verificationGap?: boolean
}): RepairRouting {
  const budget = input.budget ?? DEFAULT_REPAIR_BUDGET

  if (input.verdict === 'PASS') {
    return { action: 'pass' }
  }

  // Verification/config gap: no candidate defect to fix. A stronger model or a
  // repair cycle buys another equally-unverifiable attempt -> terminal HOLD.
  if (input.verificationGap === true) {
    return {
      action: 'hold',
      reason:
        'VERIFICATION GAP: QA could not form/run a valid assay command plan (missing ## Assay commands or no frozen plan). ' +
        'Repair cannot help; fix the packet/config, then re-dispatch.',
    }
  }

  // Scope B no-progress guard: the machine has already failed this exact
  // candidate with the same classification. Never auto-launch another model
  // repair cycle — route to a durable NO_PROGRESS HOLD for the operator.
  if (input.noProgress === true) {
    return {
      action: 'hold',
      reason:
        'NO_PROGRESS: the same candidate SHA re-failed the same machine classification ' +
        'with no new candidate in between — do not auto-launch another repair cycle.',
    }
  }

  // A FAIL MUST carry an explicit, legal disposition. Without one the engine
  // cannot safely choose REPAIR vs REPLAN, so it fails closed into HOLD — a
  // missing/invalid disposition can never accidentally select a success path.
  const d = input.disposition
  if (d !== 'REPAIR' && d !== 'REPLAN' && d !== 'ESCALATE') {
    return {
      action: 'hold',
      reason:
        `QA FAIL without a legal disposition (got ${String(d)}) — ` +
        'engine cannot choose a safe repair route; operator/Lead review required.',
    }
  }

  if (d === 'ESCALATE') {
    return {
      action: 'hold',
      reason: 'QA requested ESCALATE — operator/Lead intervention required.',
    }
  }

  if (d === 'REPAIR') {
    if (input.state.repairAttempts >= budget.maxRepairAttempts) {
      return {
        action: 'hold',
        reason: `Repair budget exhausted (${input.state.repairAttempts}/${budget.maxRepairAttempts}) — operator/Lead required.`,
      }
    }
    return { action: 'smith', repairAttempts: input.state.repairAttempts + 1 }
  }

  // REPLAN
  if (input.state.replanAttempts >= budget.maxReplanAttempts) {
    return {
      action: 'hold',
      reason: `Replan budget exhausted (${input.state.replanAttempts}/${budget.maxReplanAttempts}) — operator/Lead required.`,
    }
  }
  return { action: 'architect', replanAttempts: input.state.replanAttempts + 1 }
}

/** Atomic, single-statement increment helper for the durable attempt ledger. */
export function incrementRepairAttempt(state: RepairAttemptState): RepairAttemptState {
  return { ...state, repairAttempts: state.repairAttempts + 1 }
}

export function incrementReplanAttempt(state: RepairAttemptState): RepairAttemptState {
  return { ...state, replanAttempts: state.replanAttempts + 1 }
}
