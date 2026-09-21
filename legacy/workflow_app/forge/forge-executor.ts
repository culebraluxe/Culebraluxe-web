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
} from '@/legacy/workflow_app/forge/forge-engine-runtime'
import type { ForgeGateEvidence } from '@/legacy/workflow_app/forge/forge-facts'
import { fileOf, overlap, within } from '@/legacy/workflow_app/forge/agents/shared/path'
import { engineSql } from '@/legacy/workflow_app/engine-client'
import type { QueryExecutor } from '@/legacy/db/query-executor'
import { recordForgeQaFailure, recordForgeQaPass } from '@/legacy/db/forge-repair-ledger'
import { captureServerError } from '@/lib/server-error-capture'

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

/**
 * Complete a role task and, ONLY when this worker WON the transition, record the QA disposition.
 *
 * `complete` is the engine's completion CAS (`completeForgeRoleTask`): it resolves for the winner and
 * throws for a worker that lost the race. The disposition write runs AFTER it, so a losing QA worker
 * writes nothing and can never overwrite the winner's verdict on
 * `storyboard_story.forge_last_qa_disposition`. Routing is unaffected: the completing task's own
 * evidence rides into the transition as `pendingEvidence`, which `createForgeApplicationPort` merges
 * OVER the durable read (application-port.ts:127), so `qa_failure_route` still sees the winner's
 * disposition. A winning PASS also clears `forge_last_failure_reason`, so a stale loser FAIL cannot
 * linger.
 */
export async function completeRoleTaskThenRecordQaDisposition(input: {
  nodeId: string
  storyId: string
  outcome: ForgeRoleOutcome
  /** The engine completion CAS. It throws for a worker that lost the race. */
  complete: () => Promise<void>
  execute?: QueryExecutor
  recordFailure?: typeof recordForgeQaFailure
  recordPass?: typeof recordForgeQaPass
}): Promise<void> {
  await input.complete()
  if (input.nodeId !== 'qa_verify') return
  const recordFailure = input.recordFailure ?? recordForgeQaFailure
  const recordPass = input.recordPass ?? recordForgeQaPass
  const execute = input.execute ?? engineSql()
  if (input.outcome.evidence.qaPassed === false && input.outcome.evidence.disposition) {
    const reason =
      input.outcome.evidence.failedCommands?.join('; ') ||
      input.outcome.evidence.failedCriteria?.join('; ') ||
      'QA verification failed'
    await recordFailure(
      input.storyId,
      { disposition: input.outcome.evidence.disposition, reason },
      execute,
    )
    return
  }
  if (input.outcome.evidence.qaPassed === true) {
    await recordPass(input.storyId, execute)
  }
}

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
 * The COMPLETED signal, and nothing else. `releaseTask` refuses a completed task with
 * `TASK_NOT_RELEASABLE` and the message `Task cannot be released in status: completed`
 * (workflow_engine/lib/workflow/engine.ts), which `isAdvanceConflict` does not match — so this is
 * deliberately narrower than `isAdvanceConflict`: `not active`/`STALE_TASK`/`PROCESS_NOT_ACTIVE` are
 * real release failures and must still surface, never be folded into this no-op.
 */
export function isCompletedReleaseConflict(err: unknown): boolean {
  const code = (err as { code?: unknown } | null | undefined)?.code
  if (code === 'TASK_ALREADY_COMPLETED') return true
  const m = err instanceof Error ? err.message : String(err)
  return /cannot be released in status:\s*completed/i.test(m)
}

/** What settling a lane failure did: the task was released, or it had already completed. */
export type ForgeLaneFailureSettlement = 'released' | 'already-completed'

/**
 * Settle a claimed role task after its lane failed.
 *
 * COMPLETING IS STRICTLY STRONGER THAN RELEASING. A task that already completed cannot be released,
 * and must not be: the engine transition already advanced the process, so a release refusal is a
 * no-op, not a failure. It resolves `'already-completed'` after REPORTING the lane failure durably, so
 * the caller keeps the advance and the wave is not rejected for a run that moved forward.
 *
 * Every other release failure is still a failure: it is aggregated with the lane error and thrown, so
 * both causes are named rather than one being inferred from the other.
 */
export async function settleForgeLaneFailure(input: {
  taskId: string
  nodeId: string
  actor: string
  laneError: unknown
  release?: (taskId: string, workerId: string) => Promise<void>
  report?: (label: string, error: unknown) => void
}): Promise<ForgeLaneFailureSettlement> {
  const release = input.release ?? releaseForgeRoleTask
  try {
    await release(input.taskId, input.actor)
    return 'released'
  } catch (releaseError) {
    if (isCompletedReleaseConflict(releaseError)) {
      const report = input.report ?? ((label, error) => captureServerError(label, error))
      try {
        report('forge:lane-failure-after-completion', input.laneError)
      } catch {
        // Reporting must never turn a no-op back into a crash: the advance already stands.
      }
      return 'already-completed'
    }
    // An advance conflict that is NOT completion (stale/not active) was already non-fatal: the lane
    // error is the story, so the caller rethrows it exactly as before.
    if (isAdvanceConflict(releaseError)) return 'released'
    const cause = (e: unknown) => (e instanceof Error ? e.message : String(e))
    throw new AggregateError(
      [input.laneError, releaseError],
      `Forge role ${input.nodeId} failed and its task could not be released: ` +
        `lane failed with "${cause(input.laneError)}"; release failed with "${cause(releaseError)}"`,
    )
  }
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

/**
 * Run ONE batch of ready lanes and settle ALL of them before surfacing any failure.
 *
 * `Promise.all` rejects on the first lane and returns control while its siblings are
 * still claiming, running and writing their records — so the driver reports a failure
 * mid-write and the wave's surviving work is raced. Here every lane settles first, and
 * only then is the failure thrown: one rejection keeps its own error identity, several
 * arrive as one AggregateError naming the batch. Pure and engine-free, so the frozen
 * fence can exercise it the same way it exercises `planWave`.
 */
export async function runWaveBatch<T>(
  batch: readonly WaveLane<T>[],
  run: (lane: WaveLane<T>) => Promise<void>,
): Promise<void> {
  const settled = await Promise.allSettled(batch.map((lane) => run(lane)))
  const failures = settled
    .filter((result): result is PromiseRejectedResult => result.status === 'rejected')
    .map((result) => result.reason)
  if (failures.length === 0) return
  if (failures.length === 1) throw failures[0]
  // A FAILURE MUST NAME ITS CAUSES (the same rule as the role/release aggregate below). Measured twice
  // on 2026-09-18: a two-lane SPLIT wave reported only "2 of 2 lanes rejected" while the log carried no
  // reason and no task row was persisted, so the operator had nothing to read. With the causes in the
  // message, an I5-shaped fan-out failure explains itself.
  const causeOf = (reason: unknown): string => {
    const text = reason instanceof Error ? reason.message : String(reason)
    return text.replace(/\s+/g, ' ').trim().slice(0, 300)
  }
  throw new AggregateError(
    failures,
    `Forge wave failed: ${failures.length} of ${batch.length} lanes rejected after every lane settled — ` +
      failures.map((reason, i) => `lane ${i + 1}: ${causeOf(reason)}`).join(' | '),
  )
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
        // V11-S1: record the QA disposition durably, but ONLY on the winner-only path.
        // `completeRoleTaskThenRecordQaDisposition` runs the engine's completion CAS first and
        // writes the ledger AFTER it resolves, so a QA worker that loses the completion race
        // writes nothing and cannot overwrite the winner's verdict. Routing is unaffected: the
        // winner's own evidence rides into the transition as `pendingEvidence`. The ledger is an
        // OBSERVER; the engine is the stop.
        await completeRoleTaskThenRecordQaDisposition({
          nodeId: task.nodeId,
          storyId,
          outcome,
          complete: () =>
            completeForgeRoleTask(task.taskId, {
              transitionName: outcome.transitionName ?? 'complete',
              evidence: outcome.evidence,
              userId: actor,
            }),
        })
        // V11-S1 observers: the repair/replan counter is incremented INSIDE the
        // completion unit (see applyForgeCompletionUnit), in the same transaction as
        // the evidence merge, so a crash can no longer separate the two and undercount.
      } catch (err) {
        if (isAdvanceConflict(err)) return
        // A task that already completed cannot be released, and must not be. `settleForgeLaneFailure`
        // reports the lane failure and resolves `already-completed` so the advance stands; every other
        // release failure is aggregated with the lane error and thrown, naming both causes.
        const settlement = await settleForgeLaneFailure({
          taskId: task.taskId,
          nodeId: task.nodeId,
          actor,
          laneError: err,
        })
        if (settlement === 'already-completed') return
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
      await runWaveBatch(batch, (lane) => runReady(lane.task))
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
