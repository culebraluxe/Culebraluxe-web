import { engineSql } from '../engine-client'
import { readForgeWorkflowEvidence } from '../../db/forge-workflow-evidence'
import { listForgeHolds } from '../../db/forge-hold'
import type { ForgeGateEvidence } from './forge-facts'

export type ForgeVisibilitySnapshot = {
  storyId: string
  instance: { id: string | null; status: string | null; currentNodes: string[] }
  roleTasks: Array<{
    taskId: string
    nodeId: string
    status: string
    assignee: string | null
    claimAgeMs: number | null
  }>
  execution: Array<{ nodeId: string; status: string; workerId: string; storyRunId: string | null }>
  evidence: ForgeGateEvidence
  shaChain: {
    candidateSha: string | null
    qaVerifiedSha: string | null
    publishedSha: string | null
    deployedSha: string | null
    productionVerifiedSha: string | null
  }
  shaEquality: ReturnType<typeof forgeVisibilityEquality>
  receipts: { deploymentReceipt: string | null; productionVerificationReceipt: string | null }
  failedReleaseStage: ForgeGateEvidence['failedReleaseStage'] | null
  commandVisits: Array<{ nodeId: string; visitSequence: number; outcome: string | null }>
  splitBranches: Array<{
    index: number | null
    count: number | null
    status: string
    storyRunId: string | null
    costUsd: number | null
  }>
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

function norm(value: string | null | undefined): string | null {
  const sha = value?.trim().toLowerCase() ?? ''
  return /^[0-9a-f]{7,64}$/.test(sha) ? sha : null
}

export function forgeVisibilityEquality(chain: {
  candidateSha: string | null
  qaVerifiedSha: string | null
  publishedSha: string | null
  deployedSha: string | null
  productionVerifiedSha: string | null
}): {
  candidateEqualsQa: boolean
  publishedEqualsCandidate: boolean
  deployedEqualsPublished: boolean
  productionEqualsPublished: boolean
} {
  const candidate = norm(chain.candidateSha)
  const qa = norm(chain.qaVerifiedSha)
  const published = norm(chain.publishedSha)
  const deployed = norm(chain.deployedSha)
  const production = norm(chain.productionVerifiedSha)
  return {
    candidateEqualsQa: Boolean(candidate && qa && candidate === qa),
    publishedEqualsCandidate: Boolean(candidate && published && candidate === published),
    deployedEqualsPublished: Boolean(published && deployed && published === deployed),
    productionEqualsPublished: Boolean(published && production && published === production),
  }
}

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
  let commandVisits: ForgeVisibilitySnapshot['commandVisits'] = []
  let splitBranches: ForgeVisibilitySnapshot['splitBranches'] = []
  if (instanceId) {
    const tokens = await engineSql()`
      select node_id from tokens where process_instance_id = ${instanceId} and status = 'active'
    `
    currentNodes = (tokens as Array<{ node_id: string }>).map((t) => t.node_id)
    const tasks = await engineSql()`
      select t.id, tk.node_id, t.status, t.assignee, t.claimed_at, t.updated_at, t.form_data
      from tasks t join tokens tk on tk.id = t.token_id
      where t.process_instance_id = ${instanceId}
        and t.status in ('ready','reserved','in_progress')
    `
    const now = Date.now()
    roleTasks = (
      tasks as Array<{
        id: string
        node_id: string
        status: string
        assignee: string | null
        claimed_at?: string | null
        updated_at?: string | null
        form_data?: Record<string, unknown> | null
      }>
    ).map((r) => {
      const claimed = r.claimed_at ?? r.updated_at
      const claimAgeMs =
        r.status !== 'ready' && claimed ? Math.max(0, now - new Date(claimed).getTime()) : null
      return {
        taskId: r.id,
        nodeId: r.node_id,
        status: r.status,
        assignee: r.assignee,
        claimAgeMs,
      }
    })
    const ex = await engineSql()`
      select node_id, status, worker_id, story_run_id
      from forge_engine_task_execution
      where process_instance_id = ${instanceId}
      order by created_at desc limit 20
    `
    execution = (
      ex as Array<{ node_id: string; status: string; worker_id: string; story_run_id: string | null }>
    ).map((r) => ({ nodeId: r.node_id, status: r.status, workerId: r.worker_id, storyRunId: r.story_run_id }))

    try {
      const visits = await engineSql()`
        select node_id, visit_sequence, outcome
        from process_commands
        where process_instance_id = ${instanceId}
        order by visit_sequence desc
        limit 40
      `
      commandVisits = (
        visits as Array<{ node_id: string; visit_sequence: number; outcome: string | null }>
      ).map((r) => ({
        nodeId: r.node_id,
        visitSequence: Number(r.visit_sequence),
        outcome: r.outcome,
      }))
    } catch {
      commandVisits = []
    }

    splitBranches = (
      tasks as Array<{ node_id: string; status: string; form_data?: Record<string, unknown> | null }>
    )
      .filter((r) => r.node_id === 'smith_split_work')
      .map((r) => ({
        index: Number(r.form_data?.splitBranchIndex ?? NaN) || null,
        count: Number(r.form_data?.splitBranchCount ?? NaN) || null,
        status: r.status,
        storyRunId:
          execution.find((row) => row.nodeId === 'smith_split_work')?.storyRunId ?? null,
        costUsd: null,
      }))
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
    shaEquality: forgeVisibilityEquality(shaChain),
    receipts,
    failedReleaseStage: e.failedReleaseStage ?? null,
    commandVisits,
    splitBranches,
    holds: await listForgeHolds(storyId),
    divergenceWarning,
  }
}

export { shaKeys, receiptKeys }
