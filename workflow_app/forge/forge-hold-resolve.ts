import { appendForgeHoldRecord, type ForgeHoldResolutionInput } from '../../db/forge-hold'
import type { ForgeGateEvidence } from './forge-facts'
import {
  cancelForgeInstance,
  completeForgeRoleTask,
  findActiveForgeInstance,
  findOpenForgeTask,
} from './forge-engine-runtime'

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

/** The run's stop point: the newest open engine task and the node it is parked at. */
export type ForgeStopPoint = { taskId: string; nodeId: string }

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
