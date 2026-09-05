import { engineSql } from '../engine-client'
import type { ForgeGateEvidence } from './forge-facts'
import { validResumeTarget } from './forge-hold-resolve'
import {
  completeForgeRoleTask,
  findActiveForgeInstance,
  syncForgeStoryboardState,
} from './forge-engine-runtime'

/**
 * Judgment-lab labels (Grok / Chris). These are consult/review roles.
 * They MUST auto-run overnight so Smith can produce working code.
 * Humans review that code; they do not sit on an empty Architect gate.
 */
export const FORGE_JUDGMENT_LAB_NODES: ReadonlySet<string> = new Set([
  'architect',
  'repair_architect',
  'research_architect',
  'qa_review',
])

export function isForgeJudgmentLabNode(nodeId: string): boolean {
  return FORGE_JUDGMENT_LAB_NODES.has(nodeId)
}

export const FORGE_JUDGMENT_DECISIONS = ['accept', 'revise', 'reject'] as const
export type ForgeJudgmentDecision = (typeof FORGE_JUDGMENT_DECISIONS)[number]

export type ResolveForgeJudgmentInput = {
  storyId: string
  nodeId: string
  decision: ForgeJudgmentDecision
  resolver: string
  resumeTarget: ForgeGateEvidence['resumeTarget']
  evidence?: ForgeGateEvidence
  note?: string
}

/** Optional human override after code exists. Never the default overnight path. */
export async function resolveForgeJudgment(
  input: ResolveForgeJudgmentInput,
): Promise<{ taskId: string }> {
  if (!isForgeJudgmentLabNode(input.nodeId)) {
    throw new Error(`'${input.nodeId}' is not a judgment-lab node`)
  }
  if (!FORGE_JUDGMENT_DECISIONS.includes(input.decision)) {
    throw new Error(`invalid judgment decision '${input.decision}'`)
  }
  if (!input.resumeTarget || !validResumeTarget(input.resumeTarget)) {
    throw new Error(`invalid resumeTarget '${String(input.resumeTarget)}' for judgment resolve`)
  }

  const instanceId = await findActiveForgeInstance(input.storyId)
  if (!instanceId) {
    throw new Error(`No active FORGE_SDLC instance for story ${input.storyId}`)
  }

  const rows = await engineSql()`
    select t.id as task_id
    from tasks t
    join tokens tk on tk.id = t.token_id
    where t.process_instance_id = ${instanceId}
      and tk.node_id = ${input.nodeId}
      and t.status in ('ready', 'reserved', 'in_progress')
    limit 1
  `
  const taskId = rows[0] ? String(rows[0].task_id) : null
  if (!taskId) {
    throw new Error(`No open judgment task for ${input.nodeId} on ${input.storyId}`)
  }

  const evidence: ForgeGateEvidence = {
    ...(input.evidence ?? {}),
    resumeTarget: input.resumeTarget,
    ...(input.decision === 'reject'
      ? { failureClass: 'ARCHITECTURE_GAP' as const }
      : {}),
  }

  await completeForgeRoleTask(taskId, {
    userId: input.resolver,
    transitionName: input.decision === 'reject' ? 'fail' : 'complete',
    evidence,
  })
  await syncForgeStoryboardState(input.storyId, instanceId)
  return { taskId }
}
