// ---------------------------------------------------------------------------
// ENG-FORGE-HARDEN-04 — Forge lessons & recurring-failure detection.
//
// Repeated execution failures become durable, reviewable LESSONS so recurring
// harness deficiencies are visible improvement opportunities instead of being
// repaired independently forever. Consumes the HARDEN-02 canonical taxonomy.
//
//   FAIL -> CLASSIFY -> REPAIR -> VERIFY -> LEARN
//
// Authority boundary: Forge may DETECT a Forge problem and RECOMMEND an
// improvement. It must NEVER autonomously rewrite its own control plane — a
// harness-changing lesson becomes candidate architectural work for
// Scout/Architect/operator review. Lessons never mutate runtime state.
//
// Noise control: ordinary one-off implementation defects (e.g. BAD_IMPLEMENTATION)
// do NOT create architectural lessons by default; only systemic/harness classes
// (MISSING_GUARDRAIL, BAD_TOOL_CONTRACT, MISSING_CONTEXT, ENVIRONMENT_FAILURE,
// DEPENDENCY_FAILURE) or a proven recurrence qualify.
// ---------------------------------------------------------------------------

import type { ForgeFailureClass } from './failure-classifier'

export type ForgeLesson = {
  id: string
  failureClass: ForgeFailureClass
  affectedStage: string // e.g. 'LEAD_PRE -> SMITH'
  fingerprint: string
  suggestedImprovement: string
  affectedCapability: string
  recurrenceCount: number
  recurring: boolean
  /** Provenance back to the underlying executions. */
  origin: Array<{ storyId: string; runId?: string | null; failureClass: ForgeFailureClass }>
  provenance: { atIso: string }
}

const SYSTEMIC_LESSON_CLASSES: ReadonlySet<ForgeFailureClass> = new Set([
  'MISSING_GUARDRAIL',
  'BAD_TOOL_CONTRACT',
  'MISSING_CONTEXT',
  'ENVIRONMENT_FAILURE',
  'DEPENDENCY_FAILURE',
])

/** Stable recurrence fingerprint: failure class + affected stage. */
export function lessonFingerprint(input: {
  failureClass: ForgeFailureClass
  affectedStage: string
}): string {
  return `${input.failureClass}@${input.affectedStage}`
}

export function isSystemicLessonClass(failureClass: ForgeFailureClass): boolean {
  return SYSTEMIC_LESSON_CLASSES.has(failureClass)
}

export type LessonOccurrence = {
  failureClass: ForgeFailureClass
  affectedStage: string
  suggestedImprovement: string
  affectedCapability: string
  storyId: string
  runId?: string | null
  atIso: string
}

/**
 * Register a completed failure/repair occurrence. Returns the (possibly new or
 * updated) lesson and a non-mutating new store snapshot. Lessons are context
 * only — this function never touches runtime/routing state.
 */
export function applyLessonOccurrence(
  store: ForgeLesson[],
  occ: LessonOccurrence,
): { store: ForgeLesson[]; lesson: ForgeLesson | null } {
  // Noise control: a one-off ordinary implementation defect is not an
  // architectural lesson by default.
  if (!isSystemicLessonClass(occ.failureClass)) {
    return { store, lesson: null }
  }
  const fp = lessonFingerprint({ failureClass: occ.failureClass, affectedStage: occ.affectedStage })
  const existing = store.find((l) => l.fingerprint === fp)
  if (!existing) {
    const lesson: ForgeLesson = {
      id: `lesson-${fp}`,
      failureClass: occ.failureClass,
      affectedStage: occ.affectedStage,
      fingerprint: fp,
      suggestedImprovement: occ.suggestedImprovement,
      affectedCapability: occ.affectedCapability,
      recurrenceCount: 1,
      recurring: false,
      origin: [{ storyId: occ.storyId, runId: occ.runId ?? null, failureClass: occ.failureClass }],
      provenance: { atIso: occ.atIso },
    }
    return { store: [...store, lesson], lesson }
  }
  const updated: ForgeLesson = {
    ...existing,
    recurrenceCount: existing.recurrenceCount + 1,
    recurring: existing.recurrenceCount + 1 >= 2,
    origin: [
      ...existing.origin,
      { storyId: occ.storyId, runId: occ.runId ?? null, failureClass: occ.failureClass },
    ],
  }
  return {
    store: store.map((l) => (l.fingerprint === fp ? updated : l)),
    lesson: updated,
  }
}

/** Retrieve lessons available to future Scout/Architect context (read-only). */
export function lessonsForContext(
  store: ForgeLesson[],
  failureClass?: ForgeFailureClass,
): ForgeLesson[] {
  return failureClass
    ? store.filter((l) => l.failureClass === failureClass)
    : [...store]
}
