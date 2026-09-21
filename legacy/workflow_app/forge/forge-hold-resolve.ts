import { appendForgeHoldRecord, type ForgeHoldResolutionInput } from '@/legacy/db/forge-hold'
import type { ForgeGateEvidence } from '@/legacy/workflow_app/forge/forge-facts'
import {
  cancelForgeInstance,
  completeForgeRoleTask,
  findActiveForgeInstance,
  findOpenForgeTask,
} from '@/legacy/workflow_app/forge/forge-engine-runtime'

// ---------------------------------------------------------------------------
// ENG-FORGE-V10 S4 — explicit HOLD resolution.
// ENG-FORGE-RESUME-DOOR-01 — the door reaches a FAILED LANE too.
//
// A HOLD is a durable, resumable state. Resolve only via an explicit operation:
//   - 'resolve'  -> move the token to the named node;
//   - 'cancel'   -> terminate cancelled;
//   - 'fail'     -> terminate failed.
// Every successful resolution is recorded to forge_hold_record (NEON) with the
// resolver, the reason and the resolution — the decision is the record of why it
// moved. The resumeTarget is validated against the XML enum — never inferred from
// prose.
//
// A run that errored at a lane has an open task at that lane and NO hold task, so
// the old hold-task-only lookup could not touch it (measured 2026-09-16: story 4
// sat with a ready task at a lane that had errored). This resolver now finds the
// run's STOP POINT at any node, and:
//   - cancel  -> terminates the instance directly (no hold task required);
//   - resolve -> advances the stopped task to the `hold` gate (empty evidence),
//                then completes the hold task on 'resolve' with the validated
//                resumeTarget, so the engine's own hold_resolution decision routes
//                to the named node.
// The only evidence ever written is the hold gate's own routing target. A resume
// MOVES THE TOKEN; it never writes a ruling (no decision, no verdict).
// ---------------------------------------------------------------------------

export const FORGE_HOLD_RESUME_TARGETS: ReadonlySet<string> = new Set([
  'SCOUT',
  'DIAGNOSE',
  'ARCHITECT',
  'LEAD',
  'SMITH',
  'QA',
  'DEV_OPS',
  'PUBLISH',
  'DEPLOY',
  'SMOKE',
  'CANCEL',
])

export type ForgeHoldResolution =
  | { resolution: 'cancel'; resolver: string; note?: string; reason?: string }
  | { resolution: 'fail'; resolver: string; note?: string; reason?: string }
  | {
      resolution: 'resolve'
      resumeTarget: string
      resolver: string
      note?: string
      reason?: string
    }

export function validResumeTarget(target: string): boolean {
  return FORGE_HOLD_RESUME_TARGETS.has(target)
}

/**
 * The run's stop point: the newest open engine task and the node it is parked at — plus what
 * the door needs in order to refuse SAFELY rather than fabricate a completion
 * (ENG-FORGE-SPLIT-SIBLING-01). The extra fields are optional so a caller that only knows an
 * id still compiles; absent means "no evidence of an unstarted branch" and nothing is blocked.
 */
export type ForgeStopPoint = {
  taskId: string
  nodeId: string
  /** Null (or absent) when the task has never been claimed — genuinely unstarted work. */
  claimedAt?: string | null
  /** True when this task is a dynamic-fork branch rather than a serial lane task. */
  forkChild?: boolean
  /** Other open branches sharing this branch's fork parent. */
  openSiblings?: number
}

/**
 * The fork's WORK branch node — the node the `<dynamic-fork>` creates its children at, and the
 * one whose completion means "this unit's work is done".
 *
 * From `legacy/workflow_app/definitions/FORGE_SDLC-v1.xml`:
 *   `<dynamic-fork id="split_dispatch" … branch-node="smith_split_work" join="split_join" …/>`
 *
 * Only tasks at THIS node can fabricate work by being advanced unclaimed. A downstream
 * coordination task that happens to ride a branch token (lead_post, for example) is movable: the
 * door parks it at the hold gate with a `hold` transition, which fabricates no ruling and no
 * sibling's completion. Narrowing to the work branch is the correction that stopped this guard
 * from blocking ordinary resumes on split stories (measured 2026-09-18, an hour after the first
 * version refused a perfectly movable lead_post).
 */
const FORK_WORK_BRANCH_NODES: ReadonlySet<string> = new Set(['smith_split_work'])

/**
 * ADVANCING AN UNSTARTED FORK BRANCH FABRICATES WORK, and this is the whole of the
 * ENG-FORGE-SPLIT-SIBLING-01 fix.
 *
 * The door's job is to move a run that is PARKED. A fork work branch that has never been claimed
 * is not parked — no lane has run for it, no work item exists for it, and its proof was never
 * attempted. Advancing it with a `hold` transition marks it completed (attributed to the
 * resolver) and the fork can never recover: the engine believes the branch is done, so it never
 * re-issues it, while the join — which is told N units will run — waits for work that will now
 * never happen.
 *
 * MEASURED, not theorised: on 2026-09-18 a resume at 09:34 completed branch 0 of 2 with
 * `claimed_at null, completed_by operator`. From then on every run could only reach branch 1,
 * and `lead_post` refused with "split children never reached a terminal state:
 * unit-a-media-length" — three times, reading like a lane failure that was not one.
 *
 * Pure, so the fence asserts the DECISION. Returns the named refusal, or null when the stop
 * point is movable.
 */
export function unstartedForkBranchRefusal(stop: ForgeStopPoint): string | null {
  if (!stop.forkChild) return null
  if (stop.claimedAt) return null
  if (!FORK_WORK_BRANCH_NODES.has(stop.nodeId)) return null
  const siblings = stop.openSiblings ?? 0
  return (
    `the newest open engine task (${stop.nodeId}) is a split WORK branch that has never been claimed` +
    (siblings > 0 ? `, and ${siblings} sibling branch(es) are open with it` : '') +
    '; resolving the hold would mark a branch as done without running it (measured 2026-09-18: ' +
    'this silently completed branch 0 of 2 and made the fork join unsatisfiable forever). ' +
    'Re-dispatch the fork so the branch is issued and claimed, or terminate the run with --cancel.'
  )
}

/**
 * The engine seam the door reads and moves through. Injectable so the frozen proof
 * asserts the DECISION (which task is moved, with what evidence, and what row is
 * written) without a seeded engine instance — the Architect risk that a proof
 * driving the real engine would need one.
 */
export type ForgeResumeEngine = {
  findActiveInstance(storyId: string): Promise<string | null>
  findStopPoint(instanceId: string): Promise<ForgeStopPoint | null>
  completeTask(
    taskId: string,
    opts: { transitionName: string; userId: string; evidence: ForgeGateEvidence },
  ): Promise<void>
  cancelInstance(instanceId: string, opts: { actor: string; reason?: string }): Promise<void>
  appendHoldRecord(input: ForgeHoldResolutionInput): Promise<number>
}

/** The real engine. A refusal never reaches these writes; only a real move is recorded. */
export const forgeResumeEngine: ForgeResumeEngine = {
  findActiveInstance: findActiveForgeInstance,
  findStopPoint: findOpenForgeTask,
  completeTask: (taskId, opts) =>
    completeForgeRoleTask(taskId, {
      userId: opts.userId,
      transitionName: opts.transitionName,
      evidence: opts.evidence,
    }),
  cancelInstance: (instanceId, opts) =>
    cancelForgeInstance(instanceId, { actor: opts.actor, reason: opts.reason }),
  appendHoldRecord: (input) => appendForgeHoldRecord(input),
}

export type ForgeHoldResolveOutcome =
  | {
      outcome: 'resolved'
      taskId: string
      auditId: number
      stopNode: string
      movedTo: string | null
    }
  | { outcome: 'refused'; missing: string }

export async function resolveForgeHold(
  input: ForgeHoldResolution & { storyId: string },
  engine: ForgeResumeEngine = forgeResumeEngine,
): Promise<ForgeHoldResolveOutcome> {
  const { storyId, resolver } = input
  if (input.resolution === 'resolve' && !validResumeTarget(input.resumeTarget)) {
    throw new Error(`invalid resumeTarget '${input.resumeTarget}' for Forge HOLD`)
  }

  const instanceId = await engine.findActiveInstance(storyId)
  if (!instanceId) {
    return {
      outcome: 'refused',
      missing: `no active FORGE_SDLC instance for story ${storyId} (the run is already terminal)`,
    }
  }

  const stop = await engine.findStopPoint(instanceId)
  if (!stop) {
    return {
      outcome: 'refused',
      missing: `no open engine task on instance ${instanceId} (nothing to move; the run is already terminal)`,
    }
  }

  // GARBAGE IN, GARBAGE OUT: the door exists to move a run that is PARKED, and it must never
  // manufacture a completion. A split branch that has never been claimed has had no lane run
  // for it, so resolving the hold over it records work that never happened AND makes the
  // fork's join unsatisfiable for good (the branch can never be re-issued). Refuse, name the
  // situation, and leave the branch open so a re-dispatch can claim it. `--cancel` remains the
  // honest way to kill such a run, and it is left available on purpose.
  if (input.resolution === 'resolve') {
    const fabricated = unstartedForkBranchRefusal(stop)
    if (fabricated) {
      return { outcome: 'refused', missing: fabricated }
    }
  }

  const reason = input.reason ?? input.note ?? 'Forge HOLD'

  // CANCEL — terminate the instance. No hold task is required, which is the whole
  // point: the failed-lane case has an open lane task and no hold gate.
  if (input.resolution === 'cancel') {
    await engine.cancelInstance(instanceId, { actor: resolver, reason })
    const auditId = await record(engine, {
      instanceId,
      taskId: stop.taskId,
      storyId,
      reason,
      originatingNode: stop.nodeId,
      resumeTarget: null,
      resolver,
      resolution: 'cancel',
      resolutionNote: input.note ?? null,
    })
    return { outcome: 'resolved', taskId: stop.taskId, auditId, stopNode: stop.nodeId, movedTo: null }
  }

  // A run stopped at a lane is advanced to the hold gate FIRST (empty evidence, an
  // explicit `hold` transition — never the engine's default transition, which could
  // fabricate a ruling), so the engine's hold_resolution decision can route by the
  // validated resumeTarget. A run already at the hold gate is resolved in place.
  let holdStop = stop
  if (stop.nodeId !== 'hold') {
    await engine.completeTask(stop.taskId, {
      transitionName: 'hold',
      userId: resolver,
      evidence: {},
    })
    const after = await engine.findStopPoint(instanceId)
    if (!after || after.nodeId !== 'hold') {
      return {
        outcome: 'refused',
        missing:
          `node '${stop.nodeId}' did not advance to the hold gate, so there is no hold task ` +
          `to resolve (is there a 'hold' transition on that node?)`,
      }
    }
    holdStop = after
  }

  const transitionName = input.resolution === 'resolve' ? 'resolve' : 'fail'
  const evidence: ForgeGateEvidence =
    input.resolution === 'resolve'
      ? { resumeTarget: input.resumeTarget as ForgeGateEvidence['resumeTarget'] }
      : {}
  await engine.completeTask(holdStop.taskId, {
    transitionName,
    userId: resolver,
    evidence,
  })
  const auditId = await record(engine, {
    instanceId,
    taskId: stop.taskId,
    storyId,
    reason,
    originatingNode: stop.nodeId,
    resumeTarget: input.resolution === 'resolve' ? input.resumeTarget : null,
    resolver,
    resolution: input.resolution,
    resolutionNote: input.note ?? null,
  })
  return {
    outcome: 'resolved',
    taskId: holdStop.taskId,
    auditId,
    stopNode: stop.nodeId,
    movedTo: input.resolution === 'resolve' ? input.resumeTarget : null,
  }
}

async function record(
  engine: ForgeResumeEngine,
  input: {
    instanceId: string
    taskId: string
    storyId: string
    reason: string
    originatingNode: string
    resumeTarget: string | null
    resolver: string
    resolution: 'resolve' | 'cancel' | 'fail'
    resolutionNote: string | null
  },
): Promise<number> {
  return engine.appendHoldRecord({
    processInstanceId: input.instanceId,
    taskId: input.taskId,
    storyId: input.storyId,
    reason: input.reason,
    originatingNode: input.originatingNode,
    failureClass: null,
    resumeTarget: input.resumeTarget,
    resolver: input.resolver,
    resolution: input.resolution,
    resolutionNote: input.resolutionNote,
  })
}
