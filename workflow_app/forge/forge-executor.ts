import {
  claimForgeRoleTask,
  completeForgeRoleTask,
  findActiveForgeInstance,
  listActiveForgeRoleTasks,
  releaseForgeRoleTask,
  syncForgeStoryboardState,
  startForgeWorkflow,
  type ActiveForgeRoleTask,
  type ForgeStartFacts,
} from './forge-engine-runtime'
import type { ForgeGateEvidence } from './forge-facts'
import { engineSql } from '../engine-client'
import {
  incrementForgeReplan,
  incrementForgeRepair,
  recordForgeQaFailure,
} from '../../db/forge-repair-ledger'

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
  splitConcurrency?: number
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
      return {
        instanceId,
        status: await instanceStatus(instanceId),
        steps,
        exhausted: true,
        needsHuman: false,
        stoppedAfter,
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
        // V11-S1 observers: a repair/replan actually happened — record it so the
        // durable counts the engine reads for the NEXT QA decision stay truthful.
        if (task.nodeId === 'repair_smith') {
          await incrementForgeRepair(storyId, engineSql())
        } else if (task.nodeId === 'repair_architect') {
          await incrementForgeReplan(storyId, engineSql())
        }
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
    const splitSiblings = ready.filter((t) => t.nodeId === 'smith_split_work')
    const others = ready.filter((t) => t.nodeId !== 'smith_split_work')
    for (const t of others) await runReady(t)
    const cap = Math.min(opts.splitConcurrency ?? 1, splitSiblings.length)
    let nextSplit = 0
    const pump = async (): Promise<void> => {
      while (nextSplit < splitSiblings.length) {
        const t = splitSiblings[nextSplit++]
        await runReady(t)
      }
    }
    await Promise.all(Array.from({ length: Math.max(cap, 0) }, () => pump()))
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
