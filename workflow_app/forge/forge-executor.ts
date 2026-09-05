import {
  completeForgeRoleTask,
  findActiveForgeInstance,
  listActiveForgeRoleTasks,
  startForgeWorkflow,
  type ActiveForgeRoleTask,
  type ForgeStartFacts,
} from './forge-engine-runtime'
import type { ForgeGateEvidence } from './forge-facts'
import { engineSql } from '../engine-client'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 Item 3 — Forge role executor (drives an instance to completion).
//
// The engine drives FORGE_SDLC tokens; async role steps are task-nodes. This
// loop is the execution layer: for each open role task it runs the role (via an
// injectable runner) and completes the engine task with the resulting gate
// evidence, so downstream decisions route.
//
// `defaultForgeRoleRunner` supplies conservative evidence that drives a
// straight-line FEATURE/SOLO story (no migration/deploy) to `complete`, proving
// the full engine traversal. The REAL runner (which launches Scout/Smith/QA/
// DEV_OPS agents through agent-runtime and records real evidence) is injected
// at the cutover seam.
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
  const runner = opts.runner ?? defaultForgeRoleRunner
  const maxSteps = opts.maxSteps ?? 40
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
      return {
        instanceId,
        status: await instanceStatus(instanceId),
        steps,
        exhausted: false,
        needsHuman: true,
      }
    }

    for (const task of tasks) {
      const outcome = await runner(task.nodeId, task)
      try {
        await completeForgeRoleTask(task.taskId, {
          transitionName: outcome.transitionName ?? 'complete',
          evidence: outcome.evidence,
        })
      } catch (err) {
        // The token may already have advanced (duplicate/racing completion) —
        // recover by rescanning instead of aborting the whole drive.
        if (!isAdvanceConflict(err)) throw err
      }
      steps.push(task.nodeId)
    }
  }

  const status = await instanceStatus(instanceId)
  const tasks = await listActiveForgeRoleTasks(storyId)
  return {
    instanceId,
    status,
    steps,
    exhausted: tasks.length > 0,
    needsHuman: tasks.some((t) => FORGE_HUMAN_GATE_NODES.has(t.nodeId)),
  }
}
