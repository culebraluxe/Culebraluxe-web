import { appendForgeHoldRecord } from '../../db/forge-hold'
import { engineSql } from '../engine-client'
import type { ForgeGateEvidence } from './forge-facts'
import {
  completeForgeRoleTask,
  findActiveForgeInstance,
} from './forge-engine-runtime'

// ---------------------------------------------------------------------------
// ENG-FORGE-V10 S4 — explicit HOLD resolution.
//
// A HOLD is a durable, resumable state. Resolve only via an explicit operation:
//   - 'resolve'  -> complete the engine hold task on 'resolve' and route
//                   hold_resolution by the validated resumeTarget;
//   - 'cancel'   -> terminate cancelled;
//   - 'fail'     -> terminate failed.
// Every resolution (attempt) is recorded to forge_hold_record (NEON). The
// resumeTarget is validated against the XML enum — never inferred from prose.
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

async function findActiveHoldTask(
  processInstanceId: string,
): Promise<{ taskId: string; nodeId: string } | null> {
  const rows = await engineSql()`
    select t.id as task_id, tk.node_id as node_id
    from tasks t
    join tokens tk on tk.id = t.token_id
    where t.process_instance_id = ${processInstanceId}
      and tk.node_id = 'hold'
      and t.status in ('ready', 'reserved', 'in_progress')
    limit 1
  `
  return rows[0]
    ? { taskId: String(rows[0].task_id), nodeId: String(rows[0].node_id) }
    : null
}

export async function resolveForgeHold(
  input: ForgeHoldResolution & { storyId: string },
): Promise<{ taskId: string | null; auditId: number | null }> {
  const { storyId, resolver } = input
  if (input.resolution === 'resolve') {
    if (!validResumeTarget(input.resumeTarget)) {
      throw new Error(`invalid resumeTarget '${input.resumeTarget}' for Forge HOLD`)
    }
  }

  const instanceId = await findActiveForgeInstance(storyId)
  if (!instanceId) {
    throw new Error(`No active FORGE_SDLC instance for story ${storyId}`)
  }
  const holdTask = await findActiveHoldTask(instanceId)

  const auditId = await appendForgeHoldRecord({
    processInstanceId: instanceId,
    taskId: holdTask?.taskId ?? null,
    storyId,
    reason: input.reason ?? input.note ?? 'Forge HOLD',
    originatingNode: holdTask?.nodeId ?? 'hold',
    failureClass: null,
    resumeTarget:
      input.resolution === 'resolve' ? input.resumeTarget : null,
    resolver,
    resolution: input.resolution,
    resolutionNote: input.note ?? null,
  })

  if (holdTask) {
    await completeForgeRoleTask(holdTask.taskId, {
      userId: resolver,
      transitionName: input.resolution,
      evidence:
        input.resolution === 'resolve'
          ? { resumeTarget: input.resumeTarget as ForgeGateEvidence['resumeTarget'] }
          : {},
    })
  }

  return { taskId: holdTask?.taskId ?? null, auditId }
}
