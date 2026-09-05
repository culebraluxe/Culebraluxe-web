import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine'
import { FORGE_SDLC_KEY, FORGE_SDLC_VERSION } from '../definitions/forge-sdlc'
import { engineConfigured, engineSql } from '../engine-client'
import { startWorkflowCore } from '../start-core'
import { createForgeApplicationPort } from './application-port'
import type { ForgeGateEvidence } from './forge-facts'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 Item 3 — Forge engine runtime (the "B" path on the engine).
//
// Mirrors the RE start boundary (workflow_app/runtime.ts + start-core.ts): a
// story becomes a running FORGE_SDLC instance on the SAME shared engine DB via
// the production interactive client (engineSql -> lib/neon-interactive WebSocket
// Pool routed by resolveDbTarget, so APP_ENV=production -> PROD, else DEV).
//
//   story ready
//     -> locate active FORGE_SDLC instance for the story (idempotent)
//     -> start instance (subject_type='story', subject_id=storyId,
//        variables.workType) so classify_work routes; the instance then parks
//        at the first async role task-node.
//
// Async roles are task-nodes: when the instance reaches one, the Forge
// execution layer runs the role agent and completes the engine task via
// engine.completeTask (see listActiveForgeRoleTasks / the Forge execution
// seam). Synchronous DEV_OPS command-nodes route through the Forge
// ApplicationPort.
// ---------------------------------------------------------------------------

export async function findActiveForgeInstance(
  storyId: string,
): Promise<string | null> {
  if (!engineConfigured()) return null
  const rows = await engineSql()`
    select pi.id
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.subject_type = 'story'
      and pi.subject_id = ${storyId}
      and pi.status = 'active'
      and pd.key = ${FORGE_SDLC_KEY}
    limit 1
  `
  return (rows[0]?.id as string | undefined) ?? null
}

export type ForgeStartFacts = {
  workType: 'FEATURE' | 'BUG' | 'HOTFIX' | 'RESEARCH' | 'MIGRATION'
  /** Optional initial gate evidence (e.g. rootCauseKnown for BUG). */
  evidence?: ForgeGateEvidence
}

/**
 * Idempotently start a FORGE_SDLC engine instance for a story. Returns the
 * active instance id and whether a new instance was started.
 */
export async function startForgeWorkflow(
  storyId: string,
  input: ForgeStartFacts,
): Promise<{ instanceId: string; started: boolean }> {
  return startWorkflowCore(storyId, {
    findActive: (id) => findActiveForgeInstance(id),
    readFacts: async () => ({ ...(input.evidence ?? {}) }),
    start: async (id, facts) => {
      if (!engineConfigured()) {
        throw new Error('Workflow engine database is not configured.')
      }
      const engine = new WorkflowEngine(engineSql(), {
        app: await createForgeApplicationPort(),
      })
      const { processInstanceId } = await engine.startProcess({
        definitionKey: FORGE_SDLC_KEY,
        version: FORGE_SDLC_VERSION,
        startedBy: 'forge',
        variables: { workType: input.workType, ...facts },
        subject: { subjectType: 'story', subjectId: id },
      })
      return processInstanceId
    },
  })
}

export type ActiveForgeRoleTask = {
  taskId: string
  nodeId: string
  status: string
  candidates: string[]
  formData: Record<string, any>
}

/**
 * List the open (ready/reserved/in_progress) engine role task-nodes for a
 * story's active FORGE_SDLC instance — the async gates the Forge execution
 * layer must run and complete via engine.completeTask.
 */
export async function listActiveForgeRoleTasks(
  storyId: string,
): Promise<ActiveForgeRoleTask[]> {
  const instanceId = await findActiveForgeInstance(storyId)
  if (!instanceId) return []
  const rows = await engineSql()`
    select t.id as task_id, tk.node_id as node_id, t.status, t.candidates, t.form_data
    from tasks t
    join tokens tk on tk.id = t.token_id
    where t.process_instance_id = ${instanceId}
      and t.status in ('ready', 'reserved', 'in_progress')
    order by t.created_at, t.id
  `
  return (rows as Array<{
    task_id: string
    node_id: string
    status: string
    candidates: string[]
    form_data: Record<string, any> | null
  }>).map((r) => ({
    taskId: r.task_id,
    nodeId: r.node_id,
    status: r.status,
    candidates: r.candidates ?? [],
    formData: r.form_data ?? {},
  }))
}

export { FORGE_SDLC_KEY as FORGE_DEFINITION_KEY }

/**
 * Complete an async role task-node on a Forge engine instance (the async
 * resume the task-node model enables). The Forge execution layer runs the role
 * agent, then calls this to advance the token on the chosen transition. Any
 * decision-gate evidence is supplied via `evidence` so downstream gates route;
 * when none is supplied the engine falls back to each decision's first
 * transition (e.g. SOLO for execution_shape).
 */
export async function completeForgeRoleTask(
  taskId: string,
  opts: { transitionName?: string; evidence?: ForgeGateEvidence; userId?: string } = {},
): Promise<void> {
  if (!engineConfigured()) {
    throw new Error('Workflow engine database is not configured.')
  }
  const evidence = opts.evidence ?? {}
  const engine = new WorkflowEngine(engineSql(), {
    app: await createForgeApplicationPort({
      evidenceReader: async () => evidence,
    }),
  })
  await engine.completeTask({
    taskId,
    userId: opts.userId ?? 'forge',
    transitionName: opts.transitionName ?? 'complete',
    formData: evidence,
  })
}
