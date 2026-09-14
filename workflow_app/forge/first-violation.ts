/**
 * FIRST VIOLATION — which door actually failed first (AgentRx, arXiv 2602.02475).
 *
 * AgentRx's finding, in the captain's words: "when a 50-step agent dies at step 42, engineers
 * debug step 42. The unrecoverable cut was earlier — invented fact, tool output misread, chat
 * beating a row. Everything after is drift."
 *
 * Forge already refuses a turn when it has looped too long (the turn cap, MAP). What it could
 * not say was WHAT BROKE FIRST: the cap names the ceiling that noticed, not the door that
 * caused it. This classifies the earliest observable violation of a generation so a cap trip
 * is filed against its cause instead of its victim.
 *
 * The vocabulary is deliberately small and honest — two causes we can actually distinguish
 * from evidence, plus 'unknown' rather than a guess:
 *
 *   system          the control plane failed: the same candidate re-failed, a door refused
 *                   the lane, a claim could not be taken, evidence could not be recorded.
 *                   Nothing about the WORK is implicated; the harness is.
 *   underspecified  the work was not pinned down enough to attempt: the Architect held for
 *                   missing evidence, the acceptance was already satisfied at base, required
 *                   findings went unassigned, a proof command was absent.
 *   unknown         there is not enough evidence to name a cause — never a fabricated one.
 *
 * Pure: the caller supplies the observations.
 */
export const FIRST_VIOLATIONS = ['system', 'underspecified', 'unknown'] as const
export type FirstViolation = (typeof FIRST_VIOLATIONS)[number]

export type FirstViolationObservations = {
  /** The same candidate SHA failed more than once: the loop stopped making progress. */
  readonly repeatedCandidateFailure?: boolean
  /** A door refused the lane (launch, scope, claim, baseline, budget). */
  readonly doorRefused?: boolean
  /** The Architect/Lead held for missing evidence, or acceptance/ownership was incomplete. */
  readonly acceptanceIncomplete?: boolean
  /** Evidence could not be written (a claim, a ledger row, a findings row). */
  readonly evidenceWriteFailed?: boolean
  /** Reasons captured verbatim, recorded on the run for a human. */
  readonly reasons?: readonly string[]
}

export type FirstViolationVerdict = {
  readonly firstViol: FirstViolation
  /** The observations that decided it, so the label is auditable rather than asserted. */
  readonly because: string[]
}

/** Classify the EARLIEST identifiable failure. Order matters: a harness fault masks the work. */
export function classifyFirstViolation(
  observations: FirstViolationObservations,
): FirstViolationVerdict {
  const because: string[] = []

  // Underspecification is the cheapest cause to eliminate and the most actionable: if the
  // work was never pinned down, no amount of harness repair helps.
  if (observations.acceptanceIncomplete) {
    because.push('acceptance or ownership was incomplete before execution')
    return { firstViol: 'underspecified', because }
  }

  if (observations.doorRefused) because.push('a door refused the lane')
  if (observations.repeatedCandidateFailure) {
    because.push('the same candidate re-failed without new work')
  }
  if (observations.evidenceWriteFailed) because.push('the control plane failed to record evidence')

  for (const reason of observations.reasons ?? []) because.push(`observed: ${reason}`)

  if (because.length > 0) return { firstViol: 'system', because }
  return { firstViol: 'unknown', because: ['no door, candidate repeat or acceptance gap was observed'] }
}

/**
 * One line for the run, naming the cause and the evidence behind it. This is what turns
 * "MODEL TURN CAP" (true, but unactionable) into a pointer at the door that failed.
 */
export function renderFirstViolation(verdict: FirstViolationVerdict): string {
  const detail = verdict.because.length > 0 ? verdict.because.join('; ') : 'no evidence recorded'
  return `FIRST_VIOL=${verdict.firstViol} — ${detail}`
}
