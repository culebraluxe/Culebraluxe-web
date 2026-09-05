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
import { FORGE_JUDGMENT_LAB_NODES } from './forge-judgment'

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

export type DriveForgeStoryOptions = {
  start?: ForgeStartFacts
  runner?: ForgeRoleRunner
  maxSteps?: number
  workerId?: string
  splitConcurrency?: number
}

export type DriveForgeStoryResult = {
  instanceId: string
  status: string | null
  steps: string[]
  exhausted: boolean
  needsHuman: boolean
}

export const FORGE_HUMAN_GATE_NODES: ReadonlySet<string> = new Set([
  'hold',
  'repair_requirements',
  ...FORGE_JUDGMENT_LAB_NODES,
])

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
  const maxSteps = opts.maxSteps ?? 40
  const workerId = opts.workerId?.trim() || `forge-engine-${process.pid}`
  const steps: string[] = []

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
      await syncForgeStoryboardState(storyId, instanceId, { humanHold: true })
      return {
        instanceId,
        status: await instanceStatus(instanceId),
        steps,
        exhausted: false,
        needsHuman: true,
      }
    }

    if (!tasks.some((task) => task.status === 'ready')) {
      return {
        instanceId,
        status: await instanceStatus(instanceId),
        steps,
        exhausted: true,
        needsHuman: false,
      }
    }

    const runReady = async (task: ActiveForgeRoleTask): Promise<void> => {
      try {
        await claimForgeRoleTask(task.taskId, workerId)
      } catch (err) {
        if (isAdvanceConflict(err) || /TASK_NOT_CLAIMABLE|TASK_ALREADY_ASSIGNED/i.test(String(err))) {
          return
        }
        throw err
      }

      let outcome: ForgeRoleOutcome
      try {
        outcome = await runner(task.nodeId, task)
        await completeForgeRoleTask(task.taskId, {
          transitionName: outcome.transitionName ?? 'complete',
          evidence: outcome.evidence,
          userId: workerId,
        })
      } catch (err) {
        if (isAdvanceConflict(err)) return
        try {
          await releaseForgeRoleTask(task.taskId, workerId)
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
      await syncForgeStoryboardState(storyId, instanceId)
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
  }
}
