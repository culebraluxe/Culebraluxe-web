// ---------------------------------------------------------------------------
// WHICH PATH DRIVES A WORKER PASS — one pure decision, so it can be tested.
//
// There are two ways a Ready story can be run, and for a long time nothing chose
// between them:
//
//   engine  the ENGINE role runner (`driveForgeStory`), which persists each role's
//           deliverable — the Architect's brief, the Lead's phase/decision — and
//           enforces the deliverable gate before advancing.
//   lane    the claim-one-lane path (`scripts/agent-work.ts`), which runs a role
//           through the adapter and hands off through the follow path.
//
// The lane path CANNOT finish a story: it has no deliverable capture, so the
// architect reports Complete and leaves no brief, and the Lead handoff correctly
// refuses with `missing-architect-brief` (measured 2026-09-15). The engine is the
// path that works, and `planForgeNight().driveEngine` says so — but nothing read
// that flag, so a Ready story sat until a human ran `pnpm forge:engine --story …`
// by hand. This module is the missing consumer.
// ---------------------------------------------------------------------------

import type { ForgeNightPlan } from '@/legacy/workflow_app/forge/forge-night-driver'

/** The engine's work types, exactly as `forge-engine-worker --work-type` accepts them. */
export const ENGINE_WORK_TYPES = ['FEATURE', 'FAST', 'BUG', 'HOTFIX', 'RESEARCH', 'MIGRATION'] as const
export type EngineWorkType = (typeof ENGINE_WORK_TYPES)[number]

export type WorkerDispatch =
  | { kind: 'engine'; storyId: string; workType: EngineWorkType; reason: string }
  | { kind: 'lane'; reason: string }

/** A Ready work item, reduced to what the decision needs. */
export type DispatchCandidate = {
  storyId: string
  /** ISO timestamp of when it was queued; the OLDEST wins so work is FIFO, not LIFO. */
  queuedAt?: string | null
  /** `agent_work_item.kind` (ENG-FORGE-FACTORY-01 Phase 1), when the dispatch recorded one. */
  kind?: string | null
}

/**
 * KIND -> WORK TYPE, one table, first cut.
 *
 * A kind is a category of work; a work type selects the workflow shape the engine drives. `fix` really is
 * the bug path and `qa`/`learn` really are research-shaped, so they map; everything else takes the default
 * path. Kept here, in one place, rather than invented at the call site so it can be corrected by reading
 * one function.
 */
export function engineWorkTypeForKind(kind: string | null | undefined): EngineWorkType {
  switch ((kind ?? '').trim().toLowerCase()) {
    case 'fix':
      return 'BUG'
    case 'qa':
    case 'learn':
      return 'RESEARCH'
    default:
      return 'FEATURE'
  }
}

export function chooseWorkerDispatch(input: {
  plan: ForgeNightPlan
  ready: DispatchCandidate[]
}): WorkerDispatch {
  // The reducer brain owns hydrate/follow/publish and does its own advancement; the engine would be a
  // second writer. The brain decides, and this is where that decision finally has a reader.
  if (!input.plan.driveEngine) {
    return { kind: 'lane', reason: `routing brain is ${input.plan.brain}; the reducer path owns this pass` }
  }
  if (input.ready.length === 0) {
    return { kind: 'lane', reason: 'no Ready work item to drive; the lane path reports idle' }
  }
  const [next] = [...input.ready].sort((a, b) =>
    String(a.queuedAt ?? '').localeCompare(String(b.queuedAt ?? '')) || a.storyId.localeCompare(b.storyId),
  )
  return {
    kind: 'engine',
    storyId: next.storyId,
    workType: engineWorkTypeForKind(next.kind),
    reason: `engine brain + oldest Ready item (${next.kind ?? 'kind unrecorded'})`,
  }
}
