import { engineSql } from '../engine-client'
import { readForgeWorkflowEvidence } from '../../db/forge-workflow-evidence'
import { listForgeHolds } from '../../db/forge-hold'
import type { ForgeGateEvidence } from './forge-facts'

// ---------------------------------------------------------------------------
// ENG-FORGE-V10 S5 — one coherent operational read model for a Forge story.
//
// Reuses the engine + evidence + HOLD seams. NEON is authoritative; this is a
// read model only (no second cost ledger, no writes). It surfaces: process
// status/current node, task owner + claim age, linked work item + Story Run,
// the candidate/QA/publish/deploy/production SHA chain, release receipts,
// failed release stage, command visits, SPLIT branches, HOLD history, and an
// engine-vs-Storyboard divergence warning.
// ---------------------------------------------------------------------------

export type ForgeVisibilitySnapshot = {
  storyId: string
  instance: { id: string | null; status: string | null; currentNodes: string[] }
  roleTasks: Array<{ taskId: string; nodeId: string; status: string; assignee: string | null }>
  execution: Array<{ nodeId: string; status: string; workerId: string; storyRunId: string | null }>
  evidence: ForgeGateEvidence
  shaChain: {
    candidateSha: string | null
    qaVerifiedSha: string | null
    publishedSha: string | null
    deployedSha: string | null
    productionVerifiedSha: string | null
  }
  receipts: { deploymentReceipt: string | null; productionVerificationReceipt: string | null }
  holds: unknown[]
  divergenceWarning: string | null
}

const shaKeys = [
  'candidate_sha',
  'qa_verified_sha',
  'published_sha',
  'deployed_sha',
  'production_verified_sha',
] as const
const receiptKeys = ['deployment_receipt', 'production_verification_receipt'] as const

export async function forgeVisibilitySnapshot(
  storyId: string,
): Promise<ForgeVisibilitySnapshot> {
  const instance = await engineSql()`
    select pi.id, pi.status
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.subject_type = 'story' and pi.subject_id = ${storyId} and pd.key = 'FORGE_SDLC'
    order by pi.created_at desc limit 1
  `
  const inst = instance[0] as { id?: string; status?: string } | undefined
  const instanceId = inst?.id ?? null

  let currentNodes: string[] = []
  let roleTasks: ForgeVisibilitySnapshot['roleTasks'] = []
  let execution: ForgeVisibilitySnapshot['execution'] = []
  if (instanceId) {
    const tokens = await engineSql()`
      select node_id from tokens where process_instance_id = ${instanceId} and status = 'active'
    `
    currentNodes = (tokens as Array<{ node_id: string }>).map((t) => t.node_id)
    const tasks = await engineSql()`
      select t.id, tk.node_id, t.status, t.assignee
      from tasks t join tokens tk on tk.id = t.token_id
      where t.process_instance_id = ${instanceId}
        and t.status in ('ready','reserved','in_progress')
    `
    roleTasks = (tasks as Array<{ id: string; node_id: string; status: string; assignee: string | null }>).map(
      (r) => ({ taskId: r.id, nodeId: r.node_id, status: r.status, assignee: r.assignee }),
    )
    const ex = await engineSql()`
      select node_id, status, worker_id, story_run_id
      from forge_engine_task_execution
      where process_instance_id = ${instanceId}
      order by created_at desc limit 20
    `
    execution = (
      ex as Array<{ node_id: string; status: string; worker_id: string; story_run_id: string | null }>
    ).map((r) => ({ nodeId: r.node_id, status: r.status, workerId: r.worker_id, storyRunId: r.story_run_id }))
  }

  const evidence = await readForgeWorkflowEvidence(storyId)
  const e = evidence ?? {}
  const shaChain: ForgeVisibilitySnapshot['shaChain'] = {
    candidateSha: e.candidateSha ?? null,
    qaVerifiedSha: e.qaVerifiedSha ?? null,
    publishedSha: e.publishedSha ?? null,
    deployedSha: e.deployedSha ?? null,
    productionVerifiedSha: e.productionVerifiedSha ?? null,
  }
  const receipts: ForgeVisibilitySnapshot['receipts'] = {
    deploymentReceipt: e.deploymentReceipt ?? null,
    productionVerificationReceipt: e.productionVerificationReceipt ?? null,
  }

  // Divergence: engine instance not complete while Storyboard story says Complete.
  let divergenceWarning: string | null = null
  const story = await engineSql()`
    select status from storyboard_story where id = ${storyId}
  `
  const storyStatus = (story[0] as { status?: string } | undefined)?.status ?? null
  if (storyStatus === 'Complete' && inst && inst.status !== 'completed') {
    divergenceWarning = 'Storyboard Complete but engine instance is not completed'
  } else if (inst && inst.status === 'completed' && storyStatus !== 'Complete') {
    divergenceWarning = `Engine completed but Storyboard status is '${storyStatus ?? '(none)'}'`
  }

  return {
    storyId,
    instance: { id: instanceId, status: inst?.status ?? null, currentNodes },
    roleTasks,
    execution,
    evidence: e,
    shaChain,
    receipts,
    holds: await listForgeHolds(storyId),
    divergenceWarning,
  }
}

export { shaKeys, receiptKeys }
