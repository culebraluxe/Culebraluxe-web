import {
  claimForgeRoleTask,
  completeForgeRoleTask,
  findActiveForgeInstance,
  listActiveForgeRoleTasks,
  reconcileForgeCompletions,
  releaseForgeRoleTask,
  syncForgeStoryboardState,
  startForgeWorkflow,
  type ActiveForgeRoleTask,
  type ForgeStartFacts,
} from './forge-engine-runtime'
import type { ForgeGateEvidence } from './forge-facts'
import { fileOf, overlap, within } from './agents/shared/path'
import { engineSql } from '../engine-client'
import { recordForgeQaFailure } from '../../db/forge-repair-ledger'

export type ForgeRoleOutcome = {
  transitionName?: string
  evidence: ForgeGateEvidence
}

export type ForgeRoleRunner = (
  nodeId: string,
  task: ActiveForgeRoleTask,
) => Promise<ForgeRoleOutcome>

function defaultEvidenceFor(nodeId: string): ForgeGateEvidence {
  switch (nodeId) {
    case 'lead_pre':
      return { leadDecision: 'SOLO' }
    case 'feature_scout':
    case 'research_scout':
    case 'diagnose_scout':
    case 'repair_scout':
      return { scoutRequired: false }
    case 'architect':
    case 'repair_architect':
    case 'research_architect':
      return {}
    case 'qa_review':
      return { qaReviewRequired: false, qaReviewPassed: true }
    case 'qa_verify':
      return {
        qaPassed: true,
        publishSucceeded: true,
        migrationRequired: false,
        derivedRefreshRequired: false,
        deploymentRequired: false,
      }
    case 'production_smoke':
      return { productionVerified: true }
    default:
      return {}
  }
}

export const defaultForgeRoleRunner: ForgeRoleRunner = async (nodeId) => ({
  transitionName: 'complete',
  evidence: defaultEvidenceFor(nodeId),
})

async function instanceStatus(instanceId: string): Promise<string | null> {
  const rows = await engineSql()`
    select status from process_instances where id = ${instanceId}
  `
  return (rows[0]?.status as string | undefined) ?? null
}

/** Compact, live one-liner of the routing facts a completed node produced. */
function progressSummary(nodeId: string, evidence: ForgeGateEvidence): string {
  const picks: Array<[string, unknown]> = [
    ['qaPassed', evidence.qaPassed],
    ['candidateSha', evidence.candidateSha],
    ['leadDecision', evidence.leadDecision],
    ['publishSucceeded', evidence.publishSucceeded],
    ['failureClass', evidence.failureClass],
    ['researchDisposition', evidence.researchDisposition],
    ['verificationGap', evidence.verificationGap],
    ['repairAttempts', evidence.repairAttempts],
  ]
  const fields = picks.filter(([, v]) => v !== undefined && v !== null)
  const detail = fields.length > 0 ? ` ${JSON.stringify(Object.fromEntries(fields))}` : ''
  return `\u2192 node ${nodeId}: done${detail}`
}

export type ForgeStopRole = 'scout' | 'architect' | 'lead'

export type ForgeStopTarget =
  | { role: ForgeStopRole }
  | { node: string }

export type DriveForgeStoryOptions = {
  start?: ForgeStartFacts
  runner?: ForgeRoleRunner
  maxSteps?: number
  workerId?: string
  /** The ONE declared cap on how many ready tasks a wave may run concurrently. */
  splitConcurrency?: number
  /** Declared write surface of a ready task. Absent or null means the lane declared
   *  none, so it runs alone; a lane never runs concurrently on an unknown surface. */
  surfaceOf?: (task: ActiveForgeRoleTask) => string[] | null
  /** Live CLI telemetry: called as each engine role node is claimed / completes,
   *  so an operator sees progress instead of digging in the DB. */
  onProgress?: (message: string) => void
  /** Park the driver the moment a task in this role's terminal set completes —
   * run exactly one role (e.g. just Scout) instead of chaining the whole SDLC. */
  stopAfter?: ForgeStopTarget
}

export type DriveForgeStoryResult = {
  instanceId: string
  status: string | null
  steps: string[]
  exhausted: boolean
  /** Why there was nothing to drive, when active tasks exist but none is ready. */
  blockedReason?: string
  needsHuman: boolean
  /** The node the driver parked at (set when stopAfter was honored), else null. */
  stoppedAfter: string | null
}

/** Only true human stops. Architect/Inspector auto-run so Smith can write code overnight. */
export const FORGE_HUMAN_GATE_NODES: ReadonlySet<string> = new Set([
  'hold',
  'repair_requirements',
  // FAST_LANE (workType FAST) ends here: Smith produced code + unit tests and
  // the operator runs the QA loop manually before approving/cancelling.
  'fast_confirmation',
])

/** Terminal node(s) per stop-after role — the driver parks the moment one of
 * these completes, so a run stays inside exactly one role's wave. */
export const FORGE_ROLE_STOP_NODES: Readonly<Record<ForgeStopRole, ReadonlySet<string>>> = {
  scout: new Set(['feature_scout', 'research_scout', 'diagnose_scout', 'repair_scout']),
  architect: new Set(['architect', 'research_architect', 'repair_architect']),
  lead: new Set(['lead_pre']),
}

export function resolveForgeStopTarget(stopAfter?: ForgeStopTarget): ReadonlySet<string> | undefined {
  if (!stopAfter) return undefined
  if ('node' in stopAfter) return new Set([stopAfter.node])
  if ('role' in stopAfter) return FORGE_ROLE_STOP_NODES[stopAfter.role]
  return undefined
}

function isAdvanceConflict(err: unknown): boolean {
  const m = err instanceof Error ? err.message : String(err)
  return /already completed|not active|state changed|STALE_TASK|TASK_ALREADY_COMPLETED|PROCESS_NOT_ACTIVE/i.test(
    m,
  )
}

/**
 * One ready task offered to the wave scheduler. `lane` is the label that appears in
 * progress logs and refusal messages; `surface` is the write surface the lane declared
 * (null = none declared). `fanout` marks a lane whose disjointness the SPLIT contract
 * already proved (`smith_split_work` siblings), so those may share a batch without a
 * per-path check — every other lane needs a known, pairwise-disjoint surface.
 */
export type WaveLane<T> = {
  lane: string
  surface: string[] | null
  fanout?: boolean
  task: T
}

/** One pair of ready lanes whose declared surfaces collide. They must never share a batch, and the
 *  plan names the pair and the shared path so the deferral is STATED rather than silently reordered. */
export type WaveRefusal = { lanes: [string, string]; path: string }

export type WavePlan<T> = {
  ok: true
  batches: Array<Array<WaveLane<T>>>
  refusals: WaveRefusal[]
}

/** The path two declared surfaces collide on, or null when they are disjoint. Reuses
 *  the ONE path rule in agents/shared/path.ts rather than a second matcher. */
function sharedPath(a: readonly string[], b: readonly string[]): string | null {
  for (const left of a) {
    const l = fileOf(left)
    if (!l) continue
    for (const right of b) {
      const r = fileOf(right)
      if (!r) continue
      if (overlap(l, r)) return within(l, r) ? l : r
    }
  }
  return null
}

function hasSurface<T>(lane: WaveLane<T>): boolean {
  return Array.isArray(lane.surface) && lane.surface.length > 0
}

/**
 * Pure wave scheduler: EVERY ready lane runs, at most `cap` at once. Two lanes share a
 * batch only when their declared surfaces are disjoint, or when both are fan-out lanes
 * whose disjointness the SPLIT contract already proved; a lane that declared no surface
 * runs alone. An overlapping pair is NEVER co-scheduled — it is deferred to separate
 * batches and named in `refusals`, so a disjoint third lane still runs instead of the
 * whole wave being refused. Pure and DB-free, so the frozen fence can exercise it
 * without an engine.
 */
export function planWave<T>(lanes: readonly WaveLane<T>[], cap: number): WavePlan<T> {
  const limit = Math.max(1, Number.isFinite(cap) ? Math.trunc(cap) : 1)
  const refusals: WaveRefusal[] = []
  // Record every colliding pair once, for the progress log. The placement below never puts a
  // colliding pair in one batch, so naming the pair is what turns a whole-wave HOLD into a
  // deferral — a disjoint lane still runs. Nothing is co-scheduled at cap 1, so nothing is
  // reported as refused.
  if (limit > 1) {
    const known = lanes.filter((lane) => !lane.fanout && hasSurface(lane))
    for (let i = 0; i < known.length; i++) {
      for (let j = i + 1; j < known.length; j++) {
        const path = sharedPath(known[i].surface ?? [], known[j].surface ?? [])
        if (path) refusals.push({ lanes: [known[i].lane, known[j].lane], path })
      }
    }
  }
  const batches: Array<Array<WaveLane<T>>> = []
  for (const lane of lanes) {
    if (!lane.fanout && !hasSurface(lane)) {
      batches.push([lane])
      continue
    }
    let placed = false
    for (const batch of batches) {
      if (batch.length >= limit) continue
      const compatible = batch.every((member) => {
        if (lane.fanout) return member.fanout === true
        if (member.fanout || !hasSurface(member)) return false
        return sharedPath(lane.surface ?? [], member.surface ?? []) === null
      })
      if (!compatible) continue
      batch.push(lane)
      placed = true
      break
    }
    if (!placed) batches.push([lane])
  }
  return { ok: true, batches, refusals }
}

export async function driveForgeStory(
  storyId: string,
  opts: DriveForgeStoryOptions = {},
): Promise<DriveForgeStoryResult> {
  if (!opts.runner) {
    throw new Error(
      'Forge production execution requires an explicit real role runner; the synthetic runner is test-only.',
    )
  }
  const runner = opts.runner
  const onProgress = opts.onProgress
  const maxSteps = opts.maxSteps ?? 40
  const workerId = opts.workerId?.trim() || `forge-engine-${process.pid}`
  const steps: string[] = []
  const stopTarget = resolveForgeStopTarget(opts.stopAfter)
  let stoppedAfter: string | null = null

  let instanceId = await findActiveForgeInstance(storyId)
  if (!instanceId) {
    const started = await startForgeWorkflow(storyId, opts.start ?? { workType: 'FEATURE' })
    instanceId = started.instanceId
  }

  // RESUME FIRST — finish any completed role task whose completion unit did not commit
  // before this run's transition. A crash between the engine transition and the evidence
  // write (or between the evidence and the repair counter) leaves the task completed with
  // no receipt; reconciling here COMPLETES it from the durable engine record instead of
  // re-running the role, and the receipt keeps the effects exactly-once.
  await reconcileForgeCompletions(storyId)

  for (let i = 0; i < maxSteps; i++) {
    const tasks = await listActiveForgeRoleTasks(storyId)
    if (tasks.length === 0) break

    const humanGate = tasks.find((t) => FORGE_HUMAN_GATE_NODES.has(t.nodeId))
    if (humanGate) {
      onProgress?.(`\u23f8 human gate at ${humanGate.nodeId} — parking for operator`)
      await syncForgeStoryboardState(storyId, instanceId, { humanHold: true })
      return {
        instanceId,
        status: await instanceStatus(instanceId),
        steps,
        exhausted: false,
        needsHuman: true,
        stoppedAfter,
      }
    }

    if (!tasks.some((task) => task.status === 'ready')) {
      // NO WORK TO DRIVE IS NOT THE SAME AS NOTHING TO DO. Every active task is claimed or running: by a live
      // peer, or by a run that died holding its claim — on 2026-09-15 a disturbed run left one, and the next
      // invocation exited 2 with `steps: []`, which reads as a crash and is actually "held". Naming the
      // holders is the difference between an operator clearing a stale claim and re-running blindly, so it
      // rides the result rather than having to be inferred from the control plane.
      return {
        instanceId,
        status: await instanceStatus(instanceId),
        steps,
        exhausted: true,
        needsHuman: false,
        stoppedAfter,
        blockedReason: `no ready task; active: ${tasks
          .map((task) => `${task.nodeId}=${task.status}`)
          .join(', ')}`,
      }
    }

    const runReady = async (task: ActiveForgeRoleTask): Promise<void> => {
      // The engine restricts claims to a task's candidate set (the role). The
      // Forge executor must claim each role task AS a candidate of that task
      // and complete/release with the SAME identity (otherwise completeTask
      // rejects a non-assignee). This is the fix that lets the autonomous
      // driver actually claim+advance role tasks end to end.
      const actor =
        task.candidates && task.candidates.length > 0 ? task.candidates[0] : workerId
      try {
        await claimForgeRoleTask(task.taskId, actor)
      } catch (err) {
        if (isAdvanceConflict(err) || /TASK_NOT_CLAIMABLE|TASK_ALREADY_ASSIGNED/i.test(String(err))) {
          return
        }
        throw err
      }

      let outcome: ForgeRoleOutcome
      onProgress?.(`\u2192 ${task.nodeId}: running (claimed by ${actor})`)
      try {
        outcome = await runner(task.nodeId, task)
        // V11-S1: record the QA disposition durably BEFORE the engine advances
        // past QA, so qa_result -> qa_failure_route can route on it via the
        // durable reader. The ledger is an OBSERVER; the engine is the stop.
        if (
          task.nodeId === 'qa_verify' &&
          outcome.evidence.qaPassed === false &&
          outcome.evidence.disposition
        ) {
          const reason =
            outcome.evidence.failedCommands?.join('; ') ||
            outcome.evidence.failedCriteria?.join('; ') ||
            'QA verification failed'
          await recordForgeQaFailure(
            storyId,
            { disposition: outcome.evidence.disposition, reason },
            engineSql(),
          )
        }
        await completeForgeRoleTask(task.taskId, {
          transitionName: outcome.transitionName ?? 'complete',
          evidence: outcome.evidence,
          userId: actor,
        })
        // V11-S1 observers: the repair/replan counter is incremented INSIDE the
        // completion unit (see applyForgeCompletionUnit), in the same transaction as
        // the evidence merge, so a crash can no longer separate the two and undercount.
      } catch (err) {
        if (isAdvanceConflict(err)) return
        try {
          await releaseForgeRoleTask(task.taskId, actor)
        } catch (releaseError) {
          if (!isAdvanceConflict(releaseError)) {
            throw new AggregateError(
              [err, releaseError],
              `Forge role ${task.nodeId} failed and its task could not be released`,
            )
          }
        }
        throw err
      }
      steps.push(task.nodeId)
      onProgress?.(progressSummary(task.nodeId, outcome.evidence))
      await syncForgeStoryboardState(storyId, instanceId)
      if (stopTarget && stopTarget.has(task.nodeId)) {
        stoppedAfter = task.nodeId
      }
    }

    const ready = tasks.filter((t) => t.status === 'ready')
    // ONE declared cap governs the whole wave — not only smith_split_work siblings.
    const cap = Math.max(1, Math.trunc(opts.splitConcurrency ?? 1) || 1)
    const lanes: Array<WaveLane<ActiveForgeRoleTask>> = ready.map((task) => ({
      lane: task.nodeId,
      // smith_split_work siblings were proved disjoint by the SPLIT contract that
      // created them, so they carry no per-task surface here; every other lane runs
      // concurrently only on a surface its caller declared.
      surface: task.nodeId === 'smith_split_work' ? null : (opts.surfaceOf?.(task) ?? null),
      fanout: task.nodeId === 'smith_split_work',
      task,
    }))
    const plan = planWave(lanes, cap)
    // A colliding pair is DEFERRED, not a wave HOLD: the plan separates the pair into later
    // batches and names it here, so an operator sees why two lanes did not overlap.
    if (plan.refusals.length > 0) {
      onProgress?.(
        `\u2192 wave: deferring co-scheduling — ` +
          plan.refusals
            .map((refusal) => `${refusal.lanes[0]} / ${refusal.lanes[1]} share ${refusal.path}`)
            .join('; '),
      )
    }
    onProgress?.(
      `\u2192 wave: cap ${cap} — lanes ${ready.map((t) => t.nodeId).join(', ')}`,
    )
    for (const batch of plan.batches) {
      if (batch.length > 1) {
        onProgress?.(
          `\u2192 wave: running ${batch.map((lane) => lane.lane).join(' + ')} concurrently (cap ${cap})`,
        )
      }
      await Promise.all(batch.map((lane) => runReady(lane.task)))
    }
    // Single-role park: a stopAfter role completed this wave — do NOT advance to
    // the next role. Leave the engine parked for a human to inspect/approve.
    if (stoppedAfter) break
  }

  const status = await instanceStatus(instanceId)
  await syncForgeStoryboardState(storyId, instanceId)
  const tasks = await listActiveForgeRoleTasks(storyId)
  return {
    instanceId,
    status,
    steps,
    exhausted: tasks.length > 0,
    needsHuman: tasks.some((t) => FORGE_HUMAN_GATE_NODES.has(t.nodeId)),
    stoppedAfter,
  }
}
