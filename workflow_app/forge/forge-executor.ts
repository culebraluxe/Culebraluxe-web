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

// ---------------------------------------------------------------------------
// ENG-FORGE-V10 — Forge role executor (drives an instance to completion).
//
// The engine drives FORGE_SDLC tokens; async role steps are task-nodes. This
// loop is the execution layer: for each open role task it runs the role (via an
// injectable runner) and completes the engine task with the resulting gate
// evidence, so downstream decisions route.
//
// `defaultForgeRoleRunner` is exported only as explicit demo/test
// infrastructure. Production callers must inject a real runner; silently
// inventing QA/release success is forbidden.
// ---------------------------------------------------------------------------

export type ForgeRoleOutcome = {
  transitionName?: string
  evidence: ForgeGateEvidence
}

export type ForgeRoleRunner = (
  nodeId: string,
  task: ActiveForgeRoleTask,
) => Promise<ForgeRoleOutcome>

/** Gate evidence that advances a FEATURE/SOLO, no-migration, no-deploy story. */
function defaultEvidenceFor(nodeId: string): ForgeGateEvidence {
  switch (nodeId) {
    case 'lead_pre':
      // execution_shape -> SOLO.
      return { leadDecision: 'SOLO' }
    case 'qa_verify':
      // qa_result -> devops; publish_result + no migration/deploy -> production_smoke task.
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
  /** Durable engine-task owner used for claim-before-launch. */
  workerId?: string
}

export type DriveForgeStoryResult = {
  instanceId: string
  status: string | null
  steps: string[]
  exhausted: boolean
  /** True when the instance is waiting on a human decision gate (HOLD /
   *  requirements), not an auto-drivable role task. */
  needsHuman: boolean
}

/**
 * Task-node ids that are HUMAN decision gates (never auto-driven): a HOLD is a
 * durable intentional stop (resolve/cancel/fail), and requirements repair is a
 * Product Owner decision. The executor stops here and surfaces them rather than
 * trying to "complete" them as a role run.
 */
export const FORGE_HUMAN_GATE_NODES: ReadonlySet<string> = new Set([
  'hold',
  'repair_requirements',
])

/** Engine conflicts that mean the token already advanced — recover by rescan. */
function isAdvanceConflict(err: unknown): boolean {
  const m = err instanceof Error ? err.message : String(err)
  return /already completed|not active|state changed|STALE_TASK|TASK_ALREADY_COMPLETED|PROCESS_NOT_ACTIVE/i.test(
    m,
  )
}

/**
 * Ensure a story has a FORGE_SDLC instance, then drive it: repeatedly run each
 * open async role task and complete it with its gate evidence until no role
 * task remains (the instance should reach a terminal state). Human decision
 * gates (HOLD / requirements) stop the drive and are reported via `needsHuman`;
 * engine "already advanced" conflicts are recovered by rescanning.
 */
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

    // Stop (do not auto-complete) at a human decision gate.
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

    // Reserved/in-progress work already has a durable owner. Never relaunch it
    // merely because another worker can see it; recovery is an explicit path.
    if (!tasks.some((task) => task.status === 'ready')) {
      return {
        instanceId,
        status: await instanceStatus(instanceId),
        steps,
        exhausted: true,
        needsHuman: false,
      }
    }

    for (const task of tasks) {
      if (task.status !== 'ready') continue
      try {
        await claimForgeRoleTask(task.taskId, workerId)
      } catch (err) {
        // Another worker won the claim or the task advanced between list and
        // claim. In either case this worker must not launch duplicate work.
        if (isAdvanceConflict(err) || /TASK_NOT_CLAIMABLE|TASK_ALREADY_ASSIGNED/i.test(String(err))) {
          continue
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
        // The token may already have advanced (duplicate/racing completion) —
        // recover by rescanning instead of aborting the whole drive.
        if (isAdvanceConflict(err)) continue
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
