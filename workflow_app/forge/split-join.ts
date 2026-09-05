// ---------------------------------------------------------------------------
// ENG-FORGE-HARDEN-07 — deterministic SPLIT fan-out reduce + complete join.
//
// Forge must deterministically account for EVERY expected child of a fan-out.
// Missing is failure, not absence of evidence; silence never satisfies the
// join. Duplicate completion is idempotent. Conflicting sibling output is
// surfaced, never silently resolved. This is the deterministic reducer that
// feeds LEAD POST (it does not replace the engine's exactly-once token join).
//
// Pure + DB-free.
// ---------------------------------------------------------------------------

export type SplitChildStatus = 'completed' | 'failed' | 'cancelled'

export type SplitOutcome = {
  childId: string
  status: SplitChildStatus
  attempt: number
  /** Declared output keys this child produced (for conflict detection). */
  outputKeys?: string[]
  candidateSha?: string | null
}

export type SplitReduction = {
  expected: number
  accounted: number
  completed: string[]
  failed: string[]
  cancelled: string[]
  missing: string[]
  duplicates: string[]
  conflicts: string[]
  joinSatisfied: boolean
}

/**
 * Reduce a fan-out to a single normalized accounting. The join is satisfied
 * ONLY when every expected child reached an acceptable terminal state, nothing
 * is missing, nothing failed/cancelled (under the default policy), and no
 * sibling produced conflicting output.
 */
export function reduceSplit(input: {
  expectedIds: string[]
  outcomes: SplitOutcome[]
}): SplitReduction {
  const expected = input.expectedIds.length
  const byChild = new Map<string, SplitOutcome>()
  const duplicates: string[] = []
  for (const o of input.outcomes) {
    const existing = byChild.get(o.childId)
    if (existing) {
      duplicates.push(o.childId)
      // Idempotent accounting: prefer the higher-information outcome; the later
      // (retry) attempt wins.
      if (o.attempt >= existing.attempt) byChild.set(o.childId, o)
    } else {
      byChild.set(o.childId, o)
    }
  }

  const accounted = [...byChild.values()].map((o) => o.childId)
  const completed = [...byChild.values()].filter((o) => o.status === 'completed').map((o) => o.childId)
  const failed = [...byChild.values()].filter((o) => o.status === 'failed').map((o) => o.childId)
  const cancelled = [...byChild.values()].filter((o) => o.status === 'cancelled').map((o) => o.childId)
  const missing = input.expectedIds.filter((id) => !byChild.has(id))

  // Conflict: two DIFFERENT children both claim the same output key.
  const ownerByOutput = new Map<string, string>()
  const conflicts: string[] = []
  for (const o of byChild.values()) {
    for (const key of o.outputKeys ?? []) {
      const existingOwner = ownerByOutput.get(key)
      if (existingOwner && existingOwner !== o.childId && !conflicts.includes(key)) {
        conflicts.push(key)
      } else if (!existingOwner) {
        ownerByOutput.set(key, o.childId)
      }
    }
  }

  const joinSatisfied =
    missing.length === 0 &&
    failed.length === 0 &&
    cancelled.length === 0 &&
    conflicts.length === 0 &&
    completed.length === expected

  return {
    expected,
    accounted: accounted.length,
    completed,
    failed,
    cancelled,
    missing,
    duplicates,
    conflicts,
    joinSatisfied,
  }
}
